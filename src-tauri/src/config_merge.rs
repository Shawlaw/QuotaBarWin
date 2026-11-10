//! Pure-function spike for W1: three-way merge of a renderer config draft.
//!
//! Design: `private-doc/p0/01-w1-config-merge.md`.
//! This module is intentionally not wired into `save_config` yet; CP0b-1 only
//! needs the pure function plus the M-series tests so the field ownership table
//! can be reviewed and frozen before the command contract changes.
#![allow(dead_code)]
// The persisted provider enum currently has a single variant, so pattern
// matches on it are irrefutable; keeping the `if let` shape makes the future
// multi-variant migration a one-line change per site.
#![allow(irrefutable_let_patterns)]

use crate::config::{
    AppConfig, AppUpdateSettings, LocalApiSettings, NotificationSettings, ProviderConfig,
    ProviderSetupState, RemoteProviderRegistrySettings,
};

/// Serializable failure returned by config mutation commands.
///
/// `kind` lets the renderer tell "definitely not committed" apart from
/// "unknown, reload" without parsing error strings.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MutationFailure {
    Validation {
        message: String,
    },
    Conflict {
        message: String,
        fields: Vec<String>,
    },
    Stale {
        message: String,
    },
    Uncertain {
        message: String,
        rollback_failed: bool,
    },
    Io {
        message: String,
    },
}

impl From<MergeError> for MutationFailure {
    fn from(error: MergeError) -> Self {
        match error {
            MergeError::MissingBase => MutationFailure::Validation {
                message: "Configuration changed on disk; reload before saving".to_string(),
            },
            MergeError::StaleBase => MutationFailure::Stale {
                message: "Configuration no longer exists on disk; reload".to_string(),
            },
            MergeError::SchemaMismatch { base, current } => MutationFailure::Validation {
                message: format!(
                    "Configuration schema changed (draft {base}, disk {current}); reload before saving"
                ),
            },
            MergeError::DuplicateProviderId { id } => MutationFailure::Conflict {
                message: "Duplicate provider id in configuration".to_string(),
                fields: vec![format!("providers[{id}]")],
            },
            MergeError::FirstCreationWithProviders => MutationFailure::Validation {
                message: "Providers cannot be created through a config save".to_string(),
            },
        }
    }
}

impl From<String> for MutationFailure {
    fn from(message: String) -> Self {
        MutationFailure::Io { message }
    }
}

/// Authoritative config returned after a successful save.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveConfigOutcome {
    pub config: AppConfig,
    /// Post-commit side effects that failed (the config itself is committed).
    pub warnings: Vec<String>,
}

/// Result of a successful merge: the authoritative config plus non-blocking
/// notes about fields that were kept from disk.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct MergeNotes {
    pub warnings: Vec<String>,
    pub conflicts: Vec<String>,
}

/// Outcome of merging a draft against the persisted config.
#[derive(Debug, Clone, PartialEq)]
pub enum MergeOutcome {
    Merged {
        config: AppConfig,
        warnings: Vec<String>,
    },
    /// At least one user-owned field changed on both sides. The caller must
    /// reject the write and return the field paths to the renderer.
    Conflict { fields: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeError {
    /// The draft arrived without a base while a config file already exists.
    MissingBase,
    /// The draft was based on a config that no longer exists on disk.
    StaleBase,
    /// Base and current were written by different schema versions.
    SchemaMismatch { base: u8, current: u8 },
    /// A persisted provider list contains the same id twice. Writing the merge
    /// would silently drop one of them, so reject instead.
    DuplicateProviderId { id: String },
    /// First creation must not carry providers: installation has to go through
    /// the install command (download, checksum, setup state).
    FirstCreationWithProviders,
}

/// Merges `next` (the draft) onto `current` using `base` (the snapshot the
/// renderer loaded) to tell apart "the user changed this" from "somebody else
/// changed this".
///
/// `None` handling:
/// - `(None, None)`: first creation; providers are rejected because installs
///   must go through the install command.
/// - `(None, Some)`: reject; a renderer must always send its base.
/// - `(Some, None)`: reject as stale; never recreate a deleted config.
pub fn merge_config(
    base: Option<&AppConfig>,
    next: &AppConfig,
    current: Option<&AppConfig>,
) -> Result<MergeOutcome, MergeError> {
    match (base, current) {
        (None, None) => {
            if !next.providers.is_empty() {
                return Err(MergeError::FirstCreationWithProviders);
            }
            Ok(MergeOutcome::Merged {
                config: next.clone(),
                warnings: Vec::new(),
            })
        }
        (None, Some(_)) => Err(MergeError::MissingBase),
        (Some(_), None) => Err(MergeError::StaleBase),
        (Some(base), Some(current)) => {
            if base.schema_version != current.schema_version {
                return Err(MergeError::SchemaMismatch {
                    base: base.schema_version,
                    current: current.schema_version,
                });
            }
            merge_existing(base, next, current)
        }
    }
}

fn merge_existing(
    base: &AppConfig,
    next: &AppConfig,
    current: &AppConfig,
) -> Result<MergeOutcome, MergeError> {
    let mut notes = MergeNotes::default();

    let providers = merge_providers(
        &base.providers,
        &next.providers,
        &current.providers,
        &mut notes,
    )?;

    let config = AppConfig {
        // System-owned: schema version is never part of the draft.
        schema_version: current.schema_version,
        refresh_interval_seconds: merge_field(
            &base.refresh_interval_seconds,
            &next.refresh_interval_seconds,
            &current.refresh_interval_seconds,
            "refreshIntervalSeconds",
            &mut notes,
        ),
        display_mode: merge_field(
            &base.display_mode,
            &next.display_mode,
            &current.display_mode,
            "displayMode",
            &mut notes,
        ),
        low_quota_warning_threshold: merge_field(
            &base.low_quota_warning_threshold,
            &next.low_quota_warning_threshold,
            &current.low_quota_warning_threshold,
            "lowQuotaWarningThreshold",
            &mut notes,
        ),
        launch_at_startup: merge_field(
            &base.launch_at_startup,
            &next.launch_at_startup,
            &current.launch_at_startup,
            "launchAtStartup",
            &mut notes,
        ),
        log_level: merge_field(
            &base.log_level,
            &next.log_level,
            &current.log_level,
            "logLevel",
            &mut notes,
        ),
        log_max_bytes: merge_field(
            &base.log_max_bytes,
            &next.log_max_bytes,
            &current.log_max_bytes,
            "logMaxBytes",
            &mut notes,
        ),
        log_quota_data: merge_field(
            &base.log_quota_data,
            &next.log_quota_data,
            &current.log_quota_data,
            "logQuotaData",
            &mut notes,
        ),
        language: merge_field(
            &base.language,
            &next.language,
            &current.language,
            "language",
            &mut notes,
        ),
        theme: merge_field(
            &base.theme,
            &next.theme,
            &current.theme,
            "theme",
            &mut notes,
        ),
        network_proxy: merge_field(
            &base.network_proxy,
            &next.network_proxy,
            &current.network_proxy,
            "networkProxy",
            &mut notes,
        ),
        // Command-owned: tray geometry and secret storage mode are written by
        // dedicated commands and must never be reverted by a draft.
        tray_popup_position: keep_current(
            &base.tray_popup_position,
            &next.tray_popup_position,
            &current.tray_popup_position,
            "trayPopupPosition",
            &mut notes,
        ),
        tray_popup_size: keep_current(
            &base.tray_popup_size,
            &next.tray_popup_size,
            &current.tray_popup_size,
            "trayPopupSize",
            &mut notes,
        ),
        app_update: merge_app_update(
            &base.app_update,
            &next.app_update,
            &current.app_update,
            &mut notes,
        ),
        local_api: merge_local_api(
            &base.local_api,
            &next.local_api,
            &current.local_api,
            &mut notes,
        ),
        notifications: merge_notifications(
            &base.notifications,
            &next.notifications,
            &current.notifications,
            &mut notes,
        ),
        secrets_storage: keep_current(
            &base.secrets_storage,
            &next.secrets_storage,
            &current.secrets_storage,
            "secretsStorage",
            &mut notes,
        ),
        secrets_encryption_prompt_pending: keep_current(
            &base.secrets_encryption_prompt_pending,
            &next.secrets_encryption_prompt_pending,
            &current.secrets_encryption_prompt_pending,
            "secretsEncryptionPromptPending",
            &mut notes,
        ),
        remote_provider_registry: merge_registry(
            &base.remote_provider_registry,
            &next.remote_provider_registry,
            &current.remote_provider_registry,
            &mut notes,
        ),
        providers,
    };

    if notes.conflicts.is_empty() {
        Ok(MergeOutcome::Merged {
            config,
            warnings: notes.warnings,
        })
    } else {
        Ok(MergeOutcome::Conflict {
            fields: notes.conflicts,
        })
    }
}

fn merge_app_update(
    base: &AppUpdateSettings,
    next: &AppUpdateSettings,
    current: &AppUpdateSettings,
    notes: &mut MergeNotes,
) -> AppUpdateSettings {
    AppUpdateSettings {
        auto_check: merge_field(
            &base.auto_check,
            &next.auto_check,
            &current.auto_check,
            "appUpdate.autoCheck",
            notes,
        ),
    }
}

fn merge_local_api(
    base: &LocalApiSettings,
    next: &LocalApiSettings,
    current: &LocalApiSettings,
    notes: &mut MergeNotes,
) -> LocalApiSettings {
    LocalApiSettings {
        enabled: merge_field(
            &base.enabled,
            &next.enabled,
            &current.enabled,
            "localApi.enabled",
            notes,
        ),
        bind_target: merge_field(
            &base.bind_target,
            &next.bind_target,
            &current.bind_target,
            "localApi.bindTarget",
            notes,
        ),
        port: merge_field(
            &base.port,
            &next.port,
            &current.port,
            "localApi.port",
            notes,
        ),
    }
}

fn merge_notifications(
    base: &NotificationSettings,
    next: &NotificationSettings,
    current: &NotificationSettings,
    notes: &mut MergeNotes,
) -> NotificationSettings {
    NotificationSettings {
        toast_enabled: merge_field(
            &base.toast_enabled,
            &next.toast_enabled,
            &current.toast_enabled,
            "notifications.toastEnabled",
            notes,
        ),
        webhook_enabled: merge_field(
            &base.webhook_enabled,
            &next.webhook_enabled,
            &current.webhook_enabled,
            "notifications.webhookEnabled",
            notes,
        ),
        webhooks: merge_field(
            &base.webhooks,
            &next.webhooks,
            &current.webhooks,
            "notifications.webhooks",
            notes,
        ),
        events: merge_field(
            &base.events,
            &next.events,
            &current.events,
            "notifications.events",
            notes,
        ),
    }
}

fn merge_registry(
    base: &RemoteProviderRegistrySettings,
    next: &RemoteProviderRegistrySettings,
    current: &RemoteProviderRegistrySettings,
    notes: &mut MergeNotes,
) -> RemoteProviderRegistrySettings {
    RemoteProviderRegistrySettings {
        registry_url: merge_field(
            &base.registry_url,
            &next.registry_url,
            &current.registry_url,
            "remoteProviderRegistry.registryUrl",
            notes,
        ),
        provider_proxy_url: merge_field(
            &base.provider_proxy_url,
            &next.provider_proxy_url,
            &current.provider_proxy_url,
            "remoteProviderRegistry.providerProxyUrl",
            notes,
        ),
        auto_update: merge_field(
            &base.auto_update,
            &next.auto_update,
            &current.auto_update,
            "remoteProviderRegistry.autoUpdate",
            notes,
        ),
        sources: merge_field(
            &base.sources,
            &next.sources,
            &current.sources,
            "remoteProviderRegistry.sources",
            notes,
        ),
    }
}

/// User-owned scalar/object unit: apply the draft only when nobody else
/// changed the same field.
fn merge_field<T: PartialEq + Clone>(
    base: &T,
    next: &T,
    current: &T,
    path: &str,
    notes: &mut MergeNotes,
) -> T {
    if next == base {
        current.clone()
    } else if current == base {
        next.clone()
    } else {
        notes.conflicts.push(path.to_string());
        current.clone()
    }
}

/// Command-owned field: always keep disk state; note when the draft carried a
/// different value (normally only possible with a stale renderer snapshot).
fn keep_current<T: PartialEq + Clone>(
    base: &T,
    next: &T,
    current: &T,
    path: &str,
    notes: &mut MergeNotes,
) -> T {
    if next != base {
        notes
            .warnings
            .push(format!("{path} 由其他操作更新，已保留最新值"));
    }
    current.clone()
}

fn merge_providers(
    base: &[ProviderConfig],
    next: &[ProviderConfig],
    current: &[ProviderConfig],
    notes: &mut MergeNotes,
) -> Result<Vec<ProviderConfig>, MergeError> {
    use std::collections::HashMap;

    ensure_unique_provider_ids(base)?;
    ensure_unique_provider_ids(next)?;
    ensure_unique_provider_ids(current)?;

    let base_by_id: HashMap<&str, &ProviderConfig> = base.iter().map(|p| (id_of(p), p)).collect();
    let next_by_id: HashMap<&str, &ProviderConfig> = next.iter().map(|p| (id_of(p), p)).collect();
    let current_by_id: HashMap<&str, &ProviderConfig> =
        current.iter().map(|p| (id_of(p), p)).collect();

    let mut merged: HashMap<String, ProviderConfig> = HashMap::new();
    let mut merged_ids: Vec<String> = Vec::new();

    for id in base_by_id
        .keys()
        .chain(next_by_id.keys())
        .chain(current_by_id.keys())
    {
        if !merged_ids.iter().any(|existing| existing == id) {
            merged_ids.push((*id).to_string());
        }
    }
    merged_ids.sort();

    for id in merged_ids {
        let entry = match (
            base_by_id.get(id.as_str()),
            next_by_id.get(id.as_str()),
            current_by_id.get(id.as_str()),
        ) {
            (Some(b), Some(n), Some(c)) => Some(merge_provider(b, n, c, notes)),
            // Background-added provider: keep it.
            (None, None, Some(c)) => Some((*c).clone()),
            // User added a provider in the draft: only install commands may do
            // this, otherwise it would bypass download/checksum/setup fields.
            (None, Some(_), None) => {
                notes.conflicts.push(format!("providers[{id}]"));
                None
            }
            // Base and current both have it, the draft does not: user removed it.
            (Some(b), None, Some(c)) => {
                if b == c {
                    None
                } else {
                    notes.conflicts.push(format!("providers[{id}]"));
                    Some((*c).clone())
                }
            }
            // Draft still contains a provider the backend removed: do not
            // resurrect it.
            (Some(_), Some(_), None) => None,
            // The draft added an id that already exists on disk.
            (None, Some(_), Some(_)) => {
                notes.conflicts.push(format!("providers[{id}]"));
                None
            }
            (Some(_), None, None) => None,
            // Unreachable: the id list is built from the three maps.
            (None, None, None) => None,
        };
        if let Some(provider) = entry {
            merged.insert(id, provider);
        }
    }

    Ok(order_providers(base, next, current, &merged))
}

fn ensure_unique_provider_ids(providers: &[ProviderConfig]) -> Result<(), MergeError> {
    let mut seen = std::collections::HashSet::new();
    for provider in providers {
        let id = id_of(provider);
        if !seen.insert(id) {
            return Err(MergeError::DuplicateProviderId { id: id.to_string() });
        }
    }
    Ok(())
}

/// Applies the user's ordering when the draft reordered providers; otherwise
/// keeps the order already persisted on disk (which is where background
/// installs append). Providers only the backend knows about follow in current
/// order.
fn order_providers(
    base: &[ProviderConfig],
    next: &[ProviderConfig],
    current: &[ProviderConfig],
    merged: &std::collections::HashMap<String, ProviderConfig>,
) -> Vec<ProviderConfig> {
    let base_order: Vec<&str> = base.iter().map(id_of).collect();
    let next_known: Vec<&str> = next
        .iter()
        .map(id_of)
        .filter(|id| base_order.contains(id))
        .collect();

    let user_reordered = next_known != base_order;
    let preferred: &[ProviderConfig] = if user_reordered { next } else { current };
    let mut order: Vec<String> = preferred
        .iter()
        .map(id_of)
        .filter(|id| merged.contains_key(*id))
        .map(str::to_string)
        .collect();

    // Anything merged but absent from the preferred order (normally only a
    // backend-added provider while the user reordered) keeps current order.
    for provider in current {
        let id = id_of(provider);
        if merged.contains_key(id) && !order.iter().any(|existing| existing == id) {
            order.push(id.to_string());
        }
    }

    order
        .iter()
        .filter_map(|id| merged.get(id).cloned())
        .collect()
}

fn id_of(provider: &ProviderConfig) -> &str {
    match provider {
        ProviderConfig::Remote { id, .. } => id,
    }
}

fn merge_provider(
    base: &ProviderConfig,
    next: &ProviderConfig,
    current: &ProviderConfig,
    notes: &mut MergeNotes,
) -> ProviderConfig {
    let ProviderConfig::Remote {
        id: _base_id,
        name: base_name,
        enabled: base_enabled,
        version: base_version,
        manifest_url: base_manifest_url,
        source_url: base_source_url,
        provider_dir: base_provider_dir,
        runtime: base_runtime,
        resolved_runtime: base_resolved_runtime,
        proxy_url: base_proxy_url,
        auto_update: base_auto_update,
        update_interval_seconds: base_update_interval,
        timeout_seconds: base_timeout,
        trusted_checksum: base_checksum,
        installed_at: base_installed,
        updated_at: base_updated,
        last_checked_at: base_checked,
        window_label_overrides: base_labels,
        visible_window_ids: base_windows,
        show_in_tray: base_tray,
        notifications_enabled: base_notifications,
        show_in_screenshot: base_screenshot,
        env_vars: base_env,
        setup_state: base_state,
        setup_last_tested_at: base_tested,
    } = base
    else {
        unreachable!("ProviderConfig only has the remote variant")
    };
    let ProviderConfig::Remote {
        id: _next_id,
        name: next_name,
        enabled: next_enabled,
        version: next_version,
        manifest_url: next_manifest_url,
        source_url: next_source_url,
        provider_dir: next_provider_dir,
        runtime: next_runtime,
        resolved_runtime: next_resolved_runtime,
        proxy_url: next_proxy_url,
        auto_update: next_auto_update,
        update_interval_seconds: next_update_interval,
        timeout_seconds: next_timeout,
        trusted_checksum: next_checksum,
        installed_at: next_installed,
        updated_at: next_updated,
        last_checked_at: next_checked,
        window_label_overrides: next_labels,
        visible_window_ids: next_windows,
        show_in_tray: next_tray,
        notifications_enabled: next_notifications,
        show_in_screenshot: next_screenshot,
        env_vars: next_env,
        setup_state: next_state,
        setup_last_tested_at: next_tested,
    } = next
    else {
        unreachable!("ProviderConfig only has the remote variant")
    };
    let ProviderConfig::Remote {
        id: current_id,
        name: current_name,
        enabled: current_enabled,
        version: current_version,
        manifest_url: current_manifest_url,
        source_url: current_source_url,
        provider_dir: current_provider_dir,
        runtime: current_runtime,
        resolved_runtime: current_resolved_runtime,
        proxy_url: current_proxy_url,
        auto_update: current_auto_update,
        update_interval_seconds: current_update_interval,
        timeout_seconds: current_timeout,
        trusted_checksum: current_checksum,
        installed_at: current_installed,
        updated_at: current_updated,
        last_checked_at: current_checked,
        window_label_overrides: current_labels,
        visible_window_ids: current_windows,
        show_in_tray: current_tray,
        notifications_enabled: current_notifications,
        show_in_screenshot: current_screenshot,
        env_vars: current_env,
        setup_state: current_state,
        setup_last_tested_at: current_tested,
    } = current
    else {
        unreachable!("ProviderConfig only has the remote variant")
    };

    let id = (*current_id).clone();
    let path = |field: &str| format!("providers[{id}].{field}");

    let env_vars = merge_field(base_env, next_env, current_env, &path("envVars"), notes);

    let mut merged = ProviderConfig::Remote {
        id: id.clone(),
        name: merge_field(base_name, next_name, current_name, &path("name"), notes),
        enabled: merge_field(
            base_enabled,
            next_enabled,
            current_enabled,
            &path("enabled"),
            notes,
        ),
        version: keep_current(
            base_version,
            next_version,
            current_version,
            &path("version"),
            notes,
        ),
        manifest_url: keep_current(
            base_manifest_url,
            next_manifest_url,
            current_manifest_url,
            &path("manifestUrl"),
            notes,
        ),
        source_url: keep_current(
            base_source_url,
            next_source_url,
            current_source_url,
            &path("sourceUrl"),
            notes,
        ),
        provider_dir: keep_current(
            base_provider_dir,
            next_provider_dir,
            current_provider_dir,
            &path("providerDir"),
            notes,
        ),
        runtime: keep_current(
            base_runtime,
            next_runtime,
            current_runtime,
            &path("runtime"),
            notes,
        ),
        resolved_runtime: keep_current(
            base_resolved_runtime,
            next_resolved_runtime,
            current_resolved_runtime,
            &path("resolvedRuntime"),
            notes,
        ),
        proxy_url: keep_current(
            base_proxy_url,
            next_proxy_url,
            current_proxy_url,
            &path("proxyUrl"),
            notes,
        ),
        auto_update: keep_current(
            base_auto_update,
            next_auto_update,
            current_auto_update,
            &path("autoUpdate"),
            notes,
        ),
        update_interval_seconds: keep_current(
            base_update_interval,
            next_update_interval,
            current_update_interval,
            &path("updateIntervalSeconds"),
            notes,
        ),
        timeout_seconds: merge_field(
            base_timeout,
            next_timeout,
            current_timeout,
            &path("timeoutSeconds"),
            notes,
        ),
        trusted_checksum: keep_current(
            base_checksum,
            next_checksum,
            current_checksum,
            &path("trustedChecksum"),
            notes,
        ),
        installed_at: keep_current(
            base_installed,
            next_installed,
            current_installed,
            &path("installedAt"),
            notes,
        ),
        updated_at: keep_current(
            base_updated,
            next_updated,
            current_updated,
            &path("updatedAt"),
            notes,
        ),
        last_checked_at: keep_current(
            base_checked,
            next_checked,
            current_checked,
            &path("lastCheckedAt"),
            notes,
        ),
        window_label_overrides: merge_field(
            base_labels,
            next_labels,
            current_labels,
            &path("windowLabelOverrides"),
            notes,
        ),
        visible_window_ids: merge_field(
            base_windows,
            next_windows,
            current_windows,
            &path("visibleWindowIds"),
            notes,
        ),
        show_in_tray: merge_field(
            base_tray,
            next_tray,
            current_tray,
            &path("showInTray"),
            notes,
        ),
        notifications_enabled: merge_field(
            base_notifications,
            next_notifications,
            current_notifications,
            &path("notificationsEnabled"),
            notes,
        ),
        show_in_screenshot: merge_field(
            base_screenshot,
            next_screenshot,
            current_screenshot,
            &path("showInScreenshot"),
            notes,
        ),
        env_vars,
        setup_state: keep_current(
            base_state,
            next_state,
            current_state,
            &path("setupState"),
            notes,
        ),
        setup_last_tested_at: keep_current(
            base_tested,
            next_tested,
            current_tested,
            &path("setupLastTestedAt"),
            notes,
        ),
    };

    // Changing any binding invalidates the previous verification.
    if let ProviderConfig::Remote {
        env_vars,
        enabled,
        setup_state,
        setup_last_tested_at,
        ..
    } = &mut merged
    {
        if env_vars != current_env {
            *enabled = false;
            *setup_state = ProviderSetupState::Unverified;
            *setup_last_tested_at = None;
        }
    }

    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{default_config, SecretStorageMode};
    use std::collections::HashMap;

    fn provider(id: &str, env: &[(&str, &str)]) -> ProviderConfig {
        ProviderConfig::Remote {
            id: id.to_string(),
            name: format!("{id} account"),
            enabled: true,
            version: Some("1.0.0".to_string()),
            manifest_url: "https://example.com/provider.json".to_string(),
            source_url: "https://example.com/provider.js".to_string(),
            provider_dir: None,
            runtime: "builtin-js".to_string(),
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

    fn config_with(providers: Vec<ProviderConfig>) -> AppConfig {
        let mut config = default_config();
        config.providers = providers;
        config
    }

    fn edit(provider: &ProviderConfig, apply: impl FnOnce(&mut ProviderConfig)) -> ProviderConfig {
        let mut copy = provider.clone();
        apply(&mut copy);
        copy
    }

    fn with_env(provider: &ProviderConfig, env: &[(&str, &str)]) -> ProviderConfig {
        edit(provider, |p| {
            if let ProviderConfig::Remote { env_vars, .. } = p {
                *env_vars = env
                    .iter()
                    .map(|(key, value)| (key.to_string(), value.to_string()))
                    .collect();
            }
        })
    }

    fn with_last_checked(provider: &ProviderConfig, value: &str) -> ProviderConfig {
        edit(provider, |p| {
            if let ProviderConfig::Remote {
                last_checked_at, ..
            } = p
            {
                *last_checked_at = Some(value.to_string());
            }
        })
    }

    fn with_name(provider: &ProviderConfig, value: &str) -> ProviderConfig {
        edit(provider, |p| {
            if let ProviderConfig::Remote { name, .. } = p {
                *name = value.to_string();
            }
        })
    }

    fn with_version(provider: &ProviderConfig, version: &str, checksum: &str) -> ProviderConfig {
        edit(provider, |p| {
            if let ProviderConfig::Remote {
                version: current_version,
                trusted_checksum,
                ..
            } = p
            {
                *current_version = Some(version.to_string());
                *trusted_checksum = Some(checksum.to_string());
            }
        })
    }

    fn with_setup_state(provider: &ProviderConfig, state: ProviderSetupState) -> ProviderConfig {
        edit(provider, |p| {
            if let ProviderConfig::Remote { setup_state, .. } = p {
                *setup_state = state;
            }
        })
    }

    fn with_updated_at(provider: &ProviderConfig, value: &str) -> ProviderConfig {
        edit(provider, |p| {
            if let ProviderConfig::Remote { updated_at, .. } = p {
                *updated_at = Some(value.to_string());
            }
        })
    }

    fn merged(outcome: MergeOutcome) -> (AppConfig, Vec<String>) {
        match outcome {
            MergeOutcome::Merged { config, warnings } => (config, warnings),
            MergeOutcome::Conflict { fields } => panic!("unexpected conflict: {fields:?}"),
        }
    }

    fn conflicts(outcome: MergeOutcome) -> Vec<String> {
        match outcome {
            MergeOutcome::Conflict { fields } => fields,
            MergeOutcome::Merged { .. } => panic!("expected conflict"),
        }
    }

    fn env_of(provider: &ProviderConfig) -> HashMap<String, String> {
        match provider {
            ProviderConfig::Remote { env_vars, .. } => env_vars.clone(),
        }
    }

    fn last_checked_of(provider: &ProviderConfig) -> Option<String> {
        match provider {
            ProviderConfig::Remote {
                last_checked_at, ..
            } => last_checked_at.clone(),
        }
    }

    #[test]
    fn m1_background_last_checked_at_survives_an_unrelated_save() {
        let base = config_with(vec![provider("kimi", &[("API_KEY", "${env:OLD}")])]);
        let mut next = base.clone();
        next.refresh_interval_seconds = 120;
        let mut current = base.clone();
        current.providers[0] = with_last_checked(&current.providers[0], "2026-11-08T00:00:00Z");

        let (result, warnings) = merged(merge_config(Some(&base), &next, Some(&current)).unwrap());

        assert_eq!(result.refresh_interval_seconds, 120);
        assert_eq!(
            last_checked_of(&result.providers[0]).as_deref(),
            Some("2026-11-08T00:00:00Z")
        );
        assert!(warnings.is_empty());
    }

    #[test]
    fn m2_background_provider_update_survives_a_name_edit() {
        let base = config_with(vec![provider("kimi", &[])]);
        let mut next = base.clone();
        next.providers[0] = with_name(&next.providers[0], "Work account");
        let mut current = base.clone();
        current.providers[0] = with_version(&current.providers[0], "2.0.0", "sha256:bbbb");

        let (result, _) = merged(merge_config(Some(&base), &next, Some(&current)).unwrap());

        match &result.providers[0] {
            ProviderConfig::Remote {
                name,
                version,
                trusted_checksum,
                ..
            } => {
                assert_eq!(name, "Work account");
                assert_eq!(version.as_deref(), Some("2.0.0"));
                assert_eq!(trusted_checksum.as_deref(), Some("sha256:bbbb"));
            }
        }
    }

    #[test]
    fn m3_adopted_binding_is_not_reverted_by_an_unchanged_draft() {
        let base = config_with(vec![provider("kimi", &[("API_KEY", "${env:OLD}")])]);
        let next = base.clone();
        let mut current = base.clone();
        current.providers[0] = with_env(
            &current.providers[0],
            &[("API_KEY", "${secret:providers/kimi/API_KEY}")],
        );

        let (result, _) = merged(merge_config(Some(&base), &next, Some(&current)).unwrap());

        assert_eq!(
            env_of(&result.providers[0])
                .get("API_KEY")
                .map(String::as_str),
            Some("${secret:providers/kimi/API_KEY}")
        );
    }

    #[test]
    fn m4_user_binding_edit_conflicts_with_a_concurrent_binding_change() {
        let base = config_with(vec![provider("kimi", &[("API_KEY", "${env:OLD}")])]);
        let mut next = base.clone();
        next.providers[0] = with_env(&next.providers[0], &[("API_KEY", "${env:NEW}")]);
        let mut current = base.clone();
        current.providers[0] = with_env(
            &current.providers[0],
            &[("API_KEY", "${secret:providers/kimi/API_KEY}")],
        );

        let fields = conflicts(merge_config(Some(&base), &next, Some(&current)).unwrap());
        assert_eq!(fields, vec!["providers[kimi].envVars".to_string()]);
    }

    #[test]
    fn m5_background_installed_provider_is_kept() {
        let base = config_with(vec![provider("kimi", &[])]);
        let next = base.clone();
        let mut current = base.clone();
        current.providers.push(provider("new-provider", &[]));

        let (result, _) = merged(merge_config(Some(&base), &next, Some(&current)).unwrap());
        assert_eq!(result.providers.len(), 2);
        assert!(result
            .providers
            .iter()
            .any(|provider| id_of(provider) == "new-provider"));
    }

    #[test]
    fn m6_background_removed_provider_is_not_resurrected() {
        let base = config_with(vec![provider("kimi", &[]), provider("other", &[])]);
        let next = base.clone();
        let mut current = base.clone();
        current
            .providers
            .retain(|provider| id_of(provider) != "other");

        let (result, _) = merged(merge_config(Some(&base), &next, Some(&current)).unwrap());
        assert_eq!(result.providers.len(), 1);
        assert_eq!(id_of(&result.providers[0]), "kimi");
    }

    #[test]
    fn m7_user_removal_is_applied_when_disk_matches_base() {
        let base = config_with(vec![provider("kimi", &[]), provider("other", &[])]);
        let mut next = base.clone();
        next.providers.retain(|provider| id_of(provider) != "other");
        let current = base.clone();

        let (result, _) = merged(merge_config(Some(&base), &next, Some(&current)).unwrap());
        assert_eq!(result.providers.len(), 1);
        assert_eq!(id_of(&result.providers[0]), "kimi");
    }

    #[test]
    fn m7b_user_removal_conflicts_after_a_background_update() {
        let base = config_with(vec![provider("kimi", &[]), provider("other", &[])]);
        let mut next = base.clone();
        next.providers.retain(|provider| id_of(provider) != "other");
        let mut current = base.clone();
        current.providers[1] = with_version(&current.providers[1], "2.0.0", "sha256:cccc");

        let fields = conflicts(merge_config(Some(&base), &next, Some(&current)).unwrap());
        assert_eq!(fields, vec!["providers[other]".to_string()]);
    }

    #[test]
    fn m8_user_reorder_is_applied_and_background_providers_append() {
        let base = config_with(vec![provider("a", &[]), provider("b", &[])]);
        let mut next = base.clone();
        next.providers = vec![provider("b", &[]), provider("a", &[])];
        let mut current = base.clone();
        current.providers.push(provider("c", &[]));

        let (result, _) = merged(merge_config(Some(&base), &next, Some(&current)).unwrap());
        let order: Vec<&str> = result.providers.iter().map(id_of).collect();
        assert_eq!(order, vec!["b", "a", "c"]);
    }

    #[test]
    fn m8b_current_order_wins_when_the_user_did_not_reorder() {
        let base = config_with(vec![provider("a", &[]), provider("b", &[])]);
        let next = base.clone();
        let current = config_with(vec![
            provider("b", &[]),
            provider("a", &[]),
            provider("c", &[]),
        ]);

        let (result, _) = merged(merge_config(Some(&base), &next, Some(&current)).unwrap());
        let order: Vec<&str> = result.providers.iter().map(id_of).collect();
        assert_eq!(order, vec!["b", "a", "c"]);
    }

    #[test]
    fn m9_secret_storage_mode_written_by_command_wins() {
        let mut base = config_with(vec![]);
        base.secrets_storage = SecretStorageMode::Plaintext;
        let mut next = base.clone();
        next.secrets_storage = SecretStorageMode::Encrypted;
        let mut current = base.clone();
        current.secrets_storage = SecretStorageMode::Encrypted;

        let (result, warnings) = merged(merge_config(Some(&base), &next, Some(&current)).unwrap());
        assert_eq!(result.secrets_storage, SecretStorageMode::Encrypted);
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("secretsStorage")));
    }

    #[test]
    fn m10_draft_cannot_force_a_verified_state() {
        let base = config_with(vec![with_setup_state(
            &provider("kimi", &[]),
            ProviderSetupState::Unverified,
        )]);
        let mut next = base.clone();
        next.providers[0] = with_setup_state(&next.providers[0], ProviderSetupState::Ready);
        let current = base.clone();

        let (result, warnings) = merged(merge_config(Some(&base), &next, Some(&current)).unwrap());
        match &result.providers[0] {
            ProviderConfig::Remote { setup_state, .. } => {
                assert_eq!(*setup_state, ProviderSetupState::Unverified)
            }
        }
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("setupState")));
    }

    #[test]
    fn m11_schema_mismatch_is_rejected() {
        let base = config_with(vec![]);
        let next = base.clone();
        let current = base.clone();
        let mut stale_base = base.clone();
        stale_base.schema_version = stale_base.schema_version.saturating_sub(1);

        assert_eq!(
            merge_config(Some(&stale_base), &next, Some(&current)).unwrap_err(),
            MergeError::SchemaMismatch {
                base: stale_base.schema_version,
                current: current.schema_version,
            }
        );
    }

    #[test]
    fn m12_first_creation_is_the_only_base_less_path() {
        let draft = config_with(vec![]);

        assert!(matches!(
            merge_config(None, &draft, None).unwrap(),
            MergeOutcome::Merged { .. }
        ));

        // First creation must not smuggle in providers: install is the only
        // path that may create one.
        let with_provider = config_with(vec![provider("kimi", &[])]);
        assert_eq!(
            merge_config(None, &with_provider, None).unwrap_err(),
            MergeError::FirstCreationWithProviders
        );

        assert_eq!(
            merge_config(None, &draft, Some(&config_with(vec![]))).unwrap_err(),
            MergeError::MissingBase
        );
        assert_eq!(
            merge_config(Some(&config_with(vec![])), &draft, None).unwrap_err(),
            MergeError::StaleBase
        );
    }

    #[test]
    fn m12b_duplicate_provider_ids_are_rejected() {
        let duplicated = || config_with(vec![provider("dup", &[]), provider("dup", &[])]);
        let clean = config_with(vec![provider("dup", &[])]);

        assert_eq!(
            merge_config(Some(&duplicated()), &clean, Some(&clean)).unwrap_err(),
            MergeError::DuplicateProviderId {
                id: "dup".to_string()
            }
        );
        assert_eq!(
            merge_config(Some(&clean), &duplicated(), Some(&clean)).unwrap_err(),
            MergeError::DuplicateProviderId {
                id: "dup".to_string()
            }
        );
        assert_eq!(
            merge_config(Some(&clean), &clean, Some(&duplicated())).unwrap_err(),
            MergeError::DuplicateProviderId {
                id: "dup".to_string()
            }
        );
    }

    #[test]
    fn m13_edited_binding_resets_verification() {
        let base = config_with(vec![provider("kimi", &[("API_KEY", "${env:OLD}")])]);
        let mut next = base.clone();
        next.providers[0] = with_env(&next.providers[0], &[("API_KEY", "${env:NEW}")]);
        let current = base.clone();

        let (result, _) = merged(merge_config(Some(&base), &next, Some(&current)).unwrap());
        match &result.providers[0] {
            ProviderConfig::Remote {
                enabled,
                setup_state,
                setup_last_tested_at,
                ..
            } => {
                assert!(!enabled);
                assert_eq!(*setup_state, ProviderSetupState::Unverified);
                assert!(setup_last_tested_at.is_none());
            }
        }
    }

    #[test]
    fn m14_system_field_edited_by_the_draft_is_kept_from_disk_with_a_warning() {
        let base = config_with(vec![provider("kimi", &[])]);
        let mut next = base.clone();
        next.providers[0] = with_updated_at(&next.providers[0], "2026-01-01T00:00:00Z");
        let current = base.clone();

        let (result, warnings) = merged(merge_config(Some(&base), &next, Some(&current)).unwrap());
        match &result.providers[0] {
            ProviderConfig::Remote { updated_at, .. } => assert!(updated_at.is_none()),
        }
        assert!(warnings.iter().any(|warning| warning.contains("updatedAt")));
    }

    #[test]
    fn next_only_provider_is_rejected() {
        let base = config_with(vec![]);
        let next = config_with(vec![provider("sneaky", &[])]);
        let current = base.clone();

        let fields = conflicts(merge_config(Some(&base), &next, Some(&current)).unwrap());
        assert_eq!(fields, vec!["providers[sneaky]".to_string()]);
    }

    // --- Vertical verification against the real config file (M-series tail) ---

    fn write_config(
        temp: &tempfile::TempDir,
        name: &str,
        config: &AppConfig,
    ) -> std::path::PathBuf {
        let path = temp.path().join(name);
        crate::config::save_config_to_path(&path, config).expect("save config");
        path
    }

    fn load_config(path: &std::path::Path) -> AppConfig {
        crate::config::load_or_create_config(path)
            .expect("load config")
            .config
    }

    #[test]
    fn vertical_merge_round_trips_through_the_real_config_file() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = write_config(
            &temp,
            "config.quotaBarWin.json",
            &config_with(vec![provider("kimi", &[("API_KEY", "${env:OLD}")])]),
        );

        let base = load_config(&path);

        // Backend command adopts the credential while the renderer holds `base`.
        let mut adopted = base.clone();
        adopted.providers[0] = with_env(
            &adopted.providers[0],
            &[("API_KEY", "${secret:providers/kimi/API_KEY}")],
        );
        crate::config::save_config_to_path(&path, &adopted).expect("save adopt");

        let current = load_config(&path);
        let mut next = base.clone();
        next.refresh_interval_seconds = 60;

        let (merged_config, warnings) =
            merged(merge_config(Some(&base), &next, Some(&current)).unwrap());
        crate::config::save_config_to_path(&path, &merged_config).expect("save merged");

        let reloaded = load_config(&path);
        assert_eq!(reloaded.refresh_interval_seconds, 60);
        assert_eq!(
            env_of(&reloaded.providers[0])
                .get("API_KEY")
                .map(String::as_str),
            Some("${secret:providers/kimi/API_KEY}")
        );
        assert!(warnings.is_empty());
    }

    #[test]
    fn vertical_conflict_leaves_the_config_file_untouched() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = write_config(
            &temp,
            "config.quotaBarWin.json",
            &config_with(vec![provider("kimi", &[("API_KEY", "${env:OLD}")])]),
        );

        let base = load_config(&path);

        let mut adopted = base.clone();
        adopted.providers[0] = with_env(
            &adopted.providers[0],
            &[("API_KEY", "${secret:providers/kimi/API_KEY}")],
        );
        crate::config::save_config_to_path(&path, &adopted).expect("save adopt");

        let current = load_config(&path);
        let mut next = base.clone();
        next.providers[0] = with_env(&next.providers[0], &[("API_KEY", "${env:NEW}")]);

        let fields = conflicts(merge_config(Some(&base), &next, Some(&current)).unwrap());
        assert_eq!(fields, vec!["providers[kimi].envVars".to_string()]);

        // Caller must not write on conflict; the file keeps the adopted binding.
        let on_disk = load_config(&path);
        assert_eq!(
            env_of(&on_disk.providers[0])
                .get("API_KEY")
                .map(String::as_str),
            Some("${secret:providers/kimi/API_KEY}")
        );
    }
}
