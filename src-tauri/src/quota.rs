use std::{
    path::Path,
    sync::{Mutex, OnceLock},
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::config::{config_path_for_app, load_or_create_config, ProviderConfig};
use crate::providers::{codex, mock};
use crate::remote_provider_runner::run_remote_provider;

static SNAPSHOT_CACHE: OnceLock<Mutex<Option<AppSnapshot>>> = OnceLock::new();
static REFRESH_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn snapshot_cache() -> &'static Mutex<Option<AppSnapshot>> {
    SNAPSHOT_CACHE.get_or_init(|| Mutex::new(None))
}

fn refresh_lock() -> &'static Mutex<()> {
    REFRESH_LOCK.get_or_init(|| Mutex::new(()))
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

pub fn build_app_snapshot_from_config_path(path: &Path) -> Result<AppSnapshot, String> {
    let _guard = refresh_lock()
        .lock()
        .map_err(|_| "Refresh lock poisoned".to_string())?;
    let loaded = load_or_create_config(path)?;
    let config_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let cached = snapshot_cache()
        .lock()
        .map_err(|_| "Snapshot cache lock poisoned".to_string())?
        .clone();
    let old_providers = cached
        .as_ref()
        .map(|s| s.providers.as_slice())
        .unwrap_or(&[]);
    let mut providers = Vec::new();

    for provider in loaded.config.providers {
        for result in run_provider_with_retry(
            provider,
            config_dir,
            &loaded.recovery_messages,
            &[
                std::time::Duration::from_secs(1),
                std::time::Duration::from_secs(2),
            ],
        ) {
            if result.status == "error" {
                providers.push(merge_failed_provider_with_cache(result, old_providers));
            } else {
                providers.push(result);
            }
        }
    }

    let snapshot = AppSnapshot {
        schema_version: 1,
        providers,
        refreshed_at: Utc::now().to_rfc3339(),
    };

    *snapshot_cache()
        .lock()
        .map_err(|_| "Snapshot cache lock poisoned".to_string())? = Some(snapshot.clone());

    Ok(snapshot)
}

fn is_retryable_error(providers: &[ProviderSnapshot]) -> bool {
    providers.iter().any(|provider| {
        if provider.status != "error" {
            return false;
        }
        if provider
            .diagnostics
            .as_ref()
            .and_then(|d| d.timed_out)
            .unwrap_or(false)
        {
            return true;
        }
        let error_text = provider.error.as_deref().unwrap_or("").to_lowercase();
        let stderr_text = provider
            .diagnostics
            .as_ref()
            .and_then(|d| d.stderr.as_deref())
            .unwrap_or("")
            .to_lowercase();
        let combined = format!("{error_text} {stderr_text}");
        combined.contains("timed out")
            || combined.contains("timeout")
            || combined.contains("connection")
            || combined.contains("connect")
            || (combined.contains("codex usage api returned") && combined.contains(" 5"))
    })
}

fn run_provider_with_retry(
    provider: ProviderConfig,
    config_dir: &Path,
    recovery_messages: &[String],
    delays: &[std::time::Duration],
) -> Vec<ProviderSnapshot> {
    let mut result = run_provider_config(provider.clone(), config_dir, recovery_messages);
    for delay in delays {
        if !is_retryable_error(&result) {
            break;
        }
        std::thread::sleep(*delay);
        result = run_provider_config(provider.clone(), config_dir, recovery_messages);
    }
    result
}

fn run_provider_config(
    provider: ProviderConfig,
    config_dir: &Path,
    recovery_messages: &[String],
) -> Vec<ProviderSnapshot> {
    match provider {
        ProviderConfig::Mock { id, name, enabled } if enabled => {
            vec![mock::provider_snapshot(&id, &name, recovery_messages)]
        }
        ProviderConfig::Codex {
            id,
            name,
            enabled,
            auth_token,
            account_id,
            proxy_url,
            timeout_ms,
            window_label_overrides,
            visible_window_ids,
        } if enabled => vec![codex::provider_snapshot(
            &id,
            &name,
            &auth_token,
            account_id.as_deref(),
            proxy_url.as_deref(),
            timeout_ms,
            config_dir,
            &window_label_overrides,
            &visible_window_ids,
        )],
        ProviderConfig::Remote {
            id,
            name,
            enabled,
            provider_dir,
            runtime,
            resolved_runtime,
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
            config_dir,
            &env_vars,
            &window_label_overrides,
            &visible_window_ids,
        ),
        _ => Vec::new(),
    }
}

fn provider_config_id(provider: &ProviderConfig) -> &str {
    match provider {
        ProviderConfig::Mock { id, .. }
        | ProviderConfig::Codex { id, .. }
        | ProviderConfig::Remote { id, .. } => id,
    }
}

pub fn refresh_provider_from_config_path(
    path: &Path,
    provider_id: &str,
) -> Result<AppSnapshot, String> {
    let _guard = refresh_lock()
        .lock()
        .map_err(|_| "Refresh lock poisoned".to_string())?;
    let loaded = load_or_create_config(path)?;
    let config_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let provider = loaded
        .config
        .providers
        .iter()
        .find(|provider| provider_config_id(provider) == provider_id)
        .cloned()
        .ok_or_else(|| format!("Provider {provider_id} was not found"))?;
    let refreshed_providers = run_provider_with_retry(
        provider,
        config_dir,
        &loaded.recovery_messages,
        &[
            std::time::Duration::from_secs(1),
            std::time::Duration::from_secs(2),
        ],
    );
    let refreshed_provider_ids = refreshed_providers
        .iter()
        .map(|provider| provider.id.as_str())
        .collect::<Vec<_>>();
    let refreshed_at = Utc::now().to_rfc3339();

    let mut snapshot = snapshot_cache()
        .lock()
        .map_err(|_| "Snapshot cache lock poisoned".to_string())?
        .clone()
        .unwrap_or_else(|| AppSnapshot {
            schema_version: 1,
            providers: Vec::new(),
            refreshed_at: refreshed_at.clone(),
        });

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
                .any(|refreshed_id| *refreshed_id == provider.id)
    });

    for (offset, provider) in refreshed_providers.into_iter().enumerate() {
        let final_provider = if provider.status == "error" {
            merge_failed_provider_with_cache(provider, &cached_providers_before_remove)
        } else {
            provider
        };
        snapshot.providers.insert(
            (insert_at + offset).min(snapshot.providers.len()),
            final_provider,
        );
    }
    snapshot.refreshed_at = refreshed_at;

    *snapshot_cache()
        .lock()
        .map_err(|_| "Snapshot cache lock poisoned".to_string())? = Some(snapshot.clone());

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
pub fn get_cached_snapshot() -> Result<Option<AppSnapshot>, String> {
    snapshot_cache()
        .lock()
        .map(|snapshot| snapshot.clone())
        .map_err(|_| "Snapshot cache lock poisoned".to_string())
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

    #[test]
    fn mock_provider_returns_app_snapshot() {
        let _cache_guard = isolate_snapshot_cache();
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        let snapshot = build_app_snapshot_from_config_path(&path).expect("snapshot");

        assert_eq!(snapshot.schema_version, 1);
        assert_eq!(snapshot.providers.len(), 1);
        assert_eq!(snapshot.providers[0].name, "Codex Mock");
        assert_eq!(snapshot.providers[0].windows.len(), 2);
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
        let config = AppConfig {
            schema_version: 1,
            refresh_interval_seconds: 300,
            display_mode: "remaining".to_string(),
            low_quota_warning_threshold: 20.0,
            launch_at_startup: false,
            log_level: "info".to_string(),
            language: AppLanguage::System,
            network_proxy: None,
            tray_popup_position: None,
            remote_provider_registry: RemoteProviderRegistrySettings::default(),
            providers: vec![ProviderConfig::Mock {
                id: "disabled".to_string(),
                name: "Disabled".to_string(),
                enabled: false,
            }],
        };

        let snapshot = snapshot_from_config(config);

        assert!(snapshot.providers.is_empty());
    }

    #[test]
    fn refresh_provider_updates_only_target_provider_cache_entry() {
        let _cache_guard = isolate_snapshot_cache();
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");

        let config = AppConfig {
            schema_version: 5,
            refresh_interval_seconds: 300,
            display_mode: "remaining".to_string(),
            low_quota_warning_threshold: 20.0,
            launch_at_startup: false,
            log_level: "info".to_string(),
            language: AppLanguage::System,
            network_proxy: None,
            tray_popup_position: None,
            remote_provider_registry: RemoteProviderRegistrySettings::default(),
            providers: vec![
                ProviderConfig::Mock {
                    id: "stale-a".to_string(),
                    name: "Stale A".to_string(),
                    enabled: true,
                },
                ProviderConfig::Mock {
                    id: "stale-b".to_string(),
                    name: "Stale B".to_string(),
                    enabled: true,
                },
            ],
        };
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

        let broken_remote_provider = |id: &str, name: &str| ProviderConfig::Remote {
            id: id.to_string(),
            name: name.to_string(),
            enabled: true,
            manifest_url: "https://example.com/provider.json".to_string(),
            source_url: "https://example.com/provider.cjs".to_string(),
            provider_dir: None,
            runtime: "node".to_string(),
            resolved_runtime: None,
            proxy_url: None,
            auto_update: false,
            update_interval_seconds: 3600,
            trusted_checksum: None,
            window_label_overrides: std::collections::HashMap::new(),
            visible_window_ids: Vec::new(),
            env_vars: std::collections::HashMap::new(),
        };

        let config = AppConfig {
            schema_version: 5,
            refresh_interval_seconds: 300,
            display_mode: "remaining".to_string(),
            low_quota_warning_threshold: 20.0,
            launch_at_startup: false,
            log_level: "info".to_string(),
            language: AppLanguage::System,
            network_proxy: None,
            tray_popup_position: None,
            remote_provider_registry: RemoteProviderRegistrySettings::default(),
            providers: vec![
                ProviderConfig::Mock {
                    id: "stale-a".to_string(),
                    name: "Stale A".to_string(),
                    enabled: true,
                },
                ProviderConfig::Mock {
                    id: "stale-b".to_string(),
                    name: "Stale B".to_string(),
                    enabled: true,
                },
            ],
        };
        save_config_to_path(&path, &config).expect("save config");

        let initial = build_app_snapshot_from_config_path(&path).expect("initial snapshot");
        assert_eq!(initial.providers[0].status, "ok");
        assert!(!initial.providers[0].windows.is_empty());

        let broken_config = AppConfig {
            schema_version: 5,
            refresh_interval_seconds: 300,
            display_mode: "remaining".to_string(),
            low_quota_warning_threshold: 20.0,
            launch_at_startup: false,
            log_level: "info".to_string(),
            language: AppLanguage::System,
            network_proxy: None,
            tray_popup_position: None,
            remote_provider_registry: RemoteProviderRegistrySettings::default(),
            providers: vec![
                broken_remote_provider("stale-a", "Stale A"),
                ProviderConfig::Mock {
                    id: "stale-b".to_string(),
                    name: "Stale B".to_string(),
                    enabled: true,
                },
            ],
        };
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

        let broken_remote_provider = |id: &str, name: &str| ProviderConfig::Remote {
            id: id.to_string(),
            name: name.to_string(),
            enabled: true,
            manifest_url: "https://example.com/provider.json".to_string(),
            source_url: "https://example.com/provider.cjs".to_string(),
            provider_dir: None,
            runtime: "node".to_string(),
            resolved_runtime: None,
            proxy_url: None,
            auto_update: false,
            update_interval_seconds: 3600,
            trusted_checksum: None,
            window_label_overrides: std::collections::HashMap::new(),
            visible_window_ids: Vec::new(),
            env_vars: std::collections::HashMap::new(),
        };

        let config = AppConfig {
            schema_version: 5,
            refresh_interval_seconds: 300,
            display_mode: "remaining".to_string(),
            low_quota_warning_threshold: 20.0,
            launch_at_startup: false,
            log_level: "info".to_string(),
            language: AppLanguage::System,
            network_proxy: None,
            tray_popup_position: None,
            remote_provider_registry: RemoteProviderRegistrySettings::default(),
            providers: vec![
                ProviderConfig::Mock {
                    id: "stale-a".to_string(),
                    name: "Stale A".to_string(),
                    enabled: true,
                },
                ProviderConfig::Mock {
                    id: "stale-b".to_string(),
                    name: "Stale B".to_string(),
                    enabled: true,
                },
            ],
        };
        save_config_to_path(&path, &config).expect("save config");

        let initial = build_app_snapshot_from_config_path(&path).expect("initial snapshot");
        assert_eq!(initial.providers.len(), 2);

        let broken_config = AppConfig {
            schema_version: 5,
            refresh_interval_seconds: 300,
            display_mode: "remaining".to_string(),
            low_quota_warning_threshold: 20.0,
            launch_at_startup: false,
            log_level: "info".to_string(),
            language: AppLanguage::System,
            network_proxy: None,
            tray_popup_position: None,
            remote_provider_registry: RemoteProviderRegistrySettings::default(),
            providers: vec![
                broken_remote_provider("stale-a", "Stale A"),
                ProviderConfig::Mock {
                    id: "stale-b".to_string(),
                    name: "Stale B".to_string(),
                    enabled: true,
                },
            ],
        };
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

        let broken_provider = ProviderConfig::Remote {
            id: "broken".to_string(),
            name: "Broken".to_string(),
            enabled: true,
            manifest_url: "https://example.com/provider.json".to_string(),
            source_url: "https://example.com/provider.cjs".to_string(),
            provider_dir: None,
            runtime: "node".to_string(),
            resolved_runtime: None,
            proxy_url: None,
            auto_update: false,
            update_interval_seconds: 3600,
            trusted_checksum: None,
            window_label_overrides: std::collections::HashMap::new(),
            visible_window_ids: Vec::new(),
            env_vars: std::collections::HashMap::new(),
        };

        let config = AppConfig {
            schema_version: 5,
            refresh_interval_seconds: 300,
            display_mode: "remaining".to_string(),
            low_quota_warning_threshold: 20.0,
            launch_at_startup: false,
            log_level: "info".to_string(),
            language: AppLanguage::System,
            network_proxy: None,
            tray_popup_position: None,
            remote_provider_registry: RemoteProviderRegistrySettings::default(),
            providers: vec![broken_provider],
        };
        save_config_to_path(&path, &config).expect("save config");

        let snapshot = build_app_snapshot_from_config_path(&path).expect("snapshot");

        assert_eq!(snapshot.providers[0].id, "broken");
        assert_eq!(snapshot.providers[0].status, "error");
        assert!(snapshot.providers[0].windows.is_empty());
        assert!(snapshot.providers[0].error.is_some());
    }

    #[test]
    fn retryable_error_detects_timeout() {
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
        assert!(is_retryable_error(&providers));
    }

    #[test]
    fn retryable_error_detects_connection_failure() {
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
        assert!(is_retryable_error(&providers));
    }

    #[test]
    fn retryable_error_detects_codex_5xx() {
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
        assert!(is_retryable_error(&providers));
    }

    #[test]
    fn non_retryable_error_rejects_empty_token() {
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
        assert!(!is_retryable_error(&providers));
    }

    #[test]
    fn non_retryable_error_rejects_parse_failure() {
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
        assert!(!is_retryable_error(&providers));
    }

    #[test]
    fn non_retryable_error_rejects_auth_failure() {
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
        assert!(!is_retryable_error(&providers));
    }
}
