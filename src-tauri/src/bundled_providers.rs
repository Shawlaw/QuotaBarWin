use std::{collections::HashSet, sync::OnceLock};

use serde::Deserialize;

use crate::remote_provider::{
    parse_manifest, resolve_source_url, verify_checksum, ProviderManifest,
};

/// Provider scripts embedded into the executable at build time. The JSON file
/// is generated from `examples/remote-providers` by
/// `scripts/generate-provider-bundle.mjs`; never edit it by hand.
const PROVIDER_BUNDLE_JSON: &str = include_str!("provider_bundle.json");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderBundle {
    pub schema_version: u32,
    pub app_version: String,
    pub registry_url: String,
    pub providers: Vec<BundledProvider>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundledProvider {
    pub id: String,
    pub display_name: String,
    pub version: Option<String>,
    pub manifest_url: String,
    pub source_url: String,
    pub manifest_checksum: String,
    pub source_checksum: String,
    pub manifest: String,
    pub source: String,
}

impl BundledProvider {
    pub fn parsed_manifest(&self) -> Result<ProviderManifest, String> {
        parse_manifest(&self.manifest).map_err(|error| error.to_string())
    }
}

pub fn bundle() -> Result<&'static ProviderBundle, String> {
    static BUNDLE: OnceLock<Result<ProviderBundle, String>> = OnceLock::new();
    BUNDLE
        .get_or_init(|| {
            let bundle: ProviderBundle = serde_json::from_str(PROVIDER_BUNDLE_JSON)
                .map_err(|error| format!("invalid bundled Provider data: {error}"))?;
            validate_bundle(&bundle)?;
            Ok(bundle)
        })
        .as_ref()
        .map_err(Clone::clone)
}

fn validate_bundle(bundle: &ProviderBundle) -> Result<(), String> {
    if bundle.schema_version != 1 {
        return Err(format!(
            "unsupported bundled Provider schema version {}",
            bundle.schema_version
        ));
    }
    if bundle.registry_url.trim().is_empty() {
        return Err("bundled Provider registry URL is empty".to_string());
    }
    if bundle.providers.is_empty() {
        return Err("bundled Provider list is empty".to_string());
    }

    let mut seen = HashSet::new();
    for provider in &bundle.providers {
        if provider.id.trim().is_empty() {
            return Err("bundled Provider id is empty".to_string());
        }
        if !seen.insert(provider.id.as_str()) {
            return Err(format!("duplicate bundled Provider id '{}'", provider.id));
        }
        verify_checksum(&provider.manifest, &provider.manifest_checksum)
            .map_err(|error| format!("{} manifest: {error}", provider.id))?;
        let manifest = provider.parsed_manifest()?;
        if manifest.id != provider.id {
            return Err(format!(
                "bundled Provider '{}' manifest declares id '{}'",
                provider.id, manifest.id
            ));
        }
        let declared_source_checksum = manifest
            .checksums
            .source
            .as_deref()
            .ok_or_else(|| format!("{} manifest is missing checksums.source", provider.id))?;
        if declared_source_checksum != provider.source_checksum {
            return Err(format!(
                "{} source checksum does not match its manifest",
                provider.id
            ));
        }
        verify_checksum(&provider.source, &provider.source_checksum)
            .map_err(|error| format!("{} source: {error}", provider.id))?;
        let resolved_source_url = resolve_source_url(&provider.manifest_url, &manifest.entry);
        if resolved_source_url != provider.source_url {
            return Err(format!(
                "{} source URL '{}' does not match '{}'",
                provider.id, provider.source_url, resolved_source_url
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote_provider::ProviderRegistry;
    use std::path::PathBuf;

    fn examples_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repo root")
            .join("examples")
            .join("remote-providers")
    }

    #[test]
    fn embedded_bundle_is_valid_and_matches_the_running_app_version() {
        let bundle = bundle().expect("embedded Provider bundle should validate");
        assert_eq!(bundle.app_version, env!("CARGO_PKG_VERSION"));
        assert!(bundle.providers.len() >= 6);
        assert!(bundle
            .registry_url
            .starts_with("https://raw.githubusercontent.com/Shawlaw/QuotaBarWin/main/"));
        assert!(bundle.providers.iter().all(|provider| provider
            .manifest_url
            .starts_with("https://raw.githubusercontent.com/Shawlaw/QuotaBarWin/main/")));
    }

    #[test]
    fn embedded_bundle_matches_the_example_registry_and_sources() {
        let examples = examples_dir();
        let registry_text =
            std::fs::read_to_string(examples.join("registry.json")).expect("read example registry");
        let registry: ProviderRegistry =
            serde_json::from_str(&registry_text).expect("parse example registry");
        let bundle = bundle().expect("embedded Provider bundle");

        assert_eq!(
            bundle.providers.len(),
            registry.providers.len(),
            "regenerate the bundle after changing examples/remote-providers/registry.json"
        );

        for (registry_entry, bundled) in registry.providers.iter().zip(&bundle.providers) {
            assert_eq!(registry_entry.id, bundled.id);

            let manifest_path = examples.join(&registry_entry.provider_url);
            let manifest_text =
                std::fs::read_to_string(&manifest_path).expect("read example manifest");
            assert_eq!(
                manifest_text, bundled.manifest,
                "{} manifest drifted from the bundle",
                bundled.id
            );

            let manifest = parse_manifest(&manifest_text).expect("parse example manifest");
            let source_path = manifest_path
                .parent()
                .expect("manifest parent")
                .join(&manifest.entry);
            let source_text = std::fs::read_to_string(&source_path).expect("read example source");
            assert_eq!(
                source_text, bundled.source,
                "{} source drifted from the bundle",
                bundled.id
            );

            if let Some(expected_checksum) = registry_entry.checksum.as_deref() {
                assert_eq!(expected_checksum, bundled.manifest_checksum);
            }
            assert_eq!(
                manifest.checksums.source.as_deref(),
                Some(bundled.source_checksum.as_str())
            );
        }
    }
}
