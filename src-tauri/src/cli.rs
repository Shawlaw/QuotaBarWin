use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;

use crate::{
    app_info::app_display_version,
    config::{config_path_for_current_executable, load_or_create_config, ProviderConfig},
    quota::{
        build_app_snapshot_from_config_path, get_cached_snapshot_from_config_path,
        refresh_provider_from_config_path, AppSnapshot, ProviderSnapshot, QuotaWindow,
    },
    redact::redact_sensitive,
    remote_provider::{
        is_builtin_js_runtime, parse_manifest, resolve_runtime, validate_runtime_executable,
        verify_checksum, ProviderManifest, BUILTIN_JS_RUNTIME,
    },
    remote_provider_runner::run_remote_provider,
};

const EXIT_READY: i32 = 0;
const EXIT_LOW_QUOTA: i32 = 10;
const EXIT_UNKNOWN: i32 = 11;
const EXIT_FAILURE: i32 = 20;
const EXIT_INVALID_PROVIDER: i32 = 30;
const EXIT_USAGE: i32 = 64;

const HELP: &str = r#"QuotaBarWin CLI - machine-readable quota checks for agents

Usage:
  QuotaBarWin.Cli.exe get [--provider ID] [--window ID] [--refresh|--cached] [--config PATH]
  QuotaBarWin.Cli.exe check --provider ID --window ID --min-remaining-percent NUMBER
                            [--refresh|--cached] [--config PATH]
  QuotaBarWin.Cli.exe validate (--provider ID | --manifest PATH) [--source PATH] [--config PATH]

Commands always write one JSON object to stdout, except --help and --version.

Exit codes for check:
  0   quota is at or above the requested threshold
  10  quota is below the requested threshold; defer or switch work
  11  quota cannot be safely assessed (provider error/stale data/missing percentage)
  20  refresh, configuration, or lookup failed
  30  provider validation found one or more errors

Options:
  --refresh                         Refresh the Provider before reading (default).
  --cached                          Read the last on-disk snapshot without a network refresh.
  --provider ID                     Installed Provider instance ID.
  --window ID                       Stable quota window ID, such as 5h or weekly.
  --min-remaining-percent NUMBER    Required by check; range 0 through 100.
  --config PATH                     Use this config file instead of the normal portable/AppData config.
  --manifest PATH                   Validate a Provider manifest and its adjacent source script.
  --source PATH                     Source script to validate with --manifest (overrides manifest entry).
  --run                             With --provider, execute the cached script and validate its output.
  --json                            Accepted for script compatibility; JSON is always used.
  --help, -h                        Show this help.
  --version                         Show the CLI version.
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SnapshotMode {
    Refresh,
    Cached,
}

#[derive(Debug, Clone, PartialEq)]
enum Command {
    Get(Options),
    Check(Options),
    Validate(ValidationOptions),
    Help,
    Version,
}

#[derive(Debug, Clone, PartialEq)]
struct ValidationOptions {
    provider_id: Option<String>,
    manifest_path: Option<PathBuf>,
    source_path: Option<PathBuf>,
    config_path: Option<PathBuf>,
    run_script: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct Options {
    provider_id: Option<String>,
    window_id: Option<String>,
    min_remaining_percent: Option<f64>,
    config_path: Option<PathBuf>,
    snapshot_mode: SnapshotMode,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            provider_id: None,
            window_id: None,
            min_remaining_percent: None,
            config_path: None,
            snapshot_mode: SnapshotMode::Refresh,
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CliSnapshot {
    schema_version: u8,
    data_source: &'static str,
    refreshed_at: String,
    providers: Vec<CliProvider>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CliProvider {
    id: String,
    name: String,
    status: String,
    updated_at: Option<String>,
    windows: Vec<CliWindow>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CliWindow {
    id: String,
    label: String,
    remaining: Option<f64>,
    used: Option<f64>,
    limit: Option<f64>,
    unit: Option<String>,
    used_percent: Option<f64>,
    remaining_percent: Option<f64>,
    reset_at: Option<String>,
    reset_text: Option<String>,
    confidence: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CheckResult {
    schema_version: u8,
    decision: &'static str,
    threshold_remaining_percent: f64,
    data_source: &'static str,
    refreshed_at: String,
    provider: CliProvider,
    window: CliWindow,
}

#[derive(Serialize)]
struct CliError<'a> {
    #[serde(rename = "schemaVersion")]
    schema_version: u8,
    error: CliErrorDetail<'a>,
}

#[derive(Serialize)]
struct CliErrorDetail<'a> {
    code: &'a str,
    message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ValidationReport {
    schema_version: u8,
    target: ValidationTarget,
    valid: bool,
    errors: Vec<String>,
    warnings: Vec<String>,
    runtime: ValidationRuntime,
    checksum: ValidationChecksum,
    script_run: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ValidationTarget {
    kind: &'static str,
    provider_id: Option<String>,
    manifest_path: String,
    source_path: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ValidationRuntime {
    declared: Option<String>,
    configured: Option<String>,
    resolved: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ValidationChecksum {
    expected: Option<String>,
    actual: Option<String>,
    matches: Option<bool>,
}

pub fn run_cli() -> i32 {
    match parse_command(std::env::args().skip(1)) {
        Ok(Command::Help) => {
            print!("{HELP}");
            EXIT_READY
        }
        Ok(Command::Version) => {
            println!("QuotaBarWin CLI {}", app_display_version());
            EXIT_READY
        }
        Ok(Command::Get(options)) => match run_get(options) {
            Ok(snapshot) => emit_json(&snapshot, EXIT_READY),
            Err(error) => emit_error("get_failed", error, EXIT_FAILURE),
        },
        Ok(Command::Check(options)) => match run_check(options) {
            Ok((result, exit_code)) => emit_json(&result, exit_code),
            Err(CheckError::Unknown(result)) => emit_json(&result, EXIT_UNKNOWN),
            Err(CheckError::Failure(error)) => emit_error("check_failed", error, EXIT_FAILURE),
        },
        Ok(Command::Validate(options)) => match run_validate(options) {
            Ok(report) => {
                let exit_code = if report.valid {
                    EXIT_READY
                } else {
                    EXIT_INVALID_PROVIDER
                };
                emit_json(&report, exit_code)
            }
            Err(error) => emit_error("validate_failed", error, EXIT_FAILURE),
        },
        Err(error) => emit_error("invalid_arguments", error, EXIT_USAGE),
    }
}

fn emit_json(value: &impl Serialize, exit_code: i32) -> i32 {
    match serde_json::to_string(value) {
        Ok(json) => {
            println!("{json}");
            exit_code
        }
        Err(error) => {
            eprintln!("Failed to serialize CLI result: {error}");
            EXIT_FAILURE
        }
    }
}

fn emit_error(code: &str, error: String, exit_code: i32) -> i32 {
    let value = CliError {
        schema_version: 1,
        error: CliErrorDetail {
            code,
            message: redact_sensitive(&error),
        },
    };
    emit_json(&value, exit_code)
}

fn run_get(options: Options) -> Result<CliSnapshot, String> {
    let (snapshot, data_source) = load_snapshot(&options)?;
    let providers = select_providers(
        &snapshot,
        options.provider_id.as_deref(),
        options.window_id.as_deref(),
    )?;
    Ok(CliSnapshot {
        schema_version: 1,
        data_source,
        refreshed_at: snapshot.refreshed_at.clone(),
        providers: providers
            .iter()
            .map(|provider| cli_provider_for_window(provider, options.window_id.as_deref()))
            .collect(),
    })
}

enum CheckError {
    Unknown(CheckResult),
    Failure(String),
}

fn run_check(options: Options) -> Result<(CheckResult, i32), CheckError> {
    let provider_id = options
        .provider_id
        .as_deref()
        .ok_or_else(|| CheckError::Failure("--provider is required for check".to_string()))?;
    let window_id = options
        .window_id
        .as_deref()
        .ok_or_else(|| CheckError::Failure("--window is required for check".to_string()))?;
    let threshold = options.min_remaining_percent.ok_or_else(|| {
        CheckError::Failure("--min-remaining-percent is required for check".to_string())
    })?;
    let (snapshot, data_source) = load_snapshot(&options).map_err(CheckError::Failure)?;
    let provider = snapshot
        .providers
        .iter()
        .find(|provider| provider.id == provider_id)
        .ok_or_else(|| CheckError::Failure(format!("Provider {provider_id} was not found")))?;
    let window = provider
        .windows
        .iter()
        .find(|window| window.id == window_id)
        .ok_or_else(|| CheckError::Failure(format!("Quota window {window_id} was not found")))?;
    let provider_output = cli_provider(provider);
    let window_output = cli_window(window);
    let base = |decision| CheckResult {
        schema_version: 1,
        decision,
        threshold_remaining_percent: threshold,
        data_source,
        refreshed_at: snapshot.refreshed_at.clone(),
        provider: provider_output.clone(),
        window: window_output.clone(),
    };

    match evaluate_quota_decision(provider, window, threshold) {
        QuotaDecision::Continue => Ok((base("continue"), EXIT_READY)),
        QuotaDecision::Defer => Ok((base("defer"), EXIT_LOW_QUOTA)),
        QuotaDecision::Unknown => Err(CheckError::Unknown(base("unknown"))),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuotaDecision {
    Continue,
    Defer,
    Unknown,
}

fn evaluate_quota_decision(
    provider: &ProviderSnapshot,
    window: &QuotaWindow,
    threshold: f64,
) -> QuotaDecision {
    if matches!(provider.status.as_str(), "error" | "stale") {
        return QuotaDecision::Unknown;
    }
    match window.remaining_percent.filter(|value| value.is_finite()) {
        Some(remaining) if remaining >= threshold => QuotaDecision::Continue,
        Some(_) => QuotaDecision::Defer,
        None => QuotaDecision::Unknown,
    }
}

fn run_validate(options: ValidationOptions) -> Result<ValidationReport, String> {
    match (&options.provider_id, &options.manifest_path) {
        (Some(_), None) => validate_installed_provider(&options),
        (None, Some(manifest_path)) => {
            validate_provider_files(manifest_path, options.source_path.as_deref())
        }
        _ => Err("validate requires exactly one of --provider or --manifest".to_string()),
    }
}

fn validate_installed_provider(options: &ValidationOptions) -> Result<ValidationReport, String> {
    let provider_id = options.provider_id.as_deref().unwrap_or_default();
    let config_path = options
        .config_path
        .clone()
        .map(Ok)
        .unwrap_or_else(config_path_for_current_executable)?;
    let loaded = load_or_create_config(&config_path)?;
    let global_proxy = loaded.config.network_proxy.clone();
    let provider = loaded
        .config
        .providers
        .iter()
        .find(|provider| match provider {
            ProviderConfig::Remote { id, .. } => id == provider_id,
        })
        .ok_or_else(|| format!("Provider {provider_id} was not found"))?;
    let ProviderConfig::Remote {
        id,
        name,
        provider_dir,
        runtime,
        resolved_runtime,
        timeout_seconds,
        env_vars,
        window_label_overrides,
        visible_window_ids,
        ..
    } = provider;
    let provider_dir = provider_dir.as_ref().ok_or_else(|| {
        format!("Provider {provider_id} has no cached providerDir; reinstall the Provider")
    })?;
    let manifest_path = provider_dir.join("provider.json");
    let manifest_text = fs::read_to_string(&manifest_path).map_err(|error| error.to_string())?;
    let manifest = parse_manifest(&manifest_text).map_err(|error| error.to_string())?;
    let source_path = provider_dir.join(source_file_name(&manifest.entry));
    let mut report = validate_manifest_and_source(
        "installed",
        Some(id.clone()),
        &manifest_path,
        Some(&source_path),
        &manifest,
        Some(runtime),
        resolved_runtime.as_deref(),
    );
    if name.trim().is_empty() {
        report
            .errors
            .push("Provider config is missing name".to_string());
    }
    if *timeout_seconds == 0 {
        report
            .errors
            .push("Provider config timeoutSeconds must be at least 1".to_string());
    }
    if runtime != &manifest.runtime {
        report.errors.push(format!(
            "Provider config runtime '{}' does not match manifest runtime '{}'",
            runtime, manifest.runtime
        ));
    }
    for required in &manifest.required_env_vars {
        if !env_vars.contains_key(required) {
            report.warnings.push(format!(
                "Required environment variable {required} is not configured; runtime validation may fail"
            ));
        }
    }
    if options.run_script {
        let config_dir = config_path.parent().unwrap_or_else(|| Path::new("."));
        let result = run_remote_provider(
            id,
            name,
            Some(provider_dir),
            runtime,
            resolved_runtime.as_deref(),
            global_proxy.as_ref(),
            *timeout_seconds,
            config_dir,
            env_vars,
            window_label_overrides,
            visible_window_ids,
            None,
        );
        if result.iter().any(|provider| provider.status == "error") {
            let detail = result
                .iter()
                .find_map(|provider| provider.error.as_deref())
                .unwrap_or("Provider script did not produce a valid snapshot");
            report
                .errors
                .push(format!("Provider script run failed: {detail}"));
            report.script_run = "failed";
        } else {
            report.script_run = "passed";
        }
    }
    report.valid = report.errors.is_empty();
    Ok(report)
}

fn validate_provider_files(
    manifest_path: &Path,
    source_override: Option<&Path>,
) -> Result<ValidationReport, String> {
    let manifest_text = fs::read_to_string(manifest_path).map_err(|error| error.to_string())?;
    let manifest = parse_manifest(&manifest_text).map_err(|error| error.to_string())?;
    let source_path = source_override.map(Path::to_path_buf).or_else(|| {
        let entry = Path::new(&manifest.entry);
        if entry.is_absolute() {
            Some(entry.to_path_buf())
        } else if manifest.entry.contains("://") {
            None
        } else {
            manifest_path.parent().map(|parent| parent.join(entry))
        }
    });
    Ok(validate_manifest_and_source(
        "files",
        None,
        manifest_path,
        source_path.as_deref(),
        &manifest,
        None,
        None,
    ))
}

fn validate_manifest_and_source(
    kind: &'static str,
    provider_id: Option<String>,
    manifest_path: &Path,
    source_path: Option<&Path>,
    manifest: &ProviderManifest,
    configured_runtime: Option<&str>,
    resolved_runtime: Option<&str>,
) -> ValidationReport {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    validate_manifest_contract(manifest, &mut errors, &mut warnings);

    let mut actual_checksum = None;
    let mut checksum_matches = None;
    match source_path {
        Some(path) if path.is_file() => match fs::read_to_string(path) {
            Ok(source) => {
                actual_checksum = Some(crate::remote_provider::compute_checksum(&source));
                if let Some(expected) = manifest.checksums.source.as_deref() {
                    match verify_checksum(&source, expected) {
                        Ok(()) => checksum_matches = Some(true),
                        Err(error) => {
                            checksum_matches = Some(false);
                            errors.push(error.to_string());
                        }
                    }
                }
            }
            Err(error) => errors.push(format!("Unable to read source script: {error}")),
        },
        Some(path) => errors.push(format!("Source script does not exist: {}", path.display())),
        None => errors.push(
            "Source script cannot be resolved from manifest entry; pass --source with a local file path"
                .to_string(),
        ),
    }

    let runtime_to_check = configured_runtime.unwrap_or(&manifest.runtime);
    let mut resolved = None;
    if is_builtin_js_runtime(runtime_to_check) {
        resolved = Some(BUILTIN_JS_RUNTIME.to_string());
    } else {
        match resolve_runtime(runtime_to_check) {
            Ok(path) => {
                resolved = Some(path.display().to_string());
                if let Err(error) = validate_runtime_executable(&path) {
                    errors.push(format!("Runtime is not usable: {error}"));
                }
            }
            Err(error) => errors.push(format!("Runtime cannot be resolved: {error}")),
        }
    }
    if let Some(stored) = resolved_runtime {
        if resolved.as_deref() != Some(stored) {
            warnings.push(
                "Configured resolvedRuntime differs from the currently resolved runtime"
                    .to_string(),
            );
        }
    }

    ValidationReport {
        schema_version: 1,
        target: ValidationTarget {
            kind,
            provider_id,
            manifest_path: manifest_path.display().to_string(),
            source_path: source_path.map(|path| path.display().to_string()),
        },
        valid: errors.is_empty(),
        errors,
        warnings,
        runtime: ValidationRuntime {
            declared: Some(manifest.runtime.clone()),
            configured: configured_runtime.map(str::to_string),
            resolved,
        },
        checksum: ValidationChecksum {
            expected: manifest.checksums.source.clone(),
            actual: actual_checksum,
            matches: checksum_matches,
        },
        script_run: "notRun",
    }
}

fn validate_manifest_contract(
    manifest: &ProviderManifest,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    if manifest.display_name.trim().is_empty() {
        errors.push("Manifest is missing displayName".to_string());
    }
    if manifest.output != "provider-snapshot-v1" {
        errors.push(format!(
            "Manifest output must be provider-snapshot-v1, got '{}'",
            manifest.output
        ));
    }
    let runtime = manifest.runtime.trim();
    let supported_runtime = matches!(runtime, "builtin-js" | "node" | "python" | "pwsh" | "bash")
        || Path::new(runtime).is_absolute();
    if !supported_runtime {
        errors.push(format!(
            "Manifest runtime must be builtin-js, node, python, pwsh, bash, or an absolute executable path, got '{}'",
            manifest.runtime
        ));
    }
    if manifest.checksums.source.is_none() {
        warnings.push(
            "Manifest has no checksums.source; source integrity and automatic updates cannot be verified"
                .to_string(),
        );
    }
}

fn source_file_name(entry: &str) -> String {
    entry
        .rsplit('/')
        .next()
        .unwrap_or(entry)
        .rsplit('\\')
        .next()
        .unwrap_or(entry)
        .to_string()
}

fn load_snapshot(options: &Options) -> Result<(AppSnapshot, &'static str), String> {
    let path = match &options.config_path {
        Some(path) => path.clone(),
        None => config_path_for_current_executable()?,
    };
    match options.snapshot_mode {
        SnapshotMode::Refresh => match refresh_scope(options.provider_id.as_deref()) {
            RefreshScope::AllProviders => {
                build_app_snapshot_from_config_path(&path).map(|snapshot| (snapshot, "refresh"))
            }
            RefreshScope::SingleProvider(provider_id) => {
                refresh_provider_from_config_path(&path, provider_id)
                    .map(|snapshot| (snapshot, "refresh"))
            }
        },
        SnapshotMode::Cached => get_cached_snapshot_from_config_path(&path)?
            .map(|snapshot| (snapshot, "cache"))
            .ok_or_else(|| "No cached snapshot is available; retry without --cached".to_string()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefreshScope<'a> {
    AllProviders,
    SingleProvider(&'a str),
}

fn refresh_scope(provider_id: Option<&str>) -> RefreshScope<'_> {
    match provider_id {
        Some(provider_id) => RefreshScope::SingleProvider(provider_id),
        None => RefreshScope::AllProviders,
    }
}

fn select_providers<'a>(
    snapshot: &'a AppSnapshot,
    provider_id: Option<&str>,
    window_id: Option<&str>,
) -> Result<Vec<&'a ProviderSnapshot>, String> {
    let provider_matches = snapshot
        .providers
        .iter()
        .filter(|provider| provider_id.is_none_or(|id| provider.id == id))
        .collect::<Vec<_>>();
    if provider_id.is_some() && provider_matches.is_empty() {
        return Err(format!(
            "Provider {} was not found",
            provider_id.unwrap_or_default()
        ));
    }
    let providers = provider_matches
        .into_iter()
        .filter(|provider| {
            window_id.is_none_or(|id| provider.windows.iter().any(|window| window.id == id))
        })
        .collect::<Vec<_>>();
    if window_id.is_some() && providers.is_empty() {
        return Err(format!(
            "Quota window {} was not found",
            window_id.unwrap_or_default()
        ));
    }
    Ok(providers)
}

fn cli_provider(provider: &ProviderSnapshot) -> CliProvider {
    cli_provider_for_window(provider, None)
}

fn cli_provider_for_window(provider: &ProviderSnapshot, window_id: Option<&str>) -> CliProvider {
    CliProvider {
        id: provider.id.clone(),
        name: provider.name.clone(),
        status: provider.status.clone(),
        updated_at: provider.updated_at.clone(),
        windows: provider
            .windows
            .iter()
            .filter(|window| window_id.is_none_or(|id| window.id == id))
            .map(cli_window)
            .collect(),
    }
}

fn cli_window(window: &QuotaWindow) -> CliWindow {
    CliWindow {
        id: window.id.clone(),
        label: window.label.clone(),
        remaining: window.remaining,
        used: window.used,
        limit: window.limit,
        unit: window.unit.clone(),
        used_percent: window.used_percent,
        remaining_percent: window.remaining_percent,
        reset_at: window.reset_at.clone(),
        reset_text: window.reset_text.clone(),
        confidence: window.confidence.clone(),
    }
}

fn parse_command(arguments: impl Iterator<Item = String>) -> Result<Command, String> {
    let arguments = arguments.collect::<Vec<_>>();
    if arguments.is_empty() {
        return Ok(Command::Help);
    }
    match arguments[0].as_str() {
        "--help" | "-h" | "help" => Ok(Command::Help),
        "--version" | "-V" | "version" => Ok(Command::Version),
        "get" => parse_options(&arguments[1..]).map(Command::Get),
        "check" => parse_options(&arguments[1..])
            .and_then(validate_check_options)
            .map(Command::Check),
        "validate" => parse_validation_options(&arguments[1..]).map(Command::Validate),
        command => Err(format!("Unknown command: {command}")),
    }
}

fn validate_check_options(options: Options) -> Result<Options, String> {
    if options.provider_id.is_none() {
        return Err("--provider is required for check".to_string());
    }
    if options.window_id.is_none() {
        return Err("--window is required for check".to_string());
    }
    if options.min_remaining_percent.is_none() {
        return Err("--min-remaining-percent is required for check".to_string());
    }
    Ok(options)
}

fn parse_validation_options(arguments: &[String]) -> Result<ValidationOptions, String> {
    let mut options = ValidationOptions {
        provider_id: None,
        manifest_path: None,
        source_path: None,
        config_path: None,
        run_script: false,
    };
    let mut index = 0;
    while let Some(argument) = arguments.get(index) {
        match argument.as_str() {
            "--provider" => {
                options.provider_id = Some(option_value(arguments, &mut index, argument)?)
            }
            "--manifest" => {
                options.manifest_path = Some(PathBuf::from(option_value(
                    arguments, &mut index, argument,
                )?))
            }
            "--source" => {
                options.source_path = Some(PathBuf::from(option_value(
                    arguments, &mut index, argument,
                )?))
            }
            "--config" => {
                options.config_path = Some(PathBuf::from(option_value(
                    arguments, &mut index, argument,
                )?))
            }
            "--json" => {}
            "--run" => options.run_script = true,
            "--help" | "-h" => {
                return Err("Use `QuotaBarWin.Cli.exe --help` for command help".to_string())
            }
            _ => return Err(format!("Unknown option: {argument}")),
        }
        index += 1;
    }
    if options.provider_id.is_some() == options.manifest_path.is_some() {
        return Err("validate requires exactly one of --provider or --manifest".to_string());
    }
    if options.provider_id.is_some() && options.source_path.is_some() {
        return Err("--source can only be used with --manifest".to_string());
    }
    if options.run_script && options.provider_id.is_none() {
        return Err("--run can only be used with --provider".to_string());
    }
    Ok(options)
}

fn parse_options(arguments: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut snapshot_mode_explicit = false;
    let mut index = 0;
    while let Some(argument) = arguments.get(index) {
        match argument.as_str() {
            "--refresh" => set_snapshot_mode(
                &mut options,
                &mut snapshot_mode_explicit,
                SnapshotMode::Refresh,
            )?,
            "--cached" => set_snapshot_mode(
                &mut options,
                &mut snapshot_mode_explicit,
                SnapshotMode::Cached,
            )?,
            "--json" => {}
            "--provider" => {
                options.provider_id = Some(option_value(arguments, &mut index, argument)?)
            }
            "--window" => options.window_id = Some(option_value(arguments, &mut index, argument)?),
            "--config" => {
                options.config_path = Some(PathBuf::from(option_value(
                    arguments, &mut index, argument,
                )?))
            }
            "--min-remaining-percent" => {
                let value = option_value(arguments, &mut index, argument)?;
                let percent = value.parse::<f64>().map_err(|_| {
                    format!(
                        "--min-remaining-percent must be a number from 0 through 100, got {value}"
                    )
                })?;
                if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
                    return Err(
                        "--min-remaining-percent must be a number from 0 through 100".to_string(),
                    );
                }
                options.min_remaining_percent = Some(percent);
            }
            "--help" | "-h" => {
                return Err("Use `QuotaBarWin.Cli.exe --help` for command help".to_string())
            }
            _ => return Err(format!("Unknown option: {argument}")),
        }
        index += 1;
    }
    Ok(options)
}

fn set_snapshot_mode(
    options: &mut Options,
    snapshot_mode_explicit: &mut bool,
    requested: SnapshotMode,
) -> Result<(), String> {
    if *snapshot_mode_explicit && options.snapshot_mode != requested {
        return Err("--refresh and --cached cannot be used together".to_string());
    }
    options.snapshot_mode = requested;
    *snapshot_mode_explicit = true;
    Ok(())
}

fn option_value(arguments: &[String], index: &mut usize, option: &str) -> Result<String, String> {
    *index += 1;
    arguments
        .get(*index)
        .cloned()
        .ok_or_else(|| format!("{option} requires a value"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(status: &str, remaining_percent: Option<f64>) -> AppSnapshot {
        AppSnapshot {
            schema_version: 1,
            refreshed_at: "2026-07-10T00:00:00Z".to_string(),
            providers: vec![ProviderSnapshot {
                id: "codex-usage".to_string(),
                name: "Codex".to_string(),
                status: status.to_string(),
                source: "remote".to_string(),
                updated_at: Some("2026-07-10T00:00:00Z".to_string()),
                windows: vec![QuotaWindow {
                    id: "5h".to_string(),
                    label: "5h".to_string(),
                    remaining: None,
                    used: None,
                    limit: None,
                    unit: Some("percent".to_string()),
                    used_percent: remaining_percent.map(|value| 100.0 - value),
                    remaining_percent,
                    warning_remaining: None,
                    reset_at: Some("2026-07-10T05:00:00Z".to_string()),
                    reset_text: None,
                    confidence: "high".to_string(),
                }],
                error: None,
                diagnostics: None,
                metadata: Some(serde_json::json!({ "token": "must-not-appear" })),
            }],
        }
    }

    #[test]
    fn parses_check_options_and_cached_mode() {
        let command = parse_command(
            [
                "check",
                "--provider",
                "codex-usage",
                "--window",
                "5h",
                "--min-remaining-percent",
                "20",
                "--cached",
            ]
            .into_iter()
            .map(str::to_string),
        )
        .expect("arguments parse");
        assert_eq!(
            command,
            Command::Check(Options {
                provider_id: Some("codex-usage".to_string()),
                window_id: Some("5h".to_string()),
                min_remaining_percent: Some(20.0),
                config_path: None,
                snapshot_mode: SnapshotMode::Cached,
            })
        );
    }

    #[test]
    fn rejects_conflicting_snapshot_modes() {
        let error = parse_command(
            ["get", "--cached", "--refresh"]
                .into_iter()
                .map(str::to_string),
        )
        .expect_err("conflicting modes fail");
        assert!(error.contains("cannot be used together"));
    }

    #[test]
    fn provider_filter_selects_single_provider_refresh() {
        assert_eq!(refresh_scope(None), RefreshScope::AllProviders);
        assert_eq!(
            refresh_scope(Some("codex-usage")),
            RefreshScope::SingleProvider("codex-usage")
        );
    }

    #[test]
    fn check_requires_all_decision_inputs() {
        let error = parse_command(
            ["check", "--provider", "codex-usage"]
                .into_iter()
                .map(str::to_string),
        )
        .expect_err("missing window fails argument validation");
        assert!(error.contains("--window is required"));
    }

    #[test]
    fn parses_manifest_validation_target() {
        let command = parse_command(
            [
                "validate",
                "--manifest",
                "provider.json",
                "--source",
                "provider.cjs",
            ]
            .into_iter()
            .map(str::to_string),
        )
        .expect("arguments parse");
        assert_eq!(
            command,
            Command::Validate(ValidationOptions {
                provider_id: None,
                manifest_path: Some(PathBuf::from("provider.json")),
                source_path: Some(PathBuf::from("provider.cjs")),
                config_path: None,
                run_script: false,
            })
        );
    }

    #[test]
    fn validation_contract_rejects_unsupported_output_and_runtime() {
        let manifest = parse_manifest(
            r#"{"schemaVersion":1,"id":"test","displayName":"Test","runtime":"ruby","entry":"provider.rb","output":"app-snapshot-v1"}"#,
        )
        .expect("basic manifest parses");
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        validate_manifest_contract(&manifest, &mut errors, &mut warnings);
        assert!(errors
            .iter()
            .any(|error| error.contains("provider-snapshot-v1")));
        assert!(errors.iter().any(|error| error.contains("runtime must be")));
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn validation_contract_accepts_builtin_js_without_an_external_path() {
        let manifest = parse_manifest(
            r#"{"schemaVersion":1,"id":"builtin","displayName":"Builtin","runtime":"builtin-js","entry":"provider.js","output":"provider-snapshot-v1"}"#,
        )
        .expect("builtin manifest parses");
        let report = validate_manifest_and_source(
            "files",
            None,
            Path::new("provider.json"),
            None,
            &manifest,
            None,
            None,
        );
        assert!(report.errors.iter().all(|error| !error.contains("Runtime")));
        assert_eq!(report.runtime.resolved.as_deref(), Some("builtin-js"));
    }

    #[test]
    fn cli_snapshot_omits_metadata_and_diagnostics() {
        let provider = cli_provider(&snapshot("ok", Some(42.0)).providers[0]);
        let json = serde_json::to_string(&provider).expect("serializes");
        assert!(!json.contains("must-not-appear"));
        assert!(!json.contains("metadata"));
        assert!(!json.contains("diagnostics"));
    }

    #[test]
    fn check_uses_threshold_exit_codes() {
        let high = snapshot("ok", Some(20.0));
        assert_eq!(
            evaluate_quota_decision(&high.providers[0], &high.providers[0].windows[0], 20.0),
            QuotaDecision::Continue
        );

        let low = snapshot("ok", Some(10.0));
        assert_eq!(
            evaluate_quota_decision(&low.providers[0], &low.providers[0].windows[0], 20.0),
            QuotaDecision::Defer
        );
        assert_eq!(EXIT_LOW_QUOTA, 10);
    }

    #[test]
    fn stale_provider_requires_unknown_decision() {
        let stale = snapshot("stale", Some(55.0));
        assert_eq!(
            evaluate_quota_decision(&stale.providers[0], &stale.providers[0].windows[0], 20.0),
            QuotaDecision::Unknown
        );
        assert_eq!(EXIT_UNKNOWN, 11);
    }

    #[test]
    fn window_filter_returns_only_the_requested_window() {
        let mut value = snapshot("ok", Some(40.0));
        let mut weekly = value.providers[0].windows[0].clone();
        weekly.id = "weekly".to_string();
        weekly.label = "Weekly".to_string();
        value.providers[0].windows.push(weekly);
        let providers =
            select_providers(&value, Some("codex-usage"), Some("weekly")).expect("window found");
        let output = cli_provider_for_window(providers[0], Some("weekly"));
        assert_eq!(output.windows.len(), 1);
        assert_eq!(output.windows[0].id, "weekly");
    }
}
