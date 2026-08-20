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

use crate::{
    logger::{LogLevel, LogSink},
    proxy::ProxyConfig,
};

pub const CURRENT_CONFIG_SCHEMA_VERSION: u8 = 20;
pub const DEFAULT_LOG_MAX_BYTES: u64 = 10 * 1024 * 1024;
pub const DEFAULT_REMOTE_PROVIDER_TIMEOUT_SECONDS: u64 = 30;
pub const DEFAULT_LOCAL_API_PORT: u16 = 41833;
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
    // Legacy data is retained only so existing config files continue to deserialize. Tray quick
    // views now always anchor to the current tray click and never read or write this value.
    #[serde(default)]
    pub tray_popup_position: Option<TrayPopupPosition>,
    #[serde(default)]
    pub tray_popup_size: Option<TrayPopupSize>,
    #[serde(default = "default_app_update_settings")]
    pub app_update: AppUpdateSettings,
    #[serde(default)]
    pub local_api: LocalApiSettings,
    #[serde(default)]
    pub remote_provider_registry: RemoteProviderRegistrySettings,
    pub providers: Vec<ProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateSettings {
    #[serde(default = "default_app_update_auto_check")]
    pub auto_check: bool,
}

impl Default for AppUpdateSettings {
    fn default() -> Self {
        default_app_update_settings()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LocalApiSettings {
    #[serde(default = "default_local_api_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub bind_target: LocalApiBindTarget,
    #[serde(default = "default_local_api_port")]
    pub port: u16,
}

impl Default for LocalApiSettings {
    fn default() -> Self {
        default_local_api_settings()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum LocalApiBindTarget {
    Loopback,
    // Kept for configurations written by v1.3.0 before multi-interface listening
    // was added. New settings use `NetworkInterfaces` instead.
    NetworkInterface {
        adapter_id: String,
        #[serde(default = "default_local_api_include_loopback")]
        include_loopback: bool,
    },
    NetworkInterfaces {
        adapter_ids: Vec<String>,
        #[serde(default = "default_local_api_include_loopback")]
        include_loopback: bool,
    },
    AllNetworkInterfaces {
        #[serde(default = "default_local_api_include_loopback")]
        include_loopback: bool,
    },
}

impl Default for LocalApiBindTarget {
    fn default() -> Self {
        Self::Loopback
    }
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProviderSetupState {
    Pending,
    Unverified,
    Ready,
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
        #[serde(
            default = "default_show_in_tray",
            rename = "showInTray",
            alias = "show_in_tray",
            alias = "show-in-tray"
        )]
        show_in_tray: bool,
        #[serde(default, rename = "envVars", alias = "env_vars", alias = "env-vars")]
        env_vars: HashMap<String, String>,
        #[serde(
            default = "default_provider_setup_state",
            rename = "setupState",
            alias = "setup_state",
            alias = "setup-state"
        )]
        setup_state: ProviderSetupState,
        #[serde(
            default,
            rename = "setupLastTestedAt",
            alias = "setup_last_tested_at",
            alias = "setup-last-tested-at",
            skip_serializing_if = "Option::is_none"
        )]
        setup_last_tested_at: Option<String>,
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

fn default_app_update_auto_check() -> bool {
    true
}

fn default_app_update_settings() -> AppUpdateSettings {
    AppUpdateSettings {
        auto_check: default_app_update_auto_check(),
    }
}

fn default_local_api_enabled() -> bool {
    false
}

fn default_local_api_include_loopback() -> bool {
    true
}

fn default_local_api_port() -> u16 {
    DEFAULT_LOCAL_API_PORT
}

fn default_local_api_settings() -> LocalApiSettings {
    LocalApiSettings {
        enabled: default_local_api_enabled(),
        bind_target: LocalApiBindTarget::Loopback,
        port: default_local_api_port(),
    }
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

fn default_show_in_tray() -> bool {
    true
}

fn default_provider_setup_state() -> ProviderSetupState {
    // Configs created before the setup flow existed have already been in use;
    // keep them running without an unexpected migration prompt.
    ProviderSetupState::Ready
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
        app_update: AppUpdateSettings::default(),
        local_api: LocalApiSettings::default(),
        remote_provider_registry: RemoteProviderRegistrySettings::default(),
        providers: Vec::new(),
    }
}

fn app_data_config_path_for_file_name(file_name: &str) -> Result<PathBuf, String> {
    let app_data = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| "APPDATA is not set".to_string())?;
    Ok(app_data.join("QuotaBarWin").join(file_name))
}

fn app_data_config_path_for_app(app: &AppHandle) -> Result<PathBuf, String> {
    if cfg!(windows) {
        app_data_config_path_for_file_name(CONFIG_FILE_NAME)
    } else {
        app.path()
            .app_config_dir()
            .map(|dir| dir.join(CONFIG_FILE_NAME))
            .map_err(|error| error.to_string())
    }
}

fn legacy_app_data_config_path_for_app(app: &AppHandle) -> Result<PathBuf, String> {
    if cfg!(windows) {
        app_data_config_path_for_file_name(LEGACY_CONFIG_FILE_NAME)
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

/// Resolves the same portable-or-AppData config location as the desktop app.
///
/// The CLI lives beside `QuotaBarWin.exe` in the portable release, so checking
/// the supplied executable's directory keeps both binaries on the same config,
/// secrets, provider cache, and snapshot cache.
pub fn config_path_for_executable(executable: &Path) -> Result<PathBuf, String> {
    let exe_dir = executable
        .parent()
        .ok_or_else(|| "Unable to resolve executable directory".to_string())?;
    let app_data_path = app_data_config_path_for_file_name(CONFIG_FILE_NAME)?;
    let legacy_app_data_path = app_data_config_path_for_file_name(LEGACY_CONFIG_FILE_NAME)?;
    let portable_path = portable_config_path_for_exe_dir(exe_dir);
    let legacy_portable_path = legacy_portable_config_path_for_exe_dir(exe_dir);
    let marker_path = portable_marker_path_for_exe_dir(exe_dir);
    let preferred_path =
        config_path_from_candidates(app_data_path, portable_path, marker_path.clone());
    let legacy_path =
        config_path_from_candidates(legacy_app_data_path, legacy_portable_path, marker_path);
    migrate_legacy_config_path(&preferred_path, &legacy_path)?;
    Ok(preferred_path)
}

pub fn config_path_for_current_executable() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    config_path_for_executable(&executable)
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
    let loaded = match load_config_contents_with_migration(path, &contents) {
        Ok(loaded) => loaded,
        Err(error) => {
            let timestamp = Utc::now().format("%Y%m%d%H%M%S");
            let backup_path = path.with_file_name(format!("config.corrupt.{timestamp}.json"));
            fs::rename(path, &backup_path).map_err(|rename_error| rename_error.to_string())?;
            let config = default_config();
            save_config_to_path(path, &config)?;
            LoadedConfig {
                config,
                recovery_messages: vec![format!(
                    "config recovery kind={} action=reset-to-default sourceSchemaVersion={} targetSchemaVersion={} configFile={} backupKind=corrupt backupTimestamp={} error={}",
                    config_recovery_kind(&error),
                    schema_version_from_config_contents(&contents)
                        .map(|version| version.to_string())
                        .unwrap_or_else(|| "unknown".to_string()),
                    CURRENT_CONFIG_SCHEMA_VERSION,
                    config_file_name(path),
                    timestamp,
                    error
                )],
            }
        }
    };

    log_config_recovery_messages(path, &loaded);
    Ok(loaded)
}

fn config_file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_string()
}

fn schema_version_from_config_contents(contents: &str) -> Option<u64> {
    serde_json::from_str::<serde_json::Value>(contents)
        .ok()
        .and_then(|value| {
            value
                .get("schemaVersion")
                .and_then(serde_json::Value::as_u64)
        })
}

fn config_recovery_kind(error: &str) -> &'static str {
    if error.starts_with("Unsupported config schemaVersion") {
        "unsupported-schema"
    } else if error.contains(" at line ") && error.contains(" column ") {
        "invalid-json"
    } else {
        "invalid-config"
    }
}

fn log_config_recovery_messages(path: &Path, loaded: &LoadedConfig) {
    if loaded.recovery_messages.is_empty() {
        return;
    }

    let log = LogSink::from_config_path(path, &loaded.config);
    for message in &loaded.recovery_messages {
        let _ = log.write_unfiltered(LogLevel::Warn, "config", message);
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
                "config migration sourceSchemaVersion={original_version} targetSchemaVersion={CURRENT_CONFIG_SCHEMA_VERSION} configFile={} backupKind=pre-migration backupCreated=true",
                config_file_name(path)
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

    let version = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(14);
    if version < 15 {
        if let Some(providers) = value
            .get_mut("providers")
            .and_then(serde_json::Value::as_array_mut)
        {
            for provider in providers {
                if provider.get("kind").and_then(serde_json::Value::as_str) == Some("remote")
                    && provider.get("showInTray").is_none()
                {
                    provider["showInTray"] = serde_json::Value::Bool(true);
                }
            }
        }
        value["schemaVersion"] = serde_json::json!(15);
    }

    let version = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(15);
    if version < 16 {
        if let Some(providers) = value
            .get_mut("providers")
            .and_then(serde_json::Value::as_array_mut)
        {
            for provider in providers {
                if provider.get("kind").and_then(serde_json::Value::as_str) == Some("remote") {
                    provider
                        .as_object_mut()
                        .expect("remote provider config must be an object")
                        .remove("proxyUrl");
                }
            }
        }
        value["schemaVersion"] = serde_json::json!(16);
    }

    let version = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(16);
    if version < 17 {
        if let Some(providers) = value
            .get_mut("providers")
            .and_then(serde_json::Value::as_array_mut)
        {
            for provider in providers {
                if provider.get("kind").and_then(serde_json::Value::as_str) == Some("remote")
                    && provider.get("setupState").is_none()
                {
                    provider["setupState"] = serde_json::json!("ready");
                }
            }
        }
        value["schemaVersion"] = serde_json::json!(17);
    }

    let version = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(17);
    if version < 18 {
        // Schema 18 introduced this preference. Schema 19 below applies its current default.
        value["schemaVersion"] = serde_json::json!(18);
    }

    let version = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(18);
    if version < 19 {
        // The automatic app-update check is enabled by default. Schema 18 had written false for
        // all migrated configurations, so promote those installations to the current default.
        value["appUpdate"] = serde_json::json!({ "autoCheck": true });
        value["schemaVersion"] = serde_json::json!(19);
    }

    let version = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(19);
    if version < 20 {
        value["localApi"] = serde_json::json!({
            "enabled": false,
            "bindTarget": { "kind": "loopback" },
            "port": DEFAULT_LOCAL_API_PORT
        });
        value["schemaVersion"] = serde_json::json!(20);
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
    validate_local_api_settings(&config.local_api)?;
    if local_api_requires_access_token(&config.local_api)
        && crate::local_api_token::read_token(path)?.is_none()
    {
        return Err(
            "Set and save a local integration API access token before enabling network listeners"
                .to_string(),
        );
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let contents = serde_json::to_string_pretty(config).map_err(|error| error.to_string())?;
    fs::write(path, contents).map_err(|error| error.to_string())
}

pub fn local_api_requires_access_token(settings: &LocalApiSettings) -> bool {
    if !settings.enabled {
        return false;
    }
    match &settings.bind_target {
        LocalApiBindTarget::Loopback => false,
        LocalApiBindTarget::NetworkInterface { .. }
        | LocalApiBindTarget::AllNetworkInterfaces { .. } => true,
        LocalApiBindTarget::NetworkInterfaces { adapter_ids, .. } => !adapter_ids.is_empty(),
    }
}

pub fn validate_local_api_settings(settings: &LocalApiSettings) -> Result<(), String> {
    if settings.port == 0 {
        return Err("Local integration API port must be between 1 and 65535".to_string());
    }

    let adapter_ids: &[String] = match &settings.bind_target {
        LocalApiBindTarget::Loopback | LocalApiBindTarget::AllNetworkInterfaces { .. } => &[],
        LocalApiBindTarget::NetworkInterface { adapter_id, .. } => std::slice::from_ref(adapter_id),
        LocalApiBindTarget::NetworkInterfaces {
            adapter_ids,
            include_loopback,
        } => {
            if adapter_ids.is_empty() && !include_loopback {
                return Err(
                    "Select at least one local integration API network interface".to_string(),
                );
            }
            adapter_ids
        }
    };

    let mut seen = std::collections::HashSet::new();
    for adapter_id in adapter_ids {
        if adapter_id.trim().is_empty()
            || adapter_id.len() > 256
            || adapter_id.contains(['\r', '\n'])
            || !seen.insert(adapter_id)
        {
            return Err("Local integration API network interface is invalid".to_string());
        }
    }

    Ok(())
}

#[derive(Debug)]
struct ProviderCacheMove {
    source: PathBuf,
    destination: PathBuf,
}

#[derive(Debug, Default)]
struct ProviderCacheMigration {
    moves: Vec<ProviderCacheMove>,
    config_changed: bool,
}

impl ProviderCacheMigration {
    fn rollback(&self) -> Result<(), String> {
        let mut errors = Vec::new();
        for moved in self.moves.iter().rev() {
            if let Err(error) = move_provider_cache_dir(&moved.destination, &moved.source) {
                errors.push(format!(
                    "{} -> {}: {error}",
                    moved.destination.display(),
                    moved.source.display()
                ));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }
}

fn copy_provider_cache_dir(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir(destination).map_err(|error| error.to_string())?;
    for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_dir() {
            copy_provider_cache_dir(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn move_provider_cache_dir(source: &Path, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        return Err(format!(
            "destination already exists: {}",
            destination.display()
        ));
    }
    if !source.is_dir() {
        return Err(format!(
            "provider cache is not a directory: {}",
            source.display()
        ));
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    if fs::rename(source, destination).is_ok() {
        return Ok(());
    }

    if let Err(error) = copy_provider_cache_dir(source, destination) {
        let _ = fs::remove_dir_all(destination);
        return Err(error);
    }
    if let Err(error) = fs::remove_dir_all(source) {
        let rollback_error = fs::remove_dir_all(destination).err();
        return Err(match rollback_error {
            Some(rollback_error) => format!(
                "failed to remove source {} after copying it: {error}; failed to remove copied destination {}: {rollback_error}",
                source.display(),
                destination.display()
            ),
            None => format!(
                "failed to remove source {} after copying it: {error}",
                source.display()
            ),
        });
    }
    Ok(())
}

fn migrate_remote_provider_cache_dirs(
    config: &mut AppConfig,
    source_config_path: &Path,
    destination_config_path: &Path,
) -> Result<ProviderCacheMigration, String> {
    let original_config = config.clone();
    let mut migration = ProviderCacheMigration::default();

    for provider in &mut config.providers {
        let ProviderConfig::Remote {
            id, provider_dir, ..
        } = provider;
        let source =
            provider_dir
                .clone()
                .unwrap_or(crate::remote_provider_commands::remote_provider_dir(
                    source_config_path,
                    id,
                )?);
        let destination =
            crate::remote_provider_commands::remote_provider_dir(destination_config_path, id)?;

        if source != destination && source.exists() {
            if let Err(error) = move_provider_cache_dir(&source, &destination) {
                let rollback_error = migration.rollback().err();
                *config = original_config;
                return Err(match rollback_error {
                    Some(rollback_error) => format!(
                        "failed to move provider cache {} to {}: {error}; rollback failed: {rollback_error}",
                        source.display(),
                        destination.display()
                    ),
                    None => format!(
                        "failed to move provider cache {} to {}: {error}",
                        source.display(),
                        destination.display()
                    ),
                });
            }
            migration.moves.push(ProviderCacheMove {
                source: source.clone(),
                destination: destination.clone(),
            });
        }

        if provider_dir.as_ref() != Some(&destination) {
            *provider_dir = Some(destination);
            migration.config_changed = true;
        }
    }

    Ok(migration)
}

fn restore_file(path: &Path, original: Option<&[u8]>) -> Result<(), String> {
    match original {
        Some(contents) => fs::write(path, contents).map_err(|error| error.to_string()),
        None if path.exists() => fs::remove_file(path).map_err(|error| error.to_string()),
        None => Ok(()),
    }
}

pub fn repair_remote_provider_cache_paths(
    config_path: &Path,
    config: &mut AppConfig,
) -> Result<bool, String> {
    let original_config = config.clone();
    let migration = migrate_remote_provider_cache_dirs(config, config_path, config_path)?;
    if !migration.config_changed {
        return Ok(false);
    }
    if let Err(error) = save_config_to_path(config_path, config) {
        let rollback_error = migration.rollback().err();
        *config = original_config;
        return Err(match rollback_error {
            Some(rollback_error) => format!(
                "failed to persist repaired provider cache paths: {error}; rollback failed: {rollback_error}"
            ),
            None => format!("failed to persist repaired provider cache paths: {error}"),
        });
    }
    Ok(true)
}

pub fn load_tray_popup_size_for_app(app: &AppHandle) -> Option<TrayPopupSize> {
    let path = config_path_for_app(app).ok()?;
    load_or_create_config(&path).ok()?.config.tray_popup_size
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
    if let Some(managed_path) =
        crate::managed_secret_store::managed_secret_path_from_reference(config_dir, name)
    {
        let secret_path = managed_path?;
        return match fs::read_to_string(&secret_path) {
            Ok(secret) => {
                let secret = secret.trim().to_string();
                if secret.is_empty() {
                    Err(format!("Missing managed Provider secret {name}"))
                } else {
                    Ok(secret)
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Err(format!("Missing managed Provider secret {name}"))
            }
            Err(error) => Err(format!(
                "Unable to read managed Provider secret {name}: {error}"
            )),
        };
    }
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
    crate::local_api::reconfigure(&app);
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
        let mut config = load_or_create_config(&current_path)?.config;
        let destination_path = if enabled {
            &portable_path
        } else {
            &app_data_path
        };
        let destination_original = fs::read(destination_path).ok();
        let destination_token_path =
            crate::local_api_token::token_path_for_config_path(destination_path);
        let destination_token_original = fs::read(&destination_token_path).ok();
        let source_token = crate::local_api_token::read_token(&current_path)?;
        let migration =
            migrate_remote_provider_cache_dirs(&mut config, &current_path, destination_path)?;

        let token_result = match source_token {
            Some(token) => crate::local_api_token::write_token(destination_path, &token),
            None => crate::local_api_token::delete_token(destination_path),
        };
        if let Err(error) = token_result {
            let restore_token_error = restore_file(
                &destination_token_path,
                destination_token_original.as_deref(),
            )
            .err();
            let rollback_error = migration.rollback().err();
            return Err(format!(
                "failed to move local integration API token: {error}{}{}",
                restore_token_error
                    .as_ref()
                    .map(|value| format!(
                        "; failed to restore local integration API token: {value}"
                    ))
                    .unwrap_or_default(),
                rollback_error
                    .as_ref()
                    .map(|value| format!("; provider cache rollback failed: {value}"))
                    .unwrap_or_default()
            ));
        }

        if let Err(error) = save_config_to_path(destination_path, &config) {
            let restore_error =
                restore_file(destination_path, destination_original.as_deref()).err();
            let restore_token_error = restore_file(
                &destination_token_path,
                destination_token_original.as_deref(),
            )
            .err();
            let rollback_error = migration.rollback().err();
            return Err(format!(
                "failed to save config while switching storage mode: {error}{}{}{}",
                restore_error
                    .as_ref()
                    .map(|value| format!("; failed to restore destination config: {value}"))
                    .unwrap_or_default(),
                restore_token_error
                    .as_ref()
                    .map(|value| format!(
                        "; failed to restore local integration API token: {value}"
                    ))
                    .unwrap_or_default(),
                rollback_error
                    .as_ref()
                    .map(|value| format!("; provider cache rollback failed: {value}"))
                    .unwrap_or_default()
            ));
        }

        let marker_result = if enabled {
            fs::write(&marker_path, "QuotaBarWin portable mode\n")
                .map_err(|error| error.to_string())
        } else if marker_path.exists() {
            fs::remove_file(&marker_path).map_err(|error| error.to_string())
        } else {
            Ok(())
        };
        if let Err(error) = marker_result {
            let restore_error =
                restore_file(destination_path, destination_original.as_deref()).err();
            let restore_token_error = restore_file(
                &destination_token_path,
                destination_token_original.as_deref(),
            )
            .err();
            let rollback_error = migration.rollback().err();
            return Err(format!(
                "failed to switch storage mode marker: {error}{}{}{}",
                restore_error
                    .as_ref()
                    .map(|value| format!("; failed to restore destination config: {value}"))
                    .unwrap_or_default(),
                restore_token_error
                    .as_ref()
                    .map(|value| format!(
                        "; failed to restore local integration API token: {value}"
                    ))
                    .unwrap_or_default(),
                rollback_error
                    .as_ref()
                    .map(|value| format!("; provider cache rollback failed: {value}"))
                    .unwrap_or_default()
            ));
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|error| error.to_string())??;

    crate::tray::refresh_tray_menu(&app)?;
    crate::local_api::reconfigure(&app);
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
    crate::local_api::reconfigure(&app);
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
            show_in_tray: true,
            env_vars: HashMap::new(),
            setup_state: ProviderSetupState::Ready,
            setup_last_tested_at: None,
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
            app_update: AppUpdateSettings { auto_check: true },
            local_api: LocalApiSettings::default(),
            remote_provider_registry: RemoteProviderRegistrySettings::default(),
            providers: vec![remote_provider_config("remote")],
        };

        save_config_to_path(&path, &config).expect("save config");
        let loaded = load_or_create_config(&path).expect("load config");

        assert_eq!(loaded.config, config);
        assert!(loaded.recovery_messages.is_empty());
    }

    #[test]
    fn network_local_api_requires_a_saved_token_before_config_can_be_saved() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        let mut config = default_config();
        config.local_api = LocalApiSettings {
            enabled: true,
            bind_target: LocalApiBindTarget::AllNetworkInterfaces {
                include_loopback: true,
            },
            port: DEFAULT_LOCAL_API_PORT,
        };

        assert!(save_config_to_path(&path, &config).is_err());
        crate::local_api_token::write_token(&path, &"x".repeat(32)).expect("write token");
        save_config_to_path(&path, &config).expect("save config with token");
    }

    #[test]
    fn provider_cache_migration_moves_cache_and_can_roll_back() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source_config_path = temp.path().join("app-data").join("config.json");
        let destination_config_path = temp.path().join("portable").join("config.json");
        let source_cache =
            crate::remote_provider_commands::remote_provider_dir(&source_config_path, "provider-a")
                .expect("source cache path");
        fs::create_dir_all(&source_cache).expect("create source cache");
        fs::write(source_cache.join("provider.json"), "source cache").expect("write cache");

        let mut config = default_config();
        let mut provider = remote_provider_config("provider-a");
        let ProviderConfig::Remote { provider_dir, .. } = &mut provider;
        *provider_dir = Some(source_cache.clone());
        config.providers.push(provider);

        let migration = migrate_remote_provider_cache_dirs(
            &mut config,
            &source_config_path,
            &destination_config_path,
        )
        .expect("migrate cache");
        let destination_cache = crate::remote_provider_commands::remote_provider_dir(
            &destination_config_path,
            "provider-a",
        )
        .expect("destination cache path");

        assert!(!source_cache.exists());
        assert_eq!(
            fs::read_to_string(destination_cache.join("provider.json")).expect("read cache"),
            "source cache"
        );
        assert!(migration.config_changed);

        migration.rollback().expect("roll back cache migration");
        assert!(source_cache.exists());
        assert!(!destination_cache.exists());
    }

    #[test]
    fn provider_cache_migration_rolls_back_earlier_moves_when_later_cache_collides() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source_config_path = temp.path().join("app-data").join("config.json");
        let destination_config_path = temp.path().join("portable").join("config.json");
        let source_first = crate::remote_provider_commands::remote_provider_dir(
            &source_config_path,
            "provider-first",
        )
        .expect("first source cache");
        let source_second = crate::remote_provider_commands::remote_provider_dir(
            &source_config_path,
            "provider-second",
        )
        .expect("second source cache");
        let destination_second = crate::remote_provider_commands::remote_provider_dir(
            &destination_config_path,
            "provider-second",
        )
        .expect("second destination cache");
        for path in [&source_first, &source_second, &destination_second] {
            fs::create_dir_all(path).expect("create cache directory");
        }

        let mut config = default_config();
        for (id, cache) in [
            ("provider-first", source_first.clone()),
            ("provider-second", source_second.clone()),
        ] {
            let mut provider = remote_provider_config(id);
            let ProviderConfig::Remote { provider_dir, .. } = &mut provider;
            *provider_dir = Some(cache);
            config.providers.push(provider);
        }
        let original_config = config.clone();

        let error = migrate_remote_provider_cache_dirs(
            &mut config,
            &source_config_path,
            &destination_config_path,
        )
        .expect_err("collision rejects migration");

        assert!(error.contains("destination already exists"));
        assert!(source_first.exists());
        assert!(source_second.exists());
        assert_eq!(config, original_config);
    }

    #[test]
    fn repair_moves_legacy_provider_cache_to_active_config_directory() {
        let temp = tempfile::tempdir().expect("temp dir");
        let legacy_config_path = temp.path().join("app-data").join("config.json");
        let portable_config_path = temp.path().join("portable").join("config.json");
        let legacy_cache =
            crate::remote_provider_commands::remote_provider_dir(&legacy_config_path, "provider-a")
                .expect("legacy cache path");
        fs::create_dir_all(&legacy_cache).expect("create legacy cache");
        fs::write(legacy_cache.join("provider.json"), "legacy cache").expect("write cache");

        let mut config = default_config();
        let mut provider = remote_provider_config("provider-a");
        let ProviderConfig::Remote { provider_dir, .. } = &mut provider;
        *provider_dir = Some(legacy_cache.clone());
        config.providers.push(provider);
        save_config_to_path(&portable_config_path, &config).expect("save portable config");

        assert!(
            repair_remote_provider_cache_paths(&portable_config_path, &mut config)
                .expect("repair cache paths")
        );
        let portable_cache = crate::remote_provider_commands::remote_provider_dir(
            &portable_config_path,
            "provider-a",
        )
        .expect("portable cache path");
        assert!(!legacy_cache.exists());
        assert!(portable_cache.join("provider.json").exists());
        let loaded = load_or_create_config(&portable_config_path).expect("load repaired config");
        let ProviderConfig::Remote { provider_dir, .. } = &loaded.config.providers[0];
        assert_eq!(provider_dir.as_ref(), Some(&portable_cache));
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
    fn config_migration_v14_to_current_enables_tray_display_for_remote_providers() {
        let value = serde_json::json!({
            "schemaVersion": 14,
            "refreshIntervalSeconds": 300,
            "displayMode": "remaining",
            "lowQuotaWarningThreshold": 20,
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
                    "updateIntervalSeconds": 1800,
                    "timeoutSeconds": 30
                }
            ]
        });

        let migrated = migrate_config_value(value).expect("migrates");

        assert_eq!(
            migrated["schemaVersion"],
            serde_json::json!(CURRENT_CONFIG_SCHEMA_VERSION)
        );
        assert_eq!(
            migrated["providers"][0]["showInTray"],
            serde_json::json!(true)
        );
    }

    #[test]
    fn config_migration_v15_removes_legacy_provider_runtime_proxy() {
        let value = serde_json::json!({
            "schemaVersion": 15,
            "providers": [{
                "kind": "remote",
                "id": "remote-codex",
                "name": "Codex",
                "enabled": true,
                "manifestUrl": "https://example.com/provider.json",
                "sourceUrl": "https://example.com/provider.cjs",
                "runtime": "node",
                "proxyUrl": "socks://legacy:1080"
            }]
        });

        let migrated = migrate_config_value(value).expect("migrates");

        assert_eq!(
            migrated["schemaVersion"],
            serde_json::json!(CURRENT_CONFIG_SCHEMA_VERSION)
        );
        assert!(migrated["providers"][0].get("proxyUrl").is_none());
    }

    #[test]
    fn config_migration_v16_marks_existing_remote_providers_ready() {
        let value = serde_json::json!({
            "schemaVersion": 16,
            "providers": [{
                "kind": "remote",
                "id": "remote-codex",
                "name": "Codex",
                "enabled": true,
                "manifestUrl": "https://example.com/provider.json",
                "sourceUrl": "https://example.com/provider.js",
                "runtime": "builtin-js"
            }]
        });

        let migrated = migrate_config_value(value).expect("migrates");

        assert_eq!(
            migrated["schemaVersion"],
            serde_json::json!(CURRENT_CONFIG_SCHEMA_VERSION)
        );
        assert_eq!(
            migrated["providers"][0]["setupState"],
            serde_json::json!("ready")
        );
        assert_eq!(migrated["providers"][0]["enabled"], serde_json::json!(true));
    }

    #[test]
    fn config_migration_v17_enables_application_update_checks_by_default() {
        let value = serde_json::json!({
            "schemaVersion": 17,
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
        assert_eq!(migrated["appUpdate"]["autoCheck"], serde_json::json!(true));
        assert!(default_config().app_update.auto_check);
    }

    #[test]
    fn config_migration_v18_enables_application_update_checks_by_default() {
        let value = serde_json::json!({
            "schemaVersion": 18,
            "appUpdate": { "autoCheck": false },
            "providers": []
        });

        let migrated = migrate_config_value(value).expect("migrates");

        assert_eq!(
            migrated["schemaVersion"],
            serde_json::json!(CURRENT_CONFIG_SCHEMA_VERSION)
        );
        assert_eq!(migrated["appUpdate"]["autoCheck"], serde_json::json!(true));
    }

    #[test]
    fn config_migration_v19_adds_loopback_local_api_defaults() {
        let value = serde_json::json!({
            "schemaVersion": 19,
            "providers": []
        });

        let migrated = migrate_config_value(value).expect("migrates");

        assert_eq!(
            migrated["schemaVersion"],
            serde_json::json!(CURRENT_CONFIG_SCHEMA_VERSION)
        );
        assert_eq!(migrated["localApi"]["enabled"], serde_json::json!(false));
        assert_eq!(
            migrated["localApi"]["bindTarget"]["kind"],
            serde_json::json!("loopback")
        );
        assert_eq!(
            migrated["localApi"]["port"],
            serde_json::json!(DEFAULT_LOCAL_API_PORT)
        );
        assert!(!default_config().local_api.enabled);
    }

    #[test]
    fn local_api_accepts_multiple_or_all_network_interface_targets_without_schema_change() {
        let multiple = LocalApiSettings {
            enabled: true,
            bind_target: LocalApiBindTarget::NetworkInterfaces {
                adapter_ids: vec!["ethernet".to_string(), "wifi".to_string()],
                include_loopback: true,
            },
            port: DEFAULT_LOCAL_API_PORT,
        };
        validate_local_api_settings(&multiple).expect("multiple interfaces are valid");
        assert_eq!(
            serde_json::to_value(&multiple).expect("serialize")["bindTarget"]["kind"],
            serde_json::json!("network-interfaces")
        );

        validate_local_api_settings(&LocalApiSettings {
            enabled: true,
            bind_target: LocalApiBindTarget::AllNetworkInterfaces {
                include_loopback: true,
            },
            port: DEFAULT_LOCAL_API_PORT,
        })
        .expect("all interfaces are valid");

        validate_local_api_settings(&LocalApiSettings {
            enabled: true,
            bind_target: LocalApiBindTarget::NetworkInterfaces {
                adapter_ids: Vec::new(),
                include_loopback: true,
            },
            port: DEFAULT_LOCAL_API_PORT,
        })
        .expect("loopback-only selected interface mode is valid");
        assert!(validate_local_api_settings(&LocalApiSettings {
            enabled: true,
            bind_target: LocalApiBindTarget::NetworkInterfaces {
                adapter_ids: Vec::new(),
                include_loopback: false,
            },
            port: DEFAULT_LOCAL_API_PORT,
        })
        .is_err());

        let legacy_target: LocalApiBindTarget = serde_json::from_value(serde_json::json!({
            "kind": "network-interfaces",
            "adapterIds": ["ethernet"]
        }))
        .expect("legacy v1.3.0 target deserializes");
        assert!(matches!(
            legacy_target,
            LocalApiBindTarget::NetworkInterfaces {
                include_loopback: true,
                ..
            }
        ));
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
            app_update: AppUpdateSettings { auto_check: true },
            local_api: LocalApiSettings::default(),
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
                show_in_tray: true,
                env_vars: HashMap::from([(
                    "KIMI_API_KEY".to_string(),
                    "${secret:KIMI_API_KEY}".to_string(),
                )]),
                setup_state: ProviderSetupState::Ready,
                setup_last_tested_at: Some("2026-06-18T00:00:00Z".to_string()),
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
        assert_eq!(value["appUpdate"], serde_json::json!({ "autoCheck": true }));
        assert_eq!(
            value["providers"][0]["windowLabelOverrides"],
            serde_json::json!({ "300-minute": "5h" })
        );
        assert_eq!(
            value["providers"][0]["visibleWindowIds"],
            serde_json::json!(["5h"])
        );
        assert_eq!(value["providers"][0]["showInTray"], serde_json::json!(true));
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
            "showInTray": false,
            "envVars": { "KIMI_API_KEY": "${secret:KIMI_API_KEY}" }
        });

        let provider =
            serde_json::from_value::<ProviderConfig>(value).expect("deserialize provider");

        let ProviderConfig::Remote {
            window_label_overrides,
            visible_window_ids,
            show_in_tray,
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
        assert!(!show_in_tray);
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

        let log_contents = fs::read_to_string(crate::logger::log_path_for_config_path(&path))
            .expect("read migration log");
        assert!(log_contents.contains("\"level\":\"warn\""));
        assert!(log_contents.contains("\"target\":\"config\""));
        assert!(log_contents.contains("config migration sourceSchemaVersion=1"));
        assert!(log_contents.contains(&format!(
            "targetSchemaVersion={CURRENT_CONFIG_SCHEMA_VERSION}"
        )));
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
    fn logs_unsupported_schema_recovery_with_schema_and_backup_context() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        let unsupported_version = CURRENT_CONFIG_SCHEMA_VERSION + 1;
        fs::write(
            &path,
            format!(r#"{{"schemaVersion":{unsupported_version},"providers":[]}}"#),
        )
        .expect("write newer config");

        let loaded = load_or_create_config(&path).expect("config recovers");

        assert_eq!(loaded.config, default_config());
        assert_eq!(loaded.recovery_messages.len(), 1);
        assert!(loaded.recovery_messages[0].contains("kind=unsupported-schema"));
        assert!(loaded.recovery_messages[0]
            .contains(&format!("sourceSchemaVersion={unsupported_version}")));

        let log_contents = fs::read_to_string(crate::logger::log_path_for_config_path(&path))
            .expect("read recovery log");
        assert!(log_contents.contains("\"level\":\"warn\""));
        assert!(log_contents.contains("\"target\":\"config\""));
        assert!(log_contents.contains("kind=unsupported-schema"));
        assert!(log_contents.contains(&format!("sourceSchemaVersion={unsupported_version}")));
        assert!(log_contents.contains(&format!(
            "targetSchemaVersion={CURRENT_CONFIG_SCHEMA_VERSION}"
        )));
        assert!(log_contents.contains("backupKind=corrupt"));
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
        assert!(contents.contains("向导式配置字段"));
        assert!(contents.contains("Guided setup fields"));
        assert!(contents.contains("secrets/providers"));
        assert!(contents.contains("helpUrl"));
        assert!(contents.contains("保存并测试"));
        assert!(contents.contains("Save and test"));
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
    fn managed_provider_secret_placeholder_reads_instance_scoped_file() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = crate::managed_secret_store::ManagedSecretStore::new(temp.path());
        let reference = store
            .write("provider-a", "API_TOKEN", "scoped-secret")
            .expect("write managed secret");

        let actual = resolve_secret_value(&reference.placeholder(), temp.path())
            .expect("resolve managed secret");

        assert_eq!(actual, "scoped-secret");
    }

    #[test]
    fn invalid_managed_secret_reference_cannot_escape_config_directory() {
        let temp = tempfile::tempdir().expect("temp dir");
        let error = resolve_secret_value("${secret:providers/../API_TOKEN}", temp.path())
            .expect_err("invalid managed reference");

        assert!(error.contains("Invalid Provider instance id"));
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
