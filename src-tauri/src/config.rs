use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

pub const CURRENT_CONFIG_SCHEMA_VERSION: u8 = 5;
const CONFIG_FILE_NAME: &str = "config.quotaBarWin.json";
const LEGACY_CONFIG_FILE_NAME: &str = "config.json";
const PORTABLE_MARKER_FILE_NAME: &str = "quotabarwin.portable";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub schema_version: u8,
    pub refresh_interval_seconds: u64,
    pub display_mode: String,
    pub low_quota_warning_threshold: f64,
    #[serde(default)]
    pub launch_at_startup: bool,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    pub providers: Vec<ProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ProviderConfig {
    #[serde(rename = "mock")]
    Mock {
        id: String,
        name: String,
        enabled: bool,
    },
    #[serde(rename = "codex")]
    Codex {
        id: String,
        name: String,
        enabled: bool,
        #[serde(rename = "authToken", alias = "auth_token", alias = "auth-token")]
        auth_token: String,
        #[serde(
            default,
            rename = "accountId",
            alias = "account_id",
            alias = "account-id"
        )]
        account_id: Option<String>,
        #[serde(default, rename = "proxyUrl", alias = "proxy_url", alias = "proxy-url")]
        proxy_url: Option<String>,
        #[serde(rename = "timeoutMs", alias = "timeout_ms", alias = "timeout-ms")]
        timeout_ms: u64,
        #[serde(
            default,
            rename = "windowLabelOverrides",
            alias = "window_label_overrides",
            alias = "window-label-overrides"
        )]
        window_label_overrides: HashMap<String, String>,
        #[serde(
            default,
            rename = "visibleWindowIds",
            alias = "visible_window_ids",
            alias = "visible-window-ids"
        )]
        visible_window_ids: Vec<String>,
    },
    #[serde(rename = "command")]
    Command {
        id: String,
        name: String,
        enabled: bool,
        command: CommandSpec,
        parser: ParserSpec,
        #[serde(
            default,
            rename = "windowLabelOverrides",
            alias = "window_label_overrides",
            alias = "window-label-overrides"
        )]
        window_label_overrides: HashMap<String, String>,
        #[serde(
            default,
            rename = "visibleWindowIds",
            alias = "visible_window_ids",
            alias = "visible-window-ids"
        )]
        visible_window_ids: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommandSpec {
    pub executable: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ParserSpec {
    AppSnapshot,
    ProviderSnapshot,
    KimiCodingUsageV1,
    BigmodelQuotaLimitJsonV1,
    JsonMapping { mapping: serde_json::Value },
    RegexBlocks { rules: Vec<serde_json::Value> },
}

#[derive(Debug, Clone)]
pub struct LoadedConfig {
    pub config: AppConfig,
    pub recovery_messages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConfigStorageInfo {
    pub mode: String,
    pub config_path: String,
    pub config_dir: String,
    pub app_data_config_path: String,
    pub portable_config_path: String,
    pub portable_marker_path: String,
}

fn default_log_level() -> String {
    "info".to_string()
}

pub fn default_config() -> AppConfig {
    AppConfig {
        schema_version: CURRENT_CONFIG_SCHEMA_VERSION,
        refresh_interval_seconds: 300,
        display_mode: "remaining".to_string(),
        low_quota_warning_threshold: 20.0,
        launch_at_startup: false,
        log_level: default_log_level(),
        providers: vec![ProviderConfig::Mock {
            id: "mock-codex".to_string(),
            name: "Codex Mock".to_string(),
            enabled: true,
        }],
    }
}

fn app_data_config_path_for_app(app: &AppHandle) -> Result<PathBuf, String> {
    if cfg!(windows) {
        let app_data = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| "APPDATA is not set".to_string())?;
        Ok(app_data.join("QuotaBarWin").join(CONFIG_FILE_NAME))
    } else {
        app.path()
            .app_config_dir()
            .map(|dir| dir.join(CONFIG_FILE_NAME))
            .map_err(|error| error.to_string())
    }
}

fn legacy_app_data_config_path_for_app(app: &AppHandle) -> Result<PathBuf, String> {
    if cfg!(windows) {
        let app_data = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| "APPDATA is not set".to_string())?;
        Ok(app_data.join("QuotaBarWin").join(LEGACY_CONFIG_FILE_NAME))
    } else {
        app.path()
            .app_config_dir()
            .map(|dir| dir.join(LEGACY_CONFIG_FILE_NAME))
            .map_err(|error| error.to_string())
    }
}

fn app_exe_dir_for_app(_app: &AppHandle) -> Result<PathBuf, String> {
    std::env::current_exe()
        .map_err(|error| error.to_string())?
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "Unable to resolve app executable directory".to_string())
}

fn portable_config_path_for_exe_dir(exe_dir: &Path) -> PathBuf {
    exe_dir.join(CONFIG_FILE_NAME)
}

fn legacy_portable_config_path_for_exe_dir(exe_dir: &Path) -> PathBuf {
    exe_dir.join(LEGACY_CONFIG_FILE_NAME)
}

fn portable_marker_path_for_exe_dir(exe_dir: &Path) -> PathBuf {
    exe_dir.join(PORTABLE_MARKER_FILE_NAME)
}

fn portable_paths_for_app(app: &AppHandle) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let exe_dir = app_exe_dir_for_app(app)?;
    Ok((
        portable_config_path_for_exe_dir(&exe_dir),
        legacy_portable_config_path_for_exe_dir(&exe_dir),
        portable_marker_path_for_exe_dir(&exe_dir),
    ))
}

fn migrate_legacy_config_path(preferred_path: &Path, legacy_path: &Path) -> Result<(), String> {
    if preferred_path.exists() || !legacy_path.exists() {
        return Ok(());
    }

    fs::rename(legacy_path, preferred_path).map_err(|error| error.to_string())
}

fn config_path_from_candidates(
    app_data_path: PathBuf,
    portable_path: PathBuf,
    marker_path: PathBuf,
) -> PathBuf {
    if marker_path.exists() {
        portable_path
    } else {
        app_data_path
    }
}

pub fn config_path_for_app(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data_path = app_data_config_path_for_app(app)?;
    let legacy_app_data_path = legacy_app_data_config_path_for_app(app)?;
    let (portable_path, legacy_portable_path, marker_path) = portable_paths_for_app(app)?;
    let preferred_path =
        config_path_from_candidates(app_data_path, portable_path, marker_path.clone());
    let legacy_path =
        config_path_from_candidates(legacy_app_data_path, legacy_portable_path, marker_path);
    migrate_legacy_config_path(&preferred_path, &legacy_path)?;
    Ok(preferred_path)
}

fn config_storage_info_for_app(app: &AppHandle) -> Result<ConfigStorageInfo, String> {
    let app_data_config_path = app_data_config_path_for_app(app)?;
    let (portable_config_path, _, portable_marker_path) = portable_paths_for_app(app)?;
    let config_path = config_path_from_candidates(
        app_data_config_path.clone(),
        portable_config_path.clone(),
        portable_marker_path.clone(),
    );
    let mode = if config_path == portable_config_path {
        "portable"
    } else {
        "app-data"
    }
    .to_string();
    let config_dir = config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    Ok(ConfigStorageInfo {
        mode,
        config_path: config_path.display().to_string(),
        config_dir: config_dir.display().to_string(),
        app_data_config_path: app_data_config_path.display().to_string(),
        portable_config_path: portable_config_path.display().to_string(),
        portable_marker_path: portable_marker_path.display().to_string(),
    })
}

pub fn load_or_create_config(path: &Path) -> Result<LoadedConfig, String> {
    if !path.exists() {
        let config = default_config();
        save_config_to_path(path, &config)?;
        return Ok(LoadedConfig {
            config,
            recovery_messages: Vec::new(),
        });
    }

    let contents = fs::read_to_string(path).map_err(|error| error.to_string())?;
    match load_config_contents_with_migration(path, &contents) {
        Ok(loaded) => Ok(loaded),
        Err(error) => {
            let timestamp = Utc::now().format("%Y%m%d%H%M%S");
            let backup_path = path.with_file_name(format!("config.corrupt.{timestamp}.json"));
            fs::rename(path, &backup_path).map_err(|rename_error| rename_error.to_string())?;
            let config = default_config();
            save_config_to_path(path, &config)?;
            Ok(LoadedConfig {
                config,
                recovery_messages: vec![format!(
                    "Recovered corrupt config: {}; backup: {}",
                    error,
                    backup_path.display()
                )],
            })
        }
    }
}

fn load_config_contents_with_migration(
    path: &Path,
    contents: &str,
) -> Result<LoadedConfig, String> {
    let value =
        serde_json::from_str::<serde_json::Value>(contents).map_err(|error| error.to_string())?;
    let original_version = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(1) as u8;
    let migrated = migrate_config_value(value)?;
    let config =
        serde_json::from_value::<AppConfig>(migrated).map_err(|error| error.to_string())?;

    if original_version != CURRENT_CONFIG_SCHEMA_VERSION {
        let backup_path = backup_config(path, "pre-migration")?;
        save_config_to_path(path, &config).map_err(|error| {
            let _ = fs::copy(&backup_path, path);
            error
        })?;
        return Ok(LoadedConfig {
            config,
            recovery_messages: vec![format!(
                "Migrated config from schemaVersion {original_version} to {CURRENT_CONFIG_SCHEMA_VERSION}; backup: {}",
                backup_path.display()
            )],
        });
    }

    Ok(LoadedConfig {
        config,
        recovery_messages: Vec::new(),
    })
}

pub fn migrate_config_value(mut value: serde_json::Value) -> Result<serde_json::Value, String> {
    let version = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(1);

    if version > CURRENT_CONFIG_SCHEMA_VERSION as u64 {
        return Err(format!("Unsupported config schemaVersion {version}"));
    }

    if version < 2 {
        value["launchAtStartup"] = serde_json::Value::Bool(false);
        value["schemaVersion"] = serde_json::json!(2);
    }

    let version = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(2);
    if version < 3 {
        value["logLevel"] = serde_json::json!("info");
        value["schemaVersion"] = serde_json::json!(3);
    }

    let version = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(3);
    if version < 4 {
        add_default_window_label_overrides(&mut value);
        value["schemaVersion"] = serde_json::json!(4);
    }

    let version = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(4);
    if version < 5 {
        add_default_visible_windows(&mut value);
        value["schemaVersion"] = serde_json::json!(5);
    }

    Ok(value)
}

fn add_default_window_label_overrides(value: &mut serde_json::Value) {
    let Some(providers) = value
        .get_mut("providers")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return;
    };

    for provider in providers {
        if provider.get("kind").and_then(serde_json::Value::as_str) != Some("command") {
            continue;
        }

        if provider.get("windowLabelOverrides").is_none() {
            provider["windowLabelOverrides"] = serde_json::json!({});
        }

        if provider
            .pointer("/parser/type")
            .and_then(serde_json::Value::as_str)
            == Some("bigmodel-quota-limit-json-v1")
        {
            let Some(overrides) = provider
                .get_mut("windowLabelOverrides")
                .and_then(serde_json::Value::as_object_mut)
            else {
                continue;
            };
            overrides
                .entry("tokens-limit-3-5".to_string())
                .or_insert_with(|| serde_json::json!("5h"));
            overrides
                .entry("tokens-limit-6-1".to_string())
                .or_insert_with(|| serde_json::json!("Weekly limit"));
        }
    }
}

fn add_default_visible_windows(value: &mut serde_json::Value) {
    let Some(providers) = value
        .get_mut("providers")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return;
    };

    for provider in providers {
        if provider.get("kind").and_then(serde_json::Value::as_str) != Some("command") {
            continue;
        }

        if provider.get("visibleWindowIds").is_none() {
            provider["visibleWindowIds"] = serde_json::json!([]);
        }
    }
}

pub fn migrate_config_file(path: &Path) -> Result<AppConfig, String> {
    let original = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let backup_path = backup_config(path, "pre-migration")?;
    let value =
        serde_json::from_str::<serde_json::Value>(&original).map_err(|error| error.to_string())?;
    match migrate_config_value(value)
        .and_then(|value| {
            serde_json::from_value::<AppConfig>(value).map_err(|error| error.to_string())
        })
        .and_then(|config| {
            save_config_to_path(path, &config)?;
            Ok(config)
        }) {
        Ok(config) => Ok(config),
        Err(error) => {
            let _ = fs::copy(&backup_path, path);
            Err(error)
        }
    }
}

fn backup_config(path: &Path, reason: &str) -> Result<PathBuf, String> {
    let timestamp = Utc::now().format("%Y%m%d%H%M%S");
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| CONFIG_FILE_NAME.into());
    let backup_path = path.with_file_name(format!("{file_name}.{reason}.{timestamp}.bak"));
    fs::copy(path, &backup_path).map_err(|error| error.to_string())?;
    Ok(backup_path)
}

pub fn save_config_to_path(path: &Path, config: &AppConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let contents = serde_json::to_string_pretty(config).map_err(|error| error.to_string())?;
    fs::write(path, contents).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_config(app: AppHandle) -> Result<AppConfig, String> {
    let path = config_path_for_app(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        load_or_create_config(&path).map(|loaded| loaded.config)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn save_config(app: AppHandle, config: AppConfig) -> Result<(), String> {
    let path = config_path_for_app(&app)?;
    tauri::async_runtime::spawn_blocking(move || save_config_to_path(&path, &config))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn get_config_storage_info(app: AppHandle) -> Result<ConfigStorageInfo, String> {
    config_storage_info_for_app(&app)
}

#[tauri::command]
pub async fn set_portable_mode(app: AppHandle, enabled: bool) -> Result<ConfigStorageInfo, String> {
    let current_path = config_path_for_app(&app)?;
    let app_data_path = app_data_config_path_for_app(&app)?;
    let (portable_path, _, marker_path) = portable_paths_for_app(&app)?;

    tauri::async_runtime::spawn_blocking(move || {
        let config = load_or_create_config(&current_path)?.config;
        if enabled {
            save_config_to_path(&portable_path, &config)?;
            fs::write(&marker_path, "QuotaBarWin portable mode\n")
                .map_err(|error| error.to_string())?;
        } else {
            save_config_to_path(&app_data_path, &config)?;
            if marker_path.exists() {
                fs::remove_file(&marker_path).map_err(|error| error.to_string())?;
            }
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|error| error.to_string())??;

    config_storage_info_for_app(&app)
}

#[tauri::command]
pub async fn reset_config(app: AppHandle) -> Result<AppConfig, String> {
    let path = config_path_for_app(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        if path.exists() {
            backup_config(&path, "reset")?;
        }
        let config = default_config();
        save_config_to_path(&path, &config)?;
        Ok(config)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn open_config_folder(app: AppHandle) -> Result<(), String> {
    let path = config_path_for_app(&app)?;
    tauri::async_runtime::spawn_blocking(move || reveal_path(&path))
        .await
        .map_err(|error| error.to_string())?
}

fn reveal_path(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let dir = path.parent().unwrap_or(path);
        Command::new("xdg-open")
            .arg(dir)
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_default_config_when_missing() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");

        let loaded = load_or_create_config(&path).expect("config loads");

        assert_eq!(loaded.config, default_config());
        assert!(path.exists());
    }

    #[test]
    fn save_then_load_round_trips_config() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("nested").join("config.json");
        let config = AppConfig {
            schema_version: CURRENT_CONFIG_SCHEMA_VERSION,
            refresh_interval_seconds: 120,
            display_mode: "used".to_string(),
            low_quota_warning_threshold: 15.0,
            launch_at_startup: true,
            log_level: "debug".to_string(),
            providers: vec![ProviderConfig::Mock {
                id: "mock".to_string(),
                name: "Mock".to_string(),
                enabled: false,
            }],
        };

        save_config_to_path(&path, &config).expect("save config");
        let loaded = load_or_create_config(&path).expect("load config");

        assert_eq!(loaded.config, config);
        assert!(loaded.recovery_messages.is_empty());
    }

    #[test]
    fn provider_config_parses_all_supported_kinds() {
        let providers = serde_json::json!([
            {
                "kind": "mock",
                "id": "mock",
                "name": "Mock",
                "enabled": true
            },
            {
                "kind": "codex",
                "id": "codex",
                "name": "Codex",
                "enabled": true,
                "authToken": "${env:CODEX_ACCESS_TOKEN}",
                "accountId": "acct",
                "proxyUrl": "socks5h://127.0.0.1:7890",
                "timeoutMs": 15000,
                "windowLabelOverrides": { "weekly": "Weekly limit" },
                "visibleWindowIds": ["5h", "weekly"]
            },
            {
                "kind": "command",
                "id": "command",
                "name": "Command",
                "enabled": true,
                "command": {
                    "executable": "node",
                    "args": ["fixtures/fake_provider_snapshot.js"],
                    "timeoutMs": 15000
                },
                "parser": { "type": "provider-snapshot" },
                "windowLabelOverrides": {},
                "visibleWindowIds": []
            }
        ]);

        let parsed = serde_json::from_value::<Vec<ProviderConfig>>(providers)
            .expect("deserialize providers");

        assert!(matches!(parsed[0], ProviderConfig::Mock { .. }));
        assert!(matches!(parsed[1], ProviderConfig::Codex { .. }));
        assert!(matches!(parsed[2], ProviderConfig::Command { .. }));
    }

    #[test]
    fn config_migration_v1_to_v2() {
        let value = serde_json::json!({
            "schemaVersion": 1,
            "refreshIntervalSeconds": 300,
            "displayMode": "remaining",
            "lowQuotaWarningThreshold": 20,
            "providers": []
        });

        let migrated = migrate_config_value(value).expect("migrates");

        assert_eq!(migrated["schemaVersion"], serde_json::json!(5));
        assert_eq!(migrated["launchAtStartup"], serde_json::json!(false));
        assert_eq!(migrated["logLevel"], serde_json::json!("info"));
    }

    #[test]
    fn config_migration_v3_to_v4_adds_window_label_overrides() {
        let value = serde_json::json!({
            "schemaVersion": 3,
            "refreshIntervalSeconds": 300,
            "displayMode": "remaining",
            "lowQuotaWarningThreshold": 20,
            "launchAtStartup": false,
            "logLevel": "info",
            "providers": [{
                "kind": "command",
                "id": "bigmodel",
                "name": "BigModel",
                "enabled": true,
                "command": {
                    "executable": "curl",
                    "args": [],
                    "timeoutMs": 15000
                },
                "parser": {
                    "type": "bigmodel-quota-limit-json-v1"
                }
            }]
        });

        let migrated = migrate_config_value(value).expect("migrates");

        assert_eq!(migrated["schemaVersion"], serde_json::json!(5));
        assert_eq!(
            migrated["providers"][0]["visibleWindowIds"],
            serde_json::json!([])
        );
        assert_eq!(
            migrated["providers"][0]["windowLabelOverrides"],
            serde_json::json!({
                "tokens-limit-3-5": "5h",
                "tokens-limit-6-1": "Weekly limit"
            })
        );
    }

    #[test]
    fn config_migration_v4_preserves_custom_window_label_overrides() {
        let value = serde_json::json!({
            "schemaVersion": 3,
            "refreshIntervalSeconds": 300,
            "displayMode": "remaining",
            "lowQuotaWarningThreshold": 20,
            "launchAtStartup": false,
            "logLevel": "info",
            "providers": [{
                "kind": "command",
                "id": "bigmodel",
                "name": "BigModel",
                "enabled": true,
                "command": {
                    "executable": "curl",
                    "args": [],
                    "timeoutMs": 15000
                },
                "parser": {
                    "type": "bigmodel-quota-limit-json-v1"
                },
                "windowLabelOverrides": {
                    "tokens-limit-3-5": "Five hour custom"
                }
            }]
        });

        let migrated = migrate_config_value(value).expect("migrates");

        assert_eq!(
            migrated["providers"][0]["windowLabelOverrides"],
            serde_json::json!({
                "tokens-limit-3-5": "Five hour custom",
                "tokens-limit-6-1": "Weekly limit"
            })
        );
    }

    #[test]
    fn command_provider_fields_serialize_as_frontend_camel_case() {
        let config = AppConfig {
            schema_version: CURRENT_CONFIG_SCHEMA_VERSION,
            refresh_interval_seconds: 300,
            display_mode: "remaining".to_string(),
            low_quota_warning_threshold: 20.0,
            launch_at_startup: false,
            log_level: "info".to_string(),
            providers: vec![ProviderConfig::Command {
                id: "provider".to_string(),
                name: "Provider".to_string(),
                enabled: true,
                command: CommandSpec {
                    executable: "node".to_string(),
                    args: Vec::new(),
                    cwd: None,
                    env: None,
                    timeout_ms: 15000,
                },
                parser: ParserSpec::ProviderSnapshot,
                window_label_overrides: HashMap::from([(
                    "300-minute".to_string(),
                    "5h".to_string(),
                )]),
                visible_window_ids: vec!["5h".to_string()],
            }],
        };

        let value = serde_json::to_value(config).expect("serialize config");

        assert_eq!(
            value["providers"][0]["windowLabelOverrides"],
            serde_json::json!({ "300-minute": "5h" })
        );
        assert_eq!(
            value["providers"][0]["visibleWindowIds"],
            serde_json::json!(["5h"])
        );
        assert!(value["providers"][0]
            .get("window_label_overrides")
            .is_none());
        assert!(value["providers"][0].get("visible_window_ids").is_none());
    }

    #[test]
    fn command_provider_fields_deserialize_from_frontend_camel_case() {
        let value = serde_json::json!({
            "kind": "command",
            "id": "provider",
            "name": "Provider",
            "enabled": true,
            "command": {
                "executable": "node",
                "args": [],
                "timeoutMs": 15000
            },
            "parser": { "type": "provider-snapshot" },
            "windowLabelOverrides": { "300-minute": "5h" },
            "visibleWindowIds": ["5h"]
        });

        let provider =
            serde_json::from_value::<ProviderConfig>(value).expect("deserialize provider");

        match provider {
            ProviderConfig::Command {
                window_label_overrides,
                visible_window_ids,
                ..
            } => {
                assert_eq!(
                    window_label_overrides.get("300-minute"),
                    Some(&"5h".to_string())
                );
                assert_eq!(visible_window_ids, vec!["5h".to_string()]);
            }
            _ => panic!("expected command provider"),
        }
    }

    #[test]
    fn config_migration_backup_created() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        fs::write(
            &path,
            r#"{"schemaVersion":1,"refreshIntervalSeconds":300,"displayMode":"remaining","lowQuotaWarningThreshold":20,"providers":[]}"#,
        )
        .expect("write config");

        let loaded = load_or_create_config(&path).expect("config loads");
        let backups = fs::read_dir(temp.path())
            .expect("read dir")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .contains(".pre-migration.")
            })
            .count();

        assert_eq!(loaded.config.schema_version, CURRENT_CONFIG_SCHEMA_VERSION);
        assert_eq!(backups, 1);
    }

    #[test]
    fn config_migration_rolls_back_on_failure() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        let original = r#"{"schemaVersion":99,"refreshIntervalSeconds":300,"displayMode":"remaining","lowQuotaWarningThreshold":20,"providers":[]}"#;
        fs::write(&path, original).expect("write config");

        let result = migrate_config_file(&path);

        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&path).expect("read config"), original);
    }

    #[test]
    fn backs_up_corrupt_config() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        fs::write(&path, "{not valid json").expect("write corrupt config");

        let loaded = load_or_create_config(&path).expect("config recovers");
        let backups = fs::read_dir(temp.path())
            .expect("read dir")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".corrupt."))
            .count();

        assert_eq!(loaded.config, default_config());
        assert_eq!(backups, 1);
        assert_eq!(loaded.recovery_messages.len(), 1);
    }

    #[test]
    fn portable_marker_selects_portable_config_path() {
        let temp = tempfile::tempdir().expect("temp dir");
        let app_data_path = temp.path().join("app-data").join(CONFIG_FILE_NAME);
        let portable_path = temp.path().join("app").join(CONFIG_FILE_NAME);
        let marker_path = temp.path().join("app").join("quotabarwin.portable");

        let selected_without_marker = config_path_from_candidates(
            app_data_path.clone(),
            portable_path.clone(),
            marker_path.clone(),
        );
        assert_eq!(selected_without_marker, app_data_path);

        fs::create_dir_all(marker_path.parent().expect("marker parent")).expect("create dir");
        fs::write(&marker_path, "portable").expect("write marker");

        let selected_with_marker =
            config_path_from_candidates(app_data_path, portable_path.clone(), marker_path);
        assert_eq!(selected_with_marker, portable_path);
    }

    #[test]
    fn legacy_config_json_is_renamed_to_product_config_name() {
        let temp = tempfile::tempdir().expect("temp dir");
        let preferred_path = temp.path().join(CONFIG_FILE_NAME);
        let legacy_path = temp.path().join(LEGACY_CONFIG_FILE_NAME);
        fs::write(&legacy_path, "{}").expect("write legacy");

        migrate_legacy_config_path(&preferred_path, &legacy_path).expect("migrate legacy");

        assert!(preferred_path.exists());
        assert!(!legacy_path.exists());
    }
}
