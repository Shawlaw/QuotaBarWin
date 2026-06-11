use std::{
    path::Path,
    sync::{Mutex, OnceLock},
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::command_provider::{run_command_provider, run_script_provider};
use crate::config::{config_path_for_app, load_or_create_config, ProviderConfig};
use crate::providers::{codex, mock};

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
        } else if let (Some(used), Some(limit)) = (window.used, window.limit) {
            if limit > 0.0 {
                let used_percent = clamp_percent((used / limit) * 100.0);
                window.used_percent = Some(used_percent);
                window.remaining_percent = Some(clamp_percent(100.0 - used_percent));
            }
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
        providers.extend(run_provider_config(provider, &loaded.recovery_messages));
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

fn run_provider_config(
    provider: ProviderConfig,
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
            &window_label_overrides,
            &visible_window_ids,
        )],
        ProviderConfig::Command {
            id,
            name,
            enabled,
            command,
            parser,
            window_label_overrides,
            visible_window_ids,
        } if enabled => run_command_provider(
            &id,
            &name,
            &command,
            &parser,
            &window_label_overrides,
            &visible_window_ids,
        ),
        ProviderConfig::Script {
            id,
            name,
            enabled,
            command,
            output,
            window_label_overrides,
            visible_window_ids,
        } if enabled => run_script_provider(
            &id,
            &name,
            &command,
            &output,
            &window_label_overrides,
            &visible_window_ids,
        ),
        // Remote providers are materialized and executed separately; not yet wired here.
        ProviderConfig::Remote { .. } => Vec::new(),
        _ => Vec::new(),
    }
}

fn provider_config_id(provider: &ProviderConfig) -> &str {
    match provider {
        ProviderConfig::Mock { id, .. }
        | ProviderConfig::Codex { id, .. }
        | ProviderConfig::Command { id, .. }
        | ProviderConfig::Script { id, .. }
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
    let provider = loaded
        .config
        .providers
        .iter()
        .find(|provider| provider_config_id(provider) == provider_id)
        .cloned()
        .ok_or_else(|| format!("Provider {provider_id} was not found"))?;
    let refreshed_providers = run_provider_config(provider, &loaded.recovery_messages);
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

    snapshot.providers.retain(|provider| {
        provider.id != provider_id
            && !refreshed_provider_ids
                .iter()
                .any(|refreshed_id| *refreshed_id == provider.id)
    });

    for (offset, provider) in refreshed_providers.into_iter().enumerate() {
        snapshot
            .providers
            .insert((insert_at + offset).min(snapshot.providers.len()), provider);
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
    fn quota_window_serializes_frontend_shape() {
        let window = QuotaWindow {
            id: "weekly".to_string(),
            label: "Weekly limit".to_string(),
            used: Some(40.0),
            limit: Some(100.0),
            unit: Some("requests".to_string()),
            used_percent: Some(40.0),
            remaining_percent: Some(60.0),
            reset_at: Some("2026-06-15T00:00:00Z".to_string()),
            reset_text: Some("Monday".to_string()),
            confidence: "exact".to_string(),
        };

        let value = serde_json::to_value(&window).expect("serialize window");
        let parsed = serde_json::from_value::<QuotaWindow>(value.clone()).expect("round trip");

        assert_eq!(value["usedPercent"], serde_json::json!(40.0));
        assert_eq!(value["remainingPercent"], serde_json::json!(60.0));
        assert_eq!(value["resetAt"], serde_json::json!("2026-06-15T00:00:00Z"));
        assert!(value.get("used_percent").is_none());
        assert_eq!(parsed, window);
    }

    #[test]
    fn quota_percentages_are_in_range() {
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
            launch_at_startup: false,
            log_level: "info".to_string(),
            network_proxy: None,
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
                window_label_overrides: std::collections::HashMap::new(),
                visible_window_ids: Vec::new(),
            }],
        };

        let snapshot = snapshot_from_config(config);

        assert!(snapshot.providers.is_empty());
        assert!(!marker.exists());
    }

    #[test]
    fn refresh_provider_updates_only_target_provider_cache_entry() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        let counter_a = temp.path().join("counter-a.txt");
        let counter_b = temp.path().join("counter-b.txt");

        let provider = |id: &str, name: &str, counter: &std::path::Path| {
            ProviderConfig::Command {
            id: id.to_string(),
            name: name.to_string(),
            enabled: true,
            command: CommandSpec {
                executable: "node".to_string(),
                args: vec![
                    "-e".to_string(),
                    r#"const fs=require('node:fs');const [file,id,name]=process.argv.slice(1);const next=(Number(fs.existsSync(file)?fs.readFileSync(file,'utf8'):'0')||0)+1;fs.writeFileSync(file,String(next));console.log(JSON.stringify({id,name,status:'ok',source:'command',updatedAt:'2026-06-08T10:00:00+08:00',windows:[{id:'weekly',label:'Weekly',used:next,limit:10,unit:'requests',usedPercent:next,remainingPercent:100-next,resetAt:null,confidence:'exact'}]}));"#.to_string(),
                    counter.to_string_lossy().to_string(),
                    id.to_string(),
                    name.to_string(),
                ],
                cwd: None,
                env: None,
                timeout_ms: 2000,
            },
            parser: ParserSpec::ProviderSnapshot,
            window_label_overrides: std::collections::HashMap::new(),
            visible_window_ids: Vec::new(),
        }
        };

        let config = AppConfig {
            schema_version: 5,
            refresh_interval_seconds: 300,
            display_mode: "remaining".to_string(),
            low_quota_warning_threshold: 20.0,
            launch_at_startup: false,
            log_level: "info".to_string(),
            network_proxy: None,
            providers: vec![
                provider("provider-a", "Provider A", &counter_a),
                provider("provider-b", "Provider B", &counter_b),
            ],
        };
        save_config_to_path(&path, &config).expect("save config");

        let initial = build_app_snapshot_from_config_path(&path).expect("initial snapshot");
        let refreshed =
            refresh_provider_from_config_path(&path, "provider-a").expect("refresh provider");

        assert_eq!(initial.providers[0].windows[0].used, Some(1.0));
        assert_eq!(initial.providers[1].windows[0].used, Some(1.0));
        assert_eq!(refreshed.providers[0].id, "provider-a");
        assert_eq!(refreshed.providers[0].windows[0].used, Some(2.0));
        assert_eq!(refreshed.providers[1].id, "provider-b");
        assert_eq!(refreshed.providers[1].windows[0].used, Some(1.0));
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
