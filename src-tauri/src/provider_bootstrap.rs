use std::{collections::HashSet, path::Path};

use crate::{
    bundled_providers,
    config::{load_or_create_config, save_config_to_path, AppConfig, ProviderConfig},
    logger::{LogLevel, LogSink},
    remote_provider::load_cached_manifest,
    remote_provider_commands::{install_remote_provider_from_source, remote_provider_dir},
};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct BundleSeedReport {
    pub installed: Vec<String>,
    pub retained: Vec<String>,
    pub failed: Vec<(String, String)>,
}

/// Offers the Providers embedded in this build to the installation.
///
/// Fresh installations receive every embedded Provider. Existing installations
/// only receive Providers they have not seen yet; an id stays in
/// `seededProviderIds` after the Provider is removed so upgrades never
/// resurrect a Provider the user deleted. Installed Providers are never
/// modified, and their `manifestUrl` stays whatever the user installed.
pub fn seed_bundled_providers(
    config_path: &Path,
    log: Option<&LogSink>,
) -> Result<BundleSeedReport, String> {
    let bundle = bundled_providers::bundle()?;
    let mut report = BundleSeedReport::default();
    let mut loaded = load_or_create_config(config_path)?;
    let local_log = LogSink::from_config_path(config_path, &loaded.config);
    let log_sink = log.unwrap_or(&local_log);

    // Fresh configurations already default to the GitHub main registry. Only
    // fill in an empty value, never replace a registry the user configured.
    let configured_registry = loaded
        .config
        .remote_provider_registry
        .registry_url
        .as_deref()
        .map(str::trim)
        .unwrap_or("");
    if configured_registry.is_empty() && loaded.config.remote_provider_registry.sources.is_empty() {
        loaded.config.remote_provider_registry.registry_url = Some(bundle.registry_url.clone());
        save_config_to_path(config_path, &loaded.config)?;
        log_seed(
            log_sink,
            LogLevel::Info,
            &format!(
                "bundled provider registry aligned url={}",
                bundle.registry_url
            ),
        );
    }

    let installed_ids = installed_manifest_ids(config_path, &loaded.config);
    let installed_urls = loaded
        .config
        .providers
        .iter()
        .filter_map(manifest_url)
        .collect::<HashSet<&str>>();
    let mut seeded_ids = loaded.config.provider_bundle.seeded_provider_ids.clone();
    let mut seeded_set = seeded_ids.iter().cloned().collect::<HashSet<_>>();

    log_seed(
        log_sink,
        LogLevel::Info,
        &format!(
            "bundled provider seed started providers={} seeded={} bundleAppVersion={}",
            bundle.providers.len(),
            seeded_ids.len(),
            bundle.app_version
        ),
    );

    for provider in &bundle.providers {
        if seeded_set.contains(&provider.id) {
            continue;
        }

        let already_installed = installed_ids
            .iter()
            .any(|installed| provider_instance_matches_manifest(installed, &provider.id))
            || installed_urls.contains(provider.manifest_url.as_str());
        if already_installed {
            seeded_ids.push(provider.id.clone());
            seeded_set.insert(provider.id.clone());
            report.retained.push(provider.id.clone());
            log_seed(
                log_sink,
                LogLevel::Info,
                &format!(
                    "bundled provider retained without changes id={}",
                    provider.id
                ),
            );
            continue;
        }

        let manifest = match provider.parsed_manifest() {
            Ok(manifest) => manifest,
            Err(error) => {
                report.failed.push((provider.id.clone(), error.clone()));
                log_seed(
                    log_sink,
                    LogLevel::Warn,
                    &format!(
                        "bundled provider manifest rejected id={} error={}",
                        provider.id, error
                    ),
                );
                continue;
            }
        };

        match install_remote_provider_from_source(
            config_path,
            &provider.manifest_url,
            // Records the update-source preference only. This change never
            // downloads or applies Provider updates automatically.
            true,
            manifest,
            provider.source.clone(),
            log_sink,
        ) {
            Ok(config) => {
                seeded_ids.push(provider.id.clone());
                seeded_set.insert(provider.id.clone());
                report.installed.push(provider.id.clone());
                log_seed(
                    log_sink,
                    LogLevel::Info,
                    &format!(
                        "bundled provider installed id={} name={} version={} instance={}",
                        provider.id,
                        provider.display_name,
                        provider.version.as_deref().unwrap_or("none"),
                        provider_id(&config)
                    ),
                );
            }
            Err(error) => {
                report.failed.push((provider.id.clone(), error.clone()));
                log_seed(
                    log_sink,
                    LogLevel::Warn,
                    &format!(
                        "bundled provider install failed id={} error={}",
                        provider.id, error
                    ),
                );
            }
        }
    }

    let mut persisted = load_or_create_config(config_path)?;
    if persisted.config.provider_bundle.seeded_provider_ids != seeded_ids {
        persisted.config.provider_bundle.seeded_provider_ids = seeded_ids;
        save_config_to_path(config_path, &persisted.config)?;
    }

    log_seed(
        log_sink,
        LogLevel::Info,
        &format!(
            "bundled provider seed finished installed={} retained={} failed={}",
            report.installed.len(),
            report.retained.len(),
            report.failed.len()
        ),
    );
    Ok(report)
}

fn manifest_url(provider: &ProviderConfig) -> Option<&str> {
    match provider {
        ProviderConfig::Remote { manifest_url, .. } => Some(manifest_url.as_str()),
    }
}

fn provider_id(provider: &ProviderConfig) -> &str {
    match provider {
        ProviderConfig::Remote { id, .. } => id,
    }
}

fn installed_manifest_ids(config_path: &Path, config: &AppConfig) -> HashSet<String> {
    let mut ids = HashSet::new();
    for provider in &config.providers {
        let ProviderConfig::Remote { id, .. } = provider;
        ids.insert(id.clone());
        if let Ok(provider_dir) = remote_provider_dir(config_path, id) {
            if let Ok(manifest) = load_cached_manifest(&provider_dir) {
                ids.insert(manifest.id);
            }
        }
    }
    ids
}

fn provider_instance_matches_manifest(instance_id: &str, manifest_id: &str) -> bool {
    if instance_id == manifest_id {
        return true;
    }
    instance_id
        .strip_prefix(manifest_id)
        .and_then(|suffix| suffix.strip_prefix('-'))
        .is_some_and(|suffix| {
            !suffix.is_empty() && suffix.chars().all(|character| character.is_ascii_digit())
        })
}

fn log_seed(log: &LogSink, level: LogLevel, message: &str) {
    let _ = log.write(level, "provider_bootstrap", message);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{save_config_to_path, ProviderSetupState};

    fn config_path(temp: &tempfile::TempDir) -> std::path::PathBuf {
        temp.path().join("config.quotaBarWin.json")
    }

    #[test]
    fn fresh_install_receives_every_embedded_provider() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = config_path(&temp);

        let report = seed_bundled_providers(&path, None).expect("seed");
        let loaded = load_or_create_config(&path).expect("load config");
        let bundle = bundled_providers::bundle().expect("bundle");

        assert_eq!(report.installed.len(), bundle.providers.len(), "{report:?}");
        assert!(report.retained.is_empty());
        assert!(report.failed.is_empty());
        assert_eq!(loaded.config.providers.len(), bundle.providers.len());
        assert_eq!(
            loaded.config.provider_bundle.seeded_provider_ids,
            bundle
                .providers
                .iter()
                .map(|provider| provider.id.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            loaded
                .config
                .remote_provider_registry
                .registry_url
                .as_deref(),
            Some(bundle.registry_url.as_str())
        );

        for provider in &bundle.providers {
            let configured = loaded
                .config
                .providers
                .iter()
                .find(|configured| {
                    matches!(configured, ProviderConfig::Remote { id, .. } if id == &provider.id)
                })
                .expect("bundled provider is installed");
            let ProviderConfig::Remote {
                enabled,
                setup_state,
                manifest_url,
                auto_update,
                ..
            } = configured;
            assert!(!*enabled);
            assert_eq!(*setup_state, ProviderSetupState::Pending);
            assert_eq!(manifest_url, &provider.manifest_url);
            assert!(*auto_update);
            let provider_dir = remote_provider_dir(&path, &provider.id).expect("provider dir");
            assert!(provider_dir.join("provider.json").is_file());
            assert!(provider_dir.join("provider.js").is_file());
        }

        let second = seed_bundled_providers(&path, None).expect("second seed");
        assert!(second.installed.is_empty());
        assert!(second.retained.is_empty());
        let reloaded = load_or_create_config(&path).expect("reload");
        assert_eq!(reloaded.config.providers.len(), bundle.providers.len());
    }

    #[test]
    fn existing_provider_is_retained_and_not_modified() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = config_path(&temp);
        let bundle = bundled_providers::bundle().expect("bundle");
        let first = &bundle.providers[0];
        let manifest = first.parsed_manifest().expect("manifest");

        install_remote_provider_from_source(
            &path,
            &first.manifest_url,
            false,
            manifest,
            first.source.clone(),
            &LogSink::from_config_path(&path, &crate::config::default_config()),
        )
        .expect("install existing provider");

        let mut loaded = load_or_create_config(&path).expect("load");
        let ProviderConfig::Remote {
            name,
            enabled,
            env_vars,
            ..
        } = loaded.config.providers.first_mut().expect("provider");
        *name = "Custom Name".to_string();
        *enabled = true;
        env_vars.insert("CUSTOM".to_string(), "value".to_string());
        save_config_to_path(&path, &loaded.config).expect("save custom config");

        let report = seed_bundled_providers(&path, None).expect("seed");
        let reloaded = load_or_create_config(&path).expect("reload");

        assert_eq!(report.installed.len(), bundle.providers.len() - 1);
        assert_eq!(report.retained, vec![first.id.clone()]);
        assert_eq!(reloaded.config.providers.len(), bundle.providers.len());
        let existing = reloaded
            .config
            .providers
            .iter()
            .find(|provider| provider_id(provider) == first.id)
            .expect("existing provider");
        let ProviderConfig::Remote {
            name,
            enabled,
            env_vars,
            ..
        } = existing;
        assert_eq!(name.as_str(), "Custom Name");
        assert!(*enabled);
        assert_eq!(env_vars.get("CUSTOM").map(String::as_str), Some("value"));
    }

    #[test]
    fn removed_provider_is_not_resurrected_on_the_next_start() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = config_path(&temp);
        seed_bundled_providers(&path, None).expect("seed");

        let bundle = bundled_providers::bundle().expect("bundle");
        let removed = &bundle.providers[0];
        let mut loaded = load_or_create_config(&path).expect("load");
        loaded
            .config
            .providers
            .retain(|provider| provider_id(provider) != removed.id);
        save_config_to_path(&path, &loaded.config).expect("save removal");

        let report = seed_bundled_providers(&path, None).expect("seed after removal");

        assert!(report.installed.is_empty());
        assert!(report.retained.is_empty());
        let reloaded = load_or_create_config(&path).expect("reload");
        assert!(!reloaded
            .config
            .providers
            .iter()
            .any(|provider| provider_id(provider) == removed.id));
        assert!(reloaded
            .config
            .provider_bundle
            .seeded_provider_ids
            .contains(&removed.id));
    }

    #[test]
    fn configured_registry_is_not_overwritten() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = config_path(&temp);
        let mut config = crate::config::default_config();
        config.remote_provider_registry.registry_url =
            Some("https://example.com/custom-registry.json".to_string());
        save_config_to_path(&path, &config).expect("save");

        seed_bundled_providers(&path, None).expect("seed");

        let loaded = load_or_create_config(&path).expect("load");
        assert_eq!(
            loaded
                .config
                .remote_provider_registry
                .registry_url
                .as_deref(),
            Some("https://example.com/custom-registry.json")
        );
    }

    #[test]
    fn empty_registry_url_is_aligned_to_the_bundle_main_channel() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = config_path(&temp);
        let mut config = crate::config::default_config();
        config.remote_provider_registry.registry_url = None;
        save_config_to_path(&path, &config).expect("save");

        seed_bundled_providers(&path, None).expect("seed");

        let loaded = load_or_create_config(&path).expect("load");
        let bundle = bundled_providers::bundle().expect("bundle");
        assert_eq!(
            loaded
                .config
                .remote_provider_registry
                .registry_url
                .as_deref(),
            Some(bundle.registry_url.as_str())
        );
    }
}
