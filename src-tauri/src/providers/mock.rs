use chrono::Utc;

use crate::quota::{clamp_percent, ProviderDiagnostics, ProviderSnapshot, QuotaWindow};

fn window(id: &str, label: &str, remaining_percent: f64) -> QuotaWindow {
    let remaining_percent = clamp_percent(remaining_percent);
    let used_percent = 100.0 - remaining_percent;

    QuotaWindow {
        id: id.to_string(),
        label: label.to_string(),
        used: Some(used_percent),
        limit: Some(100.0),
        unit: Some("percent".to_string()),
        used_percent: Some(used_percent),
        remaining_percent: Some(remaining_percent),
        reset_at: None,
        reset_text: None,
        confidence: "estimated".to_string(),
    }
}

pub fn provider_snapshot() -> ProviderSnapshot {
    let checked_at = Utc::now().to_rfc3339();

    ProviderSnapshot {
        id: "codex-mock".to_string(),
        name: "Codex Mock".to_string(),
        status: "ok".to_string(),
        source: "mock".to_string(),
        updated_at: Some(checked_at.clone()),
        windows: vec![window("5h", "5h window", 72.0), window("weekly", "weekly window", 35.0)],
        error: None,
        diagnostics: Some(ProviderDiagnostics {
            checked_at,
            messages: vec!["V0 mock provider snapshot".to_string()],
            command_path: None,
            exit_code: None,
            duration_ms: None,
            timed_out: None,
            stderr: None,
        }),
        metadata: None,
    }
}
