use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;

use crate::proxy::ProxyConfig;

pub const CURRENT_CONFIG_SCHEMA_VERSION: u8 = 14;
pub const DEFAULT_LOG_MAX_BYTES: u64 = 10 * 1024 * 1024;
pub const DEFAULT_REMOTE_PROVIDER_TIMEOUT_SECONDS: u64 = 30;
pub const DEFAULT_REMOTE_PROVIDER_REGISTRY_URL: &str =
    "https://raw.githubusercontent.com/Shawlaw/QuotaBarWin/main/examples/remote-providers/registry.json";
const CONFIG_FILE_NAME: &str = "config.quotaBarWin.json";
const LEGACY_CONFIG_FILE_NAME: &str = "config.json";
const PORTABLE_MARKER_FILE_NAME: &str = "quotabarwin.portable";
const REMOTE_PROVIDER_GUIDE_FILE_NAME: &str = "remote-provider-guide.html";
const REMOTE_PROVIDER_GUIDE_HTML: &str = include_str!("remote_provider_guide.html");

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
    #[serde(default = "default_log_max_bytes")]
    pub log_max_bytes: u64,
    #[serde(default)]
    pub log_quota_data: bool,
    #[serde(default = "default_language")]
    pub language: AppLanguage,
    #[serde(default)]
    pub network_proxy: Option<ProxyConfig>,
    #[serde(default)]
    pub tray_popup_position: Option<TrayPopupPosition>,
    #[serde(default)]
    pub tray_popup_size: Option<TrayPopupSize>,
    #[serde(default)]
    pub remote_provider_registry: RemoteProviderRegistrySettings,
    pub providers: Vec<ProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteProviderRegistrySettings {
    #[serde(default = "default_remote_provider_registry_url")]
    pub registry_url: Option<String>,
    #[serde(default)]
    pub provider_proxy_url: Option<String>,
    #[serde(default = "default_remote_provider_auto_update")]
    pub auto_update: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<RemoteProviderRegistrySource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteProviderRegistrySource {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub provider_proxy_url: Option<String>,
    #[serde(default = "default_remote_provider_auto_update")]
    pub auto_update: bool,
    #[serde(default = "default_remote_provider_source_enabled")]
    pub enabled: bool,
}

impl Default for RemoteProviderRegistrySettings {
    fn default() -> Self {
        Self {
            registry_url: default_remote_provider_registry_url(),
            provider_proxy_url: None,
            auto_update: default_remote_provider_auto_update(),
            sources: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrayPopupPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrayPopupSize {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AppLanguage {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "en")]
    En,
    #[serde(rename = "zh-CN")]
    ZhCn,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ProviderConfig {
    #[serde(rename = "remote")]
    Remote {
        id: String,
        name: String,
        enabled: bool,
        #[serde(default)]
        version: Option<String>,
        #[serde(rename = "manifestUrl", alias = "manifest_url", alias = "manifest-url")]
        manifest_url: String,
        #[serde(rename = "sourceUrl", alias = "source_url", alias = "source-url")]
        source_url: String,
        #[serde(
            default,
            rename = "providerDir",
            alias = "provider_dir",
            alias = "provider-dir"
        )]
        provider_dir: Option<PathBuf>,
        runtime: String,
        #[serde(
            default,
            rename = "resolvedRuntime",
            alias = "resolved_runtime",
            alias = "resolved-runtime"
        )]
        resolved_runtime: Option<String>,
        #[serde(default, rename = "proxyUrl", alias = "proxy_url", alias = "proxy-url")]
        proxy_url: Option<String>,
        #[serde(
            default,
            rename = "autoUpdate",
            alias = "auto_update",
            alias = "auto-update"
        )]
        auto_update: bool,
        #[serde(
            rename = "updateIntervalSeconds",
            alias = "update_interval_seconds",
            alias = "update-interval-seconds",
            default = "default_update_interval_seconds"
        )]
        update_interval_seconds: u64,
        #[serde(
            rename = "timeoutSeconds",
            alias = "timeout_seconds",
            alias = "timeout-seconds",
            default = "default_remote_provider_timeout_seconds"
        )]
        timeout_seconds: u64,
        #[serde(
            default,
            rename = "trustedChecksum",
            alias = "trusted_checksum",
            alias = "trusted-checksum"
        )]
        trusted_checksum: Option<String>,
        #[serde(
            default,
            rename = "installedAt",
            alias = "installed_at",
            alias = "installed-at"
        )]
        installed_at: Option<String>,
        #[serde(
            default,
            rename = "updatedAt",
            alias = "updated_at",
            alias = "updated-at"
        )]
        updated_at: Option<String>,
        #[serde(
            default,
            rename = "lastCheckedAt",
            alias = "last_checked_at",
            alias = "last-checked-at"
        )]
        last_checked_at: Option<String>,
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
        #[serde(default, rename = "envVars", alias = "env_vars", alias = "env-vars")]
        env_vars: HashMap<String, String>,
    },
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

pub fn default_log_max_bytes() -> u64 {
    DEFAULT_LOG_MAX_BYTES
}

fn default_language() -> AppLanguage {
    AppLanguage::ZhCn
}

fn default_update_interval_seconds() -> u64 {
    3600
}

fn default_remote_provider_timeout_seconds() -> u64 {
    DEFAULT_REMOTE_PROVIDER_TIMEOUT_SECONDS
}

fn default_remote_provider_auto_update() -> bool {
    true
}

fn default_remote_provider_source_enabled() -> bool {
    true
}

fn default_remote_provider_registry_url() -> Option<String> {
    Some(DEFAULT_REMOTE_PROVIDER_REGISTRY_URL.to_string())
}

pub fn default_config() -> AppConfig {
    AppConfig {
        schema_version: CURRENT_CONFIG_SCHEMA_VERSION,
        refresh_interval_seconds: 300,
        display_mode: "remaining".to_string(),
        low_quota_warning_threshold: 20.0,
        launch_at_startup: false,
        log_level: default_log_level(),
        log_max_bytes: default_log_max_bytes(),
        log_quota_data: false,
        language: default_language(),
        network_proxy: None,
        tray_popup_position: None,
        tray_popup_size: None,
        remote_provider_registry: RemoteProviderRegistrySettings::default(),
        providers: Vec::new(),
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
        value["schemaVersion"] = serde_json::json!(4);
    }

    let version = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(4);
    if version < 5 {
        value["schemaVersion"] = serde_json::json!(5);
    }

    let version = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(5);
    if version < 6 {
        value["schemaVersion"] = serde_json::json!(6);
    }

    let version = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(6);
    if version < 7 {
        value["networkProxy"] = serde_json::json!(null);
        value["schemaVersion"] = serde_json::json!(7);
    }

    let version = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(7);
    if version < 8 {
        value["language"] = serde_json::json!("zh-CN");
        value["schemaVersion"] = serde_json::json!(8);
    }

    let version = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(8);
    if version < 9 {
        value["trayPopupPosition"] = serde_json::json!(null);
        value["schemaVersion"] = serde_json::json!(9);
    }

    let version = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(9);
    if version < 10 {
        value["remoteProviderRegistry"] = serde_json::json!({
            "registryUrl": DEFAULT_REMOTE_PROVIDER_REGISTRY_URL,
            "providerProxyUrl": null,
            "autoUpdate": true
        });
        value["schemaVersion"] = serde_json::json!(10);
    }

    let version = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(10);
    if version < 11 {
        if let Some(providers) = value
            .get_mut("providers")
            .and_then(serde_json::Value::as_array_mut)
        {
            providers.retain(|provider| {
                provider.get("kind").and_then(serde_json::Value::as_str) == Some("remote")
            });
            for provider in providers {
                if provider.get("version").is_none() {
                    provider["version"] = serde_json::Value::Null;
                }
                if provider.get("installedAt").is_none() {
                    provider["installedAt"] = serde_json::Value::Null;
                }
                if provider.get("updatedAt").is_none() {
                    provider["updatedAt"] = serde_json::Value::Null;
                }
                if provider.get("lastCheckedAt").is_none() {
                    provider["lastCheckedAt"] = serde_json::Value::Null;
                }
            }
        }
        value["schemaVersion"] = serde_json::json!(11);
    }

    let version = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(11);
    if version < 12 {
        value["trayPopupSize"] = serde_json::json!(null);
        value["schemaVersion"] = serde_json::json!(12);
    }

    let version = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(12);
    if version < 13 {
        value["logMaxBytes"] = serde_json::json!(DEFAULT_LOG_MAX_BYTES);
        value["schemaVersion"] = serde_json::json!(13);
    }

    let version = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(13);
    if version < 14 {
        if let Some(providers) = value
            .get_mut("providers")
            .and_then(serde_json::Value::as_array_mut)
        {
            for provider in providers {
                if provider.get("kind").and_then(serde_json::Value::as_str) == Some("remote")
                    && provider.get("timeoutSeconds").is_none()
                {
                    provider["timeoutSeconds"] =
                        serde_json::json!(DEFAULT_REMOTE_PROVIDER_TIMEOUT_SECONDS);
                }
            }
        }
        value["schemaVersion"] = serde_json::json!(14);
    }

    Ok(value)
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

pub fn load_tray_popup_position_for_app(app: &AppHandle) -> Option<TrayPopupPosition> {
    let path = config_path_for_app(app).ok()?;
    load_or_create_config(&path)
        .ok()?
        .config
        .tray_popup_position
}

pub fn load_tray_popup_size_for_app(app: &AppHandle) -> Option<TrayPopupSize> {
    let path = config_path_for_app(app).ok()?;
    load_or_create_config(&path).ok()?.config.tray_popup_size
}

pub fn save_tray_popup_position_for_app(
    app: &AppHandle,
    position: TrayPopupPosition,
) -> Result<(), String> {
    let path = config_path_for_app(app)?;
    let mut config = load_or_create_config(&path)?.config;
    if config.tray_popup_position == Some(position) {
        return Ok(());
    }
    config.tray_popup_position = Some(position);
    save_config_to_path(&path, &config)
}

pub fn save_tray_popup_size_for_app(app: &AppHandle, size: TrayPopupSize) -> Result<(), String> {
    let path = config_path_for_app(app)?;
    let mut config = load_or_create_config(&path)?.config;
    if config.tray_popup_size == Some(size) {
        return Ok(());
    }
    config.tray_popup_size = Some(size);
    save_config_to_path(&path, &config)
}

pub fn resolve_secret_value(value: &str, config_dir: &Path) -> Result<String, String> {
    if let Some(name) = value
        .strip_prefix("${env:")
        .and_then(|remaining| remaining.strip_suffix('}'))
    {
        return std::env::var(name).map_err(|_| format!("Missing environment variable {name}"));
    }

    if let Some(raw_path) = value
        .strip_prefix("${file:")
        .and_then(|remaining| remaining.strip_suffix('}'))
    {
        let path = normalize_file_placeholder_path(raw_path);
        return fs::read_to_string(path)
            .map(|secret| secret.trim().to_string())
            .map_err(|error| format!("Unable to read secret file {path}: {error}"));
    }

    if let Some(name) = value
        .strip_prefix("${secret:")
        .and_then(|remaining| remaining.strip_suffix('}'))
    {
        return resolve_named_secret(name, config_dir);
    }

    Ok(value.to_string())
}

pub fn resolve_named_secret(name: &str, config_dir: &Path) -> Result<String, String> {
    let name = name.trim();
    if !is_valid_secret_name(name) {
        return Err(format!("Invalid secret name {name}"));
    }

    let secret_path = config_dir.join("secrets").join(format!("{name}.txt"));
    match fs::read_to_string(&secret_path) {
        Ok(secret) => {
            let secret = secret.trim().to_string();
            if !secret.is_empty() {
                return Ok(secret);
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "Unable to read secret {name} from local secret file: {error}"
            ));
        }
    }

    std::env::var(name).map_err(|_| {
        format!("Missing secret {name}; create secrets/{name}.txt in the config folder or set environment variable {name}")
    })
}

fn is_valid_secret_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn normalize_file_placeholder_path(raw_path: &str) -> &str {
    let path = raw_path.trim();
    if path.len() >= 2 {
        let first = path.as_bytes()[0];
        let last = path.as_bytes()[path.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return &path[1..path.len() - 1];
        }
    }

    path
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
    let launch_at_startup = config.launch_at_startup;
    tauri::async_runtime::spawn_blocking(move || save_config_to_path(&path, &config))
        .await
        .map_err(|error| error.to_string())??;
    sync_launch_at_startup_for_app(&app, launch_at_startup)?;
    crate::refresh_scheduler::signal_config_changed();
    crate::tray::refresh_tray_menu(&app)
}

pub fn sync_launch_at_startup_for_app(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let autolaunch = app.autolaunch();
    let is_enabled = autolaunch.is_enabled().map_err(|error| error.to_string())?;

    if enabled && !is_enabled {
        return autolaunch.enable().map_err(|error| error.to_string());
    }

    if !enabled && is_enabled {
        return autolaunch.disable().map_err(|error| error.to_string());
    }

    Ok(())
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

    crate::tray::refresh_tray_menu(&app)?;
    crate::refresh_scheduler::signal_config_changed();
    config_storage_info_for_app(&app)
}

#[tauri::command]
pub async fn reset_config(app: AppHandle) -> Result<AppConfig, String> {
    let path = config_path_for_app(&app)?;
    let config = tauri::async_runtime::spawn_blocking(move || {
        if path.exists() {
            backup_config(&path, "reset")?;
        }
        let config = default_config();
        save_config_to_path(&path, &config)?;
        Ok::<AppConfig, String>(config)
    })
    .await
    .map_err(|error| error.to_string())??;

    crate::tray::refresh_tray_menu(&app)?;
    crate::refresh_scheduler::signal_config_changed();
    Ok(config)
}

#[tauri::command]
pub async fn open_config_folder(app: AppHandle) -> Result<(), String> {
    let path = config_path_for_app(&app)?;
    tauri::async_runtime::spawn_blocking(move || reveal_path(&path))
        .await
        .map_err(|error| error.to_string())?
}

pub fn open_app_folder_for_app(app: &AppHandle) -> Result<(), String> {
    let exe_dir = app_exe_dir_for_app(app)?;
    open_path_external(&exe_dir)
}

#[tauri::command]
pub async fn open_remote_provider_guide(app: AppHandle) -> Result<(), String> {
    let guide_path = config_path_for_app(&app)?.with_file_name(REMOTE_PROVIDER_GUIDE_FILE_NAME);
    tauri::async_runtime::spawn_blocking(move || {
        write_remote_provider_guide(&guide_path)?;
        open_path_external(&guide_path)
    })
    .await
    .map_err(|error| error.to_string())?
}

fn write_remote_provider_guide(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    fs::write(path, REMOTE_PROVIDER_GUIDE_HTML).map_err(|error| error.to_string())
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

fn open_path_external(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        Command::new("rundll32")
            .arg("url.dll,FileProtocolHandler")
            .arg(path)
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote_provider_config(id: &str) -> ProviderConfig {
        ProviderConfig::Remote {
            id: id.to_string(),
            name: "Provider".to_string(),
            enabled: true,
            version: Some("1.0.0".to_string()),
            manifest_url: "https://example.com/provider.json".to_string(),
            source_url: "https://example.com/provider.cjs".to_string(),
            provider_dir: None,
            runtime: "node".to_string(),
            resolved_runtime: None,
            proxy_url: None,
            auto_update: true,
            update_interval_seconds: 3600,
            timeout_seconds: DEFAULT_REMOTE_PROVIDER_TIMEOUT_SECONDS,
            trusted_checksum: None,
            installed_at: Some("2026-06-18T00:00:00Z".to_string()),
            updated_at: Some("2026-06-18T00:00:00Z".to_string()),
            last_checked_at: Some("2026-06-18T00:00:00Z".to_string()),
            window_label_overrides: HashMap::new(),
            visible_window_ids: Vec::new(),
            env_vars: HashMap::new(),
        }
    }

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
            log_max_bytes: DEFAULT_LOG_MAX_BYTES,
            log_quota_data: true,
            language: AppLanguage::System,
            network_proxy: None,
            tray_popup_position: Some(TrayPopupPosition { x: 111, y: 222 }),
            tray_popup_size: Some(TrayPopupSize {
                width: 420.0,
                height: 640.0,
            }),
            remote_provider_registry: RemoteProviderRegistrySettings::default(),
            providers: vec![remote_provider_config("remote")],
        };

        save_config_to_path(&path, &config).expect("save config");
        let loaded = load_or_create_config(&path).expect("load config");

        assert_eq!(loaded.config, config);
        assert!(loaded.recovery_messages.is_empty());
    }

    #[test]
    fn provider_config_parses_remote_kind_only() {
        let provider = serde_json::json!({
            "kind": "remote",
            "id": "remote-kimi",
            "name": "Remote Kimi",
            "enabled": true,
            "version": "1.0.0",
            "manifestUrl": "https://example.com/provider.json",
            "sourceUrl": "https://example.com/provider.cjs",
            "runtime": "node",
            "autoUpdate": true,
            "updateIntervalSeconds": 1800,
            "trustedChecksum": "sha256:abc123",
            "installedAt": "2026-06-18T00:00:00Z",
            "updatedAt": "2026-06-18T00:00:00Z",
            "lastCheckedAt": "2026-06-18T00:00:00Z",
            "windowLabelOverrides": {},
            "visibleWindowIds": []
        });

        let parsed =
            serde_json::from_value::<ProviderConfig>(provider).expect("deserialize provider");

        let ProviderConfig::Remote {
            timeout_seconds, ..
        } = parsed;
        assert_eq!(timeout_seconds, DEFAULT_REMOTE_PROVIDER_TIMEOUT_SECONDS);
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

        assert_eq!(
            migrated["schemaVersion"],
            serde_json::json!(CURRENT_CONFIG_SCHEMA_VERSION)
        );
        assert_eq!(migrated["launchAtStartup"], serde_json::json!(false));
        assert_eq!(migrated["logLevel"], serde_json::json!("info"));
        assert_eq!(
            migrated["logMaxBytes"],
            serde_json::json!(DEFAULT_LOG_MAX_BYTES)
        );
        assert_eq!(migrated["language"], serde_json::json!("zh-CN"));
        assert_eq!(migrated["trayPopupPosition"], serde_json::json!(null));
        assert_eq!(migrated["trayPopupSize"], serde_json::json!(null));
        assert_eq!(
            migrated["remoteProviderRegistry"],
            serde_json::json!({
                "registryUrl": DEFAULT_REMOTE_PROVIDER_REGISTRY_URL,
                "providerProxyUrl": null,
                "autoUpdate": true
            })
        );
    }

    #[test]
    fn config_migration_v10_to_current_keeps_only_remote_providers() {
        let value = serde_json::json!({
            "schemaVersion": 10,
            "refreshIntervalSeconds": 300,
            "displayMode": "remaining",
            "lowQuotaWarningThreshold": 20,
            "launchAtStartup": false,
            "logLevel": "info",
            "language": "system",
            "networkProxy": null,
            "trayPopupPosition": null,
            "trayPopupSize": null,
            "remoteProviderRegistry": {
                "registryUrl": null,
                "providerProxyUrl": null,
                "autoUpdate": true
            },
            "providers": [
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
                    "timeoutMs": 15000
                },
                {
                    "kind": "remote",
                    "id": "remote-kimi",
                    "name": "Remote Kimi",
                    "enabled": true,
                    "manifestUrl": "https://example.com/provider.json",
                    "sourceUrl": "https://example.com/provider.cjs",
                    "runtime": "node",
                    "autoUpdate": true,
                    "updateIntervalSeconds": 1800
                }
            ]
        });

        let migrated = migrate_config_value(value).expect("migrates");

        assert_eq!(
            migrated["schemaVersion"],
            serde_json::json!(CURRENT_CONFIG_SCHEMA_VERSION)
        );
        assert_eq!(migrated["providers"].as_array().unwrap().len(), 1);
        assert_eq!(
            migrated["providers"][0]["kind"],
            serde_json::json!("remote")
        );
        assert_eq!(migrated["providers"][0]["version"], serde_json::Value::Null);
        assert_eq!(
            migrated["providers"][0]["installedAt"],
            serde_json::Value::Null
        );
        assert_eq!(
            migrated["providers"][0]["timeoutSeconds"],
            serde_json::json!(DEFAULT_REMOTE_PROVIDER_TIMEOUT_SECONDS)
        );
        assert_eq!(
            migrated["logMaxBytes"],
            serde_json::json!(DEFAULT_LOG_MAX_BYTES)
        );
    }

    #[test]
    fn config_migration_v7_to_current_adds_default_chinese_language() {
        let value = serde_json::json!({
            "schemaVersion": 7,
            "refreshIntervalSeconds": 300,
            "displayMode": "remaining",
            "lowQuotaWarningThreshold": 20,
            "launchAtStartup": false,
            "logLevel": "info",
            "networkProxy": null,
            "providers": []
        });

        let migrated = migrate_config_value(value).expect("migrates");

        assert_eq!(
            migrated["schemaVersion"],
            serde_json::json!(CURRENT_CONFIG_SCHEMA_VERSION)
        );
        assert_eq!(migrated["language"], serde_json::json!("zh-CN"));
        assert_eq!(migrated["trayPopupPosition"], serde_json::json!(null));
        assert_eq!(migrated["trayPopupSize"], serde_json::json!(null));
        assert_eq!(
            migrated["logMaxBytes"],
            serde_json::json!(DEFAULT_LOG_MAX_BYTES)
        );
    }

    #[test]
    fn config_migration_v8_to_current_adds_tray_popup_position() {
        let value = serde_json::json!({
            "schemaVersion": 8,
            "refreshIntervalSeconds": 300,
            "displayMode": "remaining",
            "lowQuotaWarningThreshold": 20,
            "launchAtStartup": false,
            "logLevel": "info",
            "language": "system",
            "networkProxy": null,
            "providers": []
        });

        let migrated = migrate_config_value(value).expect("migrates");

        assert_eq!(
            migrated["schemaVersion"],
            serde_json::json!(CURRENT_CONFIG_SCHEMA_VERSION)
        );
        assert_eq!(migrated["trayPopupPosition"], serde_json::json!(null));
        assert_eq!(migrated["trayPopupSize"], serde_json::json!(null));
        assert_eq!(
            migrated["logMaxBytes"],
            serde_json::json!(DEFAULT_LOG_MAX_BYTES)
        );
    }

    #[test]
    fn config_migration_v9_to_current_adds_remote_provider_registry_settings() {
        let value = serde_json::json!({
            "schemaVersion": 9,
            "refreshIntervalSeconds": 300,
            "displayMode": "remaining",
            "lowQuotaWarningThreshold": 20,
            "launchAtStartup": false,
            "logLevel": "info",
            "language": "system",
            "networkProxy": null,
            "trayPopupPosition": null,
            "providers": []
        });

        let migrated = migrate_config_value(value).expect("migrates");

        assert_eq!(
            migrated["schemaVersion"],
            serde_json::json!(CURRENT_CONFIG_SCHEMA_VERSION)
        );
        assert_eq!(
            migrated["remoteProviderRegistry"],
            serde_json::json!({
                "registryUrl": DEFAULT_REMOTE_PROVIDER_REGISTRY_URL,
                "providerProxyUrl": null,
                "autoUpdate": true
            })
        );
        assert_eq!(migrated["trayPopupSize"], serde_json::json!(null));
        assert_eq!(
            migrated["logMaxBytes"],
            serde_json::json!(DEFAULT_LOG_MAX_BYTES)
        );
    }

    #[test]
    fn config_migration_v11_to_current_adds_tray_popup_size() {
        let value = serde_json::json!({
            "schemaVersion": 11,
            "refreshIntervalSeconds": 300,
            "displayMode": "remaining",
            "lowQuotaWarningThreshold": 20,
            "launchAtStartup": false,
            "logLevel": "info",
            "language": "system",
            "networkProxy": null,
            "trayPopupPosition": null,
            "remoteProviderRegistry": {
                "registryUrl": null,
                "providerProxyUrl": null,
                "autoUpdate": true
            },
            "providers": []
        });

        let migrated = migrate_config_value(value).expect("migrates");

        assert_eq!(
            migrated["schemaVersion"],
            serde_json::json!(CURRENT_CONFIG_SCHEMA_VERSION)
        );
        assert_eq!(migrated["trayPopupSize"], serde_json::json!(null));
        assert_eq!(
            migrated["logMaxBytes"],
            serde_json::json!(DEFAULT_LOG_MAX_BYTES)
        );
    }

    #[test]
    fn config_migration_v12_to_current_adds_log_max_bytes() {
        let value = serde_json::json!({
            "schemaVersion": 12,
            "refreshIntervalSeconds": 300,
            "displayMode": "remaining",
            "lowQuotaWarningThreshold": 20,
            "launchAtStartup": false,
            "logLevel": "info",
            "language": "system",
            "networkProxy": null,
            "trayPopupPosition": null,
            "trayPopupSize": null,
            "remoteProviderRegistry": {
                "registryUrl": null,
                "providerProxyUrl": null,
                "autoUpdate": true
            },
            "providers": []
        });

        let migrated = migrate_config_value(value).expect("migrates");

        assert_eq!(
            migrated["schemaVersion"],
            serde_json::json!(CURRENT_CONFIG_SCHEMA_VERSION)
        );
        assert_eq!(
            migrated["logMaxBytes"],
            serde_json::json!(DEFAULT_LOG_MAX_BYTES)
        );
    }

    #[test]
    fn config_migration_v13_to_current_adds_remote_provider_timeout() {
        let value = serde_json::json!({
            "schemaVersion": 13,
            "refreshIntervalSeconds": 300,
            "displayMode": "remaining",
            "lowQuotaWarningThreshold": 20,
            "launchAtStartup": false,
            "logLevel": "info",
            "logMaxBytes": DEFAULT_LOG_MAX_BYTES,
            "language": "system",
            "networkProxy": null,
            "trayPopupPosition": null,
            "trayPopupSize": null,
            "remoteProviderRegistry": {
                "registryUrl": null,
                "providerProxyUrl": null,
                "autoUpdate": true
            },
            "providers": [
                {
                    "kind": "remote",
                    "id": "remote-kimi",
                    "name": "Remote Kimi",
                    "enabled": true,
                    "manifestUrl": "https://example.com/provider.json",
                    "sourceUrl": "https://example.com/provider.cjs",
                    "runtime": "node",
                    "autoUpdate": true,
                    "updateIntervalSeconds": 1800
                }
            ]
        });

        let migrated = migrate_config_value(value).expect("migrates");

        assert_eq!(
            migrated["schemaVersion"],
            serde_json::json!(CURRENT_CONFIG_SCHEMA_VERSION)
        );
        assert_eq!(
            migrated["providers"][0]["timeoutSeconds"],
            serde_json::json!(DEFAULT_REMOTE_PROVIDER_TIMEOUT_SECONDS)
        );
    }

    #[test]
    fn non_remote_provider_configs_are_rejected() {
        let mock = serde_json::json!({
            "kind": "mock",
            "id": "mock",
            "name": "Mock",
            "enabled": true
        });
        let codex = serde_json::json!({
            "kind": "codex",
            "id": "codex",
            "name": "Codex",
            "enabled": true
        });
        let command = serde_json::json!({
            "kind": "command",
            "id": "command",
            "name": "Command",
            "enabled": true
        });
        let script = serde_json::json!({
            "kind": "script",
            "id": "script",
            "name": "Script",
            "enabled": true
        });

        assert!(serde_json::from_value::<ProviderConfig>(mock).is_err());
        assert!(serde_json::from_value::<ProviderConfig>(codex).is_err());
        assert!(serde_json::from_value::<ProviderConfig>(command).is_err());
        assert!(serde_json::from_value::<ProviderConfig>(script).is_err());
    }

    #[test]
    fn remote_provider_fields_serialize_as_frontend_camel_case() {
        let config = AppConfig {
            schema_version: CURRENT_CONFIG_SCHEMA_VERSION,
            refresh_interval_seconds: 300,
            display_mode: "remaining".to_string(),
            low_quota_warning_threshold: 20.0,
            launch_at_startup: false,
            log_level: "info".to_string(),
            log_max_bytes: DEFAULT_LOG_MAX_BYTES,
            log_quota_data: true,
            language: AppLanguage::System,
            network_proxy: None,
            tray_popup_position: None,
            tray_popup_size: Some(TrayPopupSize {
                width: 420.0,
                height: 640.0,
            }),
            remote_provider_registry: RemoteProviderRegistrySettings {
                registry_url: Some("https://example.com/registry.json".to_string()),
                provider_proxy_url: Some("http://proxy:8080".to_string()),
                auto_update: false,
                sources: Vec::new(),
            },
            providers: vec![ProviderConfig::Remote {
                id: "provider".to_string(),
                name: "Provider".to_string(),
                enabled: true,
                version: Some("1.0.0".to_string()),
                manifest_url: "https://example.com/provider.json".to_string(),
                source_url: "https://example.com/provider.cjs".to_string(),
                provider_dir: None,
                runtime: "node".to_string(),
                resolved_runtime: None,
                proxy_url: None,
                auto_update: true,
                update_interval_seconds: 3600,
                timeout_seconds: DEFAULT_REMOTE_PROVIDER_TIMEOUT_SECONDS,
                trusted_checksum: None,
                installed_at: Some("2026-06-18T00:00:00Z".to_string()),
                updated_at: Some("2026-06-18T00:00:00Z".to_string()),
                last_checked_at: Some("2026-06-18T00:00:00Z".to_string()),
                window_label_overrides: HashMap::from([(
                    "300-minute".to_string(),
                    "5h".to_string(),
                )]),
                visible_window_ids: vec!["5h".to_string()],
                env_vars: HashMap::from([(
                    "KIMI_API_KEY".to_string(),
                    "${secret:KIMI_API_KEY}".to_string(),
                )]),
            }],
        };

        let value = serde_json::to_value(config).expect("serialize config");

        assert_eq!(
            value["remoteProviderRegistry"],
            serde_json::json!({
                "registryUrl": "https://example.com/registry.json",
                "providerProxyUrl": "http://proxy:8080",
                "autoUpdate": false
            })
        );
        assert_eq!(
            value["trayPopupSize"],
            serde_json::json!({
                "width": 420.0,
                "height": 640.0
            })
        );
        assert_eq!(
            value["logMaxBytes"],
            serde_json::json!(DEFAULT_LOG_MAX_BYTES)
        );
        assert_eq!(value["logQuotaData"], serde_json::json!(true));
        assert_eq!(
            value["providers"][0]["windowLabelOverrides"],
            serde_json::json!({ "300-minute": "5h" })
        );
        assert_eq!(
            value["providers"][0]["visibleWindowIds"],
            serde_json::json!(["5h"])
        );
        assert_eq!(
            value["providers"][0]["envVars"],
            serde_json::json!({ "KIMI_API_KEY": "${secret:KIMI_API_KEY}" })
        );
        assert_eq!(
            value["providers"][0]["timeoutSeconds"],
            serde_json::json!(DEFAULT_REMOTE_PROVIDER_TIMEOUT_SECONDS)
        );
        assert_eq!(value["providers"][0]["version"], serde_json::json!("1.0.0"));
        assert_eq!(
            value["providers"][0]["installedAt"],
            serde_json::json!("2026-06-18T00:00:00Z")
        );
        assert!(value["providers"][0]
            .get("window_label_overrides")
            .is_none());
        assert!(value["providers"][0].get("visible_window_ids").is_none());
    }

    #[test]
    fn remote_provider_fields_deserialize_from_frontend_camel_case() {
        let value = serde_json::json!({
            "kind": "remote",
            "id": "provider",
            "name": "Provider",
            "enabled": true,
            "version": "1.0.0",
            "manifestUrl": "https://example.com/provider.json",
            "sourceUrl": "https://example.com/provider.cjs",
            "runtime": "node",
            "autoUpdate": true,
            "updateIntervalSeconds": 3600,
            "timeoutSeconds": 30,
            "installedAt": "2026-06-18T00:00:00Z",
            "updatedAt": "2026-06-18T00:00:00Z",
            "lastCheckedAt": "2026-06-18T00:00:00Z",
            "windowLabelOverrides": { "300-minute": "5h" },
            "visibleWindowIds": ["5h"],
            "envVars": { "KIMI_API_KEY": "${secret:KIMI_API_KEY}" }
        });

        let provider =
            serde_json::from_value::<ProviderConfig>(value).expect("deserialize provider");

        let ProviderConfig::Remote {
            window_label_overrides,
            visible_window_ids,
            env_vars,
            version,
            installed_at,
            timeout_seconds,
            ..
        } = provider;
        assert_eq!(version, Some("1.0.0".to_string()));
        assert_eq!(installed_at, Some("2026-06-18T00:00:00Z".to_string()));
        assert_eq!(timeout_seconds, DEFAULT_REMOTE_PROVIDER_TIMEOUT_SECONDS);
        assert_eq!(
            window_label_overrides.get("300-minute"),
            Some(&"5h".to_string())
        );
        assert_eq!(visible_window_ids, vec!["5h".to_string()]);
        assert_eq!(
            env_vars.get("KIMI_API_KEY"),
            Some(&"${secret:KIMI_API_KEY}".to_string())
        );
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

    #[test]
    fn write_remote_provider_guide_creates_html_file() {
        let temp = tempfile::tempdir().expect("temp dir");
        let guide_path = temp.path().join("remote-provider-guide.html");
        write_remote_provider_guide(&guide_path).expect("write guide");

        assert!(guide_path.exists());
        let contents = fs::read_to_string(&guide_path).expect("read guide");
        assert!(contents.contains("远程 Provider 指南"));
        assert!(contents.contains("Remote Provider Guide"));
        assert!(contents.contains("Manifest 格式"));
        assert!(contents.contains("Manifest format"));
        assert!(contents.contains("Implementation examples (Zhipu/BigModel)"));
        assert!(contents.contains("实现示例：Zhipu / BigModel"));
        assert!(contents.contains("Node.js"));
        assert!(contents.contains("Python"));
        assert!(contents.contains("PowerShell"));
        assert!(contents.contains("Bash"));
        assert!(contents.contains("display = section.hidden ? \"none\" : \"block\""));
    }

    #[test]
    fn secret_placeholder_prefers_local_secret_file_over_env() {
        let temp = tempfile::tempdir().expect("temp dir");
        let secret_dir = temp.path().join("secrets");
        fs::create_dir(&secret_dir).expect("create secrets dir");
        fs::write(secret_dir.join("QBWIN_TEST_PRECEDENCE.txt"), "from-file\n")
            .expect("write secret");
        std::env::set_var("QBWIN_TEST_PRECEDENCE", "from-env");

        let actual = resolve_secret_value("${secret:QBWIN_TEST_PRECEDENCE}", temp.path())
            .expect("resolve secret");

        assert_eq!(actual, "from-file");
        std::env::remove_var("QBWIN_TEST_PRECEDENCE");
    }

    #[test]
    fn secret_placeholder_falls_back_to_env() {
        let temp = tempfile::tempdir().expect("temp dir");
        std::env::set_var("QBWIN_TEST_ENV_FALLBACK", "from-env");

        let actual = resolve_secret_value("${secret:QBWIN_TEST_ENV_FALLBACK}", temp.path())
            .expect("resolve secret");

        assert_eq!(actual, "from-env");
        std::env::remove_var("QBWIN_TEST_ENV_FALLBACK");
    }

    #[test]
    fn missing_secret_error_does_not_include_secret_values() {
        let temp = tempfile::tempdir().expect("temp dir");
        std::env::remove_var("QBWIN_TEST_MISSING_SECRET");

        let error = resolve_secret_value("${secret:QBWIN_TEST_MISSING_SECRET}", temp.path())
            .expect_err("missing secret");

        assert!(error.contains("QBWIN_TEST_MISSING_SECRET"));
        assert!(!error.contains("super-secret-token"));
    }

    #[test]
    fn file_placeholder_still_accepts_quoted_paths_with_spaces() {
        let temp = tempfile::tempdir().expect("temp dir");
        let secret_dir = temp.path().join("codex secrets");
        fs::create_dir(&secret_dir).expect("create secret dir");
        let secret_path = secret_dir.join("access token.txt");
        fs::write(&secret_path, "token-from-file\n").expect("write secret");

        let actual = resolve_secret_value(
            &format!("${{file:\"{}\"}}", secret_path.display()),
            temp.path(),
        )
        .expect("resolve file placeholder");

        assert_eq!(actual, "token-from-file");
    }
}
