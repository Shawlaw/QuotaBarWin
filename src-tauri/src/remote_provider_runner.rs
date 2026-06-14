use std::{
    collections::{HashMap, HashSet},
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use chrono::Utc;
use serde::Deserialize;

use crate::{
    config::resolve_secret_value,
    quota::{
        clamp_snapshot_percentages, AppSnapshot, ProviderDiagnostics, ProviderSnapshot, QuotaWindow,
    },
    redact::redact_sensitive,
    remote_provider::{ensure_runtime_resolved, load_cached_manifest},
};

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
    config_dir: &Path,
    env_vars: &HashMap<String, String>,
    window_label_overrides: &HashMap<String, String>,
    visible_window_ids: &[String],
) -> Vec<ProviderSnapshot> {
    let Some(provider_dir) = provider_dir else {
        return vec![provider_error_snapshot(
            id,
            name,
            "Remote provider cache directory is missing",
        )];
    };

    let manifest = match load_cached_manifest(provider_dir) {
        Ok(manifest) => manifest,
        Err(error) => {
            return vec![provider_error_snapshot(
                id,
                name,
                &format!("Failed to load remote provider manifest: {error}"),
            )];
        }
    };

    let executable = match ensure_runtime_resolved(runtime, resolved_runtime) {
        Ok(path) => path,
        Err(error) => {
            return vec![provider_error_snapshot(
                id,
                name,
                &format!("Failed to resolve runtime: {error}"),
            )];
        }
    };

    let source_name = remote_source_file_name(&manifest.entry);
    let source_path = provider_dir.join(&source_name);
    if !source_path.exists() {
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
        Err(error) => return vec![provider_error_snapshot(id, name, &error)],
    };

    let env = match resolve_required_env_vars(&manifest.required_env_vars, env_vars, config_dir) {
        Ok(env) => env,
        Err(error) => return vec![provider_error_snapshot(id, name, &error)],
    };

    let command = RemoteCommandSpec {
        executable: executable.display().to_string(),
        args: vec![source_path.display().to_string()],
        cwd: Some(provider_dir.display().to_string()),
        timeout_ms: 15_000,
        env,
    };

    run_remote_command(
        id,
        name,
        &command,
        &output,
        window_label_overrides,
        visible_window_ids,
    )
}

fn run_remote_command(
    id: &str,
    name: &str,
    command: &RemoteCommandSpec,
    output: &RemoteOutputSpec,
    window_label_overrides: &HashMap<String, String>,
    visible_window_ids: &[String],
) -> Vec<ProviderSnapshot> {
    match execute_command(command) {
        Ok(result) if result.timed_out => vec![error_provider(
            id,
            name,
            "Remote provider timed out",
            Some(result),
            Some(&command.executable),
        )],
        Ok(result) if result.exit_code != Some(0) => vec![error_provider(
            id,
            name,
            "Remote provider exited with a non-zero status",
            Some(result),
            Some(&command.executable),
        )],
        Ok(result) => parse_remote_output(
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

fn execute_command(command: &RemoteCommandSpec) -> Result<RawCommandResult, String> {
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

    loop {
        if child
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_some()
        {
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

fn resolve_required_env_vars(
    required_env_vars: &[String],
    configured_env_vars: &HashMap<String, String>,
    config_dir: &Path,
) -> Result<HashMap<String, String>, String> {
    let mut env = HashMap::new();
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

fn parse_remote_output(
    id: &str,
    name: &str,
    output: &RemoteOutputSpec,
    result: RawCommandResult,
    command_path: &str,
    window_label_overrides: &HashMap<String, String>,
    visible_window_ids: &[String],
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
        Err(error) => vec![error_provider(
            id,
            name,
            &format!("Failed to parse remote provider stdout: {error}"),
            Some(result),
            Some(command_path),
        )],
    }
}

fn parse_remote_provider_snapshot_v1(
    id: &str,
    name: &str,
    stdout: &str,
) -> Result<ProviderSnapshot, serde_json::Error> {
    let raw = serde_json::from_str::<RemoteProviderSnapshotV1>(stdout)?;
    Ok(ProviderSnapshot {
        id: raw.id.unwrap_or_else(|| id.to_string()),
        name: raw.name.unwrap_or_else(|| name.to_string()),
        status: raw.status.unwrap_or_else(|| "ok".to_string()),
        source: "remote".to_string(),
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
    fn remote_provider_snapshot_output_uses_remote_source() {
        let script = r#"console.log(JSON.stringify({id:'remote-fixture',name:'Remote Fixture',status:'ok',source:'script',updatedAt:null,windows:[{id:'weekly',label:'Weekly',remainingPercent:88,confidence:'exact'}]}));"#;
        let providers = run_remote_command(
            "remote-fixture",
            "Remote Fixture",
            &node_command(script),
            &RemoteOutputSpec::ProviderSnapshotV1,
            &HashMap::new(),
            &[],
        );

        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].id, "remote-fixture");
        assert_eq!(providers[0].source, "remote");
        assert_eq!(providers[0].windows[0].remaining_percent, Some(88.0));
        assert!(providers[0].diagnostics.is_some());
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
        );

        assert_eq!(providers[0].status, "error");
        assert_eq!(providers[0].source, "remote");
        assert_eq!(
            providers[0].diagnostics.as_ref().unwrap().exit_code,
            Some(1)
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
            temp.path(),
            &env_vars,
            &HashMap::new(),
            &[],
        );

        assert_eq!(providers[0].status, "ok");
        assert_eq!(providers[0].windows[0].label, "from-config-map");
        assert!(std::env::var("QBWIN_REMOTE_CHILD_TOKEN").is_err());
    }
}
