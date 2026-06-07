use std::{
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use chrono::Utc;

use crate::{
    config::{CommandSpec, ParserSpec},
    quota::{clamp_snapshot_percentages, AppSnapshot, ProviderDiagnostics, ProviderSnapshot},
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
        Ok(result) => parse_command_output(id, name, parser, result, &command.executable),
        Err(error) => vec![error_provider(
            id,
            name,
            &error,
            None,
            Some(&command.executable),
        )],
    }
}

fn execute_command(command: &CommandSpec) -> Result<RawCommandResult, String> {
    let started = Instant::now();
    let mut process = Command::new(&command.executable);
    process.args(&command.args);
    process.stdout(Stdio::piped());
    process.stderr(Stdio::piped());

    if let Some(cwd) = &command.cwd {
        process.current_dir(cwd);
    }

    if let Some(env) = &command.env {
        for (key, value) in env {
            process.env(key, resolve_env_value(value));
        }
    }

    let mut child = process.spawn().map_err(|error| error.to_string())?;
    let timeout = Duration::from_millis(command.timeout_ms.max(1));

    loop {
        if let Some(_status) = child.try_wait().map_err(|error| error.to_string())? {
            let output = child.wait_with_output().map_err(|error| error.to_string())?;
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
            let output = child.wait_with_output().map_err(|error| error.to_string())?;
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

fn resolve_env_value(value: &str) -> String {
    if let Some(name) = value
        .strip_prefix("${env:")
        .and_then(|remaining| remaining.strip_suffix('}'))
    {
        return std::env::var(name).unwrap_or_default();
    }

    value.to_string()
}

fn parse_command_output(
    id: &str,
    name: &str,
    parser: &ParserSpec,
    result: RawCommandResult,
    command_path: &str,
) -> Vec<ProviderSnapshot> {
    let diagnostics = diagnostics_from_result(&result, Some(command_path));
    let parsed = match parser {
        ParserSpec::AppSnapshot => serde_json::from_str::<AppSnapshot>(&result.stdout)
            .map(|snapshot| snapshot.providers),
        ParserSpec::ProviderSnapshot => {
            serde_json::from_str::<ProviderSnapshot>(&result.stdout).map(|provider| vec![provider])
        }
    };

    match parsed {
        Ok(mut providers) => {
            for provider in &mut providers {
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
    use crate::config::{CommandSpec, ParserSpec};

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
        );

        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].name, "Fake Command Provider");
        assert_eq!(providers[0].source, "command");
    }

    #[test]
    fn command_provider_parses_app_snapshot_stdout() {
        let providers = run_command_provider(
            "fake-app",
            "Fake App",
            &node_command("fake_app_snapshot.js"),
            &ParserSpec::AppSnapshot,
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
        );

        assert_eq!(providers[0].status, "error");
        assert_eq!(providers[0].diagnostics.as_ref().unwrap().exit_code, Some(7));
    }

    #[test]
    fn command_provider_handles_timeout() {
        let mut command = node_command("fake_slow.js");
        command.timeout_ms = 50;

        let providers =
            run_command_provider("slow", "Slow", &command, &ParserSpec::ProviderSnapshot);

        assert_eq!(providers[0].status, "error");
        assert_eq!(providers[0].diagnostics.as_ref().unwrap().timed_out, Some(true));
    }

    #[test]
    fn command_provider_handles_invalid_json() {
        let providers = run_command_provider(
            "invalid",
            "Invalid",
            &node_command("fake_invalid_json.js"),
            &ParserSpec::ProviderSnapshot,
        );

        assert_eq!(providers[0].status, "error");
        assert!(providers[0]
            .error
            .as_ref()
            .expect("error")
            .contains("Failed to parse command stdout"));
    }
}
