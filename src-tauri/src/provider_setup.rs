use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::{
    config::{
        config_path_for_app, load_or_create_config, resolve_secret_value,
        ProviderConfig, ProviderSetupState,
    },
    managed_secret_store::ManagedSecretStore,
    provider_error::{classify_provider_error, ProviderErrorCategory},
    quota::ProviderSnapshot,
    remote_provider::{load_cached_manifest, ProviderManifest, ProviderParameter},
    remote_provider_commands::cached_provider_dir,
    remote_provider_runner::run_remote_provider,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProviderParameterSource {
    Missing,
    ManagedLocalFile,
    SecretFile,
    Environment,
    ExternalFile,
    Literal,
    Default,
    AdvancedExpression,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSetupField {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub kind: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_url: Option<String>,
    pub advanced: bool,
    /// Secret values are deliberately never included in setup descriptors.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    pub configured: bool,
    pub source: ProviderParameterSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSetupDescriptor {
    pub provider_id: String,
    pub provider_type: String,
    pub display_name: String,
    pub setup_state: ProviderSetupState,
    pub fields: Vec<ProviderSetupField>,
    /// Names of configured env vars that the manifest does not declare.
    pub unknown_env_vars: Vec<String>,
    pub has_unknown_env_vars: bool,
    pub can_auto_detect: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProviderSetupRequest {
    pub provider_id: String,
    pub display_name: String,
    #[serde(default)]
    pub values: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub secret_updates: HashMap<String, Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSetupTestResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_category: Option<ProviderErrorCategory>,
}

pub(crate) fn provider_parameters(manifest: &ProviderManifest) -> Vec<ProviderParameter> {
    if !manifest.parameters.is_empty() {
        return manifest.parameters.clone();
    }
    manifest
        .required_env_vars
        .iter()
        .map(|name| ProviderParameter {
            name: name.clone(),
            label: None,
            kind: Some("secret".to_string()),
            required: true,
            default_value: None,
            placeholder: None,
            description: None,
            options: Vec::new(),
            help_url: None,
            advanced: false,
        })
        .collect()
}

pub(crate) fn parameter_kind(parameter: &ProviderParameter) -> &str {
    match parameter.kind.as_deref() {
        Some("secret") => "secret",
        Some("number") => "number",
        Some("select") => "select",
        _ => "string",
    }
}

pub(crate) fn managed_placeholder(provider_id: &str, parameter_name: &str) -> String {
    format!("${{secret:providers/{provider_id}/{parameter_name}}}")
}

fn form_value(
    kind: &str,
    source_value: Option<&String>,
    default_value: Option<&String>,
) -> Option<serde_json::Value> {
    if kind == "secret" {
        return None;
    }

    let value = source_value.or(default_value)?;
    // A placeholder default is an instruction for the resolver, not a value a
    // user has entered. Treat an unchanged matching placeholder as blank so a
    // normal save does not inject a missing secret before a Provider can apply
    // its own configured credential fallback.
    if value.contains("${") && source_value.is_none_or(|source| Some(source) == default_value) {
        return None;
    }
    Some(serde_json::Value::String(value.clone()))
}

fn source_for_value(
    source: Option<&String>,
    default_value: Option<&String>,
    config_dir: &Path,
    provider_id: &str,
    parameter_name: &str,
    store: &ManagedSecretStore,
) -> Result<(ProviderParameterSource, bool), String> {
    let Some(source) = source.filter(|value| !value.trim().is_empty()) else {
        return Ok((
            if default_value.is_some() {
                ProviderParameterSource::Default
            } else {
                ProviderParameterSource::Missing
            },
            false,
        ));
    };
    if source == &managed_placeholder(provider_id, parameter_name) {
        return Ok((
            ProviderParameterSource::ManagedLocalFile,
            store.exists(provider_id, parameter_name)?,
        ));
    }
    if source.starts_with("${secret:") && source.ends_with('}') {
        return Ok((
            ProviderParameterSource::SecretFile,
            resolve_secret_value(source, config_dir)
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false),
        ));
    }
    if source.starts_with("${env:") && source.ends_with('}') {
        return Ok((
            ProviderParameterSource::Environment,
            resolve_secret_value(source, config_dir)
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false),
        ));
    }
    if source.starts_with("${file:") && source.ends_with('}') {
        return Ok((
            ProviderParameterSource::ExternalFile,
            resolve_secret_value(source, config_dir)
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false),
        ));
    }
    if source.contains("${") {
        return Ok((ProviderParameterSource::AdvancedExpression, true));
    }
    Ok((ProviderParameterSource::Literal, true))
}

pub(crate) fn setup_descriptor(
    config_path: &Path,
    provider: &ProviderConfig,
    manifest: &ProviderManifest,
) -> Result<ProviderSetupDescriptor, String> {
    let config_dir = config_path
        .parent()
        .ok_or_else(|| "Unable to resolve config directory".to_string())?;
    let store = ManagedSecretStore::new(
        config_dir,
        crate::config::secret_storage_mode_for_config_path(config_path),
    );
    let ProviderConfig::Remote {
        id,
        name,
        env_vars,
        setup_state,
        ..
    } = provider;
    let parameters = provider_parameters(manifest);
    let parameter_names = parameters
        .iter()
        .map(|parameter| parameter.name.as_str())
        .collect::<HashSet<_>>();
    let fields = parameters
        .iter()
        .map(|parameter| {
            let kind = parameter_kind(parameter);
            let source_value = env_vars.get(&parameter.name);
            let (source, configured) = source_for_value(
                source_value,
                parameter.default_value.as_ref(),
                config_dir,
                id,
                &parameter.name,
                &store,
            )?;
            let value = form_value(kind, source_value, parameter.default_value.as_ref());
            Ok(ProviderSetupField {
                name: parameter.name.clone(),
                label: parameter.label.clone(),
                kind: kind.to_string(),
                required: parameter.required,
                description: parameter.description.clone(),
                placeholder: parameter.placeholder.clone(),
                options: parameter.options.clone(),
                help_url: parameter.help_url.clone(),
                advanced: parameter.advanced,
                value,
                configured,
                source,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let mut unknown_env_vars = env_vars
        .keys()
        .filter(|name| !parameter_names.contains(name.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    unknown_env_vars.sort();

    Ok(ProviderSetupDescriptor {
        provider_id: id.clone(),
        provider_type: manifest.id.clone(),
        display_name: name.clone(),
        setup_state: setup_state.clone(),
        fields,
        has_unknown_env_vars: !unknown_env_vars.is_empty(),
        unknown_env_vars,
        can_auto_detect: !parameters.iter().any(|parameter| parameter.required),
    })
}

fn load_provider_and_manifest(
    config_path: &Path,
    provider_id: &str,
) -> Result<(crate::config::LoadedConfig, usize, ProviderManifest), String> {
    let loaded = load_or_create_config(config_path)?;
    let provider_index = loaded
        .config
        .providers
        .iter()
        .position(
            |provider| matches!(provider, ProviderConfig::Remote { id, .. } if id == provider_id),
        )
        .ok_or_else(|| "Provider was not found".to_string())?;
    let provider_dir = cached_provider_dir(config_path, &loaded.config.providers[provider_index])?;
    let manifest = load_cached_manifest(&provider_dir).map_err(|error| error.to_string())?;
    Ok((loaded, provider_index, manifest))
}

pub fn get_provider_setup_from_path(
    config_path: &Path,
    provider_id: &str,
) -> Result<ProviderSetupDescriptor, String> {
    let (loaded, provider_index, manifest) = load_provider_and_manifest(config_path, provider_id)?;
    setup_descriptor(
        config_path,
        &loaded.config.providers[provider_index],
        &manifest,
    )
}

/// Computes the execution fingerprint for a provider using the cache that the
/// runner would execute and the exact environment it would inject.
pub(crate) fn provider_execution_fingerprint(
    config_path: &Path,
    provider: &ProviderConfig,
    global_proxy: Option<&crate::proxy::ProxyConfig>,
) -> Result<crate::execution_fingerprint::ExecutionFingerprint, String> {
    let ProviderConfig::Remote {
        id,
        name,
        runtime,
        resolved_runtime,
        timeout_seconds,
        env_vars,
        source_url,
        trusted_checksum,
        ..
    } = provider;
    let provider_dir = cached_provider_dir(config_path, provider)?;
    let cache_root = crate::provider_cache_generation::resolve_cache_root(&provider_dir)?;
    let manifest = crate::remote_provider::load_cached_manifest(&cache_root)
        .map_err(|error| error.to_string())?;
    let manifest_bytes = std::fs::read(cache_root.join("provider.json"))
        .map_err(|error| format!("Unable to read cached provider manifest: {error}"))?;
    let source_name = crate::remote_provider::source_file_name(&manifest.entry);
    let source_bytes = std::fs::read(cache_root.join(&source_name))
        .map_err(|error| format!("Unable to read cached provider source: {error}"))?;
    let config_dir = config_path
        .parent()
        .ok_or_else(|| "Unable to resolve config directory".to_string())?;
    let effective_env = match crate::remote_provider_runner::resolve_execution_env(
        &manifest,
        env_vars,
        config_dir,
        global_proxy,
    ) {
        Ok(env) => env,
        // A missing/undecryptable secret must still produce a stable
        // fingerprint so the test can run and report the real error instead of
        // failing the fingerprint step. The marker is constant on purpose:
        // the resolver's error text depends on HashMap iteration order, and the
        // runner reports the real error separately.
        Err(_) => std::collections::BTreeMap::from([(
            "__QBWIN_ENV_RESOLUTION_ERROR__".to_string(),
            "unresolved".to_string(),
        )]),
    };
    let generation = crate::provider_cache_generation::resolve_current(&provider_dir)?
        .map(|current| current.generation)
        .unwrap_or_else(|| "legacy".to_string());
    let provider_dir_text = provider_dir.display().to_string();

    let parts = crate::execution_fingerprint::ExecutionParts {
        provider_id: id,
        name,
        cache_generation: &generation,
        manifest: &manifest_bytes,
        source: &source_bytes,
        effective_env: &effective_env,
        provider_dir: &provider_dir_text,
        runtime,
        resolved_runtime: resolved_runtime.as_deref(),
        timeout_seconds: *timeout_seconds,
        source_url,
        trusted_checksum: trusted_checksum.as_deref(),
    };
    Ok(crate::execution_fingerprint::compute(&parts))
}

#[tauri::command]
pub async fn get_provider_setup(
    app: AppHandle,
    provider_id: String,
) -> Result<ProviderSetupDescriptor, String> {
    let path = config_path_for_app(&app)?;
    tauri::async_runtime::spawn_blocking(move || get_provider_setup_from_path(&path, &provider_id))
        .await
        .map_err(|error| error.to_string())?
}

fn value_to_env_value(
    parameter: &ProviderParameter,
    value: &serde_json::Value,
) -> Result<String, String> {
    let kind = parameter_kind(parameter);
    let value = match value {
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Number(value) if kind == "number" => value.to_string(),
        _ => {
            return Err(format!(
                "Invalid value for Provider parameter {}",
                parameter.name
            ))
        }
    };
    if kind == "number"
        && value
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .is_none()
    {
        return Err(format!(
            "Provider parameter {} must be a number",
            parameter.name
        ));
    }
    if kind == "select" && !parameter.options.is_empty() && !parameter.options.contains(&value) {
        return Err(format!(
            "Invalid option for Provider parameter {}",
            parameter.name
        ));
    }
    if parameter.required && value.trim().is_empty() {
        return Err(format!("Provider parameter {} is required", parameter.name));
    }
    Ok(value)
}

pub fn save_provider_setup_to_path(
    config_path: &Path,
    request: SaveProviderSetupRequest,
) -> Result<ProviderSetupDescriptor, String> {
    if request.display_name.trim().is_empty() {
        return Err("Provider account name is required".to_string());
    }
    // Lock order: provider write lock, then the config write lock inside the
    // transaction. Secret files are written under both locks and restored on
    // any failure before the provider lock is released.
    let provider_id = request.provider_id.clone();
    crate::provider_op_lock::with_provider_write(&provider_id, || {
        save_provider_setup_under_locks(config_path, request)
    })
}

/// Runs inside the provider write lock. The config part is a read-modify-write
/// transaction; the managed-secret store is built from the config loaded
/// inside that transaction, so a concurrent storage-mode migration (W9)
/// cannot race this save into writing plaintext into an encrypted
/// installation.
fn save_provider_setup_under_locks(
    config_path: &Path,
    request: SaveProviderSetupRequest,
) -> Result<ProviderSetupDescriptor, String> {
    let config_dir = config_path
        .parent()
        .ok_or_else(|| "Unable to resolve config directory".to_string())?;

    // (parameter, previous bytes) for every secret touched by this save;
    // restored in reverse when the transaction fails.
    let mut secret_rollbacks: Vec<(String, Option<Vec<u8>>)> = Vec::new();
    let mut rollback_store: Option<ManagedSecretStore> = None;

    let outcome = crate::config::update_config_at_path(config_path, |config| {
        let index = config
            .providers
            .iter()
            .position(
                |provider| matches!(provider, ProviderConfig::Remote { id, .. } if id == &request.provider_id),
            )
            .ok_or_else(|| "Provider was not found".to_string())?;

        // Manifest load is local I/O; safe (and required) inside the
        // transaction so the descriptor always matches the committed config.
        let provider_dir =
            crate::remote_provider_commands::cached_provider_dir(config_path, &config.providers[index])?;
        let manifest = crate::remote_provider::load_cached_manifest(&provider_dir)
            .map_err(|error| error.to_string())?;

        let parameters = provider_parameters(&manifest);
        let parameters_by_name = parameters
            .iter()
            .map(|parameter| (parameter.name.as_str(), parameter))
            .collect::<HashMap<_, _>>();

        for name in request.values.keys().chain(request.secret_updates.keys()) {
            if !parameters_by_name.contains_key(name.as_str()) {
                return Err(format!("Unknown Provider parameter {name}"));
            }
        }
        for name in request.secret_updates.keys() {
            if parameter_kind(parameters_by_name[name.as_str()]) != "secret" {
                return Err(format!("Provider parameter {name} is not a secret"));
            }
        }

        let provider_id = request.provider_id.as_str();
        let store = ManagedSecretStore::new(config_dir, config.secrets_storage.clone());
        rollback_store = Some(store.clone());

        // Current bindings before any mutation, used for change detection.
        let bindings: HashMap<String, String> = match &config.providers[index] {
            ProviderConfig::Remote { env_vars, .. } => env_vars.clone(),
        };

        // D9 conservative changed detection: only a save that actually changes
        // the effective runtime inputs resets the verification state. A
        // rename-only save keeps `ready`; a default value that materializes
        // into envVars is a real change (the runner only ever sees envVars).
        let mut state_affecting_change = false;
        let mut updated_secret_placeholders: HashMap<String, String> = HashMap::new();

        for (name, update) in &request.secret_updates {
            let parameter = parameters_by_name[name.as_str()];
            if let Some(value) = update {
                if value.trim().is_empty() {
                    return Err(format!(
                        "Provider secret parameter {} cannot be empty; clear it instead",
                        parameter.name
                    ));
                }
            } else if parameter.required {
                // Clearing a required secret is allowed, but it leaves the
                // saved Provider unverified and disabled until the user
                // supplies it again.
            }

            let binding = bindings.get(name).map(String::as_str);
            let placeholder = managed_placeholder(provider_id, name);
            let is_managed_here = binding == Some(placeholder.as_str());
            // Only when the binding already points at this instance's managed
            // file does the file content represent the current value. A
            // decrypt failure maps to `None`, which classifies as "changed"
            // instead of masking a broken secret as a no-op.
            let stored_value = if is_managed_here {
                store.read_effective_value(provider_id, name).ok().flatten()
            } else {
                None
            };
            let secret_changed = match (binding, update) {
                (None, None) => false,
                (Some(_), None) => true,
                (Some(_), Some(value)) if is_managed_here => {
                    stored_value.as_deref() != Some(value.trim())
                }
                (Some(_), Some(_)) => true,
                (None, Some(_)) => true,
            };
            if secret_changed {
                state_affecting_change = true;
            }

            let previous = store.read_for_rollback(provider_id, name)?;
            secret_rollbacks.push((name.clone(), previous));
            match update {
                Some(_) if !secret_changed => {
                    // Same managed value: skip the rewrite (no pointless DPAPI
                    // churn) and keep the binding, which already is the
                    // placeholder.
                    updated_secret_placeholders.insert(name.clone(), placeholder);
                }
                Some(value) => {
                    let reference = store.write(provider_id, name, value)?;
                    updated_secret_placeholders.insert(name.clone(), reference.placeholder());
                }
                None => {
                    store.delete(provider_id, name)?;
                }
            }
        }

        let mut normalized_values = HashMap::new();
        for (name, value) in &request.values {
            let parameter = parameters_by_name[name.as_str()];
            if parameter_kind(parameter) == "secret" {
                return Err(format!(
                    "Provider secret parameter {name} must use the protected secret input"
                ));
            }
            let value = match value {
                serde_json::Value::Null if parameter.required => {
                    return Err(format!("Provider parameter {name} is required"));
                }
                serde_json::Value::Null => None,
                value => {
                    let value = value_to_env_value(parameter, value)?;
                    if value.is_empty() && !parameter.required {
                        None
                    } else {
                        Some(value)
                    }
                }
            };
            if bindings.get(name) != value.as_ref() {
                state_affecting_change = true;
            }
            normalized_values.insert(name.clone(), value);
        }

        let provider = &mut config.providers[index];
        let ProviderConfig::Remote {
            name,
            enabled,
            env_vars,
            setup_state,
            setup_last_tested_at,
            ..
        } = provider;
        *name = request.display_name.trim().to_string();
        for (parameter_name, update) in &request.secret_updates {
            match update {
                Some(_) => {
                    let Some(placeholder) = updated_secret_placeholders.get(parameter_name) else {
                        return Err(format!(
                            "Managed Provider secret reference is unavailable for {parameter_name}"
                        ));
                    };
                    env_vars.insert(parameter_name.clone(), placeholder.clone());
                }
                None => {
                    env_vars.remove(parameter_name);
                }
            }
        }
        for (parameter_name, value) in normalized_values {
            match value {
                Some(value) => {
                    env_vars.insert(parameter_name, value);
                }
                None => {
                    env_vars.remove(&parameter_name);
                }
            }
        }
        if state_affecting_change {
            *enabled = false;
            *setup_state = ProviderSetupState::Unverified;
            *setup_last_tested_at = None;
        }

        // Built from the in-transaction state, before the config commit, so a
        // descriptor failure cannot leave a committed config behind.
        setup_descriptor(config_path, &config.providers[index], &manifest)
    });

    match outcome {
        Ok(descriptor) => Ok(descriptor),
        Err(error) => {
            let mut rollback_failed = false;
            if let Some(store) = rollback_store.as_ref() {
                for (name, previous) in secret_rollbacks.iter().rev() {
                    if store
                        .restore_for_rollback(&request.provider_id, name, previous.as_deref())
                        .is_err()
                    {
                        rollback_failed = true;
                    }
                }
            }
            if rollback_failed {
                return Err(format!(
                    "Unable to save Provider setup: {error}; secret rollback failed"
                ));
            }
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn save_provider_setup(
    app: AppHandle,
    request: SaveProviderSetupRequest,
) -> Result<ProviderSetupDescriptor, String> {
    let path = config_path_for_app(&app)?;
    let descriptor =
        tauri::async_runtime::spawn_blocking(move || save_provider_setup_to_path(&path, request))
            .await
            .map_err(|error| error.to_string())??;
    crate::refresh_scheduler::signal_config_changed();
    crate::tray::refresh_tray_menu(&app)?;
    Ok(descriptor)
}

fn run_setup_test(
    config_path: &Path,
    provider: &ProviderConfig,
    global_proxy: Option<&crate::proxy::ProxyConfig>,
) -> Result<ProviderSnapshot, String> {
    let config_dir = config_path
        .parent()
        .ok_or_else(|| "Unable to resolve config directory".to_string())?;
    let ProviderConfig::Remote {
        id,
        name,
        provider_dir,
        runtime,
        resolved_runtime,
        trusted_checksum,
        version,
        timeout_seconds,
        env_vars,
        window_label_overrides,
        visible_window_ids,
        ..
    } = provider;
    let snapshots = run_remote_provider(
        id,
        name,
        provider_dir.as_deref(),
        runtime,
        resolved_runtime.as_deref(),
        trusted_checksum.as_deref(),
        version.as_deref(),
        global_proxy,
        *timeout_seconds,
        config_dir,
        env_vars,
        window_label_overrides,
        visible_window_ids,
        None,
    );
    snapshots
        .into_iter()
        .find(|snapshot| snapshot.id == *id)
        .ok_or_else(|| "Provider runner returned no result".to_string())
}

pub fn test_provider_setup_from_path(
    config_path: &Path,
    provider_id: &str,
) -> Result<ProviderSetupTestResult, String> {
    let loaded = load_or_create_config(config_path)?;
    let provider = loaded
        .config
        .providers
        .iter()
        .find(|provider| matches!(provider, ProviderConfig::Remote { id, .. } if id == provider_id))
        .cloned()
        .ok_or_else(|| "Provider was not found".to_string())?;
    let global_proxy = loaded.config.network_proxy.clone();

    // The test result only applies to the exact execution environment that was
    // validated (cache generation + resolved env + runtime + timeout + proxy).
    let fingerprint_before = crate::provider_op_lock::with_provider_read(provider_id, || {
        provider_execution_fingerprint(config_path, &provider, global_proxy.as_ref())
    })?;

    let mut snapshot = run_setup_test(config_path, &provider, global_proxy.as_ref())?;
    let success = snapshot.status != "error";
    // Setup results are intended for a short UI preview, never for retaining a
    // Provider's arbitrary metadata payload.
    snapshot.metadata = None;
    let error_category = if success {
        None
    } else {
        classify_provider_error(&snapshot)
    };

    crate::provider_op_lock::with_provider_write(provider_id, || {
        crate::config::update_config_at_path(config_path, |config| {
            let index = config
                .providers
                .iter()
                .position(
                    |provider| matches!(provider, ProviderConfig::Remote { id, .. } if id == provider_id),
                )
                .ok_or_else(|| "Provider was not found".to_string())?;
            let current_fingerprint = provider_execution_fingerprint(
                config_path,
                &config.providers[index],
                config.network_proxy.as_ref(),
            )?;
            if current_fingerprint != fingerprint_before {
                return Err(
                    "Provider configuration changed while testing; run the test again".to_string(),
                );
            }

            let ProviderConfig::Remote {
                enabled,
                setup_state,
                setup_last_tested_at,
                ..
            } = &mut config.providers[index];
            *enabled = success;
            *setup_state = if success {
                ProviderSetupState::Ready
            } else {
                ProviderSetupState::Unverified
            };
            *setup_last_tested_at = Some(Utc::now().to_rfc3339());
            Ok(())
        })
    })?;

    Ok(ProviderSetupTestResult {
        success,
        provider: Some(snapshot),
        error_category,
    })
}

#[tauri::command]
pub async fn test_provider_setup(
    app: AppHandle,
    provider_id: String,
) -> Result<ProviderSetupTestResult, String> {
    let path = config_path_for_app(&app)?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        test_provider_setup_from_path(&path, &provider_id)
    })
    .await
    .map_err(|error| error.to_string())??;
    crate::refresh_scheduler::signal_config_changed();
    crate::tray::refresh_tray_menu(&app)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        default_config, save_config_to_path, ProviderSetupState, SecretStorageMode,
    };

    fn setup_provider(temp: &tempfile::TempDir, with_required_secret: bool) -> ProviderConfig {
        let provider_dir = temp.path().join("providers/remote/setup-provider");
        std::fs::create_dir_all(&provider_dir).expect("provider dir");
        let parameters = if with_required_secret {
            serde_json::json!([
                { "name": "API_KEY", "kind": "secret", "required": true },
                { "name": "REGION", "kind": "string", "defaultValue": "CN" },
                { "name": "ACCOUNT_ID", "kind": "string", "required": false, "defaultValue": "${secret:LEGACY_ACCOUNT_ID}" },
                { "name": "RETRIES", "kind": "number", "required": false },
                { "name": "MODE", "kind": "select", "options": ["fast", "safe"] }
            ])
        } else {
            serde_json::json!([])
        };
        std::fs::write(
            provider_dir.join("provider.json"),
            serde_json::json!({
                "schemaVersion": 2,
                "id": "setup-provider",
                "displayName": "Setup Provider",
                "minAppVersion": "1.1.0",
                "runtime": "builtin-js",
                "entry": "provider.js",
                "requiredEnvVars": if with_required_secret { vec!["API_KEY"] } else { vec![] },
                "output": "provider-snapshot-v1",
                "permissions": if with_required_secret { vec!["env:API_KEY"] } else { vec![] },
                "parameters": parameters
            })
            .to_string(),
        )
        .expect("manifest");
        std::fs::write(
            provider_dir.join("provider.js"),
            "function main(qb) { return { status: 'ok', updatedAt: qb.now(), windows: [] }; }",
        )
        .expect("source");
        ProviderConfig::Remote {
            id: "setup-provider".to_string(),
            name: "Setup Provider".to_string(),
            enabled: false,
            version: Some("1.0.0".to_string()),
            manifest_url: "https://example.com/provider.json".to_string(),
            source_url: "https://example.com/provider.js".to_string(),
            provider_dir: Some(provider_dir),
            runtime: "builtin-js".to_string(),
            resolved_runtime: None,
            proxy_url: None,
            auto_update: false,
            update_interval_seconds: 3600,
            timeout_seconds: 30,
            trusted_checksum: None,
            installed_at: None,
            updated_at: None,
            last_checked_at: None,
            window_label_overrides: HashMap::new(),
            visible_window_ids: Vec::new(),
            show_in_tray: true,
            notifications_enabled: true,
            show_in_screenshot: true,
            env_vars: if with_required_secret {
                HashMap::from([
                    (
                        "CUSTOM_SETTING".to_string(),
                        "${env:CUSTOM_SETTING}".to_string(),
                    ),
                    ("API_KEY".to_string(), "${secret:LEGACY_KEY}".to_string()),
                    (
                        "ACCOUNT_ID".to_string(),
                        "${secret:LEGACY_ACCOUNT_ID}".to_string(),
                    ),
                ])
            } else {
                HashMap::new()
            },
            setup_state: ProviderSetupState::Pending,
            setup_last_tested_at: None,
        }
    }

    fn config_path_with_provider(
        temp: &tempfile::TempDir,
        provider: ProviderConfig,
    ) -> std::path::PathBuf {
        let path = temp.path().join("config.quotaBarWin.json");
        let mut config = default_config();
        // These tests exercise rollback and config-merge semantics; dedicated
        // tests below cover the encrypted storage mode.
        config.secrets_storage = SecretStorageMode::Plaintext;
        config.providers.push(provider);
        save_config_to_path(&path, &config).expect("save config");
        path
    }

    #[test]
    fn descriptor_never_returns_secret_values_and_identifies_legacy_source() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = config_path_with_provider(&temp, setup_provider(&temp, true));
        let legacy_secret_dir = temp.path().join("secrets");
        std::fs::create_dir_all(&legacy_secret_dir).expect("legacy secret dir");
        std::fs::write(legacy_secret_dir.join("LEGACY_KEY.txt"), "legacy-secret")
            .expect("legacy secret");

        let descriptor = get_provider_setup_from_path(&path, "setup-provider").expect("descriptor");
        let secret = descriptor
            .fields
            .iter()
            .find(|field| field.name == "API_KEY")
            .expect("secret field");

        assert_eq!(secret.source, ProviderParameterSource::SecretFile);
        assert!(secret.configured);
        assert!(secret.value.is_none());
        let account_id = descriptor
            .fields
            .iter()
            .find(|field| field.name == "ACCOUNT_ID")
            .expect("account id field");
        assert!(account_id.value.is_none());
        assert!(descriptor.has_unknown_env_vars);
        assert_eq!(
            descriptor.unknown_env_vars,
            vec!["CUSTOM_SETTING".to_string()]
        );
    }

    #[test]
    fn descriptor_marks_a_missing_legacy_secret_as_not_configured() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = config_path_with_provider(&temp, setup_provider(&temp, true));

        let descriptor = get_provider_setup_from_path(&path, "setup-provider").expect("descriptor");
        let secret = descriptor
            .fields
            .iter()
            .find(|field| field.name == "API_KEY")
            .expect("secret field");

        assert_eq!(secret.source, ProviderParameterSource::SecretFile);
        assert!(!secret.configured);
        assert!(secret.value.is_none());
    }

    #[test]
    fn save_setup_merges_declared_fields_and_separates_secret_from_config() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = config_path_with_provider(&temp, setup_provider(&temp, true));
        let descriptor = save_provider_setup_to_path(
            &path,
            SaveProviderSetupRequest {
                provider_id: "setup-provider".to_string(),
                display_name: "Work account".to_string(),
                values: HashMap::from([
                    ("REGION".to_string(), serde_json::json!("US")),
                    ("ACCOUNT_ID".to_string(), serde_json::Value::Null),
                    ("RETRIES".to_string(), serde_json::json!(3)),
                    ("MODE".to_string(), serde_json::json!("safe")),
                ]),
                secret_updates: HashMap::from([(
                    "API_KEY".to_string(),
                    Some("test-secret".to_string()),
                )]),
            },
        )
        .expect("save setup");

        assert_eq!(descriptor.setup_state, ProviderSetupState::Unverified);
        let saved = std::fs::read_to_string(&path).expect("config contents");
        assert!(!saved.contains("test-secret"));
        assert!(saved.contains("${secret:providers/setup-provider/API_KEY}"));
        let loaded = load_or_create_config(&path).expect("load config");
        let ProviderConfig::Remote {
            name,
            enabled,
            env_vars,
            setup_state,
            ..
        } = &loaded.config.providers[0];
        assert_eq!(name, "Work account");
        assert!(!enabled);
        assert_eq!(*setup_state, ProviderSetupState::Unverified);
        assert_eq!(
            env_vars.get("CUSTOM_SETTING"),
            Some(&"${env:CUSTOM_SETTING}".to_string())
        );
        assert_eq!(env_vars.get("MODE"), Some(&"safe".to_string()));
        assert!(!env_vars.contains_key("ACCOUNT_ID"));
        assert_eq!(
            std::fs::read_to_string(
                temp.path()
                    .join("secrets/providers/setup-provider/API_KEY.txt")
            )
            .expect("managed secret"),
            "test-secret"
        );
    }

    #[test]
    fn multiple_secrets_are_saved_replaced_and_cleared_independently() {
        let temp = tempfile::tempdir().expect("temp dir");
        let provider = setup_provider(&temp, true);
        let manifest_path = temp.path().join("providers/remote/setup-provider/provider.json");
        let mut manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&manifest_path).expect("manifest"),
        ).expect("parse manifest");
        manifest["parameters"].as_array_mut().unwrap().push(serde_json::json!({
            "name": "SECOND_KEY", "kind": "secret", "required": false,
            "defaultValue": "${secret:UNUSED_DEFAULT}"
        }));
        std::fs::write(&manifest_path, manifest.to_string()).expect("write manifest");
        let path = config_path_with_provider(&temp, provider);
        let request = |secret_updates| SaveProviderSetupRequest {
            provider_id: "setup-provider".to_string(),
            display_name: "Work".to_string(),
            values: HashMap::new(),
            secret_updates,
        };
        let store = ManagedSecretStore::new(temp.path(), SecretStorageMode::Plaintext);
        save_provider_setup_to_path(&path, request(HashMap::from([
            ("API_KEY".to_string(), Some("first-value".to_string())),
            ("SECOND_KEY".to_string(), Some("second-value".to_string())),
        ]))).expect("save both keys");
        for (name, expected) in [("API_KEY", "first-value"), ("SECOND_KEY", "second-value")] {
            assert_eq!(
                crate::provider_secret_commands::reveal_managed_secret_for_path(
                    &path, "setup-provider", name,
                ).expect("reveal key").value,
                expected,
            );
        }
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(!saved.contains("first-value"));
        assert!(!saved.contains("second-value"));

        // Both writes must roll back if a later regular-field validation fails.
        let mut invalid = request(HashMap::from([
            ("API_KEY".to_string(), Some("discard-first".to_string())),
            ("SECOND_KEY".to_string(), Some("discard-second".to_string())),
        ]));
        invalid.values.insert("RETRIES".to_string(), serde_json::json!("invalid-number"));
        assert!(save_provider_setup_to_path(&path, invalid).is_err());
        assert_eq!(store.read_effective_value("setup-provider", "API_KEY").unwrap().as_deref(), Some("first-value"));
        assert_eq!(store.read_effective_value("setup-provider", "SECOND_KEY").unwrap().as_deref(), Some("second-value"));

        let descriptor = save_provider_setup_to_path(&path, request(HashMap::from([
            ("API_KEY".to_string(), Some("replacement".to_string())),
            ("SECOND_KEY".to_string(), None),
        ]))).expect("replace first and clear second");
        assert_eq!(store.read_effective_value("setup-provider", "API_KEY").unwrap().as_deref(), Some("replacement"));
        assert!(!store.exists("setup-provider", "SECOND_KEY").unwrap());
        assert!(!descriptor.fields.iter().find(|field| field.name == "SECOND_KEY").unwrap().configured);

        // A secret default alone never creates a binding, even after another save.
        save_provider_setup_to_path(&path, request(HashMap::new())).expect("keep keys");
        let loaded = load_or_create_config(&path).unwrap();
        let ProviderConfig::Remote { env_vars, .. } = &loaded.config.providers[0];
        assert!(!env_vars.contains_key("SECOND_KEY"));
        assert!(!store.exists("setup-provider", "SECOND_KEY").unwrap());
    }

    #[test]
    fn invalid_regular_value_is_rejected_before_secret_changes() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = config_path_with_provider(&temp, setup_provider(&temp, true));
        let store = ManagedSecretStore::new(temp.path(), SecretStorageMode::Plaintext);
        store
            .write("setup-provider", "API_KEY", "previous-secret")
            .expect("write previous secret");
        let original_config = std::fs::read_to_string(&path).expect("original config");

        let error = save_provider_setup_to_path(
            &path,
            SaveProviderSetupRequest {
                provider_id: "setup-provider".to_string(),
                display_name: "Work account".to_string(),
                values: HashMap::from([("RETRIES".to_string(), serde_json::json!("not-a-number"))]),
                secret_updates: HashMap::from([(
                    "API_KEY".to_string(),
                    Some("replacement-secret".to_string()),
                )]),
            },
        )
        .expect_err("invalid regular value");

        assert!(error.contains("RETRIES"));
        assert!(!error.contains("replacement-secret"));
        assert_eq!(
            std::fs::read_to_string(
                temp.path()
                    .join("secrets/providers/setup-provider/API_KEY.txt")
            )
            .expect("managed secret"),
            "previous-secret"
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("unchanged config"),
            original_config
        );
    }

    #[test]
    fn config_save_failure_restores_previous_secret() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = config_path_with_provider(&temp, setup_provider(&temp, true));
        let store = ManagedSecretStore::new(temp.path(), SecretStorageMode::Plaintext);
        store
            .write("setup-provider", "API_KEY", "previous-secret")
            .expect("write previous secret");
        let original_config = std::fs::read_to_string(&path).expect("original config");

        crate::config::set_config_write_failure(true);
        let error = std::panic::catch_unwind(|| {
            save_provider_setup_to_path(
                &path,
                SaveProviderSetupRequest {
                    provider_id: "setup-provider".to_string(),
                    display_name: "Work account".to_string(),
                    values: HashMap::new(),
                    secret_updates: HashMap::from([(
                        "API_KEY".to_string(),
                        Some("replacement-secret".to_string()),
                    )]),
                },
            )
        });
        crate::config::set_config_write_failure(false);
        let error = error
            .expect("save must return an error, not panic")
            .expect_err("config save must fail");

        assert!(error.contains("injected config write failure"));
        assert!(!error.contains("replacement-secret"));
        assert_eq!(
            std::fs::read_to_string(
                temp.path()
                    .join("secrets/providers/setup-provider/API_KEY.txt")
            )
            .expect("restored managed secret"),
            "previous-secret"
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("unchanged config"),
            original_config
        );
    }

    fn mark_ready(provider: &mut ProviderConfig) {
        let ProviderConfig::Remote {
            enabled,
            setup_state,
            setup_last_tested_at,
            ..
        } = provider;
        *enabled = true;
        *setup_state = ProviderSetupState::Ready;
        *setup_last_tested_at = Some("2026-11-01T00:00:00Z".to_string());
    }

    fn load_setup_provider(path: &std::path::Path) -> ProviderConfig {
        load_or_create_config(path)
            .expect("load config")
            .config
            .providers
            .into_iter()
            .find(|provider| matches!(provider, ProviderConfig::Remote { id, .. } if id == "setup-provider"))
            .expect("setup provider")
    }

    fn ready_provider_with_managed_secret(temp: &tempfile::TempDir) -> std::path::PathBuf {
        let mut provider = setup_provider(temp, true);
        mark_ready(&mut provider);
        let store = ManagedSecretStore::new(temp.path(), SecretStorageMode::Plaintext);
        store
            .write("setup-provider", "API_KEY", "managed-secret")
            .expect("write managed secret");
        if let ProviderConfig::Remote { env_vars, .. } = &mut provider {
            env_vars.insert(
                "API_KEY".to_string(),
                "${secret:providers/setup-provider/API_KEY}".to_string(),
            );
        }
        config_path_with_provider(temp, provider)
    }

    #[test]
    fn rename_only_save_keeps_the_verified_state() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = ready_provider_with_managed_secret(&temp);

        save_provider_setup_to_path(
            &path,
            SaveProviderSetupRequest {
                provider_id: "setup-provider".to_string(),
                display_name: "Renamed account".to_string(),
                values: HashMap::new(),
                secret_updates: HashMap::new(),
            },
        )
        .expect("rename-only save");

        match load_setup_provider(&path) {
            ProviderConfig::Remote {
                name,
                enabled,
                setup_state,
                setup_last_tested_at,
                ..
            } => {
                assert_eq!(name, "Renamed account");
                assert!(enabled, "a rename must not disable the Provider");
                assert_eq!(setup_state, ProviderSetupState::Ready);
                assert_eq!(
                    setup_last_tested_at.as_deref(),
                    Some("2026-11-01T00:00:00Z")
                );
            }
        }
    }

    #[test]
    fn same_value_managed_secret_save_is_a_noop() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = ready_provider_with_managed_secret(&temp);
        let secret_file = temp
            .path()
            .join("secrets/providers/setup-provider/API_KEY.txt");
        let bytes_before = std::fs::read(&secret_file).expect("secret bytes before");

        save_provider_setup_to_path(
            &path,
            SaveProviderSetupRequest {
                provider_id: "setup-provider".to_string(),
                display_name: "Setup Provider".to_string(),
                values: HashMap::new(),
                secret_updates: HashMap::from([(
                    "API_KEY".to_string(),
                    Some("managed-secret".to_string()),
                )]),
            },
        )
        .expect("same-value save");

        match load_setup_provider(&path) {
            ProviderConfig::Remote {
                enabled,
                setup_state,
                setup_last_tested_at,
                ..
            } => {
                assert!(enabled, "a same-value secret save must not disable");
                assert_eq!(setup_state, ProviderSetupState::Ready);
                assert!(setup_last_tested_at.is_some());
            }
        }
        assert_eq!(
            std::fs::read(&secret_file).expect("secret bytes after"),
            bytes_before,
            "an unchanged secret must not be rewritten"
        );
    }

    #[test]
    fn clearing_a_managed_secret_resets_and_removes_the_binding() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = ready_provider_with_managed_secret(&temp);

        save_provider_setup_to_path(
            &path,
            SaveProviderSetupRequest {
                provider_id: "setup-provider".to_string(),
                display_name: "Setup Provider".to_string(),
                values: HashMap::new(),
                secret_updates: HashMap::from([("API_KEY".to_string(), None)]),
            },
        )
        .expect("clearing save");

        match load_setup_provider(&path) {
            ProviderConfig::Remote {
                enabled,
                setup_state,
                setup_last_tested_at,
                env_vars,
                ..
            } => {
                assert!(!enabled, "clearing a credential must disable");
                assert_eq!(setup_state, ProviderSetupState::Unverified);
                assert!(setup_last_tested_at.is_none());
                assert!(!env_vars.contains_key("API_KEY"));
            }
        }
        assert!(!temp
            .path()
            .join("secrets/providers/setup-provider/API_KEY.txt")
            .exists());
    }

    #[test]
    fn materializing_a_default_value_resets_the_state() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut provider = setup_provider(&temp, true);
        mark_ready(&mut provider);
        // REGION has defaultValue "CN" and is absent from envVars; the runner
        // only sees envVars, so materializing it is a real runtime change.
        let path = config_path_with_provider(&temp, provider);

        save_provider_setup_to_path(
            &path,
            SaveProviderSetupRequest {
                provider_id: "setup-provider".to_string(),
                display_name: "Setup Provider".to_string(),
                values: HashMap::from([("REGION".to_string(), serde_json::json!("CN"))]),
                secret_updates: HashMap::new(),
            },
        )
        .expect("materializing save");

        match load_setup_provider(&path) {
            ProviderConfig::Remote {
                enabled,
                setup_state,
                env_vars,
                ..
            } => {
                assert!(!enabled, "materializing a default must disable");
                assert_eq!(setup_state, ProviderSetupState::Unverified);
                assert_eq!(
                    env_vars.get("REGION").map(String::as_str),
                    Some("CN")
                );
            }
        }
    }

    #[test]
    fn save_setup_writes_dpapi_encrypted_secret_when_encryption_enabled() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.quotaBarWin.json");
        let mut config = default_config();
        config.secrets_storage = SecretStorageMode::Encrypted;
        config.providers.push(setup_provider(&temp, true));
        save_config_to_path(&path, &config).expect("save config");

        save_provider_setup_to_path(
            &path,
            SaveProviderSetupRequest {
                provider_id: "setup-provider".to_string(),
                display_name: "Work account".to_string(),
                values: HashMap::new(),
                secret_updates: HashMap::from([(
                    "API_KEY".to_string(),
                    Some("encrypted-secret".to_string()),
                )]),
            },
        )
        .expect("save setup");

        let bytes = std::fs::read(
            temp.path()
                .join("secrets/providers/setup-provider/API_KEY.txt"),
        )
        .expect("managed secret payload");
        assert!(crate::secret_encryption::is_encrypted_payload(&bytes));
        assert!(!bytes
            .windows(16)
            .any(|window| *window == b"encrypted-secret"[..]));
        assert_eq!(
            crate::secret_encryption::decrypt_managed_secret(
                &bytes,
                "providers/setup-provider/API_KEY"
            )
            .expect("decrypt"),
            "encrypted-secret"
        );
    }

    #[test]
    fn setup_test_reuses_runner_while_provider_is_disabled_and_enables_on_success() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = config_path_with_provider(&temp, setup_provider(&temp, false));

        let result = test_provider_setup_from_path(&path, "setup-provider").expect("test setup");

        assert!(result.success);
        assert_eq!(
            result
                .provider
                .as_ref()
                .map(|provider| provider.status.as_str()),
            Some("ok")
        );
        let loaded = load_or_create_config(&path).expect("load config");
        let ProviderConfig::Remote {
            enabled,
            setup_state,
            setup_last_tested_at,
            ..
        } = &loaded.config.providers[0];
        assert!(*enabled);
        assert_eq!(*setup_state, ProviderSetupState::Ready);
        assert!(setup_last_tested_at.is_some());
    }

    #[test]
    fn failed_setup_test_keeps_saved_config_and_provider_disabled() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = config_path_with_provider(&temp, setup_provider(&temp, true));

        let result = test_provider_setup_from_path(&path, "setup-provider").expect("test setup");

        assert!(!result.success);
        let loaded = load_or_create_config(&path).expect("load config");
        let ProviderConfig::Remote {
            enabled,
            setup_state,
            setup_last_tested_at,
            env_vars,
            ..
        } = &loaded.config.providers[0];
        assert!(!enabled);
        assert_eq!(*setup_state, ProviderSetupState::Unverified);
        assert!(setup_last_tested_at.is_some());
        assert_eq!(
            env_vars.get("API_KEY"),
            Some(&"${secret:LEGACY_KEY}".to_string())
        );
    }
}
