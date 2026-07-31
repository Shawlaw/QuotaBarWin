use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::{
    config::{
        config_path_for_app, load_or_create_config, resolve_secret_value, save_config_to_path,
        ProviderConfig, ProviderSetupState,
    },
    managed_secret_store::ManagedSecretStore,
    provider_error::{classify_provider_error, ProviderErrorCategory},
    quota::ProviderSnapshot,
    remote_provider::{load_cached_manifest, ProviderManifest, ProviderParameter},
    remote_provider_commands::remote_provider_dir,
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

fn cached_provider_dir(
    path: &Path,
    provider: &ProviderConfig,
) -> Result<std::path::PathBuf, String> {
    match provider {
        ProviderConfig::Remote {
            provider_dir: Some(provider_dir),
            ..
        } => Ok(provider_dir.clone()),
        ProviderConfig::Remote { id, .. } => remote_provider_dir(path, id),
    }
}

fn provider_parameters(manifest: &ProviderManifest) -> Vec<ProviderParameter> {
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

fn parameter_kind(parameter: &ProviderParameter) -> &str {
    match parameter.kind.as_deref() {
        Some("secret") => "secret",
        Some("number") => "number",
        Some("select") => "select",
        _ => "string",
    }
}

fn managed_placeholder(provider_id: &str, parameter_name: &str) -> String {
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

fn setup_descriptor(
    config_path: &Path,
    provider: &ProviderConfig,
    manifest: &ProviderManifest,
) -> Result<ProviderSetupDescriptor, String> {
    let config_dir = config_path
        .parent()
        .ok_or_else(|| "Unable to resolve config directory".to_string())?;
    let store = ManagedSecretStore::new(config_dir);
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

    Ok(ProviderSetupDescriptor {
        provider_id: id.clone(),
        provider_type: manifest.id.clone(),
        display_name: name.clone(),
        setup_state: setup_state.clone(),
        fields,
        has_unknown_env_vars: env_vars
            .keys()
            .any(|name| !parameter_names.contains(name.as_str())),
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

fn restore_secret_changes(
    store: &ManagedSecretStore,
    provider_id: &str,
    previous_values: &[(String, Option<String>)],
) -> Result<(), String> {
    for (parameter_name, previous_value) in previous_values.iter().rev() {
        store.restore_for_rollback(provider_id, parameter_name, previous_value.as_deref())?;
    }
    Ok(())
}

pub fn save_provider_setup_to_path(
    config_path: &Path,
    request: SaveProviderSetupRequest,
) -> Result<ProviderSetupDescriptor, String> {
    save_provider_setup_to_path_with(config_path, request, save_config_to_path)
}

fn save_provider_setup_to_path_with<F>(
    config_path: &Path,
    request: SaveProviderSetupRequest,
    save_config: F,
) -> Result<ProviderSetupDescriptor, String>
where
    F: FnOnce(&Path, &crate::config::AppConfig) -> Result<(), String>,
{
    if request.display_name.trim().is_empty() {
        return Err("Provider account name is required".to_string());
    }
    let (mut loaded, provider_index, manifest) =
        load_provider_and_manifest(config_path, &request.provider_id)?;
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
        normalized_values.insert(name.clone(), value);
    }

    let config_dir = config_path
        .parent()
        .ok_or_else(|| "Unable to resolve config directory".to_string())?;
    let store = ManagedSecretStore::new(config_dir);
    let provider_id = request.provider_id.as_str();
    let mut previous_secret_values = Vec::new();
    let mut updated_secret_placeholders = HashMap::new();
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
            // Clearing a required secret is allowed, but it leaves the saved
            // Provider unverified and disabled until the user supplies it again.
        }
        let previous_value = store.read_for_rollback(provider_id, name)?;
        let write_result = match update {
            Some(value) => store
                .write(provider_id, name, value)
                .map(|reference| Some(reference.placeholder())),
            None => store.delete(provider_id, name).map(|_| None),
        };
        match write_result {
            Ok(Some(placeholder)) => {
                updated_secret_placeholders.insert(name.clone(), placeholder);
            }
            Ok(None) => {}
            Err(error) => {
                let _ = restore_secret_changes(&store, provider_id, &previous_secret_values);
                return Err(error);
            }
        }
        previous_secret_values.push((name.clone(), previous_value));
    }

    let provider = &mut loaded.config.providers[provider_index];
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
                    let _ = restore_secret_changes(&store, provider_id, &previous_secret_values);
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
    *enabled = false;
    *setup_state = ProviderSetupState::Unverified;
    *setup_last_tested_at = None;

    if let Err(error) = save_config(config_path, &loaded.config) {
        let restore_result = restore_secret_changes(&store, provider_id, &previous_secret_values);
        return match restore_result {
            Ok(()) => Err(error),
            Err(_) => Err(format!(
                "Unable to save Provider setup: {error}; secret rollback failed"
            )),
        };
    }
    setup_descriptor(
        config_path,
        &loaded.config.providers[provider_index],
        &manifest,
    )
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
    let mut loaded = load_or_create_config(config_path)?;
    let provider_index = loaded
        .config
        .providers
        .iter()
        .position(
            |provider| matches!(provider, ProviderConfig::Remote { id, .. } if id == provider_id),
        )
        .ok_or_else(|| "Provider was not found".to_string())?;
    let provider = loaded.config.providers[provider_index].clone();
    let mut snapshot =
        run_setup_test(config_path, &provider, loaded.config.network_proxy.as_ref())?;
    let success = snapshot.status != "error";
    // Setup results are intended for a short UI preview, never for retaining a
    // Provider's arbitrary metadata payload.
    snapshot.metadata = None;
    let error_category = if success {
        None
    } else {
        classify_provider_error(&snapshot)
    };

    let ProviderConfig::Remote {
        enabled,
        setup_state,
        setup_last_tested_at,
        ..
    } = &mut loaded.config.providers[provider_index];
    *enabled = success;
    *setup_state = if success {
        ProviderSetupState::Ready
    } else {
        ProviderSetupState::Unverified
    };
    *setup_last_tested_at = Some(Utc::now().to_rfc3339());
    save_config_to_path(config_path, &loaded.config)?;

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
    use crate::config::{default_config, save_config_to_path, ProviderSetupState};

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
    fn invalid_regular_value_is_rejected_before_secret_changes() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = config_path_with_provider(&temp, setup_provider(&temp, true));
        let store = ManagedSecretStore::new(temp.path());
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
        let store = ManagedSecretStore::new(temp.path());
        store
            .write("setup-provider", "API_KEY", "previous-secret")
            .expect("write previous secret");
        let original_config = std::fs::read_to_string(&path).expect("original config");

        let error = save_provider_setup_to_path_with(
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
            |_, _| Err("synthetic config failure".to_string()),
        )
        .expect_err("config save must fail");

        assert!(error.contains("synthetic config failure"));
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
