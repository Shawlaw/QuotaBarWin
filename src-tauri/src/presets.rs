use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};
use tauri::{command, AppHandle};

use crate::{
    command_provider::run_single_provider_config,
    config::{config_path_for_app, CommandSpec, ProviderConfig, ScriptOutputSpec},
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

struct ProviderTemplate {
    id: &'static str,
    manifest: &'static str,
    script: &'static str,
}

const PROVIDER_TEMPLATES: &[ProviderTemplate] = &[
    ProviderTemplate {
        id: "custom-script",
        manifest: include_str!("../../providers/custom-script/provider.json"),
        script: include_str!("../../providers/custom-script/provider.cjs"),
    },
    ProviderTemplate {
        id: "kimi-coding",
        manifest: include_str!("../../providers/kimi-coding/provider.json"),
        script: include_str!("../../providers/kimi-coding/provider.cjs"),
    },
    ProviderTemplate {
        id: "bigmodel-coding-plan",
        manifest: include_str!("../../providers/bigmodel-coding-plan/provider.json"),
        script: include_str!("../../providers/bigmodel-coding-plan/provider.cjs"),
    },
];

fn script_path(template_root: &Path, id: &str) -> String {
    template_root
        .join(id)
        .join("provider.cjs")
        .to_string_lossy()
        .to_string()
}

fn materialize_builtin_provider_templates(app: &AppHandle) -> Result<PathBuf, String> {
    let config_path = config_path_for_app(app)?;
    let config_dir = config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let template_root = config_dir.join("providers").join("builtin");

    for template in PROVIDER_TEMPLATES {
        let dir = template_root.join(template.id);
        fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        let manifest_path = dir.join("provider.json");
        let script_path = dir.join("provider.cjs");
        if !manifest_path.exists() {
            fs::write(&manifest_path, template.manifest).map_err(|error| error.to_string())?;
        }
        if !script_path.exists() {
            fs::write(&script_path, template.script).map_err(|error| error.to_string())?;
        }
    }

    Ok(template_root)
}

pub fn builtin_provider_presets_for_root(template_root: &Path) -> Vec<ProviderPreset> {
    vec![
        ProviderPreset {
            id: "codex-usage".to_string(),
            display_name: "Codex Usage".to_string(),
            description: "Codex/OpenAI 5h and weekly quota via ChatGPT usage API".to_string(),
            provider_config_template: ProviderConfig::Codex {
                id: "codex".to_string(),
                name: "Codex".to_string(),
                enabled: true,
                auth_token: "${env:CODEX_ACCESS_TOKEN}".to_string(),
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
                "Provide a ChatGPT/Codex access token, or change Auth token to ${file:C:\\path\\token.txt}."
                    .to_string(),
            ),
        },
        ProviderPreset {
            id: "kimi-coding-usage".to_string(),
            display_name: "Kimi Coding Usage".to_string(),
            description: "Kimi coding quota usage via editable provider script".to_string(),
            provider_config_template: ProviderConfig::Script {
                id: "kimi-coding".to_string(),
                name: "Kimi Coding".to_string(),
                enabled: true,
                command: CommandSpec {
                    executable: "node".to_string(),
                    args: vec![script_path(template_root, "kimi-coding")],
                    cwd: None,
                    env: None,
                    timeout_ms: 15000,
                },
                output: ScriptOutputSpec::ProviderSnapshotV1,
                window_label_overrides: HashMap::from([
                    ("300-minute".to_string(), "5h".to_string()),
                    ("usage".to_string(), "Weekly limit".to_string()),
                    ("total-quota".to_string(), "Total quota".to_string()),
                ]),
                visible_window_ids: Vec::new(),
            },
            required_env_vars: vec!["KIMI_API_KEY".to_string()],
            docs: Some("Set KIMI_API_KEY in your environment.".to_string()),
        },
        ProviderPreset {
            id: "bigmodel-coding-plan".to_string(),
            display_name: "BigModel Coding Plan".to_string(),
            description: "BigModel coding plan quota via editable provider script".to_string(),
            provider_config_template: ProviderConfig::Script {
                id: "bigmodel-coding-plan".to_string(),
                name: "BigModel Coding Plan".to_string(),
                enabled: true,
                command: CommandSpec {
                    executable: "node".to_string(),
                    args: vec![script_path(template_root, "bigmodel-coding-plan")],
                    cwd: None,
                    env: None,
                    timeout_ms: 15000,
                },
                output: ScriptOutputSpec::ProviderSnapshotV1,
                window_label_overrides: HashMap::from([
                    ("tokens-limit-3-5".to_string(), "5h".to_string()),
                    ("tokens-limit-6-1".to_string(), "Weekly limit".to_string()),
                    (
                        "time-limit-5-1".to_string(),
                        "Monthly time limit".to_string(),
                    ),
                ]),
                visible_window_ids: Vec::new(),
            },
            required_env_vars: vec!["BIGMODEL_API_KEY".to_string()],
            docs: Some("Set BIGMODEL_API_KEY in your environment.".to_string()),
        },
        ProviderPreset {
            id: "opencode-quota-command".to_string(),
            display_name: "OpenCode Quota Command".to_string(),
            description: "Read an app snapshot from opencode-quota".to_string(),
            provider_config_template: ProviderConfig::Script {
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
                output: ScriptOutputSpec::AppSnapshotV1,
                window_label_overrides: HashMap::new(),
                visible_window_ids: Vec::new(),
            },
            required_env_vars: Vec::new(),
            docs: Some(
                "Install opencode-quota separately if you want to use this provider.".to_string(),
            ),
        },
        ProviderPreset {
            id: "custom-script-provider".to_string(),
            display_name: "Custom Script Provider".to_string(),
            description: "Start from an editable script provider".to_string(),
            provider_config_template: ProviderConfig::Script {
                id: "custom-script".to_string(),
                name: "Custom Script Provider".to_string(),
                enabled: true,
                command: CommandSpec {
                    executable: "node".to_string(),
                    args: vec![script_path(template_root, "custom-script")],
                    cwd: None,
                    env: None,
                    timeout_ms: 15000,
                },
                output: ScriptOutputSpec::ProviderSnapshotV1,
                window_label_overrides: HashMap::new(),
                visible_window_ids: Vec::new(),
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

#[cfg(test)]
fn builtin_provider_presets() -> Vec<ProviderPreset> {
    builtin_provider_presets_for_root(Path::new("providers/builtin"))
}

#[command]
pub fn get_provider_presets(app: AppHandle) -> Result<Vec<ProviderPreset>, String> {
    let template_root = materialize_builtin_provider_templates(&app)?;
    Ok(builtin_provider_presets_for_root(&template_root))
}

#[command]
pub async fn test_provider(provider: ProviderConfig) -> Result<ProviderSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || Ok(run_single_provider_config(&provider)))
        .await
        .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CommandSpec, ProviderConfig, ScriptOutputSpec};

    fn script_from(provider: &ProviderConfig) -> (&CommandSpec, &ScriptOutputSpec) {
        match provider {
            ProviderConfig::Script {
                command, output, ..
            } => (command, output),
            _ => panic!("expected script provider"),
        }
    }

    fn repo_file(parts: &[&str]) -> String {
        let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.pop();
        for part in parts {
            path.push(part);
        }
        path.to_string_lossy().to_string()
    }

    fn fixture_script(
        provider: ProviderConfig,
        template_id: &str,
        fixture_env: &str,
        fixture_path: &str,
    ) -> ProviderConfig {
        let script = repo_file(&["providers", template_id, "provider.cjs"]);
        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repo root")
            .join("docs")
            .join("specs")
            .join("fixtures")
            .join("provider_outputs")
            .join(fixture_path)
            .to_string_lossy()
            .to_string();

        match provider {
            ProviderConfig::Script {
                id,
                name,
                enabled,
                output,
                window_label_overrides,
                visible_window_ids,
                ..
            } => ProviderConfig::Script {
                id,
                name,
                enabled,
                command: CommandSpec {
                    executable: "node".to_string(),
                    args: vec![script],
                    cwd: None,
                    env: Some(HashMap::from([(fixture_env.to_string(), fixture)])),
                    timeout_ms: 15000,
                },
                output,
                window_label_overrides,
                visible_window_ids,
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

        assert!(preset
            .required_env_vars
            .contains(&"KIMI_API_KEY".to_string()));
        let (command, output) = script_from(&preset.provider_config_template);
        assert_eq!(command.executable, "node");
        assert_eq!(output, &ScriptOutputSpec::ProviderSnapshotV1);
    }

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
                assert_eq!(auth_token, "${env:CODEX_ACCESS_TOKEN}");
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
    fn preset_bigmodel_has_required_env_var() {
        let preset = builtin_provider_presets()
            .into_iter()
            .find(|preset| preset.id == "bigmodel-coding-plan")
            .expect("bigmodel preset");

        assert!(preset
            .required_env_vars
            .contains(&"BIGMODEL_API_KEY".to_string()));
        let (command, output) = script_from(&preset.provider_config_template);
        assert_eq!(command.executable, "node");
        assert_eq!(output, &ScriptOutputSpec::ProviderSnapshotV1);
        if let ProviderConfig::Script {
            window_label_overrides,
            ..
        } = preset.provider_config_template
        {
            assert_eq!(
                window_label_overrides.get("tokens-limit-3-5"),
                Some(&"5h".to_string())
            );
            assert_eq!(
                window_label_overrides.get("tokens-limit-6-1"),
                Some(&"Weekly limit".to_string())
            );
        } else {
            panic!("expected script provider");
        }
    }

    #[test]
    fn presets_serialize_frontend_editable_mapping_fields() {
        let presets = builtin_provider_presets();
        let value = serde_json::to_value(&presets).expect("serialize presets");
        let bigmodel = value
            .as_array()
            .expect("preset array")
            .iter()
            .find(|preset| preset["id"] == "bigmodel-coding-plan")
            .expect("bigmodel preset");
        let template = &bigmodel["providerConfigTemplate"];

        assert_eq!(
            template["windowLabelOverrides"],
            serde_json::json!({
                "tokens-limit-3-5": "5h",
                "tokens-limit-6-1": "Weekly limit",
                "time-limit-5-1": "Monthly time limit"
            })
        );
        assert_eq!(template["visibleWindowIds"], serde_json::json!([]));
    }

    #[test]
    fn preset_opencode_quota_uses_app_snapshot_parser() {
        let preset = builtin_provider_presets()
            .into_iter()
            .find(|preset| preset.id == "opencode-quota-command")
            .expect("opencode preset");
        let (command, output) = script_from(&preset.provider_config_template);

        assert_eq!(command.executable, "opencode-quota");
        assert_eq!(output, &ScriptOutputSpec::AppSnapshotV1);
    }

    #[test]
    fn preset_add_creates_provider_config() {
        let provider = provider_config_from_preset("custom-script-provider").expect("provider");

        assert!(matches!(provider, ProviderConfig::Script { .. }));
    }

    #[test]
    fn fixture_kimi_preset_parses_expected_snapshot() {
        let provider = provider_config_from_preset("kimi-coding-usage").expect("preset");
        let provider = fixture_script(
            provider,
            "kimi-coding",
            "QUOTABARWIN_KIMI_FIXTURE",
            "kimi_coding_usage.json",
        );
        let snapshot = run_single_provider_config(&provider);

        assert_eq!(snapshot.id, "kimi-coding");
        assert_eq!(snapshot.status, "ok");
        assert_eq!(snapshot.windows.len(), 3);
    }

    #[test]
    fn fixture_bigmodel_preset_parses_expected_snapshot() {
        let provider = provider_config_from_preset("bigmodel-coding-plan").expect("preset");
        let provider = fixture_script(
            provider,
            "bigmodel-coding-plan",
            "QUOTABARWIN_BIGMODEL_FIXTURE",
            "bigmodel_quota_limit.json",
        );
        let snapshot = run_single_provider_config(&provider);

        assert_eq!(snapshot.id, "bigmodel-coding-plan");
        assert_eq!(snapshot.status, "ok");
        assert_eq!(snapshot.windows.len(), 3);
        assert_eq!(snapshot.windows[0].label, "5h");
        assert_eq!(snapshot.windows[1].label, "Weekly limit");
    }
}
