use std::{collections::HashSet, path::Path};

use chrono::Utc;

use crate::{
    bundled_providers::{self, BundledProvider},
    config::{load_or_create_config, save_config_to_path, AppConfig},
    logger::{LogLevel, LogSink},
    remote_provider::{
        load_remote_provider_catalog_cache, save_remote_provider_catalog_cache,
        CachedRemoteProviderCatalog, CachedRemoteProviderCatalogEntry,
    },
};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct BundleSeedReport {
    pub materialized: Vec<String>,
    pub skipped: Vec<String>,
    pub failed: Vec<(String, String)>,
}

/// Offers the Providers embedded in this build to the installation.
///
/// Seeding is equivalent to the user having opened the add-provider page and
/// fetched the project registry once: the embedded manifests and sources are
/// materialized into that registry's catalog cache so every embedded Provider
/// is immediately installable from the add-provider page (offline included)
/// without an extra manual refresh. No Provider is installed, enabled, or
/// configured here — adding an account always goes through the add-provider
/// page and its guided setup flow.
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

    if !bundle_registry_is_configured(&loaded.config, &bundle.registry_url) {
        log_seed(
            log_sink,
            LogLevel::Info,
            "bundled provider seed skipped: the bundle registry is not configured",
        );
        return Ok(report);
    }

    log_seed(
        log_sink,
        LogLevel::Info,
        &format!(
            "bundled provider seed started providers={} bundleAppVersion={}",
            bundle.providers.len(),
            bundle.app_version
        ),
    );

    // The catalog cache is a fetch cache: entries the user already pulled may
    // be newer than this build's snapshot, so only missing ids are appended
    // and `refreshedAt` is preserved to keep existing catalog snapshot tokens
    // valid for offline installs. A cache that fails validation is rebuilt
    // from the embedded bundle.
    let existing = match load_remote_provider_catalog_cache(config_path, &bundle.registry_url) {
        Ok(cache) => cache,
        Err(error) => {
            log_seed(
                log_sink,
                LogLevel::Warn,
                &format!(
                    "bundled provider catalog cache rejected, rebuilding error={}",
                    error
                ),
            );
            None
        }
    };
    let mut cache = existing
        .unwrap_or_else(|| CachedRemoteProviderCatalog::new(&bundle.registry_url, now_timestamp()));

    let present: HashSet<String> = cache
        .entries
        .iter()
        .flat_map(|entry| [entry.provider_url.clone(), entry.id.clone()])
        .collect();
    let fetched_at = now_timestamp();
    for provider in &bundle.providers {
        if present.contains(provider.manifest_url.as_str()) || present.contains(&provider.id) {
            report.skipped.push(provider.id.clone());
            continue;
        }
        cache
            .entries
            .push(catalog_entry_from_bundled(provider, &fetched_at));
        report.materialized.push(provider.id.clone());
    }

    if report.materialized.is_empty() {
        log_seed(
            log_sink,
            LogLevel::Info,
            &format!(
                "bundled provider seed finished materialized=0 skipped={} failed=0",
                report.skipped.len()
            ),
        );
        return Ok(report);
    }

    if let Err(error) = save_remote_provider_catalog_cache(config_path, &cache) {
        report.failed = report
            .materialized
            .iter()
            .map(|id| (id.clone(), error.clone()))
            .collect();
        report.materialized.clear();
        log_seed(
            log_sink,
            LogLevel::Warn,
            &format!("bundled provider catalog cache save failed error={}", error),
        );
    } else {
        log_seed(
            log_sink,
            LogLevel::Info,
            &format!(
                "bundled provider seed finished materialized={} skipped={} failed=0",
                report.materialized.len(),
                report.skipped.len()
            ),
        );
    }

    Ok(report)
}

fn now_timestamp() -> String {
    Utc::now().to_rfc3339()
}

fn catalog_entry_from_bundled(
    provider: &BundledProvider,
    fetched_at: &str,
) -> CachedRemoteProviderCatalogEntry {
    CachedRemoteProviderCatalogEntry {
        id: provider.id.clone(),
        provider_url: provider.manifest_url.clone(),
        checksum: Some(provider.manifest_checksum.clone()),
        manifest_text: provider.manifest.clone(),
        source_url: provider.source_url.clone(),
        source: provider.source.clone(),
        source_checksum: provider.source_checksum.clone(),
        fetched_at: fetched_at.to_string(),
    }
}

/// The bundle is materialized only into registries the add-provider page
/// actually loads: with a custom registry configured, a cache for the official
/// registry would never be read.
fn bundle_registry_is_configured(config: &AppConfig, bundle_registry_url: &str) -> bool {
    let registry = &config.remote_provider_registry;
    let bundle_registry_url = bundle_registry_url.trim();
    if registry
        .sources
        .iter()
        .any(|source| source.enabled && source.url.trim() == bundle_registry_url)
    {
        return true;
    }
    registry.sources.is_empty()
        && registry
            .registry_url
            .as_deref()
            .map(str::trim)
            .is_some_and(|url| url == bundle_registry_url)
}

fn log_seed(log: &LogSink, level: LogLevel, message: &str) {
    let _ = log.write(level, "provider_bootstrap", message);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote_provider::load_remote_provider_catalog_cache;

    fn config_path(temp: &tempfile::TempDir) -> std::path::PathBuf {
        temp.path().join("config.quotaBarWin.json")
    }

    fn seed_cache_ids(path: &std::path::PathBuf) -> Vec<String> {
        let cache = load_remote_provider_catalog_cache(path, &bundled_providers::bundle()
            .expect("bundle")
            .registry_url)
            .expect("valid catalog cache")
            .expect("catalog cache exists");
        cache.entries.into_iter().map(|entry| entry.id).collect()
    }

    #[test]
    fn fresh_install_materializes_the_official_catalog_without_installing() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = config_path(&temp);

        let report = seed_bundled_providers(&path, None).expect("seed");
        let bundle = bundled_providers::bundle().expect("bundle");
        let loaded = load_or_create_config(&path).expect("load config");

        assert_eq!(
            report.materialized,
            bundle
                .providers
                .iter()
                .map(|provider| provider.id.clone())
                .collect::<Vec<_>>(),
            "{report:?}"
        );
        assert!(report.skipped.is_empty());
        assert!(report.failed.is_empty());
        // Seeding never installs Providers: adding an account goes through the
        // add-provider page instead of the old pending "complete setup" flow.
        assert!(loaded.config.providers.is_empty());
        assert_eq!(
            seed_cache_ids(&path),
            bundle
                .providers
                .iter()
                .map(|provider| provider.id.clone())
                .collect::<Vec<_>>()
        );

        // A second start is a no-op: nothing is re-materialized or rewritten.
        let second = seed_bundled_providers(&path, None).expect("second seed");
        assert!(second.materialized.is_empty());
        assert!(second.failed.is_empty());
        assert_eq!(second.skipped.len(), bundle.providers.len());
        assert_eq!(
            seed_cache_ids(&path),
            bundle
                .providers
                .iter()
                .map(|provider| provider.id.clone())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn stale_catalog_cache_gains_missing_bundled_providers_only() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = config_path(&temp);
        let bundle = bundled_providers::bundle().expect("bundle");
        assert!(bundle.providers.len() > 1, "bundle has several providers");

        // Simulate a cache written by an older build: everything except the
        // last embedded Provider, pulled from the registry some time ago.
        let mut older = CachedRemoteProviderCatalog::new(
            &bundle.registry_url,
            "2026-01-01T00:00:00+00:00".to_string(),
        );
        for provider in &bundle.providers[..bundle.providers.len() - 1] {
            older.entries.push(catalog_entry_from_bundled(
                provider,
                "2026-01-01T00:00:01+00:00",
            ));
        }
        save_remote_provider_catalog_cache(&path, &older).expect("write older cache");

        let report = seed_bundled_providers(&path, None).expect("seed");
        let added = bundle.providers.last().expect("last provider");

        assert_eq!(report.materialized, vec![added.id.clone()]);
        assert_eq!(report.skipped.len(), bundle.providers.len() - 1);

        let cache = load_remote_provider_catalog_cache(&path, &bundle.registry_url)
            .expect("valid cache")
            .expect("cache");
        assert_eq!(cache.refreshed_at, "2026-01-01T00:00:00+00:00");
        assert_eq!(cache.entries.len(), bundle.providers.len());
        let appended = cache.entries.last().expect("appended entry");
        assert_eq!(appended.id, added.id);
        assert_ne!(appended.fetched_at, "2026-01-01T00:00:01+00:00");
        // Pre-existing entries keep their identity, including fetchedAt.
        assert_eq!(cache.entries[0].fetched_at, "2026-01-01T00:00:01+00:00");
    }

    #[test]
    fn corrupted_catalog_cache_is_rebuilt_from_the_bundle() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = config_path(&temp);
        let bundle = bundled_providers::bundle().expect("bundle");
        let cache_path = crate::remote_provider::remote_provider_catalog_cache_path(
            &path,
            &bundle.registry_url,
        )
        .expect("cache path");
        std::fs::create_dir_all(cache_path.parent().expect("cache dir")).expect("cache dir");
        std::fs::write(&cache_path, "not json").expect("corrupt cache");

        let report = seed_bundled_providers(&path, None).expect("seed");

        assert_eq!(report.materialized.len(), bundle.providers.len());
        assert_eq!(seed_cache_ids(&path).len(), bundle.providers.len());
    }

    #[test]
    fn custom_registry_is_never_seeded_or_overwritten() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = config_path(&temp);
        let mut config = crate::config::default_config();
        config.remote_provider_registry.registry_url =
            Some("https://example.com/custom-registry.json".to_string());
        crate::config::save_config_to_path(&path, &config).expect("save");

        let report = seed_bundled_providers(&path, None).expect("seed");
        let bundle = bundled_providers::bundle().expect("bundle");
        let loaded = load_or_create_config(&path).expect("load");

        assert!(report.materialized.is_empty());
        assert_eq!(
            loaded
                .config
                .remote_provider_registry
                .registry_url
                .as_deref(),
            Some("https://example.com/custom-registry.json")
        );
        let cache_path = crate::remote_provider::remote_provider_catalog_cache_path(
            &path,
            &bundle.registry_url,
        )
        .expect("cache path");
        assert!(!cache_path.exists());
    }

    #[test]
    fn bundle_registry_among_custom_sources_is_seeded() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = config_path(&temp);
        let bundle = bundled_providers::bundle().expect("bundle");
        let mut config = crate::config::default_config();
        config.remote_provider_registry.registry_url =
            Some("https://example.com/custom-registry.json".to_string());
        config.remote_provider_registry.sources = vec![
            crate::config::RemoteProviderRegistrySource {
                id: "custom".to_string(),
                name: "Custom".to_string(),
                url: "https://example.com/custom-registry.json".to_string(),
                provider_proxy_url: None,
                auto_update: false,
                enabled: true,
            },
            crate::config::RemoteProviderRegistrySource {
                id: "official".to_string(),
                name: "Official".to_string(),
                url: bundle.registry_url.clone(),
                provider_proxy_url: None,
                auto_update: true,
                enabled: true,
            },
        ];
        crate::config::save_config_to_path(&path, &config).expect("save");

        let report = seed_bundled_providers(&path, None).expect("seed");

        assert_eq!(report.materialized.len(), bundle.providers.len());
        assert_eq!(seed_cache_ids(&path).len(), bundle.providers.len());
        // The custom registry keeps its primary slot.
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
    fn disabled_official_source_is_not_seeded() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = config_path(&temp);
        let bundle = bundled_providers::bundle().expect("bundle");
        let mut config = crate::config::default_config();
        config.remote_provider_registry.sources = vec![
            crate::config::RemoteProviderRegistrySource {
                id: "official".to_string(),
                name: "Official".to_string(),
                url: bundle.registry_url.clone(),
                provider_proxy_url: None,
                auto_update: true,
                enabled: false,
            },
        ];
        crate::config::save_config_to_path(&path, &config).expect("save");

        let report = seed_bundled_providers(&path, None).expect("seed");

        assert!(report.materialized.is_empty());
        let cache_path = crate::remote_provider::remote_provider_catalog_cache_path(
            &path,
            &bundle.registry_url,
        )
        .expect("cache path");
        assert!(!cache_path.exists());
    }

    #[test]
    fn empty_registry_url_is_aligned_to_the_bundle_main_channel() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = config_path(&temp);
        let mut config = crate::config::default_config();
        config.remote_provider_registry.registry_url = None;
        crate::config::save_config_to_path(&path, &config).expect("save");

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
