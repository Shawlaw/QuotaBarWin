//! Provider secret commands: adopt a literal credential into the managed,
//! instance-isolated secret store.
//!
//! Design: `private-doc/p0/02-execution-fingerprint.md` (§4). P0 only converts
//! plaintext bindings; external sources (`${secret:}`, `${env:}`, `${file:}`)
//! must be re-entered through Replace.
#![allow(dead_code)]

use std::path::Path;

use serde::Serialize;
use tauri::AppHandle;

use crate::config::{
    config_path_for_app, load_or_create_config, update_config_at_path_with, ProviderConfig,
    DEFAULT_LOG_MAX_BYTES,
};
use crate::config_merge::MutationFailure;
use crate::logger::{log_path_for_config_path, write_structured_log, LogLevel};
use crate::managed_secret_store::ManagedSecretStore;
use crate::provider_setup::{
    get_provider_setup_from_path, managed_placeholder, parameter_kind, provider_parameters,
    ProviderSetupDescriptor,
};
use crate::remote_provider::load_cached_manifest;
use crate::remote_provider_commands::cached_provider_dir;

/// Best-effort audit entry for a credential access command. Never includes the
/// secret value; `write_structured_log` redacts the message again on the way
/// out.
fn audit_secret_access(
    config_path: &Path,
    event: &str,
    provider_id: &str,
    parameter_name: &str,
    result: &str,
) {
    let _ = write_structured_log(
        &log_path_for_config_path(config_path),
        LogLevel::Info,
        "secret.access",
        &format!(
            "event={event} provider={provider_id} parameter={parameter_name} result={result}"
        ),
        DEFAULT_LOG_MAX_BYTES,
    );
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevealSecretResult {
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdoptSecretResult {
    pub descriptor: ProviderSetupDescriptor,
    pub adopted: bool,
}

fn validation(message: impl Into<String>) -> MutationFailure {
    MutationFailure::Validation {
        message: message.into(),
    }
}

fn io(error: impl std::fmt::Display) -> MutationFailure {
    MutationFailure::Io {
        message: error.to_string(),
    }
}

fn failure_message(failure: &MutationFailure) -> String {
    match failure {
        MutationFailure::Validation { message }
        | MutationFailure::Conflict { message, .. }
        | MutationFailure::Stale { message }
        | MutationFailure::Uncertain { message, .. }
        | MutationFailure::Io { message } => message.clone(),
    }
}

/// Rollback context for an in-flight adopt. While `armed`, a drop (the panic
/// path) restores the previous secret bytes best-effort; the explicit
/// [`SecretRollback::rollback`] additionally reports restore failures to the
/// caller instead of swallowing them.
struct SecretRollback {
    store: ManagedSecretStore,
    provider_id: String,
    parameter_name: String,
    log_path: std::path::PathBuf,
    previous: Option<Vec<u8>>,
    armed: bool,
}

impl SecretRollback {
    /// Disarms without restoring — used on success, where the adopted value
    /// must stay on disk.
    fn disarm(mut self) {
        self.armed = false;
    }

    fn rollback(&mut self) -> Result<(), String> {
        self.armed = false;
        self.store.restore_for_rollback(
            &self.provider_id,
            &self.parameter_name,
            self.previous.as_deref(),
        )
    }
}

impl Drop for SecretRollback {
    fn drop(&mut self) {
        if self.armed {
            // Panic path only: attempt the restore and log the outcome without
            // ever including the secret value.
            let _ = self.store.restore_for_rollback(
                &self.provider_id,
                &self.parameter_name,
                self.previous.as_deref(),
            );
            let _ = write_structured_log(
                &self.log_path,
                LogLevel::Warn,
                "secret.access",
                &format!(
                    "{{\"event\":\"adopt\",\"provider\":\"{}\",\"parameter\":\"{}\",\"result\":\"rollback attempted after panic\"}}",
                    self.provider_id, self.parameter_name
                ),
                DEFAULT_LOG_MAX_BYTES,
            );
        }
    }
}

/// Adopts `parameter_name` for `provider_id` when its binding is plaintext.
///
/// Everything — manifest validation, the managed-secret write, the read-back
/// verification, the binding rewrite, the config commit, the rollback on
/// failure and the descriptor construction — happens under the provider write
/// lock, and the config part under the config write lock as well. A failure
/// restores the previous secret bytes while the lock is still held and leaves
/// the config untouched.
pub fn adopt_parameter_for_path(
    config_path: &Path,
    provider_id: &str,
    parameter_name: &str,
) -> Result<AdoptSecretResult, MutationFailure> {
    let config_dir = config_path
        .parent()
        .ok_or_else(|| io("Unable to resolve config directory"))?
        .to_path_buf();

    crate::provider_op_lock::with_provider_write(provider_id, || {
        let mut rollback: Option<SecretRollback> = None;

        let outcome = update_config_at_path_with(config_path, |config| {
            let index = config
                .providers
                .iter()
                .position(
                    |provider| matches!(provider, ProviderConfig::Remote { id, .. } if id == provider_id),
                )
                .ok_or_else(|| validation("Provider was not found"))?;

            let provider_dir =
                cached_provider_dir(config_path, &config.providers[index]).map_err(io)?;
            let manifest = load_cached_manifest(&provider_dir).map_err(|error| {
                validation(format!("Unable to read the Provider manifest: {error}"))
            })?;
            let parameters = provider_parameters(&manifest);
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.name == parameter_name)
                .ok_or_else(|| validation("Unknown Provider parameter"))?;
            if parameter_kind(parameter) != "secret" {
                return Err(validation("Provider parameter is not a secret"));
            }

            // The store mode comes from the config loaded inside this
            // transaction, so a concurrent storage-mode migration (W9) can no
            // longer race an adopt into writing plaintext into an encrypted
            // installation.
            let store = ManagedSecretStore::new(&config_dir, config.secrets_storage.clone());
            let placeholder = managed_placeholder(provider_id, parameter_name);
            let ProviderConfig::Remote { env_vars, .. } = &mut config.providers[index];
            let current = env_vars.get(parameter_name).cloned();

            if current.as_deref() == Some(placeholder.as_str()) {
                if store.exists(provider_id, parameter_name).map_err(io)? {
                    return Ok(false);
                }
                return Err(validation(
                    "Managed secret file is missing; re-enter the secret",
                ));
            }

            let raw = current
                .ok_or_else(|| validation("Provider parameter has no binding to convert"))?;
            if raw.contains("${") {
                return Err(validation(
                    "Only plaintext values can be converted; use Replace to re-enter this credential",
                ));
            }
            if raw.is_empty() || raw != raw.trim() {
                return Err(validation(
                    "Provider parameter has leading/trailing whitespace or is empty; use Replace to re-enter it",
                ));
            }

            let previous = store
                .read_for_rollback(provider_id, parameter_name)
                .map_err(io)?;
            rollback = Some(SecretRollback {
                store: store.clone(),
                provider_id: provider_id.to_string(),
                parameter_name: parameter_name.to_string(),
                log_path: log_path_for_config_path(config_path),
                previous,
                armed: true,
            });

            store.write(provider_id, parameter_name, &raw).map_err(io)?;
            let stored = store
                .read_effective_value(provider_id, parameter_name)
                .map_err(io)?;
            if stored.as_deref() != Some(raw.as_str()) {
                return Err(MutationFailure::Uncertain {
                    message: "Adopted secret could not be verified".to_string(),
                    rollback_failed: false,
                });
            }

            env_vars.insert(parameter_name.to_string(), placeholder);
            Ok(true)
        });

        match outcome {
            Ok(adopted) => {
                // Keep the adopted value: disarm so the Drop guard does not
                // restore the previous bytes.
                if let Some(rollback) = rollback.take() {
                    rollback.disarm();
                }
                audit_secret_access(config_path, "adopt", provider_id, parameter_name, "ok");
                // Built while the provider lock is still held, so the
                // descriptor reflects the committed config and the current
                // cache generation.
                let descriptor =
                    get_provider_setup_from_path(config_path, provider_id).map_err(io)?;
                Ok(AdoptSecretResult {
                    descriptor,
                    adopted,
                })
            }
            Err(error) => {
                audit_secret_access(config_path, "adopt", provider_id, parameter_name, "failed");
                match rollback {
                    Some(mut rollback) => match rollback.rollback() {
                        Ok(()) => Err(error),
                        Err(_) => Err(MutationFailure::Uncertain {
                            message: format!(
                                "{}; restoring the previous secret failed",
                                failure_message(&error)
                            ),
                            rollback_failed: true,
                        }),
                    },
                    None => Err(error),
                }
            }
        }
    })
}

#[tauri::command]
pub async fn adopt_parameter_as_managed_secret(
    app: AppHandle,
    provider_id: String,
    parameter_name: String,
) -> Result<AdoptSecretResult, MutationFailure> {
    let path = config_path_for_app(&app).map_err(io)?;
    tauri::async_runtime::spawn_blocking(move || {
        adopt_parameter_for_path(&path, &provider_id, &parameter_name)
    })
    .await
    .map_err(io)?
}

/// Reads a managed secret for an already-loaded config snapshot.
///
/// Only `${secret:providers/<id>/<parameter>}` bindings are eligible; literal
/// values and external sources are not exposed through this path.
fn read_managed_secret_value(
    config_path: &Path,
    provider_id: &str,
    parameter_name: &str,
) -> Result<String, MutationFailure> {
    let config_dir = config_path
        .parent()
        .ok_or_else(|| io("Unable to resolve config directory"))?;
    let loaded = load_or_create_config(config_path)?;
    let provider = loaded
        .config
        .providers
        .iter()
        .find(|provider| matches!(provider, ProviderConfig::Remote { id, .. } if id == provider_id))
        .ok_or_else(|| validation("Provider was not found"))?;
    let provider_dir = cached_provider_dir(config_path, provider).map_err(io)?;
    let manifest = load_cached_manifest(&provider_dir)
        .map_err(|error| validation(format!("Unable to read the Provider manifest: {error}")))?;
    let parameters = provider_parameters(&manifest);
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.name == parameter_name)
        .ok_or_else(|| validation("Unknown Provider parameter"))?;
    if parameter_kind(parameter) != "secret" {
        return Err(validation("Provider parameter is not a secret"));
    }

    let ProviderConfig::Remote { env_vars, .. } = provider;
    let placeholder = managed_placeholder(provider_id, parameter_name);
    if env_vars.get(parameter_name) != Some(&placeholder) {
        return Err(validation(
            "Only app-managed credentials can be viewed or copied this way",
        ));
    }

    let store = ManagedSecretStore::new(config_dir, loaded.config.secrets_storage.clone());
    store
        .read_effective_value(provider_id, parameter_name)
        .map_err(io)?
        .ok_or_else(|| validation("The app-managed secret is empty or missing"))
}

pub fn reveal_managed_secret_for_path(
    config_path: &Path,
    provider_id: &str,
    parameter_name: &str,
) -> Result<RevealSecretResult, MutationFailure> {
    let value = crate::provider_op_lock::with_provider_read(provider_id, || {
        read_managed_secret_value(config_path, provider_id, parameter_name)
    });
    audit_secret_access(
        config_path,
        "reveal",
        provider_id,
        parameter_name,
        if value.is_ok() { "ok" } else { "failed" },
    );
    let value = value?;
    Ok(RevealSecretResult { value })
}

pub fn copy_managed_secret_for_path(
    config_path: &Path,
    provider_id: &str,
    parameter_name: &str,
) -> Result<(), MutationFailure> {
    let value = crate::provider_op_lock::with_provider_read(provider_id, || {
        read_managed_secret_value(config_path, provider_id, parameter_name)
    });
    audit_secret_access(
        config_path,
        "copy",
        provider_id,
        parameter_name,
        if value.is_ok() { "ok" } else { "failed" },
    );
    let value = value?;

    // The write goes through Win32 directly so the exclusion formats keep the
    // secret out of clipboard history (Win+V) and cloud clipboard sync.
    crate::secret_clipboard::copy_secret(&value).map_err(io)
}

#[tauri::command]
pub async fn reveal_managed_secret(
    app: AppHandle,
    provider_id: String,
    parameter_name: String,
) -> Result<RevealSecretResult, MutationFailure> {
    let path = config_path_for_app(&app).map_err(io)?;
    tauri::async_runtime::spawn_blocking(move || {
        reveal_managed_secret_for_path(&path, &provider_id, &parameter_name)
    })
    .await
    .map_err(io)?
}

#[tauri::command]
pub async fn copy_managed_secret(
    app: AppHandle,
    provider_id: String,
    parameter_name: String,
) -> Result<(), MutationFailure> {
    let path = config_path_for_app(&app).map_err(io)?;
    tauri::async_runtime::spawn_blocking(move || {
        copy_managed_secret_for_path(&path, &provider_id, &parameter_name)
    })
    .await
    .map_err(io)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        default_config, save_config_to_path, ProviderSetupState, SecretStorageMode,
    };
    use crate::remote_provider_commands::remote_provider_dir;
    use std::collections::HashMap;

    fn provider_config(id: &str, env: &[(&str, &str)]) -> ProviderConfig {
        ProviderConfig::Remote {
            id: id.to_string(),
            name: "Test Provider".to_string(),
            enabled: true,
            version: Some("1.0.0".to_string()),
            manifest_url: "https://example.com/provider.json".to_string(),
            source_url: "https://example.com/provider.cjs".to_string(),
            provider_dir: None,
            runtime: "node".to_string(),
            resolved_runtime: None,
            proxy_url: None,
            auto_update: false,
            update_interval_seconds: 3600,
            timeout_seconds: 30,
            trusted_checksum: Some("sha256:aaaa".to_string()),
            installed_at: None,
            updated_at: None,
            last_checked_at: None,
            window_label_overrides: HashMap::new(),
            visible_window_ids: Vec::new(),
            show_in_tray: true,
            notifications_enabled: true,
            show_in_screenshot: true,
            env_vars: env
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
            setup_state: ProviderSetupState::Ready,
            setup_last_tested_at: Some("2026-11-01T00:00:00Z".to_string()),
        }
    }

    fn write_cache(config_path: &Path, id: &str) {
        let provider_dir = remote_provider_dir(config_path, id).expect("provider dir");
        std::fs::create_dir_all(&provider_dir).expect("create cache dir");
        std::fs::write(
            provider_dir.join("provider.json"),
            format!(
                r#"{{"schemaVersion":1,"id":"{id}","displayName":"Test","runtime":"node","entry":"provider.cjs","requiredEnvVars":["API_KEY"],"output":"provider-snapshot-v1","parameters":[{{"name":"API_KEY","kind":"secret","required":true}},{{"name":"REGION","kind":"string","required":false}}]}}"#
            ),
        )
        .expect("manifest");
        std::fs::write(provider_dir.join("provider.cjs"), "module.exports = {};").expect("source");
    }

    fn setup(config_path: &Path, env: &[(&str, &str)]) {
        setup_with_id(config_path, "p", env);
    }

    fn setup_with_id(config_path: &Path, id: &str, env: &[(&str, &str)]) {
        write_cache(config_path, id);
        let mut config = default_config();
        config.secrets_storage = SecretStorageMode::Plaintext;
        config.providers = vec![provider_config(id, env)];
        save_config_to_path(config_path, &config).expect("save config");
    }

    fn provider_env(config_path: &Path) -> HashMap<String, String> {
        let config = crate::config::load_or_create_config(config_path)
            .expect("load config")
            .config;
        let ProviderConfig::Remote { env_vars, .. } = &config.providers[0];
        env_vars.clone()
    }

    #[test]
    fn adopt_converts_a_literal_secret_and_keeps_the_provider_state() {
        let temp = tempfile::tempdir().expect("temp dir");
        let config_path = temp.path().join("config.json");
        setup(&config_path, &[("API_KEY", "sk-literal-value")]);

        let result = adopt_parameter_for_path(&config_path, "p", "API_KEY").expect("adopt literal");

        assert!(result.adopted);
        let field = result
            .descriptor
            .fields
            .iter()
            .find(|field| field.name == "API_KEY")
            .expect("API_KEY field");
        assert!(field.configured);
        assert_eq!(
            provider_env(&config_path)
                .get("API_KEY")
                .map(String::as_str),
            Some("${secret:providers/p/API_KEY}")
        );
        let secret_path = temp
            .path()
            .join("secrets")
            .join("providers")
            .join("p")
            .join("API_KEY.txt");
        assert_eq!(
            std::fs::read_to_string(secret_path).expect("managed secret"),
            "sk-literal-value"
        );

        let config = crate::config::load_or_create_config(&config_path)
            .expect("reload")
            .config;
        let ProviderConfig::Remote {
            enabled,
            setup_state,
            ..
        } = &config.providers[0];
        assert!(*enabled);
        assert_eq!(*setup_state, ProviderSetupState::Ready);
    }

    #[test]
    fn adopt_is_idempotent_for_a_managed_binding() {
        let temp = tempfile::tempdir().expect("temp dir");
        let config_path = temp.path().join("config.json");
        setup(&config_path, &[("API_KEY", "sk-literal-value")]);

        adopt_parameter_for_path(&config_path, "p", "API_KEY").expect("first adopt");
        let result = adopt_parameter_for_path(&config_path, "p", "API_KEY").expect("second adopt");

        assert!(!result.adopted);
    }

    #[test]
    fn adopt_rejects_external_sources_whitespace_and_non_secret_parameters() {
        for env in [
            ("${env:OTHER_KEY}", "external source"),
            ("  padded  ", "whitespace"),
            ("", "empty"),
        ] {
            let temp = tempfile::tempdir().expect("temp dir");
            let config_path = temp.path().join("config.json");
            setup(&config_path, &[("API_KEY", env.0)]);

            let failure = adopt_parameter_for_path(&config_path, "p", "API_KEY").unwrap_err();
            assert!(
                matches!(failure, MutationFailure::Validation { .. }),
                "{} must be rejected",
                env.1
            );
        }

        let temp = tempfile::tempdir().expect("temp dir");
        let config_path = temp.path().join("config.json");
        setup(&config_path, &[("REGION", "CN")]);
        let failure = adopt_parameter_for_path(&config_path, "p", "REGION").unwrap_err();
        assert!(matches!(failure, MutationFailure::Validation { .. }));
    }

    #[test]
    fn reveal_returns_a_managed_secret_and_rejects_literal_bindings() {
        let temp = tempfile::tempdir().expect("temp dir");
        let config_path = temp.path().join("config.json");
        setup_with_id(&config_path, "p-reveal", &[("API_KEY", "sk-reveal")]);
        adopt_parameter_for_path(&config_path, "p-reveal", "API_KEY").expect("adopt");

        let revealed =
            reveal_managed_secret_for_path(&config_path, "p-reveal", "API_KEY").expect("reveal");
        assert_eq!(revealed.value, "sk-reveal");

        let literal_temp = tempfile::tempdir().expect("temp dir");
        let literal_path = literal_temp.path().join("config.json");
        setup_with_id(&literal_path, "p-literal", &[("API_KEY", "sk-literal")]);
        let failure =
            reveal_managed_secret_for_path(&literal_path, "p-literal", "API_KEY").unwrap_err();
        assert!(matches!(failure, MutationFailure::Validation { .. }));
    }

    #[test]
    fn reveal_allows_repeated_access() {
        let temp = tempfile::tempdir().expect("temp dir");
        let config_path = temp.path().join("config.json");
        setup_with_id(&config_path, "p-rate", &[("API_KEY", "sk-rate")]);
        adopt_parameter_for_path(&config_path, "p-rate", "API_KEY").expect("adopt");

        for attempt in 0..10 {
            let revealed = reveal_managed_secret_for_path(&config_path, "p-rate", "API_KEY")
                .unwrap_or_else(|error| panic!("attempt {attempt} failed: {error:?}"));
            assert_eq!(revealed.value, "sk-rate");
        }
        let audit = std::fs::read_to_string(log_path_for_config_path(&config_path))
            .expect("credential access audit log");
        assert!(audit.contains("secret.access"));
        assert!(audit.contains("reveal"));
        assert!(!audit.contains("sk-rate"));
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "uses the real interactive clipboard"]
    fn copy_writes_the_clipboard() {
        let temp = tempfile::tempdir().expect("temp dir");
        let config_path = temp.path().join("config.json");
        setup_with_id(&config_path, "p-copy", &[("API_KEY", "sk-copy")]);
        adopt_parameter_for_path(&config_path, "p-copy", "API_KEY").expect("adopt");

        copy_managed_secret_for_path(&config_path, "p-copy", "API_KEY").expect("copy");
        let mut clipboard = arboard::Clipboard::new().expect("clipboard");
        assert_eq!(clipboard.get_text().expect("text"), "sk-copy");
    }
}
