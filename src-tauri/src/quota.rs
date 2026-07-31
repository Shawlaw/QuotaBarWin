use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard, OnceLock},
};

use chrono::{DateTime, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::config::{config_path_for_app, load_or_create_config, AppConfig, ProviderConfig};
use crate::logger::{LogLevel, LogSink};
use crate::remote_provider_runner::run_remote_provider;

const SNAPSHOT_CACHE_FILE_NAME: &str = "last_snapshot.quotaBarWin.json";
const REFRESH_LOCK_FILE_NAME: &str = ".refresh.quotaBarWin.lock";
const OPTIMISTIC_PERCENT_JUMP_THRESHOLD: f64 = 20.0;
const CONFIRMATION_PERCENT_TOLERANCE: f64 = 20.0;
const RESET_REGRESSION_TOLERANCE_SECONDS: i64 = 2 * 60;

static SNAPSHOT_CACHE: OnceLock<Mutex<Option<AppSnapshot>>> = OnceLock::new();
static REFRESH_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn snapshot_cache() -> &'static Mutex<Option<AppSnapshot>> {
    SNAPSHOT_CACHE.get_or_init(|| Mutex::new(None))
}

fn refresh_lock() -> &'static Mutex<()> {
    REFRESH_LOCK.get_or_init(|| Mutex::new(()))
}

fn acquire_refresh_locks(path: &Path) -> Result<(MutexGuard<'static, ()>, fs::File), String> {
    let lock_path = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(REFRESH_LOCK_FILE_NAME);
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let file = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(lock_path)
        .map_err(|error| error.to_string())?;
    file.lock_exclusive().map_err(|error| error.to_string())?;
    let process_lock = refresh_lock()
        .lock()
        .map_err(|_| "Refresh lock poisoned".to_string())?;
    Ok((process_lock, file))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remaining: Option<f64>,
    pub used: Option<f64>,
    pub limit: Option<f64>,
    pub unit: Option<String>,
    pub used_percent: Option<f64>,
    pub remaining_percent: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warning_remaining: Option<f64>,
    pub reset_at: Option<String>,
    pub reset_text: Option<String>,
    pub confidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDiagnostics {
    pub checked_at: String,
    pub messages: Vec<String>,
    pub command_path: Option<String>,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
    pub timed_out: Option<bool>,
    pub stderr: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSnapshot {
    pub id: String,
    pub name: String,
    pub status: String,
    pub source: String,
    pub updated_at: Option<String>,
    pub windows: Vec<QuotaWindow>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub diagnostics: Option<ProviderDiagnostics>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub schema_version: u8,
    pub providers: Vec<ProviderSnapshot>,
    pub refreshed_at: String,
}

pub fn clamp_percent(percent: f64) -> f64 {
    percent.clamp(0.0, 100.0)
}

pub fn clamp_snapshot_percentages(provider: &mut ProviderSnapshot) {
    for window in &mut provider.windows {
        if let Some(used_percent) = window.used_percent {
            let used = clamp_percent(used_percent);
            window.used_percent = Some(used);
            window.remaining_percent = Some(clamp_percent(100.0 - used));
        } else if let Some(remaining_percent) = window.remaining_percent {
            let remaining = clamp_percent(remaining_percent);
            window.remaining_percent = Some(remaining);
            window.used_percent = Some(clamp_percent(100.0 - remaining));
        } else if let (Some(used), Some(limit)) = (window.used, window.limit) {
            if limit > 0.0 {
                let used_percent = clamp_percent((used / limit) * 100.0);
                window.used_percent = Some(used_percent);
                window.remaining_percent = Some(clamp_percent(100.0 - used_percent));
            }
        } else if let (Some(remaining), Some(limit)) = (window.remaining, window.limit) {
            if limit > 0.0 {
                let remaining_percent = clamp_percent((remaining / limit) * 100.0);
                window.remaining_percent = Some(remaining_percent);
                window.used_percent = Some(clamp_percent(100.0 - remaining_percent));
            }
        }
    }
}

fn merge_failed_provider_with_cache(
    failed: ProviderSnapshot,
    cached_providers: &[ProviderSnapshot],
) -> ProviderSnapshot {
    if let Some(cached) = cached_providers.iter().find(|p| p.id == failed.id) {
        let mut fallback = cached.clone();
        fallback.status = "stale".to_string();
        fallback.error = failed.error;
        fallback.diagnostics = failed.diagnostics;
        fallback
    } else {
        failed
    }
}

fn snapshot_cache_path_for_config_path(path: &Path) -> PathBuf {
    path.parent()
        .unwrap_or_else(|| Path::new("."))
        .join(SNAPSHOT_CACHE_FILE_NAME)
}

fn snapshot_for_disk_cache(snapshot: &AppSnapshot) -> AppSnapshot {
    let mut cached = snapshot.clone();
    for provider in &mut cached.providers {
        provider.metadata = None;
    }
    cached
}

fn persist_snapshot_cache(path: &Path, snapshot: &AppSnapshot) -> Result<(), String> {
    let cache_path = snapshot_cache_path_for_config_path(path);
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let temporary_path = cache_path.with_file_name(format!(".{SNAPSHOT_CACHE_FILE_NAME}.tmp"));
    let contents = serde_json::to_string_pretty(&snapshot_for_disk_cache(snapshot))
        .map_err(|error| error.to_string())?;
    fs::write(&temporary_path, contents).map_err(|error| error.to_string())?;
    if cache_path.exists() {
        fs::remove_file(&cache_path).map_err(|error| error.to_string())?;
    }
    fs::rename(&temporary_path, &cache_path).map_err(|error| {
        let _ = fs::remove_file(&temporary_path);
        error.to_string()
    })
}

fn filter_snapshot_for_config(mut snapshot: AppSnapshot, config: &AppConfig) -> AppSnapshot {
    let mut providers = Vec::new();
    for config_provider in &config.providers {
        let ProviderConfig::Remote {
            id,
            name,
            enabled,
            window_label_overrides,
            visible_window_ids,
            ..
        } = config_provider;
        if !enabled {
            continue;
        }

        if let Some(provider) = snapshot
            .providers
            .iter()
            .find(|provider| provider.id == *id)
        {
            let mut provider = provider.clone();
            provider.name = name.clone();
            provider.windows = provider
                .windows
                .into_iter()
                .filter(|window| {
                    visible_window_ids.is_empty() || visible_window_ids.contains(&window.id)
                })
                .map(|mut window| {
                    if let Some(label) = window_label_overrides.get(&window.id) {
                        window.label = label.clone();
                    }
                    window
                })
                .collect();
            providers.push(provider);
        }
    }
    snapshot.providers = providers;
    snapshot
}

fn read_snapshot_cache_from_disk(
    path: &Path,
    config: &AppConfig,
) -> Result<Option<AppSnapshot>, String> {
    let cache_path = snapshot_cache_path_for_config_path(path);
    if !cache_path.exists() {
        return Ok(None);
    }

    let contents = fs::read_to_string(&cache_path).map_err(|error| error.to_string())?;
    let snapshot = serde_json::from_str::<AppSnapshot>(&contents)
        .map_err(|error| format!("Failed to parse snapshot cache: {error}"))?;
    Ok(Some(filter_snapshot_for_config(snapshot, config)))
}

pub fn get_cached_snapshot_from_config_path(path: &Path) -> Result<Option<AppSnapshot>, String> {
    let loaded = load_or_create_config(path)?;

    cached_snapshot_for_refresh(path, &loaded.config)
}

fn cached_snapshot_for_refresh(
    path: &Path,
    config: &AppConfig,
) -> Result<Option<AppSnapshot>, String> {
    if let Some(snapshot) = snapshot_cache()
        .lock()
        .map_err(|_| "Snapshot cache lock poisoned".to_string())?
        .clone()
    {
        return Ok(Some(filter_snapshot_for_config(snapshot, config)));
    }

    let snapshot = match read_snapshot_cache_from_disk(path, config) {
        Ok(snapshot) => snapshot,
        Err(_) => None,
    };
    if let Some(snapshot) = snapshot.clone() {
        *snapshot_cache()
            .lock()
            .map_err(|_| "Snapshot cache lock poisoned".to_string())? = Some(snapshot);
    }
    Ok(snapshot)
}

pub fn build_app_snapshot_from_config_path(path: &Path) -> Result<AppSnapshot, String> {
    let _locks = acquire_refresh_locks(path)?;
    let loaded = load_or_create_config(path)?;
    let log = LogSink::from_config_path(path, &loaded.config);
    let config_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let global_proxy = loaded.config.network_proxy.clone();
    let should_log_quota_data = loaded.config.log_quota_data;
    let cached = cached_snapshot_for_refresh(path, &loaded.config)?;
    let old_providers = cached
        .as_ref()
        .map(|s| s.providers.as_slice())
        .unwrap_or(&[]);
    let mut providers = Vec::new();
    let enabled_count = loaded
        .config
        .providers
        .iter()
        .filter(|provider| matches!(provider, ProviderConfig::Remote { enabled: true, .. }))
        .count();
    let _ = log.write(
        LogLevel::Info,
        "quota",
        &format!("snapshot refresh started providers={enabled_count}"),
    );
    for provider in loaded.config.providers {
        let provider_id = provider_config_id(&provider).to_string();
        let results = run_provider_with_retry(
            provider.clone(),
            config_dir,
            global_proxy.as_ref(),
            &loaded.recovery_messages,
            &[
                std::time::Duration::from_secs(1),
                std::time::Duration::from_secs(2),
            ],
            Some(&log),
        );
        for result in stabilize_provider_results(
            &provider,
            results,
            old_providers,
            config_dir,
            global_proxy.as_ref(),
            &loaded.recovery_messages,
            Some(&log),
        ) {
            if result.status == "error" {
                let _ = log.write(
                    LogLevel::Warn,
                    "quota",
                    &format!(
                        "provider refresh failed id={} error={}",
                        provider_id,
                        result.error.as_deref().unwrap_or("unknown")
                    ),
                );
                providers.push(merge_failed_provider_with_cache(result, old_providers));
            } else {
                let _ = log.write(
                    LogLevel::Info,
                    "quota",
                    &format!(
                        "provider refresh succeeded id={} status={} windows={}",
                        provider_id,
                        result.status,
                        result.windows.len()
                    ),
                );
                providers.push(result);
            }
        }
    }

    let snapshot = AppSnapshot {
        schema_version: 1,
        providers,
        refreshed_at: Utc::now().to_rfc3339(),
    };

    if should_log_quota_data {
        log_quota_data(&log, &snapshot.providers);
    }

    *snapshot_cache()
        .lock()
        .map_err(|_| "Snapshot cache lock poisoned".to_string())? = Some(snapshot.clone());

    if let Err(error) = persist_snapshot_cache(path, &snapshot) {
        let _ = log.write(
            LogLevel::Warn,
            "quota",
            &format!("failed to persist snapshot cache: {error}"),
        );
    }

    let _ = log.write(
        LogLevel::Info,
        "quota",
        &format!(
            "snapshot refresh finished providers={} refreshedAt={}",
            snapshot.providers.len(),
            snapshot.refreshed_at
        ),
    );

    Ok(snapshot)
}

fn should_retry_provider_result(providers: &[ProviderSnapshot]) -> bool {
    providers.iter().any(|provider| {
        if provider.status != "error" {
            return false;
        }
        !is_non_retryable_error(provider)
    })
}

fn log_quota_data(log: &LogSink, providers: &[ProviderSnapshot]) {
    for provider in providers {
        if provider.windows.is_empty() {
            let _ = log.write_unfiltered(
                LogLevel::Info,
                "quota_data",
                &format!(
                    "quota data providerId={} providerStatus={} windows=0",
                    log_string(&provider.id),
                    log_string(&provider.status)
                ),
            );
            continue;
        }

        for window in &provider.windows {
            let _ = log.write_unfiltered(
                LogLevel::Info,
                "quota_data",
                &format!(
                    "quota data providerId={} providerStatus={} windowId={} used={} limit={} remaining={} usedPercent={} remainingPercent={} resetAt={} confidence={}",
                    log_string(&provider.id),
                    log_string(&provider.status),
                    log_string(&window.id),
                    log_number(window.used),
                    log_number(window.limit),
                    log_number(window.remaining),
                    log_number(window.used_percent),
                    log_number(window.remaining_percent),
                    log_optional_string(window.reset_at.as_deref()),
                    log_string(&window.confidence)
                ),
            );
        }
    }
}

fn log_number(value: Option<f64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn log_optional_string(value: Option<&str>) -> String {
    value.map(log_string).unwrap_or_else(|| "null".to_string())
}

fn log_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn is_non_retryable_error(provider: &ProviderSnapshot) -> bool {
    if provider
        .diagnostics
        .as_ref()
        .and_then(|d| d.timed_out)
        .unwrap_or(false)
    {
        return false;
    }

    let error_text = provider.error.as_deref().unwrap_or("").to_lowercase();
    let stderr_text = provider
        .diagnostics
        .as_ref()
        .and_then(|d| d.stderr.as_deref())
        .unwrap_or("")
        .to_lowercase();
    let combined = format!("{error_text} {stderr_text}");

    const LOCAL_CONFIGURATION_ERRORS: &[&str] = &[
        "remote provider cache directory is missing",
        "failed to load remote provider manifest",
        "failed to resolve runtime",
        "cached source file does not exist",
        "unsupported remote provider output type",
        "unable to resolve remote provider environment variable",
        "unable to resolve remote provider proxy url",
        "failed to parse remote provider stdout",
    ];
    if LOCAL_CONFIGURATION_ERRORS
        .iter()
        .any(|pattern| combined.contains(pattern))
    {
        return true;
    }

    const AUTH_OR_INPUT_ERRORS: &[&str] = &[
        " 400",
        "400:",
        "bad request",
        " 401",
        "401:",
        "unauthorized",
        " 403",
        "403:",
        "forbidden",
        "access token is empty",
        "api key is empty",
        "token is empty",
        "invalid token",
        "invalid api key",
        "missing access token",
        "missing api key",
        "missing token",
        "no access token",
        "authentication failed",
        "permission denied",
    ];
    AUTH_OR_INPUT_ERRORS
        .iter()
        .any(|pattern| combined.contains(pattern))
}

fn run_provider_with_retry(
    provider: ProviderConfig,
    config_dir: &Path,
    global_proxy: Option<&crate::proxy::ProxyConfig>,
    recovery_messages: &[String],
    delays: &[std::time::Duration],
    log: Option<&LogSink>,
) -> Vec<ProviderSnapshot> {
    let provider_id = provider_config_id(&provider).to_string();
    let mut result = run_provider_config(
        provider.clone(),
        config_dir,
        global_proxy,
        recovery_messages,
        log,
    );
    for (attempt, delay) in delays.iter().enumerate() {
        if !should_retry_provider_result(&result) {
            break;
        }
        if let Some(log) = log {
            let _ = log.write(
                LogLevel::Warn,
                "quota",
                &format!(
                    "provider refresh retry scheduled id={} attempt={} delayMs={}",
                    provider_id,
                    attempt + 2,
                    delay.as_millis()
                ),
            );
        }
        std::thread::sleep(*delay);
        result = run_provider_config(
            provider.clone(),
            config_dir,
            global_proxy,
            recovery_messages,
            log,
        );
    }
    result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VerificationReason {
    InvalidProviderUpdatedAt,
    RegressedProviderUpdatedAt,
    InvalidResetAt,
    RegressedResetAt,
    OptimisticPercentJump,
}

impl VerificationReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::InvalidProviderUpdatedAt => "invalid-provider-updated-at",
            Self::RegressedProviderUpdatedAt => "regressed-provider-updated-at",
            Self::InvalidResetAt => "invalid-reset-at",
            Self::RegressedResetAt => "regressed-reset-at",
            Self::OptimisticPercentJump => "optimistic-percent-jump",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfirmationDecision {
    AcceptConfirmed,
    AcceptReverted,
    PreservePrevious,
}

fn stabilize_provider_results(
    provider_config: &ProviderConfig,
    initial_results: Vec<ProviderSnapshot>,
    previous_providers: &[ProviderSnapshot],
    config_dir: &Path,
    global_proxy: Option<&crate::proxy::ProxyConfig>,
    recovery_messages: &[String],
    log: Option<&LogSink>,
) -> Vec<ProviderSnapshot> {
    let candidates = initial_results
        .iter()
        .filter_map(|provider| {
            if provider.status == "error" {
                return None;
            }
            let previous = previous_providers
                .iter()
                .find(|previous| previous.id == provider.id)?;
            let (window_id, reason) = verification_required(previous, provider)?;
            Some((provider.id.as_str(), window_id, reason))
        })
        .collect::<Vec<_>>();

    if candidates.is_empty() {
        return initial_results;
    }

    if let Some(log) = log {
        for (provider_id, window_id, reason) in &candidates {
            let _ = log.write(
                LogLevel::Warn,
                "quota",
                &format!(
                    "quota verification detected id={} windowId={} reason={} action=confirm",
                    provider_id,
                    window_id,
                    reason.as_str()
                ),
            );
        }
        let _ = log.write(
            LogLevel::Info,
            "quota",
            &format!(
                "quota verification refresh started providerId={} candidates={}",
                provider_config_id(provider_config),
                candidates.len()
            ),
        );
    }

    let confirmation_results = run_provider_with_retry(
        provider_config.clone(),
        config_dir,
        global_proxy,
        recovery_messages,
        &[
            std::time::Duration::from_secs(1),
            std::time::Duration::from_secs(2),
        ],
        log,
    );

    initial_results
        .into_iter()
        .map(|initial| {
            let Some(previous) = previous_providers
                .iter()
                .find(|previous| previous.id == initial.id)
            else {
                return initial;
            };
            let Some((window_id, reason)) = verification_required(previous, &initial) else {
                return initial;
            };
            let confirmation = confirmation_results
                .iter()
                .find(|confirmation| confirmation.id == initial.id);
            let decision = confirmation
                .map(|confirmation| confirmation_decision(previous, &initial, confirmation))
                .unwrap_or(ConfirmationDecision::PreservePrevious);

            if let Some(log) = log {
                let outcome = match decision {
                    ConfirmationDecision::AcceptConfirmed => "confirmed",
                    ConfirmationDecision::AcceptReverted => "reverted",
                    ConfirmationDecision::PreservePrevious => "pending",
                };
                let _ = log.write(
                    match decision {
                        ConfirmationDecision::PreservePrevious => LogLevel::Warn,
                        _ => LogLevel::Info,
                    },
                    "quota",
                    &format!(
                        "quota verification finished id={} windowId={} reason={} outcome={}",
                        initial.id,
                        window_id,
                        reason.as_str(),
                        outcome
                    ),
                );
            }

            match decision {
                ConfirmationDecision::AcceptConfirmed | ConfirmationDecision::AcceptReverted => {
                    confirmation.cloned().unwrap_or(initial)
                }
                ConfirmationDecision::PreservePrevious => {
                    verification_pending_provider(previous, confirmation, reason)
                }
            }
        })
        .collect()
}

fn verification_required(
    previous: &ProviderSnapshot,
    candidate: &ProviderSnapshot,
) -> Option<(String, VerificationReason)> {
    if let Some(reason) = provider_updated_at_reason(previous, candidate) {
        return Some(("<provider>".to_string(), reason));
    }

    for candidate_window in &candidate.windows {
        let Some(previous_window) = previous
            .windows
            .iter()
            .find(|window| window.id == candidate_window.id)
        else {
            continue;
        };
        if let Some(reason) = reset_at_reason(previous_window, candidate_window) {
            return Some((candidate_window.id.clone(), reason));
        }
        if is_optimistic_percent_jump(previous_window, candidate_window) {
            return Some((
                candidate_window.id.clone(),
                VerificationReason::OptimisticPercentJump,
            ));
        }
    }
    None
}

fn confirmation_decision(
    previous: &ProviderSnapshot,
    initial: &ProviderSnapshot,
    confirmation: &ProviderSnapshot,
) -> ConfirmationDecision {
    if confirmation.status != "ok" {
        return ConfirmationDecision::PreservePrevious;
    }
    if provider_updated_at_reason(previous, confirmation).is_some()
        || confirmation
            .windows
            .iter()
            .filter_map(|window| {
                previous
                    .windows
                    .iter()
                    .find(|previous_window| previous_window.id == window.id)
                    .map(|previous_window| reset_at_reason(previous_window, window))
            })
            .any(|reason| reason.is_some())
    {
        return ConfirmationDecision::PreservePrevious;
    }

    let still_optimistic = confirmation.windows.iter().any(|confirmation_window| {
        previous
            .windows
            .iter()
            .find(|previous_window| previous_window.id == confirmation_window.id)
            .is_some_and(|previous_window| {
                is_optimistic_percent_jump(previous_window, confirmation_window)
            })
    });
    if !still_optimistic {
        return ConfirmationDecision::AcceptReverted;
    }

    if initial_and_confirmation_match_new_state(previous, initial, confirmation) {
        ConfirmationDecision::AcceptConfirmed
    } else {
        ConfirmationDecision::PreservePrevious
    }
}

fn provider_updated_at_reason(
    previous: &ProviderSnapshot,
    candidate: &ProviderSnapshot,
) -> Option<VerificationReason> {
    let candidate_updated_at = candidate.updated_at.as_deref()?;
    let Some(candidate_time) = parse_rfc3339(candidate_updated_at) else {
        return Some(VerificationReason::InvalidProviderUpdatedAt);
    };
    let previous_time = previous.updated_at.as_deref().and_then(parse_rfc3339);
    if previous.updated_at.is_some() && previous_time.is_none() {
        return None;
    }
    if previous_time.is_some_and(|previous_time| candidate_time <= previous_time) {
        return Some(VerificationReason::RegressedProviderUpdatedAt);
    }
    None
}

fn reset_at_reason(previous: &QuotaWindow, candidate: &QuotaWindow) -> Option<VerificationReason> {
    let Some(candidate_reset_at) = candidate.reset_at.as_deref() else {
        return None;
    };
    let Some(candidate_time) = parse_rfc3339(candidate_reset_at) else {
        return Some(VerificationReason::InvalidResetAt);
    };
    let previous_time = previous.reset_at.as_deref().and_then(parse_rfc3339);
    if previous.reset_at.is_some() && previous_time.is_none() {
        return None;
    }
    if previous_time.is_some_and(|previous_time| {
        candidate_time.timestamp() < previous_time.timestamp() - RESET_REGRESSION_TOLERANCE_SECONDS
    }) {
        return Some(VerificationReason::RegressedResetAt);
    }
    None
}

fn parse_rfc3339(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|time| time.with_timezone(&Utc))
}

fn is_optimistic_percent_jump(previous: &QuotaWindow, candidate: &QuotaWindow) -> bool {
    let (Some(previous_used), Some(candidate_used)) =
        (previous.used_percent, candidate.used_percent)
    else {
        return false;
    };
    previous_used.is_finite()
        && candidate_used.is_finite()
        && previous_used - candidate_used >= OPTIMISTIC_PERCENT_JUMP_THRESHOLD
}

fn initial_and_confirmation_match_new_state(
    previous: &ProviderSnapshot,
    initial: &ProviderSnapshot,
    confirmation: &ProviderSnapshot,
) -> bool {
    confirmation.windows.iter().any(|confirmation_window| {
        let Some(previous_window) = previous
            .windows
            .iter()
            .find(|window| window.id == confirmation_window.id)
        else {
            return false;
        };
        let Some(initial_window) = initial
            .windows
            .iter()
            .find(|window| window.id == confirmation_window.id)
        else {
            return false;
        };
        let (Some(initial_used), Some(confirmation_used)) = (
            initial_window.used_percent,
            confirmation_window.used_percent,
        ) else {
            return false;
        };
        is_optimistic_percent_jump(previous_window, initial_window)
            && is_optimistic_percent_jump(previous_window, confirmation_window)
            && (initial_used - confirmation_used).abs() <= CONFIRMATION_PERCENT_TOLERANCE
    })
}

fn verification_pending_provider(
    previous: &ProviderSnapshot,
    confirmation: Option<&ProviderSnapshot>,
    reason: VerificationReason,
) -> ProviderSnapshot {
    let mut pending = previous.clone();
    pending.status = "stale".to_string();
    pending.error = Some(format!(
        "Latest quota result is awaiting a consistent confirmation ({})",
        reason.as_str()
    ));
    pending.diagnostics = confirmation.and_then(|provider| provider.diagnostics.clone());
    for window in &mut pending.windows {
        window.confidence = "unknown".to_string();
    }
    pending
}

fn run_provider_config(
    provider: ProviderConfig,
    config_dir: &Path,
    global_proxy: Option<&crate::proxy::ProxyConfig>,
    _recovery_messages: &[String],
    log: Option<&LogSink>,
) -> Vec<ProviderSnapshot> {
    match provider {
        ProviderConfig::Remote {
            id,
            name,
            enabled,
            provider_dir,
            runtime,
            resolved_runtime,
            timeout_seconds,
            window_label_overrides,
            visible_window_ids,
            env_vars,
            ..
        } if enabled => run_remote_provider(
            &id,
            &name,
            provider_dir.as_deref(),
            &runtime,
            resolved_runtime.as_deref(),
            global_proxy,
            timeout_seconds,
            config_dir,
            &env_vars,
            &window_label_overrides,
            &visible_window_ids,
            log,
        ),
        ProviderConfig::Remote { .. } => Vec::new(),
    }
}

fn provider_config_id(provider: &ProviderConfig) -> &str {
    match provider {
        ProviderConfig::Remote { id, .. } => id,
    }
}

pub fn refresh_provider_from_config_path(
    path: &Path,
    provider_id: &str,
) -> Result<AppSnapshot, String> {
    let _locks = acquire_refresh_locks(path)?;
    let loaded = load_or_create_config(path)?;
    let log = LogSink::from_config_path(path, &loaded.config);
    let config_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let global_proxy = loaded.config.network_proxy.clone();
    let should_log_quota_data = loaded.config.log_quota_data;
    let _ = log.write(
        LogLevel::Info,
        "quota",
        &format!("single provider refresh started id={provider_id}"),
    );
    let provider = loaded
        .config
        .providers
        .iter()
        .find(|provider| provider_config_id(provider) == provider_id)
        .cloned()
        .ok_or_else(|| format!("Provider {provider_id} was not found"))?;
    let refreshed_at = Utc::now().to_rfc3339();

    let mut snapshot =
        cached_snapshot_for_refresh(path, &loaded.config)?.unwrap_or_else(|| AppSnapshot {
            schema_version: 1,
            providers: Vec::new(),
            refreshed_at: refreshed_at.clone(),
        });

    let refreshed_providers = run_provider_with_retry(
        provider.clone(),
        config_dir,
        global_proxy.as_ref(),
        &loaded.recovery_messages,
        &[
            std::time::Duration::from_secs(1),
            std::time::Duration::from_secs(2),
        ],
        Some(&log),
    );
    let refreshed_providers = stabilize_provider_results(
        &provider,
        refreshed_providers,
        &snapshot.providers,
        config_dir,
        global_proxy.as_ref(),
        &loaded.recovery_messages,
        Some(&log),
    );
    let refreshed_provider_ids = refreshed_providers
        .iter()
        .map(|provider| provider.id.clone())
        .collect::<Vec<_>>();

    let insert_at = snapshot
        .providers
        .iter()
        .position(|provider| provider.id == provider_id)
        .unwrap_or_else(|| {
            loaded
                .config
                .providers
                .iter()
                .take_while(|provider| provider_config_id(provider) != provider_id)
                .filter(|provider| {
                    let id = provider_config_id(provider);
                    snapshot
                        .providers
                        .iter()
                        .any(|snapshot_provider| snapshot_provider.id == id)
                })
                .count()
        });

    let cached_providers_before_remove = snapshot.providers.clone();

    snapshot.providers.retain(|provider| {
        provider.id != provider_id
            && !refreshed_provider_ids
                .iter()
                .any(|refreshed_id| refreshed_id == &provider.id)
    });

    let mut final_refreshed_providers = Vec::new();
    for (offset, provider) in refreshed_providers.into_iter().enumerate() {
        let final_provider = if provider.status == "error" {
            merge_failed_provider_with_cache(provider, &cached_providers_before_remove)
        } else {
            provider
        };
        final_refreshed_providers.push(final_provider.clone());
        snapshot.providers.insert(
            (insert_at + offset).min(snapshot.providers.len()),
            final_provider,
        );
    }
    snapshot.refreshed_at = refreshed_at;

    if should_log_quota_data {
        log_quota_data(&log, &final_refreshed_providers);
    }

    *snapshot_cache()
        .lock()
        .map_err(|_| "Snapshot cache lock poisoned".to_string())? = Some(snapshot.clone());

    if let Err(error) = persist_snapshot_cache(path, &snapshot) {
        let _ = log.write(
            LogLevel::Warn,
            "quota",
            &format!("failed to persist snapshot cache: {error}"),
        );
    }

    let _ = log.write(
        LogLevel::Info,
        "quota",
        &format!(
            "single provider refresh finished id={} refreshedProviders={} snapshotProviders={}",
            provider_id,
            refreshed_provider_ids.len(),
            snapshot.providers.len()
        ),
    );

    Ok(snapshot)
}

#[tauri::command]
pub async fn refresh_snapshot(app: AppHandle) -> Result<AppSnapshot, String> {
    let path = config_path_for_app(&app)?;
    tauri::async_runtime::spawn_blocking(move || build_app_snapshot_from_config_path(&path))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn refresh_provider(app: AppHandle, provider_id: String) -> Result<AppSnapshot, String> {
    let path = config_path_for_app(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        refresh_provider_from_config_path(&path, &provider_id)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn get_cached_snapshot(app: AppHandle) -> Result<Option<AppSnapshot>, String> {
    let path = config_path_for_app(&app)?;
    get_cached_snapshot_from_config_path(&path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        save_config_to_path, AppConfig, AppLanguage, RemoteProviderRegistrySettings,
    };
    use std::sync::{Mutex, MutexGuard};

    static TEST_SNAPSHOT_CACHE_LOCK: Mutex<()> = Mutex::new(());

    fn isolate_snapshot_cache() -> MutexGuard<'static, ()> {
        let guard = TEST_SNAPSHOT_CACHE_LOCK.lock().expect("test cache lock");
        *snapshot_cache().lock().expect("lock") = None;
        guard
    }

    fn snapshot_from_config(config: AppConfig) -> AppSnapshot {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        save_config_to_path(&path, &config).expect("save config");
        build_app_snapshot_from_config_path(&path).expect("snapshot")
    }

    fn test_config(providers: Vec<ProviderConfig>) -> AppConfig {
        AppConfig {
            schema_version: crate::config::CURRENT_CONFIG_SCHEMA_VERSION,
            refresh_interval_seconds: 300,
            display_mode: "remaining".to_string(),
            low_quota_warning_threshold: 20.0,
            launch_at_startup: false,
            log_level: "info".to_string(),
            log_max_bytes: crate::config::DEFAULT_LOG_MAX_BYTES,
            log_quota_data: false,
            language: AppLanguage::System,
            network_proxy: None,
            tray_popup_position: None,
            tray_popup_size: None,
            remote_provider_registry: RemoteProviderRegistrySettings::default(),
            providers,
        }
    }

    fn remote_provider(
        temp: &tempfile::TempDir,
        id: &str,
        name: &str,
        enabled: bool,
        used_percent: f64,
    ) -> ProviderConfig {
        let provider_dir = temp.path().join(id);
        std::fs::create_dir_all(&provider_dir).expect("create provider dir");
        std::fs::write(
            provider_dir.join("provider.json"),
            serde_json::json!({
                "schemaVersion": 1,
                "id": id,
                "displayName": name,
                "version": "1.0.0",
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
            format!(
                r#"console.log(JSON.stringify({{id:{id:?},name:{name:?},status:"ok",updatedAt:null,windows:[{{id:"weekly",label:"Weekly",usedPercent:{used_percent},confidence:"exact"}}]}}));"#
            ),
        )
        .expect("write source");

        ProviderConfig::Remote {
            id: id.to_string(),
            name: name.to_string(),
            enabled,
            version: Some("1.0.0".to_string()),
            manifest_url: "https://example.com/provider.json".to_string(),
            source_url: "https://example.com/provider.cjs".to_string(),
            provider_dir: Some(provider_dir),
            runtime: "node".to_string(),
            resolved_runtime: None,
            proxy_url: None,
            auto_update: false,
            update_interval_seconds: 3600,
            timeout_seconds: crate::config::DEFAULT_REMOTE_PROVIDER_TIMEOUT_SECONDS,
            trusted_checksum: None,
            installed_at: None,
            updated_at: None,
            last_checked_at: None,
            window_label_overrides: std::collections::HashMap::new(),
            visible_window_ids: Vec::new(),
            show_in_tray: true,
            env_vars: std::collections::HashMap::new(),
            setup_state: crate::config::ProviderSetupState::Ready,
            setup_last_tested_at: None,
        }
    }

    fn broken_remote_provider(id: &str, name: &str) -> ProviderConfig {
        ProviderConfig::Remote {
            id: id.to_string(),
            name: name.to_string(),
            enabled: true,
            version: None,
            manifest_url: "https://example.com/provider.json".to_string(),
            source_url: "https://example.com/provider.cjs".to_string(),
            provider_dir: None,
            runtime: "node".to_string(),
            resolved_runtime: None,
            proxy_url: None,
            auto_update: false,
            update_interval_seconds: 3600,
            timeout_seconds: crate::config::DEFAULT_REMOTE_PROVIDER_TIMEOUT_SECONDS,
            trusted_checksum: None,
            installed_at: None,
            updated_at: None,
            last_checked_at: None,
            window_label_overrides: std::collections::HashMap::new(),
            visible_window_ids: Vec::new(),
            show_in_tray: true,
            env_vars: std::collections::HashMap::new(),
            setup_state: crate::config::ProviderSetupState::Ready,
            setup_last_tested_at: None,
        }
    }

    fn cached_provider(id: &str, name: &str) -> ProviderSnapshot {
        ProviderSnapshot {
            id: id.to_string(),
            name: name.to_string(),
            status: "ok".to_string(),
            source: "remote".to_string(),
            updated_at: Some("2026-06-08T10:00:00Z".to_string()),
            windows: vec![QuotaWindow {
                id: "weekly".to_string(),
                label: "Weekly".to_string(),
                remaining: None,
                used: None,
                limit: None,
                unit: None,
                used_percent: Some(20.0),
                remaining_percent: Some(80.0),
                warning_remaining: None,
                reset_at: None,
                reset_text: None,
                confidence: "exact".to_string(),
            }],
            error: None,
            diagnostics: None,
            metadata: None,
        }
    }

    fn verification_snapshot(
        used_percent: f64,
        updated_at: &str,
        reset_at: Option<&str>,
    ) -> ProviderSnapshot {
        ProviderSnapshot {
            id: "remote-a".to_string(),
            name: "Remote A".to_string(),
            status: "ok".to_string(),
            source: "remote".to_string(),
            updated_at: Some(updated_at.to_string()),
            windows: vec![QuotaWindow {
                id: "5h".to_string(),
                label: "5h".to_string(),
                remaining: None,
                used: None,
                limit: None,
                unit: Some("percent".to_string()),
                used_percent: Some(used_percent),
                remaining_percent: Some(100.0 - used_percent),
                warning_remaining: None,
                reset_at: reset_at.map(str::to_string),
                reset_text: None,
                confidence: "exact".to_string(),
            }],
            error: None,
            diagnostics: None,
            metadata: None,
        }
    }

    #[test]
    fn optimistic_quota_jump_requires_a_matching_second_sample() {
        let previous =
            verification_snapshot(80.0, "2026-01-01T00:00:00Z", Some("2030-01-01T00:00:00Z"));
        let initial =
            verification_snapshot(5.0, "2026-01-01T00:01:00Z", Some("2030-01-01T00:00:00Z"));
        let confirmation =
            verification_snapshot(3.0, "2026-01-01T00:02:00Z", Some("2030-01-01T00:00:00Z"));

        assert_eq!(
            verification_required(&previous, &initial),
            Some(("5h".to_string(), VerificationReason::OptimisticPercentJump))
        );
        assert_eq!(
            confirmation_decision(&previous, &initial, &confirmation),
            ConfirmationDecision::AcceptConfirmed
        );
    }

    #[test]
    fn verification_accepts_a_confirmation_that_reverts_to_the_prior_range() {
        let previous =
            verification_snapshot(80.0, "2026-01-01T00:00:00Z", Some("2030-01-01T00:00:00Z"));
        let initial =
            verification_snapshot(5.0, "2026-01-01T00:01:00Z", Some("2030-01-01T00:00:00Z"));
        let confirmation =
            verification_snapshot(79.0, "2026-01-01T00:02:00Z", Some("2030-01-01T00:00:00Z"));

        assert_eq!(
            confirmation_decision(&previous, &initial, &confirmation),
            ConfirmationDecision::AcceptReverted
        );
    }

    #[test]
    fn codex_log_style_quota_rebound_does_not_publish_the_transient_recovery() {
        // This mirrors the observed Codex pattern: used 75% (25% remaining),
        // then a one-off used 6% (94% remaining), followed by used 82%
        // (18% remaining). The middle response must never become the snapshot.
        let previous =
            verification_snapshot(75.0, "2026-01-01T00:00:00Z", Some("2030-01-01T00:00:00Z"));
        let transient_recovery =
            verification_snapshot(6.0, "2026-01-01T00:01:00Z", Some("2030-01-01T00:00:00Z"));
        let rebound =
            verification_snapshot(82.0, "2026-01-01T00:02:00Z", Some("2030-01-01T00:00:00Z"));

        assert_eq!(
            verification_required(&previous, &transient_recovery),
            Some(("5h".to_string(), VerificationReason::OptimisticPercentJump))
        );
        assert_eq!(
            confirmation_decision(&previous, &transient_recovery, &rebound),
            ConfirmationDecision::AcceptReverted
        );
    }

    #[test]
    fn invalid_reset_at_is_confirmed_before_a_new_sample_is_published() {
        let previous =
            verification_snapshot(40.0, "2026-01-01T00:00:00Z", Some("2030-01-01T00:00:00Z"));
        let initial = verification_snapshot(40.0, "2026-01-01T00:01:00Z", Some("not-a-time"));
        let confirmation =
            verification_snapshot(40.0, "2026-01-01T00:02:00Z", Some("2030-01-01T00:00:00Z"));

        assert_eq!(
            verification_required(&previous, &initial),
            Some(("5h".to_string(), VerificationReason::InvalidResetAt))
        );
        assert_eq!(
            confirmation_decision(&previous, &initial, &confirmation),
            ConfirmationDecision::AcceptReverted
        );
    }

    #[test]
    fn repeated_invalid_reset_at_preserves_the_previous_snapshot_as_stale() {
        let previous =
            verification_snapshot(40.0, "2026-01-01T00:00:00Z", Some("2030-01-01T00:00:00Z"));
        let initial = verification_snapshot(40.0, "2026-01-01T00:01:00Z", Some("not-a-time"));
        let confirmation =
            verification_snapshot(40.0, "2026-01-01T00:02:00Z", Some("still-not-a-time"));

        assert_eq!(
            confirmation_decision(&previous, &initial, &confirmation),
            ConfirmationDecision::PreservePrevious
        );
        let pending = verification_pending_provider(
            &previous,
            Some(&confirmation),
            VerificationReason::InvalidResetAt,
        );
        assert_eq!(pending.status, "stale");
        assert_eq!(pending.windows[0].confidence, "unknown");
    }

    #[test]
    fn regressed_provider_timestamp_requires_confirmation() {
        let previous =
            verification_snapshot(40.0, "2026-01-01T00:02:00Z", Some("2030-01-01T00:00:00Z"));
        let candidate =
            verification_snapshot(40.0, "2026-01-01T00:01:00Z", Some("2030-01-01T00:00:00Z"));

        assert_eq!(
            verification_required(&previous, &candidate),
            Some((
                "<provider>".to_string(),
                VerificationReason::RegressedProviderUpdatedAt
            ))
        );
    }

    #[test]
    fn refresh_confirms_an_optimistic_jump_and_records_the_verification_flow() {
        let _cache_guard = isolate_snapshot_cache();
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        let provider = remote_provider(&temp, "remote-a", "Remote A", true, 80.0);
        let provider_dir = match &provider {
            ProviderConfig::Remote { provider_dir, .. } => {
                provider_dir.clone().expect("provider dir")
            }
        };
        std::fs::write(
            provider_dir.join("provider.cjs"),
            r#"
const fs = require("node:fs");
const path = require("node:path");
const statePath = path.join(__dirname, "verification-state.txt");
const count = Number(fs.existsSync(statePath) ? fs.readFileSync(statePath, "utf8") : "0");
fs.writeFileSync(statePath, String(count + 1));
const usedPercent = count === 0 ? 5 : 3;
console.log(JSON.stringify({
  status: "ok",
  updatedAt: new Date().toISOString(),
  windows: [{
    id: "5h",
    label: "5h",
    usedPercent,
    resetAt: "2030-01-01T00:00:00Z",
    confidence: "exact"
  }]
}));
"#,
        )
        .expect("write sequenced provider");
        let config = test_config(vec![provider]);
        save_config_to_path(&path, &config).expect("save config");

        *snapshot_cache().lock().expect("lock") = Some(AppSnapshot {
            schema_version: 1,
            providers: vec![verification_snapshot(
                80.0,
                "2026-01-01T00:00:00Z",
                Some("2030-01-01T00:00:00Z"),
            )],
            refreshed_at: "2026-01-01T00:00:00Z".to_string(),
        });

        let refreshed =
            refresh_provider_from_config_path(&path, "remote-a").expect("refresh provider");
        assert_eq!(refreshed.providers[0].status, "ok");
        assert_eq!(refreshed.providers[0].windows[0].used_percent, Some(3.0));

        let log = fs::read_to_string(path.with_file_name("quotabarwin.log")).expect("read log");
        assert!(log.contains("quota verification detected id=remote-a windowId=5h"));
        assert!(log.contains("quota verification refresh started providerId=remote-a candidates=1"));
        assert!(log.contains("quota verification finished id=remote-a windowId=5h"));
        assert!(log.contains("outcome=confirmed"));
    }

    #[test]
    fn remote_provider_returns_app_snapshot() {
        let _cache_guard = isolate_snapshot_cache();
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        let config = test_config(vec![remote_provider(
            &temp, "remote-a", "Remote A", true, 28.0,
        )]);
        save_config_to_path(&path, &config).expect("save config");
        let snapshot = build_app_snapshot_from_config_path(&path).expect("snapshot");

        assert_eq!(snapshot.schema_version, 1);
        assert_eq!(snapshot.providers.len(), 1);
        assert_eq!(snapshot.providers[0].name, "Remote A");
        assert_eq!(snapshot.providers[0].source, "remote");
        assert_eq!(snapshot.providers[0].windows.len(), 1);
    }

    #[test]
    fn quota_data_logging_is_disabled_by_default() {
        let _cache_guard = isolate_snapshot_cache();
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        let config = test_config(vec![remote_provider(
            &temp, "remote-a", "Remote A", true, 28.0,
        )]);
        save_config_to_path(&path, &config).expect("save config");

        build_app_snapshot_from_config_path(&path).expect("snapshot");

        let log_contents =
            fs::read_to_string(path.with_file_name("quotabarwin.log")).expect("read log");
        assert!(!log_contents.contains("\"target\":\"quota_data\""));
    }

    #[test]
    fn quota_data_logging_writes_refreshed_window_values_when_enabled() {
        let _cache_guard = isolate_snapshot_cache();
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        let mut config = test_config(vec![remote_provider(
            &temp, "remote-a", "Remote A", true, 28.0,
        )]);
        config.log_quota_data = true;
        save_config_to_path(&path, &config).expect("save config");

        build_app_snapshot_from_config_path(&path).expect("snapshot");

        let log_contents =
            fs::read_to_string(path.with_file_name("quotabarwin.log")).expect("read log");
        assert!(log_contents.contains("\"target\":\"quota_data\""));
        assert!(log_contents.contains("providerId=\\\"remote-a\\\""));
        assert!(log_contents.contains("windowId=\\\"weekly\\\""));
        assert!(log_contents.contains("usedPercent=28"));
        assert!(log_contents.contains("remainingPercent=72"));
    }

    #[test]
    fn quota_window_serializes_frontend_shape() {
        let window = QuotaWindow {
            id: "weekly".to_string(),
            label: "Weekly limit".to_string(),
            remaining: Some(60.0),
            used: Some(40.0),
            limit: Some(100.0),
            unit: Some("requests".to_string()),
            used_percent: Some(40.0),
            remaining_percent: Some(60.0),
            warning_remaining: Some(20.0),
            reset_at: Some("2026-06-15T00:00:00Z".to_string()),
            reset_text: Some("Monday".to_string()),
            confidence: "exact".to_string(),
        };

        let value = serde_json::to_value(&window).expect("serialize window");
        let parsed = serde_json::from_value::<QuotaWindow>(value.clone()).expect("round trip");

        assert_eq!(value["usedPercent"], serde_json::json!(40.0));
        assert_eq!(value["remaining"], serde_json::json!(60.0));
        assert_eq!(value["warningRemaining"], serde_json::json!(20.0));
        assert_eq!(value["remainingPercent"], serde_json::json!(60.0));
        assert_eq!(value["resetAt"], serde_json::json!("2026-06-15T00:00:00Z"));
        assert!(value.get("used_percent").is_none());
        assert_eq!(parsed, window);
    }

    #[test]
    fn quota_percentages_are_in_range() {
        let _cache_guard = isolate_snapshot_cache();
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        let config = test_config(vec![remote_provider(
            &temp, "remote-a", "Remote A", true, 28.0,
        )]);
        save_config_to_path(&path, &config).expect("save config");
        let snapshot = build_app_snapshot_from_config_path(&path).expect("snapshot");

        for provider in snapshot.providers {
            for window in provider.windows {
                let used = window.used_percent.expect("used percent exists");
                let remaining = window.remaining_percent.expect("remaining percent exists");

                assert!((0.0..=100.0).contains(&used));
                assert!((0.0..=100.0).contains(&remaining));
                assert_eq!(remaining, 100.0 - used);
            }
        }
    }

    #[test]
    fn disabled_provider_is_not_included() {
        let _cache_guard = isolate_snapshot_cache();
        let temp = tempfile::tempdir().expect("temp dir");
        let config = test_config(vec![remote_provider(
            &temp, "disabled", "Disabled", false, 28.0,
        )]);

        let snapshot = snapshot_from_config(config);

        assert!(snapshot.providers.is_empty());
    }

    #[test]
    fn cached_snapshot_persists_to_disk_for_cold_start() {
        let _cache_guard = isolate_snapshot_cache();
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        let config = test_config(vec![remote_provider(
            &temp, "remote-a", "Remote A", true, 28.0,
        )]);
        save_config_to_path(&path, &config).expect("save config");
        let initial = build_app_snapshot_from_config_path(&path).expect("snapshot");
        assert_eq!(initial.providers.len(), 1);
        assert!(snapshot_cache_path_for_config_path(&path).exists());

        *snapshot_cache().lock().expect("lock") = None;
        let cached = get_cached_snapshot_from_config_path(&path)
            .expect("read cache")
            .expect("cache exists");

        assert_eq!(cached.providers.len(), 1);
        assert_eq!(cached.providers[0].id, "remote-a");
    }

    #[test]
    fn disk_snapshot_cache_is_filtered_by_current_config() {
        let _cache_guard = isolate_snapshot_cache();
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        let config = test_config(vec![
            remote_provider(&temp, "remote-b", "Remote B", true, 40.0),
            remote_provider(&temp, "remote-a", "Remote A", false, 20.0),
        ]);
        save_config_to_path(&path, &config).expect("save config");
        let snapshot = AppSnapshot {
            schema_version: 1,
            refreshed_at: "2026-06-08T10:00:00Z".to_string(),
            providers: vec![
                cached_provider("remote-a", "Remote A"),
                cached_provider("remote-b", "Remote B"),
                cached_provider("removed", "Removed"),
            ],
        };
        persist_snapshot_cache(&path, &snapshot).expect("persist cache");

        let cached = get_cached_snapshot_from_config_path(&path)
            .expect("read cache")
            .expect("cache exists");

        assert_eq!(
            cached
                .providers
                .iter()
                .map(|provider| provider.id.as_str())
                .collect::<Vec<_>>(),
            vec!["remote-b"]
        );
    }

    #[test]
    fn memory_snapshot_cache_is_filtered_by_current_config() {
        let _cache_guard = isolate_snapshot_cache();
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        let initial_config = test_config(vec![
            remote_provider(&temp, "remote-a", "Remote A", true, 20.0),
            remote_provider(&temp, "remote-b", "Remote B", true, 40.0),
        ]);
        save_config_to_path(&path, &initial_config).expect("save config");
        build_app_snapshot_from_config_path(&path).expect("snapshot");

        let mut reordered_config = test_config(vec![
            remote_provider(&temp, "remote-b", "Remote B", true, 40.0),
            remote_provider(&temp, "remote-a", "Remote A", false, 20.0),
        ]);
        let ProviderConfig::Remote {
            name,
            window_label_overrides,
            visible_window_ids,
            ..
        } = &mut reordered_config.providers[0];
        *name = "Renamed B".to_string();
        window_label_overrides.insert("weekly".to_string(), "Renamed Weekly".to_string());
        visible_window_ids.push("weekly".to_string());
        save_config_to_path(&path, &reordered_config).expect("save reordered config");

        let cached = get_cached_snapshot_from_config_path(&path)
            .expect("read cache")
            .expect("cache exists");

        assert_eq!(
            cached
                .providers
                .iter()
                .map(|provider| provider.id.as_str())
                .collect::<Vec<_>>(),
            vec!["remote-b"]
        );
        assert_eq!(cached.providers[0].name, "Renamed B");
        assert_eq!(cached.providers[0].windows[0].label, "Renamed Weekly");
    }

    #[test]
    fn disk_snapshot_cache_omits_provider_metadata() {
        let _cache_guard = isolate_snapshot_cache();
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        let config = test_config(vec![remote_provider(
            &temp, "remote-a", "Remote A", true, 28.0,
        )]);
        save_config_to_path(&path, &config).expect("save config");
        let mut provider = cached_provider("remote-a", "Remote A");
        provider.metadata = Some(serde_json::json!({ "secretLike": "do-not-persist" }));
        let snapshot = AppSnapshot {
            schema_version: 1,
            refreshed_at: "2026-06-08T10:00:00Z".to_string(),
            providers: vec![provider],
        };

        persist_snapshot_cache(&path, &snapshot).expect("persist cache");
        let contents = fs::read_to_string(snapshot_cache_path_for_config_path(&path))
            .expect("read cache file");

        assert!(!contents.contains("secretLike"));
        assert!(!contents.contains("do-not-persist"));
    }

    #[test]
    fn refresh_provider_updates_only_target_provider_cache_entry() {
        let _cache_guard = isolate_snapshot_cache();
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");

        let config = test_config(vec![
            remote_provider(&temp, "stale-a", "Stale A", true, 20.0),
            remote_provider(&temp, "stale-b", "Stale B", true, 40.0),
        ]);
        save_config_to_path(&path, &config).expect("save config");

        let initial = build_app_snapshot_from_config_path(&path).expect("initial snapshot");
        let refreshed =
            refresh_provider_from_config_path(&path, "stale-a").expect("refresh provider");

        assert_eq!(initial.providers.len(), 2);
        assert_eq!(refreshed.providers[0].id, "stale-a");
        assert_eq!(refreshed.providers[1].id, "stale-b");
        assert_eq!(refreshed.providers.len(), 2);
    }

    #[test]
    fn percentages_are_clamped_to_0_100() {
        let mut provider = ProviderSnapshot {
            id: "clamp".to_string(),
            name: "Clamp".to_string(),
            status: "ok".to_string(),
            source: "remote".to_string(),
            updated_at: None,
            windows: vec![QuotaWindow {
                id: "window".to_string(),
                label: "Window".to_string(),
                remaining: None,
                used: None,
                limit: None,
                unit: None,
                used_percent: Some(140.0),
                remaining_percent: None,
                warning_remaining: None,
                reset_at: None,
                reset_text: None,
                confidence: "estimated".to_string(),
            }],
            error: None,
            diagnostics: None,
            metadata: None,
        };

        clamp_snapshot_percentages(&mut provider);

        assert_eq!(provider.windows[0].used_percent, Some(100.0));
        assert_eq!(provider.windows[0].remaining_percent, Some(0.0));
    }

    #[test]
    fn refresh_provider_failure_falls_back_to_cached_snapshot() {
        let _cache_guard = isolate_snapshot_cache();
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");

        let config = test_config(vec![
            remote_provider(&temp, "stale-a", "Stale A", true, 20.0),
            remote_provider(&temp, "stale-b", "Stale B", true, 40.0),
        ]);
        save_config_to_path(&path, &config).expect("save config");

        let initial = build_app_snapshot_from_config_path(&path).expect("initial snapshot");
        assert_eq!(initial.providers[0].status, "ok");
        assert!(!initial.providers[0].windows.is_empty());

        let broken_config = test_config(vec![
            broken_remote_provider("stale-a", "Stale A"),
            remote_provider(&temp, "stale-b", "Stale B", true, 40.0),
        ]);
        save_config_to_path(&path, &broken_config).expect("save broken config");

        let refreshed =
            refresh_provider_from_config_path(&path, "stale-a").expect("refresh provider");

        assert_eq!(refreshed.providers[0].id, "stale-a");
        assert_eq!(refreshed.providers[0].status, "stale");
        assert!(!refreshed.providers[0].windows.is_empty());
        assert!(refreshed.providers[0].error.is_some());
        assert_eq!(refreshed.providers[1].id, "stale-b");
        assert_eq!(refreshed.providers[1].status, "ok");
        assert!(!refreshed.providers[1].windows.is_empty());
    }

    #[test]
    fn build_snapshot_failure_falls_back_to_cached_provider_data() {
        let _cache_guard = isolate_snapshot_cache();
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");

        let config = test_config(vec![
            remote_provider(&temp, "stale-a", "Stale A", true, 20.0),
            remote_provider(&temp, "stale-b", "Stale B", true, 40.0),
        ]);
        save_config_to_path(&path, &config).expect("save config");

        let initial = build_app_snapshot_from_config_path(&path).expect("initial snapshot");
        assert_eq!(initial.providers.len(), 2);

        let broken_config = test_config(vec![
            broken_remote_provider("stale-a", "Stale A"),
            remote_provider(&temp, "stale-b", "Stale B", true, 40.0),
        ]);
        save_config_to_path(&path, &broken_config).expect("save broken config");

        let snapshot = build_app_snapshot_from_config_path(&path).expect("snapshot");

        assert_eq!(snapshot.providers[0].id, "stale-a");
        assert_eq!(snapshot.providers[0].status, "stale");
        assert!(!snapshot.providers[0].windows.is_empty());
        assert!(snapshot.providers[0].error.is_some());
        assert_eq!(snapshot.providers[1].id, "stale-b");
        assert_eq!(snapshot.providers[1].status, "ok");
        assert!(!snapshot.providers[1].windows.is_empty());
    }

    #[test]
    fn refresh_provider_failure_without_cache_returns_error() {
        let _cache_guard = isolate_snapshot_cache();
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");

        let config = test_config(vec![broken_remote_provider("broken", "Broken")]);
        save_config_to_path(&path, &config).expect("save config");

        let snapshot = build_app_snapshot_from_config_path(&path).expect("snapshot");

        assert_eq!(snapshot.providers[0].id, "broken");
        assert_eq!(snapshot.providers[0].status, "error");
        assert!(snapshot.providers[0].windows.is_empty());
        assert!(snapshot.providers[0].error.is_some());
    }

    #[test]
    fn retry_decision_retries_timeout() {
        let providers = vec![ProviderSnapshot {
            id: "test".to_string(),
            name: "Test".to_string(),
            status: "error".to_string(),
            source: "remote".to_string(),
            updated_at: None,
            windows: vec![],
            error: Some("Remote provider timed out".to_string()),
            diagnostics: None,
            metadata: None,
        }];
        assert!(should_retry_provider_result(&providers));
    }

    #[test]
    fn retry_decision_retries_connection_failure() {
        let providers = vec![ProviderSnapshot {
            id: "test".to_string(),
            name: "Test".to_string(),
            status: "error".to_string(),
            source: "native".to_string(),
            updated_at: None,
            windows: vec![],
            error: Some("error sending request: connection refused".to_string()),
            diagnostics: None,
            metadata: None,
        }];
        assert!(should_retry_provider_result(&providers));
    }

    #[test]
    fn retry_decision_retries_codex_5xx() {
        let providers = vec![ProviderSnapshot {
            id: "test".to_string(),
            name: "Test".to_string(),
            status: "error".to_string(),
            source: "native".to_string(),
            updated_at: None,
            windows: vec![],
            error: Some("Codex usage API returned 503: service unavailable".to_string()),
            diagnostics: None,
            metadata: None,
        }];
        assert!(should_retry_provider_result(&providers));
    }

    #[test]
    fn retry_decision_retries_unknown_error_by_default() {
        let providers = vec![ProviderSnapshot {
            id: "test".to_string(),
            name: "Test".to_string(),
            status: "error".to_string(),
            source: "remote".to_string(),
            updated_at: None,
            windows: vec![],
            error: Some("Remote provider exited with a non-zero status".to_string()),
            diagnostics: Some(ProviderDiagnostics {
                checked_at: "2026-06-15T00:00:00Z".to_string(),
                messages: vec![],
                command_path: Some("node".to_string()),
                exit_code: Some(1),
                duration_ms: Some(100),
                timed_out: Some(false),
                stderr: Some("temporary upstream failure".to_string()),
            }),
            metadata: None,
        }];
        assert!(should_retry_provider_result(&providers));
    }

    #[test]
    fn retry_decision_skips_empty_token() {
        let providers = vec![ProviderSnapshot {
            id: "test".to_string(),
            name: "Test".to_string(),
            status: "error".to_string(),
            source: "native".to_string(),
            updated_at: None,
            windows: vec![],
            error: Some("Codex access token is empty".to_string()),
            diagnostics: None,
            metadata: None,
        }];
        assert!(!should_retry_provider_result(&providers));
    }

    #[test]
    fn retry_decision_skips_parse_failure() {
        let providers = vec![ProviderSnapshot {
            id: "test".to_string(),
            name: "Test".to_string(),
            status: "error".to_string(),
            source: "remote".to_string(),
            updated_at: None,
            windows: vec![],
            error: Some("Failed to parse remote provider stdout: invalid JSON".to_string()),
            diagnostics: None,
            metadata: None,
        }];
        assert!(!should_retry_provider_result(&providers));
    }

    #[test]
    fn retry_decision_skips_auth_failure() {
        let providers = vec![ProviderSnapshot {
            id: "test".to_string(),
            name: "Test".to_string(),
            status: "error".to_string(),
            source: "native".to_string(),
            updated_at: None,
            windows: vec![],
            error: Some("Codex usage API returned 401: unauthorized".to_string()),
            diagnostics: None,
            metadata: None,
        }];
        assert!(!should_retry_provider_result(&providers));
    }

    #[test]
    fn retry_decision_skips_local_configuration_failure() {
        let providers = vec![ProviderSnapshot {
            id: "test".to_string(),
            name: "Test".to_string(),
            status: "error".to_string(),
            source: "remote".to_string(),
            updated_at: None,
            windows: vec![],
            error: Some("Failed to resolve runtime: node was not found".to_string()),
            diagnostics: None,
            metadata: None,
        }];
        assert!(!should_retry_provider_result(&providers));
    }
}
