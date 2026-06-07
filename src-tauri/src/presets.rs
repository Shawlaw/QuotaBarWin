use serde::{Deserialize, Serialize};
use tauri::command;

use crate::{
    command_provider::run_single_provider_config,
    config::{CommandSpec, ParserSpec, ProviderConfig},
    quota::ProviderSnapshot,
};

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
    vec![
        ProviderPreset {
            id: "kimi-coding-usage".to_string(),
            display_name: "Kimi Coding Usage".to_string(),
            description: "Kimi coding quota usage via curl".to_string(),
            provider_config_template: ProviderConfig::Command {
                id: "kimi-coding".to_string(),
                name: "Kimi Coding".to_string(),
                enabled: true,
                command: CommandSpec {
                    executable: "curl".to_string(),
                    args: vec![
                        "-s".to_string(),
                        "-H".to_string(),
                        "Authorization: Bearer ${env:KIMI_API_KEY}".to_string(),
                        "https://api.kimi.com/coding/v1/usages".to_string(),
                    ],
                    cwd: None,
                    env: None,
                    timeout_ms: 15000,
                },
                parser: ParserSpec::KimiCodingUsageV1,
            },
            required_env_vars: vec!["KIMI_API_KEY".to_string()],
            docs: Some("Set KIMI_API_KEY in your environment.".to_string()),
        },
        ProviderPreset {
            id: "bigmodel-zai-coding-plan".to_string(),
            display_name: "BigModel / Z.ai Coding Plan".to_string(),
            description: "BigModel/Z.ai coding plan quota via curl".to_string(),
            provider_config_template: ProviderConfig::Command {
                id: "bigmodel-coding-plan".to_string(),
                name: "BigModel / Z.ai Coding Plan".to_string(),
                enabled: true,
                command: CommandSpec {
                    executable: "curl".to_string(),
                    args: vec![
                        "-s".to_string(),
                        "-H".to_string(),
                        "Authorization: Bearer ${env:BIGMODEL_API_KEY}".to_string(),
                        "https://open.bigmodel.cn/api/monitor/usage/quota/limit".to_string(),
                    ],
                    cwd: None,
                    env: None,
                    timeout_ms: 15000,
                },
                parser: ParserSpec::BigmodelQuotaLimitJsonV1,
            },
            required_env_vars: vec!["BIGMODEL_API_KEY".to_string()],
            docs: Some("Set BIGMODEL_API_KEY in your environment.".to_string()),
        },
        ProviderPreset {
            id: "opencode-quota-command".to_string(),
            display_name: "OpenCode Quota Command".to_string(),
            description: "Read an app snapshot from opencode-quota".to_string(),
            provider_config_template: ProviderConfig::Command {
                id: "opencode-quota".to_string(),
                name: "OpenCode Quota".to_string(),
                enabled: true,
                command: CommandSpec {
                    executable: "opencode-quota".to_string(),
                    args: vec!["show".to_string(), "--json".to_string()],
                    cwd: None,
                    env: None,
                    timeout_ms: 15000,
                },
                parser: ParserSpec::AppSnapshot,
            },
            required_env_vars: Vec::new(),
            docs: Some("Install opencode-quota separately if you want to use this provider.".to_string()),
        },
        ProviderPreset {
            id: "custom-command-provider".to_string(),
            display_name: "Custom Command Provider".to_string(),
            description: "Start from an editable command provider".to_string(),
            provider_config_template: ProviderConfig::Command {
                id: "custom-command".to_string(),
                name: "Custom Command Provider".to_string(),
                enabled: true,
                command: CommandSpec {
                    executable: "node".to_string(),
                    args: vec!["fixtures/fake_provider_snapshot.js".to_string()],
                    cwd: None,
                    env: None,
                    timeout_ms: 15000,
                },
                parser: ParserSpec::ProviderSnapshot,
            },
            required_env_vars: Vec::new(),
            docs: None,
        },
    ]
}

#[cfg(test)]
pub fn provider_config_from_preset(preset_id: &str) -> Option<ProviderConfig> {
    builtin_provider_presets()
        .into_iter()
        .find(|preset| preset.id == preset_id)
        .map(|preset| preset.provider_config_template)
}

#[command]
pub fn get_provider_presets() -> Result<Vec<ProviderPreset>, String> {
    Ok(builtin_provider_presets())
}

#[command]
pub fn test_provider(provider: ProviderConfig) -> Result<ProviderSnapshot, String> {
    Ok(run_single_provider_config(&provider))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CommandSpec, ParserSpec, ProviderConfig};

    fn command_from(provider: &ProviderConfig) -> (&CommandSpec, &ParserSpec) {
        match provider {
            ProviderConfig::Command {
                command, parser, ..
            } => (command, parser),
            _ => panic!("expected command provider"),
        }
    }

    fn fixture_command(provider: ProviderConfig, script: &str) -> ProviderConfig {
        let script = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repo root")
            .join("fixtures")
            .join("commands")
            .join(script)
            .to_string_lossy()
            .to_string();

        match provider {
            ProviderConfig::Command {
                id,
                name,
                enabled,
                parser,
                ..
            } => ProviderConfig::Command {
                id,
                name,
                enabled,
                command: CommandSpec {
                    executable: "node".to_string(),
                    args: vec![script],
                    cwd: None,
                    env: None,
                    timeout_ms: 15000,
                },
                parser,
            },
            other => other,
        }
    }

    #[test]
    fn preset_kimi_has_required_env_var() {
        let preset = builtin_provider_presets()
            .into_iter()
            .find(|preset| preset.id == "kimi-coding-usage")
            .expect("kimi preset");

        assert!(preset.required_env_vars.contains(&"KIMI_API_KEY".to_string()));
        let (command, parser) = command_from(&preset.provider_config_template);
        assert!(command.args.iter().any(|arg| arg.contains("${env:KIMI_API_KEY}")));
        assert_eq!(parser, &ParserSpec::KimiCodingUsageV1);
    }

    #[test]
    fn preset_bigmodel_has_required_env_var() {
        let preset = builtin_provider_presets()
            .into_iter()
            .find(|preset| preset.id == "bigmodel-zai-coding-plan")
            .expect("bigmodel preset");

        assert!(preset
            .required_env_vars
            .contains(&"BIGMODEL_API_KEY".to_string()));
        let (command, parser) = command_from(&preset.provider_config_template);
        assert!(command
            .args
            .iter()
            .any(|arg| arg.contains("${env:BIGMODEL_API_KEY}")));
        assert_eq!(parser, &ParserSpec::BigmodelQuotaLimitJsonV1);
    }

    #[test]
    fn preset_opencode_quota_uses_app_snapshot_parser() {
        let preset = builtin_provider_presets()
            .into_iter()
            .find(|preset| preset.id == "opencode-quota-command")
            .expect("opencode preset");
        let (command, parser) = command_from(&preset.provider_config_template);

        assert_eq!(command.executable, "opencode-quota");
        assert_eq!(parser, &ParserSpec::AppSnapshot);
    }

    #[test]
    fn preset_add_creates_provider_config() {
        let provider = provider_config_from_preset("custom-command-provider").expect("provider");

        assert!(matches!(provider, ProviderConfig::Command { .. }));
    }

    #[test]
    fn fixture_kimi_preset_parses_expected_snapshot() {
        let provider = provider_config_from_preset("kimi-coding-usage").expect("preset");
        let provider = fixture_command(provider, "kimi_usage_fixture.js");
        let snapshot = test_provider(provider).expect("snapshot");

        assert_eq!(snapshot.id, "kimi-coding");
        assert_eq!(snapshot.status, "ok");
        assert_eq!(snapshot.windows.len(), 3);
    }

    #[test]
    fn fixture_bigmodel_preset_parses_expected_snapshot() {
        let provider = provider_config_from_preset("bigmodel-zai-coding-plan").expect("preset");
        let provider = fixture_command(provider, "bigmodel_quota_fixture.js");
        let snapshot = test_provider(provider).expect("snapshot");

        assert_eq!(snapshot.id, "bigmodel-coding-plan");
        assert_eq!(snapshot.status, "ok");
        assert_eq!(snapshot.windows.len(), 3);
    }
}
