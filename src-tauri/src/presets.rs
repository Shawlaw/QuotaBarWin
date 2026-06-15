use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle};

use crate::config::ProviderConfig;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPreset {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub provider_config_template: ProviderConfig,
    #[serde(default)]
    pub required_env_vars: Vec<String>,
    #[serde(default)]
    pub docs: Option<String>,
}

pub fn builtin_provider_presets() -> Vec<ProviderPreset> {
    Vec::new()
}

#[cfg(test)]
pub fn provider_config_from_preset(preset_id: &str) -> Option<ProviderConfig> {
    builtin_provider_presets()
        .into_iter()
        .find(|preset| preset.id == preset_id)
        .map(|preset| preset.provider_config_template)
}

#[command]
pub fn get_provider_presets(_app: AppHandle) -> Result<Vec<ProviderPreset>, String> {
    Ok(builtin_provider_presets())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_do_not_include_builtin_providers() {
        let presets = builtin_provider_presets();

        assert!(presets.is_empty());
    }

    #[test]
    fn preset_add_returns_none_without_builtin_provider_config() {
        assert!(provider_config_from_preset("codex-usage").is_none());
    }
}
