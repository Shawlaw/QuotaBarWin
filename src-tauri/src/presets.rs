use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    vec![ProviderPreset {
        id: "codex-usage".to_string(),
        display_name: "Codex Usage".to_string(),
        description: "Codex/OpenAI 5h and weekly quota via ChatGPT usage API".to_string(),
        provider_config_template: ProviderConfig::Codex {
            id: "codex".to_string(),
            name: "Codex".to_string(),
            enabled: true,
            auth_token: "${secret:CODEX_ACCESS_TOKEN}".to_string(),
            account_id: None,
            proxy_url: None,
            timeout_ms: 15000,
            window_label_overrides: HashMap::from([
                ("5h".to_string(), "5h".to_string()),
                ("weekly".to_string(), "Weekly limit".to_string()),
            ]),
            visible_window_ids: vec!["5h".to_string(), "Weekly limit".to_string()],
        },
        required_env_vars: vec!["CODEX_ACCESS_TOKEN".to_string()],
        docs: Some(
            "Provide a ChatGPT/Codex access token in secrets/CODEX_ACCESS_TOKEN.txt, or set CODEX_ACCESS_TOKEN in the environment."
                .to_string(),
        ),
    }]
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
    fn preset_codex_uses_native_token_provider() {
        let preset = builtin_provider_presets()
            .into_iter()
            .find(|preset| preset.id == "codex-usage")
            .expect("codex preset");

        assert!(preset
            .required_env_vars
            .contains(&"CODEX_ACCESS_TOKEN".to_string()));
        match preset.provider_config_template {
            ProviderConfig::Codex {
                auth_token,
                account_id,
                proxy_url,
                window_label_overrides,
                visible_window_ids,
                ..
            } => {
                assert_eq!(auth_token, "${secret:CODEX_ACCESS_TOKEN}");
                assert_eq!(account_id, None);
                assert_eq!(proxy_url, None);
                assert_eq!(window_label_overrides.get("5h"), Some(&"5h".to_string()));
                assert_eq!(
                    window_label_overrides.get("weekly"),
                    Some(&"Weekly limit".to_string())
                );
                assert_eq!(
                    visible_window_ids,
                    vec!["5h".to_string(), "Weekly limit".to_string()]
                );
            }
            _ => panic!("expected codex provider"),
        }
    }

    #[test]
    fn presets_do_not_include_local_script_or_command_providers() {
        let presets = builtin_provider_presets();

        assert_eq!(presets.len(), 1);
        assert!(presets.iter().all(|preset| {
            matches!(
                preset.provider_config_template,
                ProviderConfig::Codex { .. }
            )
        }));
    }

    #[test]
    fn preset_add_creates_codex_provider_config() {
        let provider = provider_config_from_preset("codex-usage").expect("provider");

        assert!(matches!(provider, ProviderConfig::Codex { .. }));
    }
}
