use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::{
    config::{
        config_path_for_app, load_or_create_config, save_config_to_path, AppConfig, ProviderConfig,
        RemoteProviderRegistrySettings, DEFAULT_REMOTE_PROVIDER_TIMEOUT_SECONDS,
    },
    logger::{LogLevel, LogSink},
    proxy::ProxyConfig,
    remote_provider::{
        cache_remote_provider, check_update, compute_checksum, fetch_manifest, fetch_manifest_text,
        fetch_provider_registry, fetch_source, is_builtin_js_runtime, load_cached_manifest,
        parse_manifest, resolve_provider_url, resolve_runtime, resolve_source_url,
        validate_runtime_executable, ProviderManifest, UpdateInfo,
    },
};

const FETCH_TIMEOUT: Duration = Duration::from_secs(30);
const REMOTE_PROVIDERS_DIR: &str = "providers/remote";

pub(crate) fn remote_provider_dir(config_path: &Path, id: &str) -> Result<PathBuf, String> {
    let config_dir = config_path
        .parent()
        .ok_or_else(|| "Unable to resolve config directory for provider cache".to_string())?;
    Ok(config_dir.join(REMOTE_PROVIDERS_DIR).join(id))
}

fn cached_provider_dir(config_path: &Path, provider: &ProviderConfig) -> Result<PathBuf, String> {
    match provider {
        ProviderConfig::Remote {
            provider_dir: Some(provider_dir),
            ..
        } => Ok(provider_dir.clone()),
        ProviderConfig::Remote { id, .. } => remote_provider_dir(config_path, id),
    }
}

fn provider_id(provider: &ProviderConfig) -> &str {
    match provider {
        ProviderConfig::Remote { id, .. } => id,
    }
}

fn provider_manifest_url(provider: &ProviderConfig) -> &str {
    match provider {
        ProviderConfig::Remote { manifest_url, .. } => manifest_url,
    }
}

fn provider_name(provider: &ProviderConfig) -> &str {
    match provider {
        ProviderConfig::Remote { name, .. } => name,
    }
}

fn provider_matches_manifest(
    provider: &ProviderConfig,
    manifest_id: &str,
    manifest_url: &str,
) -> bool {
    provider_id(provider) == manifest_id || provider_manifest_url(provider) == manifest_url
}

fn installed_provider_instance_count(
    config: &AppConfig,
    manifest_id: &str,
    manifest_url: &str,
) -> usize {
    config
        .providers
        .iter()
        .filter(|provider| provider_matches_manifest(provider, manifest_id, manifest_url))
        .count()
}

fn unique_provider_id(config: &AppConfig, base_id: &str) -> (String, usize) {
    if !config
        .providers
        .iter()
        .any(|provider| provider_id(provider) == base_id)
    {
        return (base_id.to_string(), 1);
    }

    for index in 2.. {
        let candidate = format!("{base_id}-{index}");
        if !config
            .providers
            .iter()
            .any(|provider| provider_id(provider) == candidate)
        {
            return (candidate, index);
        }
    }

    unreachable!("unbounded provider id search should always return")
}

fn unique_provider_name(config: &AppConfig, base_name: &str, preferred_index: usize) -> String {
    let candidate = if preferred_index <= 1 {
        base_name.to_string()
    } else {
        format!("{base_name} {preferred_index}")
    };
    if !config
        .providers
        .iter()
        .any(|provider| provider_name(provider) == candidate)
    {
        return candidate;
    }

    for index in preferred_index.max(2).. {
        let candidate = format!("{base_name} {index}");
        if !config
            .providers
            .iter()
            .any(|provider| provider_name(provider) == candidate)
        {
            return candidate;
        }
    }

    unreachable!("unbounded provider name search should always return")
}

fn log_remote(log: &LogSink, level: LogLevel, message: &str) {
    let _ = log.write(level, "remote_provider_commands", message);
}

fn resolve_manifest_runtime(manifest: &ProviderManifest) -> Result<Option<PathBuf>, String> {
    if is_builtin_js_runtime(&manifest.runtime) {
        return Ok(None);
    }
    resolve_runtime(&manifest.runtime)
        .and_then(|path| {
            validate_runtime_executable(&path)?;
            Ok(path)
        })
        .map(Some)
        .map_err(|error| error.to_string())
}

#[derive(Debug, Clone)]
struct RegistryUpdateSource {
    url: String,
    proxy_url: Option<String>,
}

#[derive(Debug, Clone)]
struct RegistryUpdateCandidate {
    manifest_url: String,
    proxy_url: Option<String>,
    manifest_checksum: Option<String>,
}

fn configured_registry_update_sources(
    settings: &RemoteProviderRegistrySettings,
) -> Vec<RegistryUpdateSource> {
    if !settings.sources.is_empty() {
        return settings
            .sources
            .iter()
            .filter(|source| source.enabled && !source.url.trim().is_empty())
            .map(|source| RegistryUpdateSource {
                url: source.url.trim().to_string(),
                proxy_url: source
                    .provider_proxy_url
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string),
            })
            .collect();
    }

    settings
        .registry_url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .map(|url| RegistryUpdateSource {
            url: url.to_string(),
            proxy_url: settings
                .provider_proxy_url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        })
        .into_iter()
        .collect()
}

fn load_registry_update_candidates(
    settings: &RemoteProviderRegistrySettings,
    global_proxy: Option<&ProxyConfig>,
    log: &LogSink,
) -> Result<HashMap<String, RegistryUpdateCandidate>, String> {
    let mut candidates = HashMap::new();
    for source in configured_registry_update_sources(settings) {
        log_remote(
            log,
            LogLevel::Info,
            &format!(
                "provider update source fetch started registryUrl={}",
                source.url
            ),
        );
        let registry = fetch_provider_registry(
            &source.url,
            source.proxy_url.as_deref(),
            global_proxy,
            FETCH_TIMEOUT,
        )
        .map_err(|error| {
            format!(
                "failed to load Provider update source '{}': {error}",
                source.url
            )
        })?;
        log_remote(
            log,
            LogLevel::Info,
            &format!(
                "provider update source fetched registryUrl={} providers={}",
                source.url,
                registry.providers.len()
            ),
        );
        for entry in registry.providers {
            candidates
                .entry(entry.id)
                .or_insert_with(|| RegistryUpdateCandidate {
                    manifest_url: resolve_provider_url(&source.url, &entry.provider_url),
                    proxy_url: source.proxy_url.clone(),
                    manifest_checksum: entry.checksum,
                });
        }
    }
    Ok(candidates)
}

fn update_is_available(new_checksum: Option<&str>, trusted_checksum: Option<&str>) -> bool {
    match (new_checksum, trusted_checksum) {
        (Some(new), Some(trusted)) => new != trusted,
        (Some(_), None) => true,
        (None, _) => false,
    }
}

fn check_registry_update(
    candidate: &RegistryUpdateCandidate,
    expected_manifest_id: &str,
    current_version: Option<String>,
    trusted_checksum: Option<&str>,
    global_proxy: Option<&ProxyConfig>,
) -> Result<UpdateInfo, String> {
    let manifest_text = fetch_manifest_text(
        &candidate.manifest_url,
        candidate.proxy_url.as_deref(),
        global_proxy,
        FETCH_TIMEOUT,
    )
    .map_err(|error| error.to_string())?;
    if let Some(expected_checksum) = candidate.manifest_checksum.as_deref() {
        crate::remote_provider::verify_checksum(&manifest_text, expected_checksum)
            .map_err(|error| error.to_string())?;
    }
    let manifest = parse_manifest(&manifest_text).map_err(|error| error.to_string())?;
    if manifest.id != expected_manifest_id {
        return Err(format!(
            "update source manifest id '{}' does not match installed Provider id '{}'",
            manifest.id, expected_manifest_id
        ));
    }

    let new_checksum = manifest.checksums.source.clone();
    Ok(UpdateInfo {
        id: expected_manifest_id.to_string(),
        available: update_is_available(new_checksum.as_deref(), trusted_checksum),
        new_checksum,
        update_manifest_url: Some(candidate.manifest_url.clone()),
        current_version,
        new_version: manifest.version,
        checked_at: Some(Utc::now().to_rfc3339()),
    })
}

fn find_provider_config_mut<'a>(
    config: &'a mut AppConfig,
    id: &str,
) -> Option<&'a mut ProviderConfig> {
    config
        .providers
        .iter_mut()
        .find(|provider| provider_id(provider) == id)
}

fn find_provider_config<'a>(config: &'a AppConfig, id: &str) -> Option<&'a ProviderConfig> {
    config
        .providers
        .iter()
        .find(|provider| provider_id(provider) == id)
}

#[tauri::command]
pub async fn get_network_proxy(app: AppHandle) -> Result<Option<ProxyConfig>, String> {
    let path = config_path_for_app(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        load_or_create_config(&path).map(|loaded| loaded.config.network_proxy)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn set_network_proxy(app: AppHandle, proxy: Option<ProxyConfig>) -> Result<(), String> {
    let path = config_path_for_app(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut loaded = load_or_create_config(&path)?;
        loaded.config.network_proxy = proxy;
        save_config_to_path(&path, &loaded.config)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_installed_remote_provider_manifest(
    app: AppHandle,
    id: String,
) -> Result<ProviderManifest, String> {
    let path = config_path_for_app(&app)?;

    tauri::async_runtime::spawn_blocking(move || {
        let loaded = load_or_create_config(&path)?;
        let provider = find_provider_config(&loaded.config, &id)
            .ok_or_else(|| "provider not found".to_string())?;
        let provider_dir = cached_provider_dir(&path, provider)
            .map_err(|e| format!("failed to resolve cache directory: {e}"))?;
        load_cached_manifest(&provider_dir).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

fn install_remote_provider_from_manifest(
    path: &Path,
    url: &str,
    proxy_url: Option<&str>,
    auto_update: bool,
    manifest: ProviderManifest,
    log: &LogSink,
) -> Result<ProviderConfig, String> {
    let mut loaded = load_or_create_config(path)?;
    let global_proxy = loaded.config.network_proxy.clone();
    log_remote(
        log,
        LogLevel::Info,
        &format!(
            "provider install started manifestId={} manifestUrl={} autoUpdateRequested={} proxyConfigured={}",
            manifest.id,
            url,
            auto_update,
            proxy_url.map(|value| !value.trim().is_empty()).unwrap_or(false)
        ),
    );

    let (instance_id, instance_index) = unique_provider_id(&loaded.config, &manifest.id);
    let default_name = manifest
        .default_config
        .name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&manifest.display_name);
    let instance_name = unique_provider_name(&loaded.config, default_name, instance_index);
    log_remote(
        log,
        LogLevel::Info,
        &format!(
            "provider install instance selected manifestId={} providerId={} providerName={}",
            manifest.id, instance_id, instance_name
        ),
    );

    let source_url = resolve_source_url(url, &manifest.entry);
    log_remote(
        log,
        LogLevel::Info,
        &format!(
            "provider source fetch started providerId={} manifestId={} sourceUrl={}",
            instance_id, manifest.id, source_url
        ),
    );
    let source = fetch_source(&source_url, proxy_url, global_proxy.as_ref(), FETCH_TIMEOUT)
        .map_err(|e| e.to_string())?;
    log_remote(
        log,
        LogLevel::Info,
        &format!(
            "provider source fetched providerId={} manifestId={} bytes={}",
            instance_id,
            manifest.id,
            source.len()
        ),
    );

    let actual_auto_update = manifest.checksums.source.is_some() && auto_update;

    if let Some(expected) = manifest.checksums.source.as_ref() {
        crate::remote_provider::verify_checksum(&source, expected).map_err(|e| e.to_string())?;
        log_remote(
            log,
            LogLevel::Info,
            &format!(
                "provider source checksum verified providerId={} manifestId={}",
                instance_id, manifest.id
            ),
        );
    }

    log_remote(
        log,
        LogLevel::Info,
        &format!(
            "provider runtime resolve started providerId={} manifestId={} runtime={}",
            instance_id, manifest.id, manifest.runtime
        ),
    );
    let resolved_runtime_path = resolve_manifest_runtime(&manifest)?;
    match &resolved_runtime_path {
        Some(path) => log_remote(
            log,
            LogLevel::Info,
            &format!(
                "provider runtime resolved providerId={} manifestId={} path={}",
                instance_id,
                manifest.id,
                path.display()
            ),
        ),
        None => log_remote(
            log,
            LogLevel::Info,
            &format!(
                "provider runtime resolved providerId={} manifestId={} runtime=builtin-js embedded=true",
                instance_id, manifest.id
            ),
        ),
    }

    let provider_dir = remote_provider_dir(path, &instance_id)
        .map_err(|e| format!("failed to resolve cache directory: {e}"))?;

    let parent = provider_dir
        .parent()
        .expect("provider dir has parent")
        .to_path_buf();
    cache_remote_provider(
        &parent,
        &instance_id,
        url,
        &manifest,
        &source,
        resolved_runtime_path.as_deref(),
    )
    .map_err(|e| e.to_string())?;
    log_remote(
        log,
        LogLevel::Info,
        &format!(
            "provider cached providerId={} manifestId={} dir={}",
            instance_id,
            manifest.id,
            provider_dir.display()
        ),
    );

    let now = Utc::now().to_rfc3339();
    let config = ProviderConfig::Remote {
        id: instance_id.clone(),
        name: instance_name,
        enabled: false,
        version: manifest.version.clone(),
        manifest_url: url.to_string(),
        source_url,
        provider_dir: Some(provider_dir.clone()),
        runtime: manifest.runtime.clone(),
        resolved_runtime: resolved_runtime_path.map(|path| path.display().to_string()),
        // This proxy is scoped to downloading the registry/manifest/source.
        // Runtime proxy selection belongs to the Provider environment instead.
        proxy_url: None,
        auto_update: actual_auto_update,
        update_interval_seconds: 3600,
        timeout_seconds: manifest
            .default_config
            .timeout_seconds
            .unwrap_or(DEFAULT_REMOTE_PROVIDER_TIMEOUT_SECONDS),
        trusted_checksum: Some(compute_checksum(&source)),
        installed_at: Some(now.clone()),
        updated_at: Some(now.clone()),
        last_checked_at: Some(now),
        window_label_overrides: manifest.default_config.window_label_overrides.clone(),
        visible_window_ids: manifest.default_config.visible_window_ids.clone(),
        show_in_tray: true,
        env_vars: manifest.default_config.env_vars.clone(),
        setup_state: crate::config::ProviderSetupState::Pending,
        setup_last_tested_at: None,
    };

    loaded.config.providers.push(config.clone());
    save_config_to_path(path, &loaded.config)?;
    log_remote(
        log,
        LogLevel::Info,
        &format!(
            "provider install finished providerId={} manifestId={} autoUpdate={}",
            instance_id, manifest.id, actual_auto_update
        ),
    );
    Ok(config)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryInstallFailure {
    pub id: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryInstallResult {
    pub installed: Vec<ProviderConfig>,
    pub skipped: Vec<String>,
    pub failed: Vec<RegistryInstallFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryMigrationResult {
    pub migrated: Vec<String>,
    pub skipped: Vec<String>,
    pub failed: Vec<RegistryInstallFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteProviderCatalogEntry {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub provider_url: String,
    #[serde(default)]
    pub checksum: Option<String>,
    pub installed: bool,
    pub installed_count: usize,
    #[serde(default)]
    pub error: Option<String>,
}

#[tauri::command]
pub async fn preview_remote_provider_registry(
    app: AppHandle,
    url: String,
    proxy_url: Option<String>,
) -> Result<Vec<RemoteProviderCatalogEntry>, String> {
    let path = config_path_for_app(&app)?;
    let proxy_url_ref = proxy_url.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let loaded = load_or_create_config(&path)?;
        let log = LogSink::from_config_path(&path, &loaded.config);
        let global_proxy = loaded.config.network_proxy.clone();
        log_remote(
            &log,
            LogLevel::Info,
            &format!(
                "registry preview started url={} proxyConfigured={}",
                url,
                proxy_url_ref
                    .as_deref()
                    .map(|value| !value.trim().is_empty())
                    .unwrap_or(false)
            ),
        );

        let registry = fetch_provider_registry(
            &url,
            proxy_url_ref.as_deref(),
            global_proxy.as_ref(),
            FETCH_TIMEOUT,
        )
        .map_err(|e| e.to_string())?;

        let mut entries = Vec::new();
        for entry in registry.providers {
            let provider_url = resolve_provider_url(&url, &entry.provider_url);
            let installed_count =
                installed_provider_instance_count(&loaded.config, &entry.id, &provider_url);
            let installed = installed_count > 0;
            let mut catalog_entry = RemoteProviderCatalogEntry {
                id: entry.id.clone(),
                display_name: entry.id.clone(),
                version: None,
                description: None,
                provider_url: provider_url.clone(),
                checksum: entry.checksum.clone(),
                installed,
                installed_count,
                error: None,
            };

            match fetch_manifest_text(
                &provider_url,
                proxy_url_ref.as_deref(),
                global_proxy.as_ref(),
                FETCH_TIMEOUT,
            ) {
                Ok(manifest_text) => {
                    if let Some(expected) = entry.checksum.as_ref() {
                        if let Err(error) =
                            crate::remote_provider::verify_checksum(&manifest_text, expected.trim())
                        {
                            catalog_entry.error = Some(error.to_string());
                            entries.push(catalog_entry);
                            continue;
                        }
                    }

                    match parse_manifest(&manifest_text) {
                        Ok(manifest) => {
                            if manifest.id != entry.id {
                                catalog_entry.error = Some(format!(
                                    "registry id '{}' does not match manifest id '{}'",
                                    entry.id, manifest.id
                                ));
                            }
                            catalog_entry.display_name = manifest.display_name;
                            catalog_entry.version = manifest.version;
                            catalog_entry.description = manifest.description;
                        }
                        Err(error) => {
                            catalog_entry.error = Some(error.to_string());
                        }
                    }
                }
                Err(error) => {
                    catalog_entry.error = Some(error.to_string());
                }
            }

            entries.push(catalog_entry);
        }

        log_remote(
            &log,
            LogLevel::Info,
            &format!("registry preview finished entries={}", entries.len()),
        );
        Ok(entries)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn install_remote_provider_manifest(
    app: AppHandle,
    url: String,
    checksum: Option<String>,
    proxy_url: Option<String>,
    auto_update: bool,
) -> Result<ProviderConfig, String> {
    let path = config_path_for_app(&app)?;
    let proxy_url_ref = proxy_url.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let loaded = load_or_create_config(&path)?;
        let log = LogSink::from_config_path(&path, &loaded.config);
        let global_proxy = loaded.config.network_proxy.clone();
        log_remote(
            &log,
            LogLevel::Info,
            &format!(
                "provider manifest install started url={} autoUpdate={} proxyConfigured={}",
                url,
                auto_update,
                proxy_url_ref
                    .as_deref()
                    .map(|value| !value.trim().is_empty())
                    .unwrap_or(false)
            ),
        );

        let manifest_text = fetch_manifest_text(
            &url,
            proxy_url_ref.as_deref(),
            global_proxy.as_ref(),
            FETCH_TIMEOUT,
        )
        .map_err(|e| e.to_string())?;

        if let Some(expected) = checksum.as_ref().filter(|value| !value.trim().is_empty()) {
            crate::remote_provider::verify_checksum(&manifest_text, expected.trim())
                .map_err(|e| e.to_string())?;
        }

        let manifest = parse_manifest(&manifest_text).map_err(|e| e.to_string())?;
        install_remote_provider_from_manifest(
            &path,
            &url,
            proxy_url_ref.as_deref(),
            auto_update,
            manifest,
            &log,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn install_remote_provider_registry(
    app: AppHandle,
    url: String,
    proxy_url: Option<String>,
    auto_update: bool,
) -> Result<RegistryInstallResult, String> {
    let path = config_path_for_app(&app)?;
    let proxy_url_ref = proxy_url.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let loaded = load_or_create_config(&path)?;
        let log = LogSink::from_config_path(&path, &loaded.config);
        let global_proxy = loaded.config.network_proxy.clone();
        log_remote(
            &log,
            LogLevel::Info,
            &format!(
                "registry install started url={} autoUpdate={} proxyConfigured={}",
                url,
                auto_update,
                proxy_url_ref
                    .as_deref()
                    .map(|value| !value.trim().is_empty())
                    .unwrap_or(false)
            ),
        );
        let registry = fetch_provider_registry(
            &url,
            proxy_url_ref.as_deref(),
            global_proxy.as_ref(),
            FETCH_TIMEOUT,
        )
        .map_err(|e| e.to_string())?;
        log_remote(
            &log,
            LogLevel::Info,
            &format!(
                "registry fetched url={} providers={}",
                url,
                registry.providers.len()
            ),
        );

        let mut persisted = load_or_create_config(&path)?;
        let existing_sources = persisted.config.remote_provider_registry.sources.clone();
        persisted.config.remote_provider_registry = RemoteProviderRegistrySettings {
            registry_url: Some(url.clone()),
            provider_proxy_url: proxy_url_ref
                .clone()
                .filter(|value| !value.trim().is_empty()),
            auto_update,
            sources: existing_sources,
        };
        save_config_to_path(&path, &persisted.config)?;
        log_remote(&log, LogLevel::Info, "registry settings persisted");

        let mut result = RegistryInstallResult {
            installed: Vec::new(),
            skipped: Vec::new(),
            failed: Vec::new(),
        };

        for entry in registry.providers {
            let provider_url = resolve_provider_url(&url, &entry.provider_url);
            if loaded
                .config
                .providers
                .iter()
                .any(|p| provider_matches_manifest(p, &entry.id, &provider_url))
            {
                log_remote(
                    &log,
                    LogLevel::Info,
                    &format!(
                        "registry entry skipped id={} reason=already-installed",
                        entry.id
                    ),
                );
                result.skipped.push(entry.id);
                continue;
            }

            log_remote(
                &log,
                LogLevel::Info,
                &format!(
                    "registry entry manifest fetch started id={} providerUrl={}",
                    entry.id, provider_url
                ),
            );

            let manifest_text = match fetch_manifest_text(
                &provider_url,
                proxy_url_ref.as_deref(),
                global_proxy.as_ref(),
                FETCH_TIMEOUT,
            ) {
                Ok(text) => {
                    log_remote(
                        &log,
                        LogLevel::Info,
                        &format!(
                            "registry entry manifest fetched id={} bytes={}",
                            entry.id,
                            text.len()
                        ),
                    );
                    text
                }
                Err(error) => {
                    log_remote(
                        &log,
                        LogLevel::Warn,
                        &format!(
                            "registry entry manifest fetch failed id={} error={}",
                            entry.id, error
                        ),
                    );
                    result.failed.push(RegistryInstallFailure {
                        id: entry.id,
                        error: error.to_string(),
                    });
                    continue;
                }
            };

            if let Some(expected) = entry.checksum.as_ref() {
                if let Err(error) =
                    crate::remote_provider::verify_checksum(&manifest_text, expected)
                {
                    log_remote(
                        &log,
                        LogLevel::Warn,
                        &format!(
                            "registry entry checksum failed id={} error={}",
                            entry.id, error
                        ),
                    );
                    result.failed.push(RegistryInstallFailure {
                        id: entry.id,
                        error: error.to_string(),
                    });
                    continue;
                }
                log_remote(
                    &log,
                    LogLevel::Info,
                    &format!("registry entry checksum verified id={}", entry.id),
                );
            }

            let manifest = match parse_manifest(&manifest_text) {
                Ok(manifest) => {
                    log_remote(
                        &log,
                        LogLevel::Info,
                        &format!(
                            "registry entry manifest parsed id={} manifestId={} version={}",
                            entry.id,
                            manifest.id,
                            manifest.version.as_deref().unwrap_or("none")
                        ),
                    );
                    manifest
                }
                Err(error) => {
                    log_remote(
                        &log,
                        LogLevel::Warn,
                        &format!(
                            "registry entry manifest parse failed id={} error={}",
                            entry.id, error
                        ),
                    );
                    result.failed.push(RegistryInstallFailure {
                        id: entry.id,
                        error: error.to_string(),
                    });
                    continue;
                }
            };

            match install_remote_provider_from_manifest(
                &path,
                &provider_url,
                proxy_url_ref.as_deref(),
                auto_update,
                manifest,
                &log,
            ) {
                Ok(config) => {
                    log_remote(
                        &log,
                        LogLevel::Info,
                        &format!("registry entry installed id={}", provider_id(&config)),
                    );
                    result.installed.push(config)
                }
                Err(error) => {
                    log_remote(
                        &log,
                        LogLevel::Warn,
                        &format!(
                            "registry entry install failed id={} error={}",
                            entry.id, error
                        ),
                    );
                    result.failed.push(RegistryInstallFailure {
                        id: entry.id,
                        error,
                    })
                }
            }
        }

        log_remote(
            &log,
            LogLevel::Info,
            &format!(
                "registry install finished installed={} skipped={} failed={}",
                result.installed.len(),
                result.skipped.len(),
                result.failed.len()
            ),
        );
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn migrate_remote_providers_to_registry(
    app: AppHandle,
    url: String,
    proxy_url: Option<String>,
) -> Result<RegistryMigrationResult, String> {
    let path = config_path_for_app(&app)?;
    let proxy_url_ref = proxy_url.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let mut loaded = load_or_create_config(&path)?;
        let log = LogSink::from_config_path(&path, &loaded.config);
        let global_proxy = loaded.config.network_proxy.clone();
        log_remote(
            &log,
            LogLevel::Info,
            &format!(
                "provider source migration started registryUrl={} proxyConfigured={}",
                url,
                proxy_url_ref
                    .as_deref()
                    .map(|value| !value.trim().is_empty())
                    .unwrap_or(false)
            ),
        );
        let registry = fetch_provider_registry(
            &url,
            proxy_url_ref.as_deref(),
            global_proxy.as_ref(),
            FETCH_TIMEOUT,
        )
        .map_err(|error| error.to_string())?;
        let mut result = RegistryMigrationResult {
            migrated: Vec::new(),
            skipped: Vec::new(),
            failed: Vec::new(),
        };

        for provider in &mut loaded.config.providers {
            let instance_id = provider_id(provider).to_string();
            let provider_dir = match cached_provider_dir(&path, provider) {
                Ok(path) => path,
                Err(error) => {
                    result.failed.push(RegistryInstallFailure {
                        id: instance_id,
                        error,
                    });
                    continue;
                }
            };
            let installed_manifest = match load_cached_manifest(&provider_dir) {
                Ok(manifest) => manifest,
                Err(error) => {
                    result.failed.push(RegistryInstallFailure {
                        id: instance_id,
                        error: format!("could not load the installed Provider manifest: {error}"),
                    });
                    continue;
                }
            };
            let Some(entry) = registry
                .providers
                .iter()
                .find(|entry| entry.id == installed_manifest.id)
            else {
                result.skipped.push(instance_id);
                continue;
            };

            let manifest_url = resolve_provider_url(&url, &entry.provider_url);
            let migration = (|| -> Result<(), String> {
                let manifest_text = fetch_manifest_text(
                    &manifest_url,
                    proxy_url_ref.as_deref(),
                    global_proxy.as_ref(),
                    FETCH_TIMEOUT,
                )
                .map_err(|error| error.to_string())?;
                if let Some(expected) = entry.checksum.as_deref() {
                    crate::remote_provider::verify_checksum(&manifest_text, expected)
                        .map_err(|error| error.to_string())?;
                }
                let manifest = parse_manifest(&manifest_text).map_err(|error| error.to_string())?;
                if manifest.id != installed_manifest.id {
                    return Err(format!(
                        "registry entry '{}' resolved to manifest id '{}' instead of '{}'",
                        entry.id, manifest.id, installed_manifest.id
                    ));
                }
                let source_url = resolve_source_url(&manifest_url, &manifest.entry);
                let source = fetch_source(
                    &source_url,
                    proxy_url_ref.as_deref(),
                    global_proxy.as_ref(),
                    FETCH_TIMEOUT,
                )
                .map_err(|error| error.to_string())?;
                let expected_checksum = manifest
                    .checksums
                    .source
                    .as_deref()
                    .ok_or_else(|| "manifest does not contain a source checksum".to_string())?;
                crate::remote_provider::verify_checksum(&source, expected_checksum)
                    .map_err(|error| error.to_string())?;
                let resolved_runtime = resolve_manifest_runtime(&manifest)?;
                let parent = provider_dir
                    .parent()
                    .ok_or_else(|| "provider cache directory has no parent".to_string())?;
                cache_remote_provider(
                    parent,
                    &instance_id,
                    &manifest_url,
                    &manifest,
                    &source,
                    resolved_runtime.as_deref(),
                )
                .map_err(|error| error.to_string())?;

                let ProviderConfig::Remote {
                    manifest_url: current_manifest_url,
                    source_url: current_source_url,
                    trusted_checksum,
                    version,
                    runtime,
                    resolved_runtime: current_resolved_runtime,
                    updated_at,
                    last_checked_at,
                    ..
                } = provider;
                *current_manifest_url = manifest_url.clone();
                *current_source_url = source_url;
                *trusted_checksum = Some(compute_checksum(&source));
                *version = manifest.version;
                *runtime = manifest.runtime;
                *current_resolved_runtime = resolved_runtime
                    .as_ref()
                    .map(|path| path.display().to_string());
                let now = Utc::now().to_rfc3339();
                *updated_at = Some(now.clone());
                *last_checked_at = Some(now);
                Ok(())
            })();

            match migration {
                Ok(()) => {
                    log_remote(
                        &log,
                        LogLevel::Info,
                        &format!(
                            "provider source migration finished providerId={} manifestId={}",
                            instance_id, installed_manifest.id
                        ),
                    );
                    result.migrated.push(instance_id);
                }
                Err(error) => {
                    log_remote(
                        &log,
                        LogLevel::Warn,
                        &format!(
                            "provider source migration failed providerId={} error={}",
                            instance_id, error
                        ),
                    );
                    result.failed.push(RegistryInstallFailure {
                        id: instance_id,
                        error,
                    });
                }
            }
        }

        if !result.migrated.is_empty() {
            save_config_to_path(&path, &loaded.config)?;
        }
        log_remote(
            &log,
            LogLevel::Info,
            &format!(
                "provider source migration finished migrated={} skipped={} failed={}",
                result.migrated.len(),
                result.skipped.len(),
                result.failed.len()
            ),
        );
        Ok(result)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn remove_remote_provider(
    app: AppHandle,
    id: String,
    delete_managed_secrets: Option<bool>,
) -> Result<(), String> {
    let path = config_path_for_app(&app)?;

    tauri::async_runtime::spawn_blocking(move || {
        let mut loaded = load_or_create_config(&path)?;
        let log = LogSink::from_config_path(&path, &loaded.config);
        log_remote(
            &log,
            LogLevel::Info,
            &format!("provider remove started id={id}"),
        );
        let provider_dir = loaded
            .config
            .providers
            .iter()
            .find(|provider| provider_id(provider) == id)
            .map(|provider| cached_provider_dir(&path, provider))
            .transpose()?
            .unwrap_or(remote_provider_dir(&path, &id)?);
        loaded
            .config
            .providers
            .retain(|provider| provider_id(provider) != id);
        save_config_to_path(&path, &loaded.config)?;
        log_remote(
            &log,
            LogLevel::Info,
            &format!("provider removed from config id={id}"),
        );

        if provider_dir.exists() {
            fs::remove_dir_all(&provider_dir).map_err(|e| e.to_string())?;
            log_remote(
                &log,
                LogLevel::Info,
                &format!(
                    "provider cache removed id={} dir={}",
                    id,
                    provider_dir.display()
                ),
            );
        } else {
            log_remote(
                &log,
                LogLevel::Info,
                &format!(
                    "provider cache absent id={} dir={}",
                    id,
                    provider_dir.display()
                ),
            );
        }
        if delete_managed_secrets.unwrap_or(true) {
            let config_dir = path
                .parent()
                .ok_or_else(|| "Unable to resolve config directory".to_string())?;
            let secret_dir = crate::managed_secret_store::managed_provider_secret_dir(config_dir, &id)?;
            crate::managed_secret_store::ManagedSecretStore::new(config_dir)
                .delete_provider_secrets(&id)
                .map_err(|error| {
                    format!(
                        "{error}; Provider configuration was removed, but managed secret files may remain at {}",
                        secret_dir.display()
                    )
                })?;
            log_remote(
                &log,
                LogLevel::Info,
                &format!("managed provider secrets removed id={id}"),
            );
        }
        log_remote(
            &log,
            LogLevel::Info,
            &format!("provider remove finished id={id}"),
        );
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn refresh_remote_provider(
    app: AppHandle,
    id: String,
) -> Result<crate::remote_provider::UpdateInfo, String> {
    check_remote_updates_inner(app, Some(id))
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| "provider not found".to_string())
}

#[tauri::command]
pub async fn check_remote_updates(app: AppHandle) -> Result<Vec<UpdateInfo>, String> {
    check_remote_updates_inner(app, None).await
}

async fn check_remote_updates_inner(
    app: AppHandle,
    only_id: Option<String>,
) -> Result<Vec<UpdateInfo>, String> {
    let path = config_path_for_app(&app)?;

    tauri::async_runtime::spawn_blocking(move || {
        let mut loaded = load_or_create_config(&path)?;
        let log = LogSink::from_config_path(&path, &loaded.config);
        let global_proxy = loaded.config.network_proxy.clone();
        let registry_update_candidates = load_registry_update_candidates(
            &loaded.config.remote_provider_registry,
            global_proxy.as_ref(),
            &log,
        )?;
        let mut config_changed = false;
        log_remote(
            &log,
            LogLevel::Info,
            &format!(
                "remote update check started onlyId={} providers={}",
                only_id.as_deref().unwrap_or("all"),
                loaded.config.providers.len()
            ),
        );

        let mut updates = Vec::new();
        for provider in &mut loaded.config.providers {
            let ProviderConfig::Remote {
                id,
                manifest_url,
                provider_dir,
                trusted_checksum,
                version,
                last_checked_at,
                ..
            } = provider;

            if let Some(ref only) = only_id {
                if id != only {
                    continue;
                }
            }

            log_remote(
                &log,
                LogLevel::Info,
                &format!("remote update check provider started id={} manifestUrl={}", id, manifest_url),
            );
            let provider_dir = provider_dir
                .clone()
                .unwrap_or(remote_provider_dir(&path, id)?);
            let installed_manifest_id = load_cached_manifest(&provider_dir)
                .map(|manifest| manifest.id)
                .unwrap_or_else(|_| id.clone());
            let mut update = if let Some(candidate) = registry_update_candidates.get(&installed_manifest_id) {
                log_remote(
                    &log,
                    LogLevel::Info,
                    &format!(
                        "remote update check provider using registry source id={} manifestId={} manifestUrl={}",
                        id, installed_manifest_id, candidate.manifest_url
                    ),
                );
                check_registry_update(
                    candidate,
                    &installed_manifest_id,
                    version.clone(),
                    trusted_checksum.as_deref(),
                    global_proxy.as_ref(),
                )
            } else {
                check_update(
                    &provider_dir,
                    manifest_url,
                    None,
                    global_proxy.as_ref(),
                    trusted_checksum.as_deref(),
                    FETCH_TIMEOUT,
                )
                .map(|mut update| {
                    update.current_version = version.clone().or(update.current_version);
                    update
                })
                .map_err(|error| error.to_string())
            }
            .map_err(|e| {
                log_remote(
                    &log,
                    LogLevel::Warn,
                    &format!("remote update check provider failed id={} error={}", id, e),
                );
                e.to_string()
            })?;
            update.id = id.clone();
            log_remote(
                &log,
                LogLevel::Info,
                &format!(
                    "remote update check provider finished id={} available={} currentVersion={} newVersion={} checkedAt={}",
                    id,
                    update.available,
                    update.current_version.as_deref().unwrap_or("none"),
                    update.new_version.as_deref().unwrap_or("none"),
                    update.checked_at.as_deref().unwrap_or("none")
                ),
            );

            if update.checked_at != *last_checked_at {
                *last_checked_at = update.checked_at.clone();
                config_changed = true;
            }

            updates.push(update);
        }
        if config_changed {
            save_config_to_path(&path, &loaded.config)?;
            log_remote(&log, LogLevel::Info, "remote update check persisted config changes");
        }
        log_remote(
            &log,
            LogLevel::Info,
            &format!("remote update check finished updates={}", updates.len()),
        );
        Ok(updates)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn apply_remote_update(
    app: AppHandle,
    id: String,
    update_manifest_url: Option<String>,
) -> Result<(), String> {
    let path = config_path_for_app(&app)?;

    tauri::async_runtime::spawn_blocking(move || {
        let mut loaded = load_or_create_config(&path)?;
        let log = LogSink::from_config_path(&path, &loaded.config);
        let global_proxy = loaded.config.network_proxy.clone();
        let has_selected_registry_update = update_manifest_url
            .as_deref()
            .map(str::trim)
            .map(|url| !url.is_empty())
            .unwrap_or(false);
        let registry_update_candidates = has_selected_registry_update
            .then(|| {
                load_registry_update_candidates(
                    &loaded.config.remote_provider_registry,
                    global_proxy.as_ref(),
                    &log,
                )
            })
            .transpose()?;
        log_remote(
            &log,
            LogLevel::Info,
            &format!("remote update apply started id={id}"),
        );

        let provider = find_provider_config_mut(&mut loaded.config, &id)
            .ok_or("provider not found")?;
        let provider_dir = cached_provider_dir(&path, provider)
            .map_err(|e| format!("failed to resolve cache directory: {e}"))?;
        let installed_manifest_url = match provider {
            ProviderConfig::Remote { manifest_url, .. } => manifest_url.clone(),
        };
        let installed_manifest_id = load_cached_manifest(&provider_dir)
            .map(|manifest| manifest.id)
            .unwrap_or_else(|_| id.clone());
        let requested_update_manifest_url = update_manifest_url
            .as_deref()
            .map(str::trim)
            .filter(|url| !url.is_empty());
        let (selected_manifest_url, update_proxy_url, manifest_checksum) =
            if let Some(requested_url) = requested_update_manifest_url {
                let candidate = registry_update_candidates
                    .as_ref()
                    .expect("a selected registry update loads registry candidates")
                    .get(&installed_manifest_id)
                    .filter(|candidate| candidate.manifest_url == requested_url)
                    .ok_or_else(|| {
                        "the selected Provider update is no longer offered by an enabled source; check for updates again"
                            .to_string()
                    })?;
                (
                    candidate.manifest_url.clone(),
                    candidate.proxy_url.clone(),
                    candidate.manifest_checksum.clone(),
                )
            } else {
                (installed_manifest_url, None, None)
            };

        log_remote(
            &log,
            LogLevel::Info,
            &format!(
                "remote update apply manifest fetch started id={} manifestUrl={}",
                id, selected_manifest_url
            ),
        );
        let manifest = if let Some(expected_manifest_checksum) = manifest_checksum.as_deref() {
            let manifest_text = fetch_manifest_text(
                &selected_manifest_url,
                update_proxy_url.as_deref(),
                global_proxy.as_ref(),
                FETCH_TIMEOUT,
            )
            .map_err(|error| error.to_string())?;
            crate::remote_provider::verify_checksum(&manifest_text, expected_manifest_checksum)
                .map_err(|error| error.to_string())?;
            let manifest = parse_manifest(&manifest_text).map_err(|error| error.to_string())?;
            if manifest.id != installed_manifest_id {
                return Err(format!(
                    "update source manifest id '{}' does not match installed Provider id '{}'",
                    manifest.id, installed_manifest_id
                ));
            }
            manifest
        } else {
            fetch_manifest(
                &selected_manifest_url,
                None,
                global_proxy.as_ref(),
                FETCH_TIMEOUT,
            )
            .map_err(|error| error.to_string())?
        };
        let new_source_url = resolve_source_url(&selected_manifest_url, &manifest.entry);
        log_remote(
            &log,
            LogLevel::Info,
            &format!(
                "remote update apply source fetch started id={} sourceUrl={}",
                id, new_source_url
            ),
        );
        let source = fetch_source(
            &new_source_url,
            update_proxy_url.as_deref(),
            global_proxy.as_ref(),
            FETCH_TIMEOUT,
        )
            .map_err(|e| {
                log_remote(
                    &log,
                    LogLevel::Warn,
                    &format!(
                        "remote update apply source fetch failed id={} error={}",
                        id, e
                    ),
                );
                e.to_string()
            })?;

        if let Some(expected) = manifest.checksums.source.as_ref() {
            crate::remote_provider::verify_checksum(&source, expected).map_err(|e| {
                log_remote(
                    &log,
                    LogLevel::Warn,
                    &format!("remote update apply checksum failed id={} error={}", id, e),
                );
                e.to_string()
            })?;
            log_remote(
                &log,
                LogLevel::Info,
                &format!("remote update apply checksum verified id={id}"),
            );
        } else {
            log_remote(
                &log,
                LogLevel::Warn,
                &format!("remote update apply failed id={id} reason=missing-source-checksum"),
            );
            return Err("manifest does not contain a source checksum".to_string());
        }

        let parent = provider_dir
            .parent()
            .expect("provider dir has parent")
            .to_path_buf();
        let new_resolved_runtime = resolve_manifest_runtime(&manifest)?;
        cache_remote_provider(
            &parent,
            &id,
            &selected_manifest_url,
            &manifest,
            &source,
            new_resolved_runtime.as_deref(),
        )
        .map_err(|e| {
            log_remote(
                &log,
                LogLevel::Warn,
                &format!("remote update apply cache failed id={} error={}", id, e),
            );
            e.to_string()
        })?;
        log_remote(
            &log,
            LogLevel::Info,
            &format!(
                "remote update apply cached id={} dir={}",
                id,
                provider_dir.display()
            ),
        );

        let ProviderConfig::Remote {
            ref mut manifest_url,
            ref mut trusted_checksum,
            ref mut source_url,
            ref mut version,
            ref mut runtime,
            ref mut resolved_runtime,
            ref mut updated_at,
            ref mut last_checked_at,
            ..
        } = find_provider_config_mut(&mut loaded.config, &id).ok_or("provider not found")?;
        *manifest_url = selected_manifest_url;
        *trusted_checksum = Some(compute_checksum(&source));
        *source_url = new_source_url;
        *version = manifest.version;
        *runtime = manifest.runtime;
        *resolved_runtime = new_resolved_runtime
            .as_ref()
            .map(|path| path.display().to_string());
        let now = Utc::now().to_rfc3339();
        *updated_at = Some(now.clone());
        *last_checked_at = Some(now);

        save_config_to_path(&path, &loaded.config)?;
        log_remote(
            &log,
            LogLevel::Info,
            &format!("remote update apply finished id={id}"),
        );
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn embedded_runtime_requires_no_external_executable() {
        let manifest = crate::remote_provider::parse_manifest(
            r#"{"schemaVersion":2,"id":"builtin","displayName":"Builtin","minAppVersion":"1.1.0","runtime":"builtin-js","entry":"provider.js","output":"provider-snapshot-v1"}"#,
        )
        .expect("builtin manifest");
        assert_eq!(resolve_manifest_runtime(&manifest).expect("runtime"), None);
    }

    #[test]
    fn update_sources_use_enabled_sources_or_the_legacy_registry() {
        let settings = RemoteProviderRegistrySettings {
            registry_url: Some("https://legacy.example.com/registry.json".to_string()),
            provider_proxy_url: Some("http://legacy-proxy.example.com".to_string()),
            auto_update: true,
            sources: vec![
                crate::config::RemoteProviderRegistrySource {
                    id: "disabled".to_string(),
                    name: "Disabled".to_string(),
                    url: "https://disabled.example.com/registry.json".to_string(),
                    provider_proxy_url: None,
                    auto_update: true,
                    enabled: false,
                },
                crate::config::RemoteProviderRegistrySource {
                    id: "selected".to_string(),
                    name: "Selected".to_string(),
                    url: " https://selected.example.com/registry.json ".to_string(),
                    provider_proxy_url: Some(" http://source-proxy.example.com ".to_string()),
                    auto_update: true,
                    enabled: true,
                },
            ],
        };

        let sources = configured_registry_update_sources(&settings);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].url, "https://selected.example.com/registry.json");
        assert_eq!(
            sources[0].proxy_url.as_deref(),
            Some("http://source-proxy.example.com")
        );

        let legacy_sources = configured_registry_update_sources(&RemoteProviderRegistrySettings {
            sources: Vec::new(),
            ..settings
        });
        assert_eq!(legacy_sources.len(), 1);
        assert_eq!(
            legacy_sources[0].url,
            "https://legacy.example.com/registry.json"
        );
        assert_eq!(
            legacy_sources[0].proxy_url.as_deref(),
            Some("http://legacy-proxy.example.com")
        );
    }

    fn remote_provider(id: &str, name: &str, manifest_url: &str) -> ProviderConfig {
        ProviderConfig::Remote {
            id: id.to_string(),
            name: name.to_string(),
            enabled: true,
            version: Some("1.0.0".to_string()),
            manifest_url: manifest_url.to_string(),
            source_url: manifest_url.replace("provider.json", "provider.cjs"),
            provider_dir: None,
            runtime: "node".to_string(),
            resolved_runtime: None,
            proxy_url: None,
            auto_update: true,
            update_interval_seconds: 3600,
            timeout_seconds: DEFAULT_REMOTE_PROVIDER_TIMEOUT_SECONDS,
            trusted_checksum: None,
            installed_at: None,
            updated_at: None,
            last_checked_at: None,
            window_label_overrides: HashMap::new(),
            visible_window_ids: Vec::new(),
            show_in_tray: true,
            env_vars: HashMap::new(),
            setup_state: crate::config::ProviderSetupState::Ready,
            setup_last_tested_at: None,
        }
    }

    #[test]
    fn provider_instances_get_unique_ids_and_names() {
        let mut config = crate::config::default_config();
        config.providers = vec![
            remote_provider(
                "kimi-coding",
                "Kimi Coding",
                "https://example.com/kimi/provider.json",
            ),
            remote_provider(
                "kimi-coding-2",
                "Kimi Coding 2",
                "https://example.com/kimi/provider.json",
            ),
        ];

        assert_eq!(
            unique_provider_id(&config, "kimi-coding"),
            ("kimi-coding-3".to_string(), 3)
        );
        assert_eq!(
            unique_provider_name(&config, "Kimi Coding", 3),
            "Kimi Coding 3"
        );
        assert_eq!(
            installed_provider_instance_count(
                &config,
                "kimi-coding",
                "https://example.com/kimi/provider.json",
            ),
            2
        );
    }

    #[test]
    fn installed_provider_matching_accepts_legacy_id_or_manifest_url() {
        let mut config = crate::config::default_config();
        config.providers = vec![
            remote_provider(
                "kimi-coding",
                "Kimi Coding",
                "https://mirror.example.com/kimi/provider.json",
            ),
            remote_provider(
                "team-kimi",
                "Team Kimi",
                "https://example.com/kimi/provider.json",
            ),
        ];

        assert_eq!(
            installed_provider_instance_count(
                &config,
                "kimi-coding",
                "https://example.com/kimi/provider.json",
            ),
            2
        );
    }
}
