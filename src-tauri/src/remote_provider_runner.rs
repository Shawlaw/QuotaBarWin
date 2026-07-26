use std::{
    collections::{HashMap, HashSet},
    io::{BufRead, BufReader, Read},
    path::Path,
    process::{ChildStderr, ChildStdout, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use chrono::Utc;
use serde::Deserialize;

use crate::{
    builtin_js::{run_builtin_js_provider, BuiltinJsRun},
    config::resolve_secret_value,
    logger::{LogLevel, LogSink},
    proxy::{select_proxy_url, ProxyConfig},
    quota::{
        clamp_snapshot_percentages, AppSnapshot, ProviderDiagnostics, ProviderSnapshot, QuotaWindow,
    },
    redact::redact_sensitive,
    remote_provider::{
        ensure_runtime_resolved, is_builtin_js_runtime, load_cached_manifest, ProviderManifest,
        BUILTIN_JS_RUNTIME,
    },
};

const MAX_CAPTURED_STDERR_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone)]
struct RemoteCommandSpec {
    executable: String,
    args: Vec<String>,
    cwd: Option<String>,
    timeout_ms: u64,
    env: HashMap<String, String>,
}

#[derive(Debug, Clone)]
struct RawCommandResult {
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
    duration_ms: u64,
    timed_out: bool,
}

#[derive(Debug, Clone)]
enum RemoteOutputSpec {
    ProviderSnapshotV1,
    AppSnapshotV1,
}

impl RemoteOutputSpec {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "provider-snapshot-v1" => Ok(Self::ProviderSnapshotV1),
            "app-snapshot-v1" => Ok(Self::AppSnapshotV1),
            other => Err(format!("Unsupported remote provider output type '{other}'")),
        }
    }
}

pub fn provider_error_snapshot(id: &str, name: &str, error: &str) -> ProviderSnapshot {
    error_provider(id, name, error, None, None)
}

fn remote_source_file_name(entry: &str) -> String {
    entry
        .rsplit('/')
        .next()
        .unwrap_or(entry)
        .rsplit('\\')
        .next()
        .unwrap_or(entry)
        .to_string()
}

pub fn run_remote_provider(
    id: &str,
    name: &str,
    provider_dir: Option<&Path>,
    runtime: &str,
    resolved_runtime: Option<&str>,
    global_proxy: Option<&ProxyConfig>,
    timeout_seconds: u64,
    config_dir: &Path,
    env_vars: &HashMap<String, String>,
    window_label_overrides: &HashMap<String, String>,
    visible_window_ids: &[String],
    log: Option<&LogSink>,
) -> Vec<ProviderSnapshot> {
    log_provider(
        log,
        LogLevel::Info,
        id,
        &format!(
            "remote provider run started name={} runtime={} providerDirPresent={}",
            name,
            runtime,
            provider_dir.is_some()
        ),
    );
    let Some(provider_dir) = provider_dir else {
        log_provider(
            log,
            LogLevel::Error,
            id,
            "remote provider cache directory is missing",
        );
        return vec![provider_error_snapshot(
            id,
            name,
            "Remote provider cache directory is missing",
        )];
    };

    let manifest = match load_cached_manifest(provider_dir) {
        Ok(manifest) => {
            log_provider(
                log,
                LogLevel::Info,
                id,
                &format!(
                    "cached manifest loaded manifestId={} version={}",
                    manifest.id,
                    manifest.version.as_deref().unwrap_or("none")
                ),
            );
            manifest
        }
        Err(error) => {
            log_provider(
                log,
                LogLevel::Error,
                id,
                &format!("failed to load cached manifest: {error}"),
            );
            return vec![provider_error_snapshot(
                id,
                name,
                &format!("Failed to load remote provider manifest: {error}"),
            )];
        }
    };

    let executable = if is_builtin_js_runtime(runtime) {
        log_provider(log, LogLevel::Info, id, "using embedded builtin-js runtime");
        None
    } else {
        match ensure_runtime_resolved(runtime, resolved_runtime) {
            Ok(path) => {
                log_provider(
                    log,
                    LogLevel::Info,
                    id,
                    &format!("runtime resolved path={}", path.display()),
                );
                Some(path)
            }
            Err(error) => {
                log_provider(
                    log,
                    LogLevel::Error,
                    id,
                    &format!("failed to resolve runtime: {error}"),
                );
                return vec![provider_error_snapshot(
                    id,
                    name,
                    &format!("Failed to resolve runtime: {error}"),
                )];
            }
        }
    };

    let source_name = remote_source_file_name(&manifest.entry);
    let source_path = provider_dir.join(&source_name);
    if !source_path.exists() {
        log_provider(
            log,
            LogLevel::Error,
            id,
            &format!("cached source file missing path={}", source_path.display()),
        );
        return vec![provider_error_snapshot(
            id,
            name,
            &format!(
                "Cached source file does not exist: {}",
                source_path.display()
            ),
        )];
    }

    let output = match RemoteOutputSpec::parse(&manifest.output) {
        Ok(output) => output,
        Err(error) => {
            log_provider(log, LogLevel::Error, id, &error);
            return vec![provider_error_snapshot(id, name, &error)];
        }
    };

    let mut env = match resolve_required_env_vars(&manifest.required_env_vars, env_vars, config_dir)
    {
        Ok(env) => {
            log_provider(
                log,
                LogLevel::Debug,
                id,
                &format!(
                    "remote provider environment resolved required={} configured={} injected={}",
                    manifest.required_env_vars.len(),
                    env_vars.len(),
                    env.len()
                ),
            );
            env
        }
        Err(error) => {
            log_provider(log, LogLevel::Error, id, &error);
            return vec![provider_error_snapshot(id, name, &error)];
        }
    };
    // A Provider's explicitly configured environment value owns its runtime
    // proxy. The project proxy is a fallback only and never overrides it.
    let configured_proxy = env
        .get("QBWIN_PROXY_URL")
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| select_proxy_url(None, global_proxy));
    if let Some(proxy_url) = configured_proxy {
        match resolve_secret_value(&proxy_url, config_dir) {
            Ok(proxy_url) if !proxy_url.trim().is_empty() => {
                env.insert("QBWIN_PROXY_URL".to_string(), proxy_url.trim().to_string());
                log_provider(log, LogLevel::Debug, id, "provider proxy URL resolved");
            }
            Ok(_) => {}
            Err(error) => {
                log_provider(
                    log,
                    LogLevel::Error,
                    id,
                    &format!("unable to resolve provider proxy URL: {error}"),
                );
                return vec![provider_error_snapshot(
                    id,
                    name,
                    &format!("Unable to resolve remote provider proxy URL: {error}"),
                )];
            }
        }
    }
    inject_provider_metadata_env(&mut env, id, name, &manifest, timeout_seconds);

    if is_builtin_js_runtime(runtime) {
        return run_builtin_js_remote_provider(
            id,
            name,
            &manifest,
            &source_path,
            &env,
            timeout_seconds,
            window_label_overrides,
            visible_window_ids,
            log,
        );
    }

    let command = RemoteCommandSpec {
        executable: executable
            .expect("external runtime must have a resolved executable")
            .display()
            .to_string(),
        args: vec![source_path.display().to_string()],
        cwd: Some(provider_dir.display().to_string()),
        timeout_ms: timeout_seconds.max(1).saturating_mul(1000),
        env,
    };

    run_remote_command(
        id,
        name,
        &command,
        &output,
        window_label_overrides,
        visible_window_ids,
        log,
    )
}

fn run_builtin_js_remote_provider(
    id: &str,
    name: &str,
    manifest: &ProviderManifest,
    source_path: &Path,
    env: &HashMap<String, String>,
    timeout_seconds: u64,
    window_label_overrides: &HashMap<String, String>,
    visible_window_ids: &[String],
    log: Option<&LogSink>,
) -> Vec<ProviderSnapshot> {
    let source = match std::fs::read_to_string(source_path) {
        Ok(source) => source,
        Err(error) => {
            let error = format!(
                "Failed to read builtin-js source '{}': {}",
                source_path.display(),
                error
            );
            log_provider(log, LogLevel::Error, id, &error);
            return vec![error_provider(
                id,
                name,
                &error,
                None,
                Some(BUILTIN_JS_RUNTIME),
            )];
        }
    };
    let proxy_url = env.get("QBWIN_PROXY_URL").map(String::as_str);
    let result = run_builtin_js_provider(BuiltinJsRun {
        provider_id: id,
        provider_name: name,
        manifest,
        source: &source,
        env,
        timeout: Duration::from_secs(timeout_seconds.max(1)),
        proxy_url,
        log,
    });
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            let error = redact_sensitive(&error);
            log_provider(
                log,
                LogLevel::Error,
                id,
                &format!("builtin-js provider execution failed: {error}"),
            );
            return vec![error_provider(
                id,
                name,
                &format!("Builtin-js provider failed: {error}"),
                None,
                Some(BUILTIN_JS_RUNTIME),
            )];
        }
    };
    let mut provider = match parse_remote_provider_snapshot_v1(id, name, &result.json) {
        Ok(provider) => provider,
        Err(error) => {
            log_provider(
                log,
                LogLevel::Error,
                id,
                &format!("failed to parse builtin-js provider result: {error}"),
            );
            return vec![error_provider(
                id,
                name,
                &format!("Failed to parse builtin-js provider result: {error}"),
                None,
                Some(BUILTIN_JS_RUNTIME),
            )];
        }
    };
    let window_count = provider.windows.len();
    apply_visible_windows(&mut provider, visible_window_ids);
    apply_window_label_overrides(&mut provider, window_label_overrides);
    provider.source = "remote".to_string();
    if provider.diagnostics.is_none() {
        provider.diagnostics = Some(ProviderDiagnostics {
            checked_at: Utc::now().to_rfc3339(),
            messages: Vec::new(),
            command_path: Some(BUILTIN_JS_RUNTIME.to_string()),
            exit_code: None,
            duration_ms: Some(result.duration_ms),
            timed_out: Some(false),
            stderr: None,
        });
    }
    clamp_snapshot_percentages(&mut provider);
    log_provider(
        log,
        LogLevel::Info,
        id,
        &format!(
            "builtin-js output parsed providers=1 windows={} durationMs={} resultBytes={}",
            window_count,
            result.duration_ms,
            result.json.len()
        ),
    );
    vec![provider]
}

fn log_provider(log: Option<&LogSink>, level: LogLevel, id: &str, message: &str) {
    if let Some(log) = log {
        let _ = log.write(
            level,
            "remote_provider_runner",
            &format!("providerId={id} {message}"),
        );
    }
}

fn inject_provider_metadata_env(
    env: &mut HashMap<String, String>,
    id: &str,
    name: &str,
    manifest: &ProviderManifest,
    timeout_seconds: u64,
) {
    env.insert("QBWIN_PROVIDER_ID".to_string(), id.to_string());
    env.insert(
        "QBWIN_PROVIDER_MANIFEST_ID".to_string(),
        manifest.id.clone(),
    );
    env.insert("QBWIN_PROVIDER_NAME".to_string(), name.to_string());
    if let Some(version) = manifest
        .version
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        env.insert("QBWIN_PROVIDER_VERSION".to_string(), version.to_string());
    }
    if let Some(checksum) = manifest
        .checksums
        .source
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        env.insert(
            "QBWIN_PROVIDER_SOURCE_CHECKSUM".to_string(),
            checksum.to_string(),
        );
    }
    env.insert(
        "QBWIN_PROVIDER_TIMEOUT_SECONDS".to_string(),
        timeout_seconds.max(1).to_string(),
    );
}

fn run_remote_command(
    id: &str,
    name: &str,
    command: &RemoteCommandSpec,
    output: &RemoteOutputSpec,
    window_label_overrides: &HashMap<String, String>,
    visible_window_ids: &[String],
    log: Option<&LogSink>,
) -> Vec<ProviderSnapshot> {
    log_provider(
        log,
        LogLevel::Info,
        id,
        &format!(
            "remote command started executable={} cwd={} timeoutMs={}",
            command.executable,
            command.cwd.as_deref().unwrap_or("none"),
            command.timeout_ms
        ),
    );
    match execute_command(id, command, log) {
        Ok(result) if result.timed_out => {
            log_provider(
                log,
                LogLevel::Warn,
                id,
                &format!(
                    "remote command timed out timeoutOrigin=host configuredTimeoutMs={} durationMs={} exitCode={:?} stderrBytes={} stderrPreview={}",
                    command.timeout_ms,
                    result.duration_ms,
                    result.exit_code,
                    result.stderr.len(),
                    stderr_preview_for_log(&result.stderr)
                ),
            );
            vec![error_provider(
                id,
                name,
                "Remote provider timed out",
                Some(result),
                Some(&command.executable),
            )]
        }
        Ok(result) if result.exit_code != Some(0) => {
            log_provider(
                log,
                LogLevel::Warn,
                id,
                &format!(
                    "remote command exited nonzero failureOrigin=provider durationMs={} exitCode={:?} stderrBytes={} stderrPreview={}",
                    result.duration_ms,
                    result.exit_code,
                    result.stderr.len(),
                    stderr_preview_for_log(&result.stderr)
                ),
            );
            vec![error_provider(
                id,
                name,
                "Remote provider exited with a non-zero status",
                Some(result),
                Some(&command.executable),
            )]
        }
        Ok(result) => parse_remote_output(
            id,
            name,
            output,
            result,
            &command.executable,
            window_label_overrides,
            visible_window_ids,
            log,
        ),
        Err(error) => {
            log_provider(
                log,
                LogLevel::Error,
                id,
                &format!("failed to execute remote command: {error}"),
            );
            vec![error_provider(
                id,
                name,
                &error,
                None,
                Some(&command.executable),
            )]
        }
    }
}

fn stderr_preview_for_log(stderr: &str) -> String {
    let trimmed = stderr.trim();
    if trimmed.is_empty() {
        return "none".to_string();
    }
    let single_line = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    let redacted = redact_sensitive(&single_line);
    if redacted.chars().count() > 160 {
        format!("{}...", redacted.chars().take(160).collect::<String>())
    } else {
        redacted
    }
}

fn execute_command(
    provider_id: &str,
    command: &RemoteCommandSpec,
    log: Option<&LogSink>,
) -> Result<RawCommandResult, String> {
    let started = Instant::now();
    let mut process = Command::new(&command.executable);
    process.args(&command.args);
    process.stdout(Stdio::piped());
    process.stderr(Stdio::piped());
    for (name, value) in &command.env {
        process.env(name, value);
    }
    suppress_command_window(&mut process);

    if let Some(cwd) = &command.cwd {
        process.current_dir(cwd);
    }

    let mut child = process.spawn().map_err(|error| error.to_string())?;
    let timeout = Duration::from_millis(command.timeout_ms.max(1));
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture remote provider stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture remote provider stderr".to_string())?;
    let stdout_handle = thread::spawn(move || read_child_stdout(stdout));
    let provider_id_for_stderr = provider_id.to_string();
    let log_for_stderr = log.cloned();
    let stderr_handle =
        thread::spawn(move || read_child_stderr(stderr, &provider_id_for_stderr, log_for_stderr));

    let (status, timed_out) = loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            break (status, false);
        }

        if started.elapsed() >= timeout {
            let _ = child.kill();
            let status = child.wait().map_err(|error| error.to_string())?;
            break (status, true);
        }

        thread::sleep(Duration::from_millis(10));
    };
    let stdout = stdout_handle
        .join()
        .map_err(|_| "Failed to join remote provider stdout reader".to_string())??;
    let stderr = stderr_handle
        .join()
        .map_err(|_| "Failed to join remote provider stderr reader".to_string())??;

    Ok(RawCommandResult {
        stdout,
        stderr,
        exit_code: status.code(),
        duration_ms: started.elapsed().as_millis() as u64,
        timed_out,
    })
}

fn read_child_stdout(stdout: ChildStdout) -> Result<String, String> {
    let mut output = String::new();
    BufReader::new(stdout)
        .read_to_string(&mut output)
        .map_err(|error| error.to_string())?;
    Ok(output)
}

fn read_child_stderr(
    stderr: ChildStderr,
    provider_id: &str,
    log: Option<LogSink>,
) -> Result<String, String> {
    let mut captured = String::new();
    let mut reader = BufReader::new(stderr);
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .map_err(|error| error.to_string())?;
        if bytes == 0 {
            break;
        }
        let raw_line = line.trim_end_matches(['\r', '\n']);
        if !raw_line.trim().is_empty() {
            log_provider_stderr_line(log.as_ref(), provider_id, raw_line);
            let redacted = captured_stderr_line(raw_line);
            append_bounded_stderr(&mut captured, &redacted);
        }
    }
    Ok(captured)
}

fn append_bounded_stderr(captured: &mut String, line: &str) {
    if !captured.is_empty() {
        captured.push('\n');
    }
    captured.push_str(line);
    if captured.len() <= MAX_CAPTURED_STDERR_BYTES {
        return;
    }

    let mut start = captured.len().saturating_sub(MAX_CAPTURED_STDERR_BYTES);
    while start < captured.len() && !captured.is_char_boundary(start) {
        start += 1;
    }
    let tail = captured[start..].to_string();
    captured.clear();
    captured.push_str(&tail);
}

fn log_provider_stderr_line(log: Option<&LogSink>, provider_id: &str, line: &str) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        log_provider(
            log,
            LogLevel::Info,
            provider_id,
            &format!("providerLog structured=false line={line}"),
        );
        return;
    };
    let level = value
        .get("level")
        .and_then(serde_json::Value::as_str)
        .map(LogLevel::parse)
        .unwrap_or(LogLevel::Info);
    log_provider(
        log,
        level,
        provider_id,
        &format!(
            "providerLog structured=true {}",
            provider_log_summary(&value)
        ),
    );
}

fn captured_stderr_line(line: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(line) {
        Ok(value) => redact_sensitive(&format!(
            "providerLog structured=true {}",
            provider_log_summary(&value)
        )),
        Err(_) => redact_sensitive(line),
    }
}

fn provider_log_summary(value: &serde_json::Value) -> String {
    let level = value
        .get("level")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("info");
    let stage = value
        .get("stage")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let message = value
        .get("message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let version = value
        .get("version")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("none");
    let source_checksum = value
        .get("sourceChecksum")
        .and_then(serde_json::Value::as_str)
        .map(short_checksum)
        .unwrap_or_else(|| "none".to_string());
    let mut parts = vec![
        format!("level={level}"),
        format!("stage={stage}"),
        format!("message={message}"),
        format!("version={version}"),
        format!("sourceChecksum={source_checksum}"),
    ];
    if let Some(object) = value.as_object() {
        for (key, value) in object {
            if matches!(
                key.as_str(),
                "level" | "stage" | "message" | "providerId" | "version" | "sourceChecksum"
            ) {
                continue;
            }
            if let Some(field) = provider_log_extra_field(key, value) {
                parts.push(field);
            }
        }
    }
    parts.join(" ")
}

fn provider_log_extra_field(key: &str, value: &serde_json::Value) -> Option<String> {
    let redacted_keys = [
        "api_key",
        "apikey",
        "token",
        "secret",
        "cookie",
        "authorization",
    ];
    if redacted_keys
        .iter()
        .any(|pattern| key.to_ascii_lowercase().contains(pattern))
    {
        return Some(format!("{key}=[REDACTED]"));
    }
    match value {
        serde_json::Value::Bool(value) => Some(format!("{key}={value}")),
        serde_json::Value::Number(value) => Some(format!("{key}={value}")),
        serde_json::Value::String(value) => {
            let normalized = value.split_whitespace().collect::<Vec<_>>().join("_");
            let truncated = if normalized.chars().count() > 80 {
                format!("{}...", normalized.chars().take(80).collect::<String>())
            } else {
                normalized
            };
            Some(format!("{key}={}", redact_sensitive(&truncated)))
        }
        serde_json::Value::Null => Some(format!("{key}=null")),
        _ => None,
    }
}

fn short_checksum(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= 11 {
        return trimmed.to_string();
    }
    format!("{}...", &trimmed[..11])
}

fn resolve_required_env_vars(
    required_env_vars: &[String],
    configured_env_vars: &HashMap<String, String>,
    config_dir: &Path,
) -> Result<HashMap<String, String>, String> {
    let mut env = HashMap::new();
    for (name, source) in configured_env_vars {
        let name = name.trim();
        if name.is_empty() || source.trim().is_empty() {
            continue;
        }
        let value = resolve_secret_value(source, config_dir).map_err(|error| {
            format!("Unable to resolve remote provider environment variable {name}: {error}")
        })?;
        env.insert(name.to_string(), value);
    }

    for name in required_env_vars {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let configured_value = configured_env_vars
            .get(name)
            .map(String::as_str)
            .unwrap_or_else(|| "");
        let source = if configured_value.trim().is_empty() {
            format!("${{secret:{name}}}")
        } else {
            configured_value.to_string()
        };
        let value = resolve_secret_value(&source, config_dir).map_err(|error| {
            format!("Unable to resolve remote provider environment variable {name}: {error}")
        })?;
        env.insert(name.to_string(), value);
    }
    Ok(env)
}

#[cfg(windows)]
fn suppress_command_window(command: &mut Command) {
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn suppress_command_window(_command: &mut Command) {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteProviderSnapshotV1 {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
    windows: Vec<RemoteQuotaWindowV1>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    diagnostics: Option<ProviderDiagnostics>,
    #[serde(default)]
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteQuotaWindowV1 {
    id: String,
    label: String,
    #[serde(default)]
    remaining: Option<f64>,
    #[serde(default)]
    used: Option<f64>,
    #[serde(default)]
    limit: Option<f64>,
    #[serde(default)]
    unit: Option<String>,
    #[serde(default)]
    used_percent: Option<f64>,
    #[serde(default)]
    remaining_percent: Option<f64>,
    #[serde(default)]
    warning_remaining: Option<f64>,
    #[serde(default)]
    reset_at: Option<String>,
    #[serde(default)]
    reset_text: Option<String>,
    #[serde(default)]
    confidence: Option<String>,
}

fn parse_remote_output(
    id: &str,
    name: &str,
    output: &RemoteOutputSpec,
    result: RawCommandResult,
    command_path: &str,
    window_label_overrides: &HashMap<String, String>,
    visible_window_ids: &[String],
    log: Option<&LogSink>,
) -> Vec<ProviderSnapshot> {
    let diagnostics = diagnostics_from_result(&result, Some(command_path));
    let parsed = match output {
        RemoteOutputSpec::ProviderSnapshotV1 => {
            parse_remote_provider_snapshot_v1(id, name, &result.stdout)
                .map(|provider| vec![provider])
        }
        RemoteOutputSpec::AppSnapshotV1 => {
            serde_json::from_str::<AppSnapshot>(&result.stdout).map(|snapshot| snapshot.providers)
        }
    };

    match parsed {
        Ok(mut providers) => {
            let window_count = providers
                .iter()
                .map(|provider| provider.windows.len())
                .sum::<usize>();
            log_provider(
                log,
                LogLevel::Info,
                id,
                &format!(
                    "remote output parsed providers={} windows={} durationMs={} stdoutBytes={} stderrBytes={}",
                    providers.len(),
                    window_count,
                    result.duration_ms,
                    result.stdout.len(),
                    result.stderr.len()
                ),
            );
            for provider in &mut providers {
                apply_visible_windows(provider, visible_window_ids);
                apply_window_label_overrides(provider, window_label_overrides);
                provider.source = "remote".to_string();
                if provider.diagnostics.is_none() {
                    provider.diagnostics = Some(diagnostics.clone());
                }
                clamp_snapshot_percentages(provider);
            }
            providers
        }
        Err(error) => {
            log_provider(
                log,
                LogLevel::Error,
                id,
                &format!(
                    "failed to parse remote provider stdout: {error}; stdoutBytes={} stderrBytes={}",
                    result.stdout.len(),
                    result.stderr.len()
                ),
            );
            vec![error_provider(
                id,
                name,
                &format!("Failed to parse remote provider stdout: {error}"),
                Some(result),
                Some(command_path),
            )]
        }
    }
}

fn parse_remote_provider_snapshot_v1(
    id: &str,
    name: &str,
    stdout: &str,
) -> Result<ProviderSnapshot, serde_json::Error> {
    let raw = serde_json::from_str::<RemoteProviderSnapshotV1>(stdout)?;
    let _script_id = raw.id;
    let _script_name = raw.name;
    Ok(ProviderSnapshot {
        id: id.to_string(),
        name: name.to_string(),
        status: raw.status.unwrap_or_else(|| "ok".to_string()),
        source: "remote".to_string(),
        updated_at: raw.updated_at,
        windows: raw
            .windows
            .into_iter()
            .map(|window| QuotaWindow {
                id: window.id,
                label: window.label,
                remaining: window.remaining,
                used: window.used,
                limit: window.limit,
                unit: window.unit,
                used_percent: window.used_percent,
                remaining_percent: window.remaining_percent,
                warning_remaining: window.warning_remaining,
                reset_at: window.reset_at,
                reset_text: window.reset_text,
                confidence: window.confidence.unwrap_or_else(|| "unknown".to_string()),
            })
            .collect(),
        error: raw.error,
        diagnostics: raw.diagnostics,
        metadata: raw.metadata,
    })
}

fn apply_visible_windows(provider: &mut ProviderSnapshot, visible_window_ids: &[String]) {
    if visible_window_ids.is_empty() {
        return;
    }

    let original_windows = provider.windows.clone();
    let mut selected_indexes = HashSet::new();
    let mut visible_windows = Vec::new();

    for visible in visible_window_ids {
        let visible = visible.trim();
        if visible.is_empty() {
            continue;
        }

        for (index, window) in original_windows.iter().enumerate() {
            if selected_indexes.contains(&index) || visible != window.id {
                continue;
            }
            visible_windows.push(window.clone());
            selected_indexes.insert(index);
        }

        for (index, window) in original_windows.iter().enumerate() {
            if selected_indexes.contains(&index) || visible != window.label {
                continue;
            }
            visible_windows.push(window.clone());
            selected_indexes.insert(index);
        }
    }

    provider.windows = visible_windows;
}

fn apply_window_label_overrides(
    provider: &mut ProviderSnapshot,
    overrides: &HashMap<String, String>,
) {
    if overrides.is_empty() {
        return;
    }

    for window in &mut provider.windows {
        if let Some(label) = overrides
            .get(&window.id)
            .or_else(|| overrides.get(&window.label))
            .filter(|label| !label.trim().is_empty())
        {
            window.label = label.trim().to_string();
        }
    }
}

fn error_provider(
    id: &str,
    name: &str,
    error: &str,
    result: Option<RawCommandResult>,
    command_path: Option<&str>,
) -> ProviderSnapshot {
    ProviderSnapshot {
        id: id.to_string(),
        name: name.to_string(),
        status: "error".to_string(),
        source: "remote".to_string(),
        updated_at: Some(Utc::now().to_rfc3339()),
        windows: Vec::new(),
        error: Some(redact_sensitive(error)),
        diagnostics: result
            .as_ref()
            .map(|raw| diagnostics_from_result(raw, command_path))
            .or_else(|| {
                Some(ProviderDiagnostics {
                    checked_at: Utc::now().to_rfc3339(),
                    messages: vec![redact_sensitive(error)],
                    command_path: command_path.map(str::to_string),
                    exit_code: None,
                    duration_ms: None,
                    timed_out: None,
                    stderr: None,
                })
            }),
        metadata: None,
    }
}

fn diagnostics_from_result(
    result: &RawCommandResult,
    command_path: Option<&str>,
) -> ProviderDiagnostics {
    ProviderDiagnostics {
        checked_at: Utc::now().to_rfc3339(),
        messages: Vec::new(),
        command_path: command_path.map(str::to_string),
        exit_code: result.exit_code,
        duration_ms: Some(result.duration_ms),
        timed_out: Some(result.timed_out),
        stderr: if result.stderr.trim().is_empty() {
            None
        } else {
            Some(redact_sensitive(&result.stderr))
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxy::{ProxyConfig, ProxyKind};
    use std::{io::Write, net::TcpListener};

    fn run_official_kimi_source(raw: serde_json::Value) -> ProviderSnapshot {
        let listener = TcpListener::bind("127.0.0.1:0").expect("Kimi test listener");
        let address = listener.local_addr().expect("Kimi test address");
        let response_body = serde_json::to_string(&raw).expect("Kimi test response");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("Kimi test connection");
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream
                .write_all(response.as_bytes())
                .expect("Kimi response");
        });
        let provider_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repo root")
            .join("examples")
            .join("remote-providers")
            .join("kimi-coding");
        let mut manifest = load_cached_manifest(&provider_dir).expect("Kimi manifest");
        manifest.permissions = vec![
            "env:KIMI_API_KEY".to_string(),
            format!("net:http://{address}"),
        ];
        let source = std::fs::read_to_string(provider_dir.join(&manifest.entry))
            .expect("Kimi source")
            .replace(
                "https://api.kimi.com/coding/v1/usages",
                &format!("http://{address}/coding/v1/usages"),
            );
        let env = HashMap::from([("KIMI_API_KEY".to_string(), "test-token".to_string())]);
        let result = run_builtin_js_provider(BuiltinJsRun {
            provider_id: "kimi-coding",
            provider_name: "Kimi Coding",
            manifest: &manifest,
            source: &source,
            env: &env,
            timeout: Duration::from_secs(2),
            proxy_url: None,
            log: None,
        })
        .expect("Kimi builtin-js result");
        server.join().expect("Kimi server");
        parse_remote_provider_snapshot_v1("kimi-coding", "Kimi Coding", &result.json)
            .expect("Kimi snapshot")
    }

    fn node_command(script: &str) -> RemoteCommandSpec {
        RemoteCommandSpec {
            executable: "node".to_string(),
            args: vec!["-e".to_string(), script.to_string()],
            cwd: None,
            timeout_ms: 2000,
            env: HashMap::new(),
        }
    }

    #[test]
    fn builtin_kimi_treats_unknown_five_hour_usage_as_full_remaining() {
        let provider = run_official_kimi_source(serde_json::json!({
            "user": { "region": "REGION_CN", "membership": { "level": "LEVEL_INTERMEDIATE" } },
            "usage": { "limit": "100", "used": "15", "remaining": "85", "resetTime": "2026-06-12T02:35:14.207781Z" },
            "limits": [{
                "window": { "duration": 300, "timeUnit": "TIME_UNIT_MINUTE" },
                "detail": { "used": "", "remaining": "", "limit": "", "resetTime": "2026-06-07T16:35:14.207781Z" }
            }]
        }));
        let five_hour = provider
            .windows
            .iter()
            .find(|window| window.id == "300-minute")
            .expect("five-hour window");
        assert_eq!(five_hour.used, None);
        assert_eq!(five_hour.remaining, None);
        assert_eq!(five_hour.limit, None);
        assert_eq!(five_hour.used_percent, Some(0.0));
        assert_eq!(five_hour.remaining_percent, Some(100.0));
        assert_eq!(five_hour.confidence, "estimated");
    }

    #[test]
    fn builtin_kimi_does_not_create_a_missing_five_hour_window() {
        let provider = run_official_kimi_source(serde_json::json!({
            "usage": { "limit": "100", "used": "15", "remaining": "85", "resetTime": "2026-06-12T02:35:14.207781Z" },
            "limits": [{
                "window": { "duration": 60, "timeUnit": "TIME_UNIT_MINUTE" },
                "detail": { "used": "", "remaining": "", "limit": "" }
            }]
        }));
        assert!(!provider
            .windows
            .iter()
            .any(|window| window.id == "300-minute"));
        let weekly = provider
            .windows
            .iter()
            .find(|window| window.id == "usage")
            .expect("weekly window");
        assert_eq!(weekly.used_percent, Some(15.0));
        assert_eq!(weekly.remaining_percent, Some(85.0));
        assert_eq!(weekly.confidence, "exact");
    }

    #[test]
    fn remote_provider_snapshot_output_uses_remote_source() {
        let script = r#"console.log(JSON.stringify({id:'remote-fixture',name:'Remote Fixture',status:'ok',source:'script',updatedAt:null,windows:[{id:'weekly',label:'Weekly',remainingPercent:88,confidence:'exact'}]}));"#;
        let providers = run_remote_command(
            "remote-fixture",
            "Remote Fixture",
            &node_command(script),
            &RemoteOutputSpec::ProviderSnapshotV1,
            &HashMap::new(),
            &[],
            None,
        );

        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].id, "remote-fixture");
        assert_eq!(providers[0].source, "remote");
        assert_eq!(providers[0].windows[0].remaining_percent, Some(88.0));
        assert!(providers[0].diagnostics.is_some());
    }

    #[test]
    fn remote_provider_snapshot_uses_local_instance_identity() {
        let script = r#"console.log(JSON.stringify({id:'manifest-id',name:'Script Name',status:'ok',updatedAt:null,windows:[]}));"#;
        let providers = run_remote_command(
            "manifest-id-2",
            "Team Account",
            &node_command(script),
            &RemoteOutputSpec::ProviderSnapshotV1,
            &HashMap::new(),
            &[],
            None,
        );

        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].id, "manifest-id-2");
        assert_eq!(providers[0].name, "Team Account");
    }

    #[test]
    fn remote_provider_errors_use_remote_source() {
        let providers = run_remote_command(
            "bad",
            "Bad",
            &node_command("process.stderr.write('boom');process.exit(1)"),
            &RemoteOutputSpec::ProviderSnapshotV1,
            &HashMap::new(),
            &[],
            None,
        );

        assert_eq!(providers[0].status, "error");
        assert_eq!(providers[0].source, "remote");
        assert_eq!(
            providers[0].diagnostics.as_ref().unwrap().exit_code,
            Some(1)
        );
    }

    #[test]
    fn remote_provider_stderr_lines_are_forwarded_to_app_log() {
        let temp = tempfile::tempdir().expect("temp dir");
        let config_path = temp.path().join("config.json");
        let config = crate::config::default_config();
        let log = LogSink::from_config_path(&config_path, &config);
        let script = r#"process.stderr.write(JSON.stringify({level:"info",stage:"fixture.start",message:"hello",version:"1.2.3",sourceChecksum:"sha256:abcdefghijklmnopqrstuvwxyz1234567890"}) + "\n");console.log(JSON.stringify({windows:[]}));"#;

        let providers = run_remote_command(
            "remote-log",
            "Remote Log",
            &node_command(script),
            &RemoteOutputSpec::ProviderSnapshotV1,
            &HashMap::new(),
            &[],
            Some(&log),
        );

        assert_eq!(providers[0].status, "ok");
        assert!(providers[0]
            .diagnostics
            .as_ref()
            .and_then(|diagnostics| diagnostics.stderr.as_deref())
            .unwrap_or("")
            .contains("stage=fixture.start"));
        let log_contents = std::fs::read_to_string(config_path.with_file_name("quotabarwin.log"))
            .expect("read log");
        assert!(
            log_contents.contains("providerLog structured=true"),
            "{log_contents}"
        );
        assert!(
            log_contents.contains("stage=fixture.start"),
            "{log_contents}"
        );
        assert!(log_contents.contains("version=1.2.3"), "{log_contents}");
        assert!(
            log_contents.contains("sourceChecksum=sha256:abcd..."),
            "{log_contents}"
        );
    }

    #[test]
    fn window_label_overrides_apply_after_remote_parsing() {
        let script = r#"console.log(JSON.stringify({windows:[{id:'weekly',label:'Weekly',remainingPercent:88,confidence:'exact'}]}));"#;
        let mut overrides = HashMap::new();
        overrides.insert("weekly".to_string(), "Team weekly".to_string());

        let providers = run_remote_command(
            "remote-fixture",
            "Remote Fixture",
            &node_command(script),
            &RemoteOutputSpec::ProviderSnapshotV1,
            &overrides,
            &["Weekly".to_string()],
            None,
        );

        assert_eq!(providers[0].windows.len(), 1);
        assert_eq!(providers[0].windows[0].label, "Team weekly");
    }

    #[test]
    fn visible_windows_follow_config_order_and_original_labels() {
        let script = r#"console.log(JSON.stringify({windows:[{id:'five',label:'5h',remainingPercent:80,confidence:'exact'},{id:'weekly',label:'Weekly',remainingPercent:70,confidence:'exact'},{id:'daily',label:'Weekly',remainingPercent:60,confidence:'exact'}]}));"#;
        let providers = run_remote_command(
            "remote-fixture",
            "Remote Fixture",
            &node_command(script),
            &RemoteOutputSpec::ProviderSnapshotV1,
            &HashMap::new(),
            &[
                "Weekly".to_string(),
                "five".to_string(),
                "Weekly".to_string(),
            ],
            None,
        );

        let windows = &providers[0].windows;
        assert_eq!(windows.len(), 3);
        assert_eq!(windows[0].id, "weekly");
        assert_eq!(windows[1].id, "daily");
        assert_eq!(windows[2].id, "five");
    }

    #[test]
    fn remote_provider_injects_child_env_from_config_map() {
        let temp = tempfile::tempdir().expect("temp dir");
        let provider_dir = temp.path().join("provider");
        std::fs::create_dir(&provider_dir).expect("create provider dir");
        std::fs::write(
            provider_dir.join("provider.json"),
            serde_json::json!({
                "schemaVersion": 1,
                "id": "remote-env",
                "displayName": "Remote Env",
                "runtime": "node",
                "entry": "provider.cjs",
                "requiredEnvVars": ["QBWIN_REMOTE_CHILD_TOKEN"],
                "output": "provider-snapshot-v1"
            })
            .to_string(),
        )
        .expect("write manifest");
        std::fs::write(
            provider_dir.join("provider.cjs"),
            r#"console.log(JSON.stringify({windows:[{id:"token",label:process.env.QBWIN_REMOTE_CHILD_TOKEN || "",remainingPercent:50,confidence:"exact"}]}));"#,
        )
        .expect("write source");
        std::env::remove_var("QBWIN_REMOTE_CHILD_TOKEN");

        let env_vars = HashMap::from([(
            "QBWIN_REMOTE_CHILD_TOKEN".to_string(),
            "from-config-map".to_string(),
        )]);
        let providers = run_remote_provider(
            "remote-env",
            "Remote Env",
            Some(&provider_dir),
            "node",
            None,
            None,
            crate::config::DEFAULT_REMOTE_PROVIDER_TIMEOUT_SECONDS,
            temp.path(),
            &env_vars,
            &HashMap::new(),
            &[],
            None,
        );

        assert_eq!(providers[0].status, "ok");
        assert_eq!(providers[0].windows[0].label, "from-config-map");
        assert!(std::env::var("QBWIN_REMOTE_CHILD_TOKEN").is_err());
    }

    #[test]
    fn builtin_js_provider_runs_without_an_external_runtime() {
        let temp = tempfile::tempdir().expect("temp dir");
        let provider_dir = temp.path().join("provider");
        std::fs::create_dir(&provider_dir).expect("create provider dir");
        std::fs::write(
            provider_dir.join("provider.json"),
            serde_json::json!({
                "schemaVersion": 1,
                "id": "builtin-env",
                "displayName": "Builtin Env",
                "runtime": "builtin-js",
                "entry": "provider.js",
                "requiredEnvVars": ["BUILTIN_TOKEN"],
                "output": "provider-snapshot-v1",
                "permissions": ["env:BUILTIN_TOKEN"]
            })
            .to_string(),
        )
        .expect("write manifest");
        std::fs::write(
            provider_dir.join("provider.js"),
            r#"
                function main(qb) {
                  return {
                    metadata: { provider: qb.meta.providerId },
                    windows: [{ id: "token", label: qb.env.get("BUILTIN_TOKEN"), remainingPercent: 50, confidence: "exact" }]
                  };
                }
            "#,
        )
        .expect("write source");

        let env_vars = HashMap::from([(
            "BUILTIN_TOKEN".to_string(),
            "from-builtin-config".to_string(),
        )]);
        let providers = run_remote_provider(
            "builtin-env",
            "Builtin Env",
            Some(&provider_dir),
            "builtin-js",
            None,
            None,
            crate::config::DEFAULT_REMOTE_PROVIDER_TIMEOUT_SECONDS,
            temp.path(),
            &env_vars,
            &HashMap::new(),
            &[],
            None,
        );

        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].status, "ok", "{:?}", providers[0]);
        assert_eq!(providers[0].windows[0].label, "from-builtin-config");
        assert_eq!(
            providers[0].metadata.as_ref().unwrap()["provider"],
            "builtin-env"
        );
        assert_eq!(
            providers[0]
                .diagnostics
                .as_ref()
                .and_then(|diagnostics| diagnostics.command_path.as_deref()),
            Some("builtin-js")
        );
    }

    #[test]
    fn official_time_flies_provider_runs_on_builtin_js() {
        let temp = tempfile::tempdir().expect("temp dir");
        let provider_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repo root")
            .join("examples")
            .join("remote-providers")
            .join("time-flies");
        let providers = run_remote_provider(
            "time-flies",
            "Time Flies",
            Some(&provider_dir),
            "builtin-js",
            None,
            None,
            5,
            temp.path(),
            &HashMap::new(),
            &HashMap::new(),
            &[],
            None,
        );

        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].status, "ok");
        assert_eq!(providers[0].windows.len(), 5);
        assert!(providers[0]
            .windows
            .iter()
            .all(|window| window.unit.as_deref() == Some("minutes")));
    }

    #[test]
    fn official_builtin_js_providers_load_without_node_modules() {
        let providers_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repo root")
            .join("examples")
            .join("remote-providers");
        for provider_id in [
            "kimi-coding",
            "bigmodel-coding-plan",
            "deepseek-balance",
            "codex-usage",
        ] {
            let provider_dir = providers_root.join(provider_id);
            let manifest = load_cached_manifest(&provider_dir).expect("manifest");
            let source =
                std::fs::read_to_string(provider_dir.join(&manifest.entry)).expect("source");
            let result = run_builtin_js_provider(BuiltinJsRun {
                provider_id,
                provider_name: provider_id,
                manifest: &manifest,
                source: &format!("{source}\nfunction main(qb) {{ return {{ windows: [] }}; }}"),
                env: &HashMap::new(),
                timeout: Duration::from_secs(5),
                proxy_url: None,
                log: None,
            });
            assert!(
                result.is_ok(),
                "{provider_id} should parse in builtin-js: {result:?}"
            );
        }
    }

    #[test]
    fn remote_provider_injects_host_metadata_env_vars() {
        let temp = tempfile::tempdir().expect("temp dir");
        let provider_dir = temp.path().join("provider");
        std::fs::create_dir(&provider_dir).expect("create provider dir");
        std::fs::write(
            provider_dir.join("provider.json"),
            serde_json::json!({
                "schemaVersion": 1,
                "id": "remote-meta",
                "displayName": "Remote Meta",
                "version": "2.3.4",
                "runtime": "node",
                "entry": "provider.cjs",
                "requiredEnvVars": [],
                "output": "provider-snapshot-v1",
                "checksums": {
                    "source": "sha256:abc123"
                }
            })
            .to_string(),
        )
        .expect("write manifest");
        std::fs::write(
            provider_dir.join("provider.cjs"),
            r#"console.log(JSON.stringify({metadata:{id:process.env.QBWIN_PROVIDER_ID,version:process.env.QBWIN_PROVIDER_VERSION,checksum:process.env.QBWIN_PROVIDER_SOURCE_CHECKSUM,timeout:process.env.QBWIN_PROVIDER_TIMEOUT_SECONDS},windows:[]}));"#,
        )
        .expect("write source");

        let providers = run_remote_provider(
            "remote-meta",
            "Remote Meta",
            Some(&provider_dir),
            "node",
            None,
            None,
            42,
            temp.path(),
            &HashMap::new(),
            &HashMap::new(),
            &[],
            None,
        );

        assert_eq!(providers[0].status, "ok");
        let metadata = providers[0].metadata.as_ref().expect("metadata");
        assert_eq!(metadata["id"], serde_json::json!("remote-meta"));
        assert_eq!(metadata["version"], serde_json::json!("2.3.4"));
        assert_eq!(metadata["checksum"], serde_json::json!("sha256:abc123"));
        assert_eq!(metadata["timeout"], serde_json::json!("42"));
    }

    #[test]
    fn remote_provider_injects_optional_config_env_vars() {
        let temp = tempfile::tempdir().expect("temp dir");
        let provider_dir = temp.path().join("provider");
        std::fs::create_dir(&provider_dir).expect("create provider dir");
        std::fs::write(
            provider_dir.join("provider.json"),
            serde_json::json!({
                "schemaVersion": 1,
                "id": "remote-optional-env",
                "displayName": "Remote Optional Env",
                "runtime": "node",
                "entry": "provider.cjs",
                "requiredEnvVars": [],
                "output": "provider-snapshot-v1"
            })
            .to_string(),
        )
        .expect("write manifest");
        std::fs::write(
            provider_dir.join("provider.cjs"),
            r#"console.log(JSON.stringify({windows:[{id:"optional",label:process.env.QBWIN_REMOTE_OPTIONAL || "",remainingPercent:50,confidence:"exact"}]}));"#,
        )
        .expect("write source");
        std::env::remove_var("QBWIN_REMOTE_OPTIONAL");

        let env_vars = HashMap::from([(
            "QBWIN_REMOTE_OPTIONAL".to_string(),
            "from-optional-config".to_string(),
        )]);
        let providers = run_remote_provider(
            "remote-optional-env",
            "Remote Optional Env",
            Some(&provider_dir),
            "node",
            None,
            None,
            crate::config::DEFAULT_REMOTE_PROVIDER_TIMEOUT_SECONDS,
            temp.path(),
            &env_vars,
            &HashMap::new(),
            &[],
            None,
        );

        assert_eq!(providers[0].status, "ok");
        assert_eq!(providers[0].windows[0].label, "from-optional-config");
        assert!(std::env::var("QBWIN_REMOTE_OPTIONAL").is_err());
    }

    #[test]
    fn remote_provider_injects_global_proxy_url_as_a_fallback() {
        let temp = tempfile::tempdir().expect("temp dir");
        let provider_dir = temp.path().join("provider");
        std::fs::create_dir(&provider_dir).expect("create provider dir");
        std::fs::write(
            provider_dir.join("provider.json"),
            serde_json::json!({
                "schemaVersion": 1,
                "id": "remote-proxy",
                "displayName": "Remote Proxy",
                "runtime": "node",
                "entry": "provider.cjs",
                "requiredEnvVars": [],
                "output": "provider-snapshot-v1"
            })
            .to_string(),
        )
        .expect("write manifest");
        std::fs::write(
            provider_dir.join("provider.cjs"),
            r#"console.log(JSON.stringify({windows:[{id:"proxy",label:process.env.QBWIN_PROXY_URL || "",remainingPercent:50,confidence:"exact"}]}));"#,
        )
        .expect("write source");

        let providers = run_remote_provider(
            "remote-proxy",
            "Remote Proxy",
            Some(&provider_dir),
            "node",
            None,
            Some(&ProxyConfig {
                kind: ProxyKind::Socks5,
                url: "socks5h://localhost:10818".to_string(),
            }),
            crate::config::DEFAULT_REMOTE_PROVIDER_TIMEOUT_SECONDS,
            temp.path(),
            &HashMap::new(),
            &HashMap::new(),
            &[],
            None,
        );

        assert_eq!(providers[0].status, "ok");
        assert_eq!(providers[0].windows[0].label, "socks5h://localhost:10818");
    }

    #[test]
    fn remote_provider_environment_proxy_overrides_global_proxy() {
        let temp = tempfile::tempdir().expect("temp dir");
        let provider_dir = temp.path().join("provider");
        std::fs::create_dir(&provider_dir).expect("create provider dir");
        std::fs::write(
            provider_dir.join("provider.json"),
            serde_json::json!({
                "schemaVersion": 1,
                "id": "remote-proxy-precedence",
                "displayName": "Remote Proxy Precedence",
                "runtime": "node",
                "entry": "provider.cjs",
                "requiredEnvVars": [],
                "output": "provider-snapshot-v1"
            })
            .to_string(),
        )
        .expect("write manifest");
        std::fs::write(
            provider_dir.join("provider.cjs"),
            r#"console.log(JSON.stringify({windows:[{id:"proxy",label:process.env.QBWIN_PROXY_URL || "",remainingPercent:50,confidence:"exact"}]}));"#,
        )
        .expect("write source");

        let env_vars = HashMap::from([(
            "QBWIN_PROXY_URL".to_string(),
            "socks5h://provider:1080".to_string(),
        )]);
        let global_proxy = ProxyConfig {
            kind: ProxyKind::Http,
            url: "http://global:8080".to_string(),
        };
        let providers = run_remote_provider(
            "remote-proxy-precedence",
            "Remote Proxy Precedence",
            Some(&provider_dir),
            "node",
            None,
            Some(&global_proxy),
            crate::config::DEFAULT_REMOTE_PROVIDER_TIMEOUT_SECONDS,
            temp.path(),
            &env_vars,
            &HashMap::new(),
            &[],
            None,
        );

        assert_eq!(providers[0].status, "ok");
        assert_eq!(providers[0].windows[0].label, "socks5h://provider:1080");
    }
}
