use std::{
    collections::HashMap,
    fs,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use chrono::Utc;
use serde::Deserialize;

use crate::{
    config::{CommandSpec, ParserSpec, ScriptOutputSpec},
    parser::{parse_bigmodel_quota_limit_json, parse_kimi_coding_usage},
    quota::{
        clamp_snapshot_percentages, AppSnapshot, ProviderDiagnostics, ProviderSnapshot, QuotaWindow,
    },
    redact::redact_sensitive,
};

#[derive(Debug, Clone)]
pub struct RawCommandResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub timed_out: bool,
}

pub fn run_command_provider(
    id: &str,
    name: &str,
    command: &CommandSpec,
    parser: &ParserSpec,
    window_label_overrides: &HashMap<String, String>,
    visible_window_ids: &[String],
) -> Vec<ProviderSnapshot> {
    match execute_command(command) {
        Ok(result) if result.timed_out => vec![error_provider(
            id,
            name,
            "Command timed out",
            Some(result),
            Some(&command.executable),
        )],
        Ok(result) if result.exit_code != Some(0) => vec![error_provider(
            id,
            name,
            "Command exited with a non-zero status",
            Some(result),
            Some(&command.executable),
        )],
        Ok(result) => parse_command_output(
            id,
            name,
            parser,
            result,
            &command.executable,
            window_label_overrides,
            visible_window_ids,
        ),
        Err(error) => vec![error_provider(
            id,
            name,
            &error,
            None,
            Some(&command.executable),
        )],
    }
}

pub fn run_script_provider(
    id: &str,
    name: &str,
    command: &CommandSpec,
    output: &ScriptOutputSpec,
    window_label_overrides: &HashMap<String, String>,
    visible_window_ids: &[String],
) -> Vec<ProviderSnapshot> {
    match execute_command(command) {
        Ok(result) if result.timed_out => vec![error_provider(
            id,
            name,
            "Command timed out",
            Some(result),
            Some(&command.executable),
        )],
        Ok(result) if result.exit_code != Some(0) => vec![error_provider(
            id,
            name,
            "Command exited with a non-zero status",
            Some(result),
            Some(&command.executable),
        )],
        Ok(result) => parse_script_output(
            id,
            name,
            output,
            result,
            &command.executable,
            window_label_overrides,
            visible_window_ids,
        ),
        Err(error) => vec![error_provider(
            id,
            name,
            &error,
            None,
            Some(&command.executable),
        )],
    }
}

pub fn run_single_provider_config(provider: &crate::config::ProviderConfig) -> ProviderSnapshot {
    match provider {
        crate::config::ProviderConfig::Mock { id, name, .. } => {
            crate::providers::mock::provider_snapshot(id, name, &[])
        }
        crate::config::ProviderConfig::Codex {
            id,
            name,
            auth_token,
            account_id,
            proxy_url,
            timeout_ms,
            window_label_overrides,
            visible_window_ids,
            ..
        } => crate::providers::codex::provider_snapshot(
            id,
            name,
            auth_token,
            account_id.as_deref(),
            proxy_url.as_deref(),
            *timeout_ms,
            window_label_overrides,
            visible_window_ids,
        ),
        crate::config::ProviderConfig::Command {
            id,
            name,
            command,
            parser,
            window_label_overrides,
            visible_window_ids,
            ..
        } => run_command_provider(
            id,
            name,
            command,
            parser,
            window_label_overrides,
            visible_window_ids,
        )
        .into_iter()
        .next()
        .unwrap_or_else(|| error_provider(id, name, "Provider returned no snapshot", None, None)),
        crate::config::ProviderConfig::Script {
            id,
            name,
            command,
            output,
            window_label_overrides,
            visible_window_ids,
            ..
        } => run_script_provider(
            id,
            name,
            command,
            output,
            window_label_overrides,
            visible_window_ids,
        )
        .into_iter()
        .next()
        .unwrap_or_else(|| error_provider(id, name, "Provider returned no snapshot", None, None)),
    }
}

fn execute_command(command: &CommandSpec) -> Result<RawCommandResult, String> {
    let started = Instant::now();
    let mut process = Command::new(&command.executable);
    let args = resolve_args(&command.args)?;
    process.args(args);
    process.stdout(Stdio::piped());
    process.stderr(Stdio::piped());
    suppress_command_window(&mut process);

    if let Some(cwd) = &command.cwd {
        process.current_dir(cwd);
    }

    if let Some(env) = &command.env {
        for (key, value) in env {
            process.env(key, resolve_env_value(value)?);
        }
    }

    let mut child = process.spawn().map_err(|error| error.to_string())?;
    let timeout = Duration::from_millis(command.timeout_ms.max(1));

    loop {
        if let Some(_status) = child.try_wait().map_err(|error| error.to_string())? {
            let output = child
                .wait_with_output()
                .map_err(|error| error.to_string())?;
            return Ok(RawCommandResult {
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: redact_sensitive(&String::from_utf8_lossy(&output.stderr)),
                exit_code: output.status.code(),
                duration_ms: started.elapsed().as_millis() as u64,
                timed_out: false,
            });
        }

        if started.elapsed() >= timeout {
            let _ = child.kill();
            let output = child
                .wait_with_output()
                .map_err(|error| error.to_string())?;
            return Ok(RawCommandResult {
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: redact_sensitive(&String::from_utf8_lossy(&output.stderr)),
                exit_code: output.status.code(),
                duration_ms: started.elapsed().as_millis() as u64,
                timed_out: true,
            });
        }

        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(windows)]
fn suppress_command_window(command: &mut Command) {
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn suppress_command_window(_command: &mut Command) {}

fn resolve_env_value(value: &str) -> Result<String, String> {
    if let Some(name) = value
        .strip_prefix("${env:")
        .and_then(|remaining| remaining.strip_suffix('}'))
    {
        return std::env::var(name).map_err(|_| format!("Missing environment variable {name}"));
    }

    resolve_placeholders(value)
}

fn resolve_args(args: &[String]) -> Result<Vec<String>, String> {
    args.iter().map(|arg| resolve_placeholders(arg)).collect()
}

fn resolve_placeholders(input: &str) -> Result<String, String> {
    let with_env = resolve_env_placeholders(input)?;
    resolve_file_placeholders(&with_env)
}

fn resolve_env_placeholders(input: &str) -> Result<String, String> {
    let mut output = String::new();
    let mut remaining = input;

    while let Some(start) = remaining.find("${env:") {
        output.push_str(&remaining[..start]);
        let after_start = &remaining[start + 6..];
        let Some(end) = after_start.find('}') else {
            output.push_str(&remaining[start..]);
            return Ok(output);
        };
        let name = &after_start[..end];
        let value =
            std::env::var(name).map_err(|_| format!("Missing environment variable {name}"))?;
        output.push_str(&value);
        remaining = &after_start[end + 1..];
    }

    output.push_str(remaining);
    Ok(output)
}

fn resolve_file_placeholders(input: &str) -> Result<String, String> {
    let mut output = String::new();
    let mut remaining = input;

    while let Some(start) = remaining.find("${file:") {
        output.push_str(&remaining[..start]);
        let after_start = &remaining[start + 7..];
        let Some(end) = after_start.find('}') else {
            output.push_str(&remaining[start..]);
            return Ok(output);
        };
        let path = normalize_file_placeholder_path(&after_start[..end]);
        let value = fs::read_to_string(path)
            .map_err(|error| format!("Unable to read secret file {path}: {error}"))?;
        output.push_str(&value);
        remaining = &after_start[end + 1..];
    }

    output.push_str(remaining);
    Ok(output.trim().to_string())
}

fn normalize_file_placeholder_path(raw_path: &str) -> &str {
    let path = raw_path.trim();
    if path.len() >= 2 {
        let first = path.as_bytes()[0];
        let last = path.as_bytes()[path.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return &path[1..path.len() - 1];
        }
    }

    path
}

fn parse_command_output(
    id: &str,
    name: &str,
    parser: &ParserSpec,
    result: RawCommandResult,
    command_path: &str,
    window_label_overrides: &HashMap<String, String>,
    visible_window_ids: &[String],
) -> Vec<ProviderSnapshot> {
    let diagnostics = diagnostics_from_result(&result, Some(command_path));
    let parsed = match parser {
        ParserSpec::AppSnapshot => {
            serde_json::from_str::<AppSnapshot>(&result.stdout).map(|snapshot| snapshot.providers)
        }
        ParserSpec::ProviderSnapshot => {
            serde_json::from_str::<ProviderSnapshot>(&result.stdout).map(|provider| vec![provider])
        }
        ParserSpec::KimiCodingUsageV1 => {
            Ok(vec![parse_kimi_coding_usage(id, name, &result.stdout)])
        }
        ParserSpec::BigmodelQuotaLimitJsonV1 => Ok(vec![parse_bigmodel_quota_limit_json(
            id,
            name,
            &result.stdout,
        )]),
        ParserSpec::JsonMapping { .. } => Ok(vec![error_provider(
            id,
            name,
            "json-mapping parser is not configured in V2",
            Some(result.clone()),
            Some(command_path),
        )]),
        ParserSpec::RegexBlocks { .. } => Ok(vec![error_provider(
            id,
            name,
            "regex-blocks parser is not configured in V2",
            Some(result.clone()),
            Some(command_path),
        )]),
    };

    match parsed {
        Ok(mut providers) => {
            for provider in &mut providers {
                apply_window_label_overrides(provider, window_label_overrides);
                apply_visible_windows(provider, visible_window_ids);
                provider.source = "command".to_string();
                if provider.diagnostics.is_none() {
                    provider.diagnostics = Some(diagnostics.clone());
                }
                clamp_snapshot_percentages(provider);
            }
            providers
        }
        Err(error) => vec![error_provider(
            id,
            name,
            &format!("Failed to parse command stdout: {error}"),
            Some(result),
            Some(command_path),
        )],
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScriptProviderSnapshotV1 {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
    windows: Vec<ScriptQuotaWindowV1>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    diagnostics: Option<ProviderDiagnostics>,
    #[serde(default)]
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScriptQuotaWindowV1 {
    id: String,
    label: String,
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
    reset_at: Option<String>,
    #[serde(default)]
    reset_text: Option<String>,
    #[serde(default)]
    confidence: Option<String>,
}

fn parse_script_output(
    id: &str,
    name: &str,
    output: &ScriptOutputSpec,
    result: RawCommandResult,
    command_path: &str,
    window_label_overrides: &HashMap<String, String>,
    visible_window_ids: &[String],
) -> Vec<ProviderSnapshot> {
    let diagnostics = diagnostics_from_result(&result, Some(command_path));
    let parsed = match output {
        ScriptOutputSpec::ProviderSnapshotV1 => parse_script_provider_snapshot_v1(
            id,
            name,
            &result.stdout,
        )
        .map(|provider| vec![provider]),
        ScriptOutputSpec::AppSnapshotV1 => {
            serde_json::from_str::<AppSnapshot>(&result.stdout).map(|snapshot| snapshot.providers)
        }
    };

    match parsed {
        Ok(mut providers) => {
            for provider in &mut providers {
                apply_window_label_overrides(provider, window_label_overrides);
                apply_visible_windows(provider, visible_window_ids);
                provider.source = "script".to_string();
                if provider.diagnostics.is_none() {
                    provider.diagnostics = Some(diagnostics.clone());
                }
                clamp_snapshot_percentages(provider);
            }
            providers
        }
        Err(error) => vec![error_provider(
            id,
            name,
            &format!("Failed to parse script stdout: {error}"),
            Some(result),
            Some(command_path),
        )],
    }
}

fn parse_script_provider_snapshot_v1(
    id: &str,
    name: &str,
    stdout: &str,
) -> Result<ProviderSnapshot, serde_json::Error> {
    let raw = serde_json::from_str::<ScriptProviderSnapshotV1>(stdout)?;
    Ok(ProviderSnapshot {
        id: raw.id.unwrap_or_else(|| id.to_string()),
        name: raw.name.unwrap_or_else(|| name.to_string()),
        status: raw.status.unwrap_or_else(|| "ok".to_string()),
        source: raw.source.unwrap_or_else(|| "script".to_string()),
        updated_at: raw.updated_at,
        windows: raw
            .windows
            .into_iter()
            .map(|window| QuotaWindow {
                id: window.id,
                label: window.label,
                used: window.used,
                limit: window.limit,
                unit: window.unit,
                used_percent: window.used_percent,
                remaining_percent: window.remaining_percent,
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

    provider.windows.retain(|window| {
        visible_window_ids.iter().any(|visible| {
            let visible = visible.trim();
            !visible.is_empty() && (visible == window.id || visible == window.label)
        })
    });
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
        source: "command".to_string(),
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
    use crate::config::{CommandSpec, ParserSpec, ScriptOutputSpec};

    fn fixture(name: &str) -> String {
        let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest
            .parent()
            .expect("repo root")
            .join("fixtures")
            .join(name)
            .to_string_lossy()
            .to_string()
    }

    fn node_command(script: &str) -> CommandSpec {
        CommandSpec {
            executable: "node".to_string(),
            args: vec![fixture(script)],
            cwd: None,
            env: None,
            timeout_ms: 2000,
        }
    }

    #[test]
    fn command_provider_parses_provider_snapshot_stdout() {
        let providers = run_command_provider(
            "fake",
            "Fake",
            &node_command("fake_provider_snapshot.js"),
            &ParserSpec::ProviderSnapshot,
            &HashMap::new(),
            &[],
        );

        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].name, "Fake Command Provider");
        assert_eq!(providers[0].source, "command");
    }

    #[test]
    fn script_provider_accepts_minimal_provider_snapshot_v1_stdout() {
        let providers = run_script_provider(
            "script-fixture",
            "Script Fixture",
            &node_command("fake_script_provider_snapshot_v1.js"),
            &ScriptOutputSpec::ProviderSnapshotV1,
            &HashMap::new(),
            &[],
        );

        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].id, "script-fixture");
        assert_eq!(providers[0].name, "Script Fixture");
        assert_eq!(providers[0].source, "script");
        assert_eq!(providers[0].windows[0].remaining_percent, Some(88.0));
        assert!(providers[0].diagnostics.is_some());
    }

    #[test]
    fn execute_command_passes_resolved_cli_args_in_order() {
        let temp = tempfile::tempdir().expect("temp dir");
        let secret_path = temp.path().join("api key.txt");
        std::fs::write(&secret_path, "secret from file\n").expect("write secret");
        let command = CommandSpec {
            executable: "node".to_string(),
            args: vec![
                "-e".to_string(),
                "console.log(JSON.stringify(process.argv.slice(1)))".to_string(),
                "--".to_string(),
                "--provider".to_string(),
                "two words".to_string(),
                format!("${{file:\"{}\"}}", secret_path.display()),
            ],
            cwd: None,
            env: None,
            timeout_ms: 2000,
        };

        let result = execute_command(&command).expect("execute command");
        let args =
            serde_json::from_str::<Vec<String>>(result.stdout.trim()).expect("parse stdout args");

        assert_eq!(
            args,
            vec![
                "--provider".to_string(),
                "two words".to_string(),
                "secret from file".to_string()
            ]
        );
    }

    #[test]
    fn command_provider_parses_app_snapshot_stdout() {
        let providers = run_command_provider(
            "fake-app",
            "Fake App",
            &node_command("fake_app_snapshot.js"),
            &ParserSpec::AppSnapshot,
            &HashMap::new(),
            &[],
        );

        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].id, "fake-app-provider");
    }

    #[test]
    fn command_provider_handles_non_zero_exit() {
        let providers = run_command_provider(
            "bad",
            "Bad",
            &node_command("fake_error.js"),
            &ParserSpec::ProviderSnapshot,
            &HashMap::new(),
            &[],
        );

        assert_eq!(providers[0].status, "error");
        assert_eq!(
            providers[0].diagnostics.as_ref().unwrap().exit_code,
            Some(7)
        );
    }

    #[test]
    fn command_provider_handles_timeout() {
        let mut command = node_command("fake_slow.js");
        command.timeout_ms = 50;

        let providers = run_command_provider(
            "slow",
            "Slow",
            &command,
            &ParserSpec::ProviderSnapshot,
            &HashMap::new(),
            &[],
        );

        assert_eq!(providers[0].status, "error");
        assert_eq!(
            providers[0].diagnostics.as_ref().unwrap().timed_out,
            Some(true)
        );
    }

    #[test]
    fn command_provider_handles_invalid_json() {
        let providers = run_command_provider(
            "invalid",
            "Invalid",
            &node_command("fake_invalid_json.js"),
            &ParserSpec::ProviderSnapshot,
            &HashMap::new(),
            &[],
        );

        assert_eq!(providers[0].status, "error");
        assert!(providers[0]
            .error
            .as_ref()
            .expect("error")
            .contains("Failed to parse command stdout"));
    }

    #[test]
    fn missing_env_var_returns_error_without_secret() {
        let mut command = node_command("fake_provider_snapshot.js");
        command
            .args
            .push("Authorization: Bearer ${env:QUOTABARWIN_TEST_MISSING_SECRET}".to_string());

        let providers = run_command_provider(
            "env",
            "Env",
            &command,
            &ParserSpec::ProviderSnapshot,
            &HashMap::new(),
            &[],
        );

        assert_eq!(providers[0].status, "error");
        let error = providers[0].error.as_ref().expect("error");
        assert!(error.contains("QUOTABARWIN_TEST_MISSING_SECRET"));
        assert!(!error.contains("KIMI_API_KEY="));
    }

    #[test]
    fn window_label_overrides_apply_after_parsing() {
        let mut overrides = HashMap::new();
        overrides.insert("weekly".to_string(), "Custom label".to_string());

        let providers = run_command_provider(
            "fake",
            "Fake",
            &node_command("fake_provider_snapshot.js"),
            &ParserSpec::ProviderSnapshot,
            &overrides,
            &[],
        );

        assert_eq!(providers[0].windows[0].label, "Custom label");
    }

    #[test]
    fn window_label_overrides_can_match_existing_label() {
        let mut overrides = HashMap::new();
        overrides.insert("Weekly".to_string(), "Team weekly".to_string());

        let providers = run_command_provider(
            "fake",
            "Fake",
            &node_command("fake_provider_snapshot.js"),
            &ParserSpec::ProviderSnapshot,
            &overrides,
            &[],
        );

        assert_eq!(providers[0].windows[0].label, "Team weekly");
    }

    #[test]
    fn visible_windows_filter_after_label_overrides() {
        let mut overrides = HashMap::new();
        overrides.insert("weekly".to_string(), "Team weekly".to_string());
        let visible = vec!["Team weekly".to_string()];

        let providers = run_command_provider(
            "fake",
            "Fake",
            &node_command("fake_provider_snapshot.js"),
            &ParserSpec::ProviderSnapshot,
            &overrides,
            &visible,
        );

        assert_eq!(providers[0].windows.len(), 1);
        assert_eq!(providers[0].windows[0].label, "Team weekly");
    }

    #[test]
    fn file_secret_placeholder_reads_trimmed_file() {
        let temp = tempfile::tempdir().expect("temp dir");
        let secret_path = temp.path().join("kimi.key");
        std::fs::write(&secret_path, "secret-from-file\n").expect("write secret");
        let input = format!("Authorization: Bearer ${{file:{}}}", secret_path.display());

        let output = resolve_placeholders(&input).expect("resolve file");

        assert_eq!(output, "Authorization: Bearer secret-from-file");
    }

    #[test]
    fn file_secret_placeholder_accepts_paths_with_spaces() {
        let temp = tempfile::tempdir().expect("temp dir");
        let secret_dir = temp.path().join("kimi secrets");
        std::fs::create_dir(&secret_dir).expect("create secret dir");
        let secret_path = secret_dir.join("api key.txt");
        std::fs::write(&secret_path, "secret-from-spaced-path\n").expect("write secret");
        let input = format!("Authorization: Bearer ${{file:{}}}", secret_path.display());

        let output = resolve_placeholders(&input).expect("resolve file");

        assert_eq!(output, "Authorization: Bearer secret-from-spaced-path");
    }

    #[test]
    fn file_secret_placeholder_accepts_quoted_paths() {
        let temp = tempfile::tempdir().expect("temp dir");
        let secret_dir = temp.path().join("bigmodel secrets");
        std::fs::create_dir(&secret_dir).expect("create secret dir");
        let secret_path = secret_dir.join("api key.txt");
        std::fs::write(&secret_path, "secret-from-quoted-path\n").expect("write secret");
        let input = format!(
            "Authorization: Bearer ${{file:\"{}\"}}",
            secret_path.display()
        );

        let output = resolve_placeholders(&input).expect("resolve file");

        assert_eq!(output, "Authorization: Bearer secret-from-quoted-path");
    }

    #[test]
    fn missing_file_secret_returns_path_not_secret() {
        let temp = tempfile::tempdir().expect("temp dir");
        let secret_path = temp.path().join("missing.key");
        let input = format!("Authorization: Bearer ${{file:{}}}", secret_path.display());

        let error = resolve_placeholders(&input).expect_err("missing file");

        assert!(error.contains("Unable to read secret file"));
        assert!(error.contains("missing.key"));
        assert!(!error.contains("Bearer"));
    }
}
