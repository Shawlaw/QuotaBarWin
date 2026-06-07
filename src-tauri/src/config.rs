use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

pub const CURRENT_CONFIG_SCHEMA_VERSION: u8 = 3;

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
    #[serde(rename = "command")]
    Command {
        id: String,
        name: String,
        enabled: bool,
        command: CommandSpec,
        parser: ParserSpec,
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

pub fn config_path_for_app(app: &AppHandle) -> Result<PathBuf, String> {
    if cfg!(windows) {
        let app_data = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| "APPDATA is not set".to_string())?;
        Ok(app_data.join("QuotaBarWin").join("config.json"))
    } else {
        app.path()
            .app_config_dir()
            .map(|dir| dir.join("config.json"))
            .map_err(|error| error.to_string())
    }
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

fn load_config_contents_with_migration(path: &Path, contents: &str) -> Result<LoadedConfig, String> {
    let value = serde_json::from_str::<serde_json::Value>(contents).map_err(|error| error.to_string())?;
    let original_version = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(1) as u8;
    let migrated = migrate_config_value(value)?;
    let config = serde_json::from_value::<AppConfig>(migrated).map_err(|error| error.to_string())?;

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

    Ok(value)
}

pub fn migrate_config_file(path: &Path) -> Result<AppConfig, String> {
    let original = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let backup_path = backup_config(path, "pre-migration")?;
    let value = serde_json::from_str::<serde_json::Value>(&original).map_err(|error| error.to_string())?;
    match migrate_config_value(value)
        .and_then(|value| serde_json::from_value::<AppConfig>(value).map_err(|error| error.to_string()))
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
    let backup_path = path.with_file_name(format!("config.{reason}.{timestamp}.json"));
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
    fn config_migration_v1_to_v2() {
        let value = serde_json::json!({
            "schemaVersion": 1,
            "refreshIntervalSeconds": 300,
            "displayMode": "remaining",
            "lowQuotaWarningThreshold": 20,
            "providers": []
        });

        let migrated = migrate_config_value(value).expect("migrates");

        assert_eq!(migrated["schemaVersion"], serde_json::json!(3));
        assert_eq!(migrated["launchAtStartup"], serde_json::json!(false));
        assert_eq!(migrated["logLevel"], serde_json::json!("info"));
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
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("config.pre-migration."))
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
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("config.corrupt."))
            .count();

        assert_eq!(loaded.config, default_config());
        assert_eq!(backups, 1);
        assert_eq!(loaded.recovery_messages.len(), 1);
    }
}
