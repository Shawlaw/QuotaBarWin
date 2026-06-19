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
        RemoteProviderRegistrySettings,
    },
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
) -> Result<ProviderConfig, String> {
    let mut loaded = load_or_create_config(path)?;
    let global_proxy = loaded.config.network_proxy.clone();

    if loaded
        .config
        .providers
        .iter()
        .any(|p| provider_id(p) == manifest.id)
    {
        return Err(format!(
            "id conflict: a provider with id '{}' already exists",
            manifest.id
        ));
    }

    let source_url = resolve_source_url(url, &manifest.entry);
    let source = fetch_source(&source_url, proxy_url, global_proxy.as_ref(), FETCH_TIMEOUT)
        .map_err(|e| e.to_string())?;

    let actual_auto_update = manifest.checksums.source.is_some() && auto_update;

    if let Some(expected) = manifest.checksums.source.as_ref() {
        crate::remote_provider::verify_checksum(&source, expected).map_err(|e| e.to_string())?;
    }

    let resolved_runtime_path = resolve_runtime(&manifest.runtime)
        .and_then(|path| {
            validate_runtime_executable(&path)?;
            Ok(path)
        })
        .map_err(|e| e.to_string())?;

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
        let global_proxy = loaded.config.network_proxy.clone();
        let registry = fetch_provider_registry(
            &url,
            proxy_url_ref.as_deref(),
            global_proxy.as_ref(),
            FETCH_TIMEOUT,
        )
        .map_err(|e| e.to_string())?;

        let mut persisted = load_or_create_config(&path)?;
        persisted.config.remote_provider_registry = RemoteProviderRegistrySettings {
            registry_url: Some(url.clone()),
            provider_proxy_url: proxy_url_ref
                .clone()
                .filter(|value| !value.trim().is_empty()),
            auto_update,
        };
        save_config_to_path(&path, &persisted.config)?;

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
                result.skipped.push(entry.id);
                continue;
            }

            let provider_url = resolve_provider_url(&url, &entry.provider_url);

            let manifest_text = match fetch_manifest_text(
                &provider_url,
                proxy_url_ref.as_deref(),
                global_proxy.as_ref(),
                FETCH_TIMEOUT,
            ) {
                Ok(text) => text,
                Err(error) => {
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
                    result.failed.push(RegistryInstallFailure {
                        id: entry.id,
                        error: error.to_string(),
                    });
                    continue;
                }
            }

            let manifest = match parse_manifest(&manifest_text) {
                Ok(manifest) => manifest,
                Err(error) => {
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
            ) {
                Ok(config) => result.installed.push(config),
                Err(error) => result.failed.push(RegistryInstallFailure {
                    id: entry.id,
                    error,
                }),
            }
        }

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
        loaded
            .config
            .providers
            .retain(|provider| provider_id(provider) != id);
        save_config_to_path(&path, &loaded.config)?;

        let provider_dir = remote_provider_dir(&app_handle, &id)
            .map_err(|e| format!("failed to resolve cache directory: {e}"))?;
        if provider_dir.exists() {
            fs::remove_dir_all(&provider_dir).map_err(|e| e.to_string())?;
        }
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
        let global_proxy = loaded.config.network_proxy.clone();
        let mut config_changed = false;

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
            .map_err(|e| e.to_string())?;

            if update.checked_at != *last_checked_at {
                *last_checked_at = update.checked_at.clone();
                config_changed = true;
            }

            if update.available && *auto_update {
                if let Some(new_checksum) = &update.new_checksum {
                    let manifest = fetch_manifest(
                        manifest_url,
                        proxy_url.as_deref(),
                        global_proxy.as_ref(),
                        FETCH_TIMEOUT,
                    )
                    .map_err(|e| e.to_string())?;
                    let new_source_url = resolve_source_url(manifest_url, &manifest.entry);
                    let source = fetch_source(
                        &new_source_url,
                        proxy_url.as_deref(),
                        global_proxy.as_ref(),
                        FETCH_TIMEOUT,
                    )
                    .map_err(|e| e.to_string())?;
                    crate::remote_provider::verify_checksum(&source, new_checksum)
                        .map_err(|e| e.to_string())?;

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
                    .map_err(|e| e.to_string())?;
                    *trusted_checksum = Some(compute_checksum(&source));
                    *source_url = new_source_url;
                    *version = manifest.version.clone();
                    *updated_at = Some(Utc::now().to_rfc3339());
                    config_changed = true;
                }
            }

            updates.push(update);
        }
        if config_changed {
            save_config_to_path(&path, &loaded.config)?;
        }
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
        let global_proxy = loaded.config.network_proxy.clone();

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

        let manifest = fetch_manifest(
            &manifest_url,
            proxy_url.as_deref(),
            global_proxy.as_ref(),
            FETCH_TIMEOUT,
        )
        .map_err(|e| e.to_string())?;
        let new_source_url = resolve_source_url(&manifest_url, &manifest.entry);
        let source = fetch_source(
            &new_source_url,
            proxy_url.as_deref(),
            global_proxy.as_ref(),
            FETCH_TIMEOUT,
        )
        .map_err(|e| e.to_string())?;

        if let Some(expected) = manifest.checksums.source.as_ref() {
            crate::remote_provider::verify_checksum(&source, expected)
                .map_err(|e| e.to_string())?;
        } else {
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
        .map_err(|e| e.to_string())?;

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

        save_config_to_path(&path, &loaded.config)
    })
    .await
    .map_err(|e| e.to_string())?
}
