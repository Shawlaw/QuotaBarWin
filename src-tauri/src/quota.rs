use std::{
    path::Path,
    sync::{Mutex, OnceLock},
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::command_provider::run_command_provider;
use crate::config::{config_path_for_app, load_or_create_config, ProviderConfig};
use crate::providers::mock;

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
    pub used: Option<f64>,
    pub limit: Option<f64>,
    pub unit: Option<String>,
    pub used_percent: Option<f64>,
    pub remaining_percent: Option<f64>,
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
        }
    }
}

pub fn build_app_snapshot_from_config_path(path: &Path) -> Result<AppSnapshot, String> {
    let _guard = refresh_lock()
        .lock()
        .map_err(|_| "Refresh lock poisoned".to_string())?;
    let loaded = load_or_create_config(path)?;
    let mut providers = Vec::new();

    for provider in loaded.config.providers {
        match provider {
            ProviderConfig::Mock { id, name, enabled } if enabled => {
                providers.push(mock::provider_snapshot(&id, &name, &loaded.recovery_messages));
            }
            ProviderConfig::Command {
                id,
                name,
                enabled,
                command,
                parser,
            } if enabled => {
                providers.extend(run_command_provider(&id, &name, &command, &parser));
            }
            _ => {}
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

#[tauri::command]
pub fn refresh_snapshot(app: AppHandle) -> Result<AppSnapshot, String> {
    let path = config_path_for_app(&app)?;
    build_app_snapshot_from_config_path(&path)
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
    use crate::config::{save_config_to_path, AppConfig, CommandSpec, ParserSpec};

    fn snapshot_from_config(config: AppConfig) -> AppSnapshot {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        save_config_to_path(&path, &config).expect("save config");
        build_app_snapshot_from_config_path(&path).expect("snapshot")
    }

    #[test]
    fn mock_provider_returns_app_snapshot() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        let snapshot = build_app_snapshot_from_config_path(&path).expect("snapshot");

        assert_eq!(snapshot.schema_version, 1);
        assert_eq!(snapshot.providers.len(), 1);
        assert_eq!(snapshot.providers[0].name, "Codex Mock");
        assert_eq!(snapshot.providers[0].windows.len(), 2);
    }

    #[test]
    fn quota_percentages_are_in_range() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        let snapshot = build_app_snapshot_from_config_path(&path).expect("snapshot");

        for provider in snapshot.providers {
            for window in provider.windows {
                let used = window.used_percent.expect("used percent exists");
                let remaining = window
                    .remaining_percent
                    .expect("remaining percent exists");

                assert!((0.0..=100.0).contains(&used));
                assert!((0.0..=100.0).contains(&remaining));
                assert_eq!(remaining, 100.0 - used);
            }
        }
    }

    #[test]
    fn command_provider_does_not_execute_disabled_provider() {
        let temp = tempfile::tempdir().expect("temp dir");
        let marker = temp.path().join("marker.txt");
        let script = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repo root")
            .join("fixtures")
            .join("fake_marker.js");
        let config = AppConfig {
            schema_version: 1,
            refresh_interval_seconds: 300,
            display_mode: "remaining".to_string(),
            low_quota_warning_threshold: 20.0,
            providers: vec![ProviderConfig::Command {
                id: "disabled".to_string(),
                name: "Disabled".to_string(),
                enabled: false,
                command: CommandSpec {
                    executable: "node".to_string(),
                    args: vec![
                        script.to_string_lossy().to_string(),
                        marker.to_string_lossy().to_string(),
                    ],
                    cwd: None,
                    env: None,
                    timeout_ms: 1000,
                },
                parser: ParserSpec::ProviderSnapshot,
            }],
        };

        let snapshot = snapshot_from_config(config);

        assert!(snapshot.providers.is_empty());
        assert!(!marker.exists());
    }

    #[test]
    fn percentages_are_clamped_to_0_100() {
        let mut provider = ProviderSnapshot {
            id: "clamp".to_string(),
            name: "Clamp".to_string(),
            status: "ok".to_string(),
            source: "command".to_string(),
            updated_at: None,
            windows: vec![QuotaWindow {
                id: "window".to_string(),
                label: "Window".to_string(),
                used: None,
                limit: None,
                unit: None,
                used_percent: Some(140.0),
                remaining_percent: None,
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
}
