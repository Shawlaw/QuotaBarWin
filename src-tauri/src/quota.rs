use chrono::Utc;
use serde::Serialize;

use crate::providers::mock;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    pub id: String,
    pub label: String,
    pub used: Option<f64>,
    pub limit: Option<f64>,
    pub unit: Option<String>,
    pub used_percent: Option<f64>,
    pub remaining_percent: Option<f64>,
    pub reset_at: Option<String>,
    pub reset_text: Option<String>,
    pub confidence: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDiagnostics {
    pub checked_at: String,
    pub messages: Vec<String>,
    pub command_path: Option<String>,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
    pub timed_out: Option<bool>,
    pub stderr: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSnapshot {
    pub id: String,
    pub name: String,
    pub status: String,
    pub source: String,
    pub updated_at: Option<String>,
    pub windows: Vec<QuotaWindow>,
    pub error: Option<String>,
    pub diagnostics: Option<ProviderDiagnostics>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub schema_version: u8,
    pub providers: Vec<ProviderSnapshot>,
    pub refreshed_at: String,
}

pub fn clamp_percent(percent: f64) -> f64 {
    percent.clamp(0.0, 100.0)
}

pub fn build_app_snapshot() -> AppSnapshot {
    AppSnapshot {
        schema_version: 1,
        providers: vec![mock::provider_snapshot()],
        refreshed_at: Utc::now().to_rfc3339(),
    }
}

#[tauri::command]
pub fn refresh_snapshot() -> Result<AppSnapshot, String> {
    Ok(build_app_snapshot())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_provider_returns_app_snapshot() {
        let snapshot = build_app_snapshot();

        assert_eq!(snapshot.schema_version, 1);
        assert_eq!(snapshot.providers.len(), 1);
        assert_eq!(snapshot.providers[0].name, "Codex Mock");
        assert_eq!(snapshot.providers[0].windows.len(), 2);
    }

    #[test]
    fn quota_percentages_are_in_range() {
        let snapshot = build_app_snapshot();

        for provider in snapshot.providers {
            for window in provider.windows {
                let used = window.used_percent.expect("used percent exists");
                let remaining = window
                    .remaining_percent
                    .expect("remaining percent exists");

                assert!((0.0..=100.0).contains(&used));
                assert!((0.0..=100.0).contains(&remaining));
                assert_eq!(remaining, 100.0 - used);
            }
        }
    }
}
