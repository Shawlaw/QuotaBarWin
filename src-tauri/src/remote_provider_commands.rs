use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::{
    config::{
        config_path_for_app, load_or_create_config, save_config_to_path, AppConfig, ProviderConfig,
        RemoteProviderRegistrySettings, DEFAULT_REMOTE_PROVIDER_TIMEOUT_SECONDS,
    },
    logger::{LogLevel, LogSink},
    proxy::ProxyConfig,
    remote_provider::{
        cache_remote_provider, check_update, compute_checksum, fetch_manifest, fetch_manifest_text,
        fetch_provider_registry, fetch_source, load_cached_meta, parse_manifest,
        resolve_provider_url, resolve_runtime, resolve_source_url, validate_runtime_executable,
        ProviderManifest, UpdateInfo,
    },
};

const FETCH_TIMEOUT: Duration = Duration::from_secs(30);
const REMOTE_PROVIDERS_DIR: &str = "providers/remote";

fn remote_provider_dir(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(app_data_dir.join(REMOTE_PROVIDERS_DIR).join(id))
}

fn provider_id(provider: &ProviderConfig) -> &str {
    match provider {
        ProviderConfig::Remote { id, .. } => id,
    }
}

fn log_remote(log: &LogSink, level: LogLevel, message: &str) {
    let _ = log.write(level, "remote_provider_commands", message);
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

fn install_remote_provider_from_manifest(
    app_handle: &AppHandle,
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
            "provider install started id={} manifestUrl={} autoUpdateRequested={} proxyConfigured={}",
            manifest.id,
            url,
            auto_update,
            proxy_url.map(|value| !value.trim().is_empty()).unwrap_or(false)
        ),
    );

    if loaded
        .config
        .providers
        .iter()
        .any(|p| provider_id(p) == manifest.id)
    {
        log_remote(
            log,
            LogLevel::Warn,
            &format!(
                "provider install skipped id={} reason=id-conflict",
                manifest.id
            ),
        );
        return Err(format!(
            "id conflict: a provider with id '{}' already exists",
            manifest.id
        ));
    }

    let source_url = resolve_source_url(url, &manifest.entry);
    log_remote(
        log,
        LogLevel::Info,
        &format!(
            "provider source fetch started id={} sourceUrl={}",
            manifest.id, source_url
        ),
    );
    let source = fetch_source(&source_url, proxy_url, global_proxy.as_ref(), FETCH_TIMEOUT)
        .map_err(|e| e.to_string())?;
    log_remote(
        log,
        LogLevel::Info,
        &format!(
            "provider source fetched id={} bytes={}",
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
            &format!("provider source checksum verified id={}", manifest.id),
        );
    }

    log_remote(
        log,
        LogLevel::Info,
        &format!(
            "provider runtime resolve started id={} runtime={}",
            manifest.id, manifest.runtime
        ),
    );
    let resolved_runtime_path = resolve_runtime(&manifest.runtime)
        .and_then(|path| {
            validate_runtime_executable(&path)?;
            Ok(path)
        })
        .map_err(|e| e.to_string())?;
    log_remote(
        log,
        LogLevel::Info,
        &format!(
            "provider runtime resolved id={} path={}",
            manifest.id,
            resolved_runtime_path.display()
        ),
    );

    let provider_dir = remote_provider_dir(app_handle, &manifest.id)
        .map_err(|e| format!("failed to resolve cache directory: {e}"))?;

    let parent = provider_dir
        .parent()
        .expect("provider dir has parent")
        .to_path_buf();
    cache_remote_provider(
        &parent,
        &manifest.id,
        url,
        &manifest,
        &source,
        Some(&resolved_runtime_path),
    )
    .map_err(|e| e.to_string())?;
    log_remote(
        log,
        LogLevel::Info,
        &format!(
            "provider cached id={} dir={}",
            manifest.id,
            provider_dir.display()
        ),
    );

    let now = Utc::now().to_rfc3339();
    let config = ProviderConfig::Remote {
        id: manifest.id.clone(),
        name: manifest.display_name.clone(),
        enabled: true,
        version: manifest.version.clone(),
        manifest_url: url.to_string(),
        source_url,
        provider_dir: Some(provider_dir.clone()),
        runtime: manifest.runtime.clone(),
        resolved_runtime: Some(resolved_runtime_path.display().to_string()),
        proxy_url: proxy_url.map(|s| s.to_string()),
        auto_update: actual_auto_update,
        update_interval_seconds: 3600,
        timeout_seconds: DEFAULT_REMOTE_PROVIDER_TIMEOUT_SECONDS,
        trusted_checksum: Some(compute_checksum(&source)),
        installed_at: Some(now.clone()),
        updated_at: Some(now.clone()),
        last_checked_at: Some(now),
        window_label_overrides: Default::default(),
        visible_window_ids: Default::default(),
        env_vars: Default::default(),
    };

    loaded.config.providers.push(config.clone());
    save_config_to_path(path, &loaded.config)?;
    log_remote(
        log,
        LogLevel::Info,
        &format!(
            "provider install finished id={} autoUpdate={}",
            manifest.id, actual_auto_update
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

#[tauri::command]
pub async fn install_remote_provider_registry(
    app: AppHandle,
    url: String,
    proxy_url: Option<String>,
    auto_update: bool,
) -> Result<RegistryInstallResult, String> {
    let path = config_path_for_app(&app)?;
    let app_handle = app.clone();
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
        persisted.config.remote_provider_registry = RemoteProviderRegistrySettings {
            registry_url: Some(url.clone()),
            provider_proxy_url: proxy_url_ref
                .clone()
                .filter(|value| !value.trim().is_empty()),
            auto_update,
        };
        save_config_to_path(&path, &persisted.config)?;
        log_remote(&log, LogLevel::Info, "registry settings persisted");

        let mut result = RegistryInstallResult {
            installed: Vec::new(),
            skipped: Vec::new(),
            failed: Vec::new(),
        };

        for entry in registry.providers {
            if loaded
                .config
                .providers
                .iter()
                .any(|p| provider_id(p) == entry.id)
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

            let provider_url = resolve_provider_url(&url, &entry.provider_url);
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
                &app_handle,
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
pub async fn remove_remote_provider(app: AppHandle, id: String) -> Result<(), String> {
    let path = config_path_for_app(&app)?;
    let app_handle = app.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let mut loaded = load_or_create_config(&path)?;
        let log = LogSink::from_config_path(&path, &loaded.config);
        log_remote(
            &log,
            LogLevel::Info,
            &format!("provider remove started id={id}"),
        );
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

        let provider_dir = remote_provider_dir(&app_handle, &id)
            .map_err(|e| format!("failed to resolve cache directory: {e}"))?;
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
    let app_handle = app.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let mut loaded = load_or_create_config(&path)?;
        let log = LogSink::from_config_path(&path, &loaded.config);
        let global_proxy = loaded.config.network_proxy.clone();
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
                proxy_url,
                trusted_checksum,
                auto_update,
                version,
                source_url,
                updated_at,
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
            let provider_dir = remote_provider_dir(&app_handle, id)
                .map_err(|e| format!("failed to resolve cache directory: {e}"))?;
            let update = check_update(
                &provider_dir,
                manifest_url,
                proxy_url.as_deref(),
                global_proxy.as_ref(),
                trusted_checksum.as_deref(),
                FETCH_TIMEOUT,
            )
            .map_err(|e| {
                log_remote(
                    &log,
                    LogLevel::Warn,
                    &format!("remote update check provider failed id={} error={}", id, e),
                );
                e.to_string()
            })?;
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

            if update.available && *auto_update {
                if let Some(new_checksum) = &update.new_checksum {
                    log_remote(
                        &log,
                        LogLevel::Info,
                        &format!("remote auto-update started id={id}"),
                    );
                    let manifest = fetch_manifest(
                        manifest_url,
                        proxy_url.as_deref(),
                        global_proxy.as_ref(),
                        FETCH_TIMEOUT,
                    )
                    .map_err(|e| {
                        log_remote(
                            &log,
                            LogLevel::Warn,
                            &format!("remote auto-update manifest fetch failed id={} error={}", id, e),
                        );
                        e.to_string()
                    })?;
                    let new_source_url = resolve_source_url(manifest_url, &manifest.entry);
                    log_remote(
                        &log,
                        LogLevel::Info,
                        &format!("remote auto-update source fetch started id={} sourceUrl={}", id, new_source_url),
                    );
                    let source = fetch_source(
                        &new_source_url,
                        proxy_url.as_deref(),
                        global_proxy.as_ref(),
                        FETCH_TIMEOUT,
                    )
                    .map_err(|e| {
                        log_remote(
                            &log,
                            LogLevel::Warn,
                            &format!("remote auto-update source fetch failed id={} error={}", id, e),
                        );
                        e.to_string()
                    })?;
                    crate::remote_provider::verify_checksum(&source, new_checksum)
                        .map_err(|e| {
                            log_remote(
                                &log,
                                LogLevel::Warn,
                                &format!("remote auto-update checksum failed id={} error={}", id, e),
                            );
                            e.to_string()
                        })?;
                    log_remote(
                        &log,
                        LogLevel::Info,
                        &format!("remote auto-update checksum verified id={id}"),
                    );

                    let parent = provider_dir
                        .parent()
                        .expect("provider dir has parent")
                        .to_path_buf();
                    let resolved_runtime = load_cached_meta(&provider_dir)
                        .map_err(|e| e.to_string())?
                        .and_then(|meta| meta.resolved_runtime)
                        .map(PathBuf::from);
                    cache_remote_provider(
                        &parent,
                        id,
                        manifest_url,
                        &manifest,
                        &source,
                        resolved_runtime.as_deref(),
                    )
                    .map_err(|e| {
                        log_remote(
                            &log,
                            LogLevel::Warn,
                            &format!("remote auto-update cache failed id={} error={}", id, e),
                        );
                        e.to_string()
                    })?;
                    *trusted_checksum = Some(compute_checksum(&source));
                    *source_url = new_source_url;
                    *version = manifest.version.clone();
                    *updated_at = Some(Utc::now().to_rfc3339());
                    config_changed = true;
                    log_remote(
                        &log,
                        LogLevel::Info,
                        &format!(
                            "remote auto-update finished id={} version={}",
                            id,
                            version.as_ref().map(String::as_str).unwrap_or("none")
                        ),
                    );
                }
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
pub async fn apply_remote_update(app: AppHandle, id: String) -> Result<(), String> {
    let path = config_path_for_app(&app)?;
    let app_handle = app.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let mut loaded = load_or_create_config(&path)?;
        let log = LogSink::from_config_path(&path, &loaded.config);
        let global_proxy = loaded.config.network_proxy.clone();
        log_remote(
            &log,
            LogLevel::Info,
            &format!("remote update apply started id={id}"),
        );

        let provider =
            find_provider_config_mut(&mut loaded.config, &id).ok_or("provider not found")?;
        let ProviderConfig::Remote {
            manifest_url,
            proxy_url,
            ..
        } = provider;

        let manifest_url = manifest_url.clone();
        let proxy_url = proxy_url.clone();
        let provider_dir = remote_provider_dir(&app_handle, &id)
            .map_err(|e| format!("failed to resolve cache directory: {e}"))?;

        log_remote(
            &log,
            LogLevel::Info,
            &format!(
                "remote update apply manifest fetch started id={} manifestUrl={}",
                id, manifest_url
            ),
        );
        let manifest = fetch_manifest(
            &manifest_url,
            proxy_url.as_deref(),
            global_proxy.as_ref(),
            FETCH_TIMEOUT,
        )
        .map_err(|e| {
            log_remote(
                &log,
                LogLevel::Warn,
                &format!(
                    "remote update apply manifest fetch failed id={} error={}",
                    id, e
                ),
            );
            e.to_string()
        })?;
        let new_source_url = resolve_source_url(&manifest_url, &manifest.entry);
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
            proxy_url.as_deref(),
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
        let resolved_runtime = load_cached_meta(&provider_dir)
            .map_err(|e| e.to_string())?
            .and_then(|meta| meta.resolved_runtime)
            .map(PathBuf::from);
        cache_remote_provider(
            &parent,
            &id,
            &manifest_url,
            &manifest,
            &source,
            resolved_runtime.as_deref(),
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
            ref mut trusted_checksum,
            ref mut source_url,
            ref mut version,
            ref mut updated_at,
            ref mut last_checked_at,
            ..
        } = find_provider_config_mut(&mut loaded.config, &id).ok_or("provider not found")?;
        *trusted_checksum = Some(compute_checksum(&source));
        *source_url = new_source_url;
        *version = manifest.version;
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
