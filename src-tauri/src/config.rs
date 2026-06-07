use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub schema_version: u8,
    pub refresh_interval_seconds: u64,
    pub display_mode: String,
    pub low_quota_warning_threshold: f64,
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
}

#[derive(Debug, Clone)]
pub struct LoadedConfig {
    pub config: AppConfig,
    pub recovery_messages: Vec<String>,
}

pub fn default_config() -> AppConfig {
    AppConfig {
        schema_version: 1,
        refresh_interval_seconds: 300,
        display_mode: "remaining".to_string(),
        low_quota_warning_threshold: 20.0,
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
    match serde_json::from_str::<AppConfig>(&contents) {
        Ok(config) => Ok(LoadedConfig {
            config,
            recovery_messages: Vec::new(),
        }),
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

pub fn save_config_to_path(path: &Path, config: &AppConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let contents = serde_json::to_string_pretty(config).map_err(|error| error.to_string())?;
    fs::write(path, contents).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_config(app: AppHandle) -> Result<AppConfig, String> {
    let path = config_path_for_app(&app)?;
    load_or_create_config(&path).map(|loaded| loaded.config)
}

#[tauri::command]
pub fn save_config(app: AppHandle, config: AppConfig) -> Result<(), String> {
    let path = config_path_for_app(&app)?;
    save_config_to_path(&path, &config)
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
