use std::{
    collections::{HashMap, HashSet},
    path::Path,
    time::Duration,
};

use chrono::{SecondsFormat, TimeDelta, Utc};
use serde_json::{json, Map, Value};

use crate::{
    config::resolve_secret_value,
    quota::{clamp_percent, ProviderDiagnostics, ProviderSnapshot, QuotaWindow},
    redact::redact_sensitive,
};

const CODEX_USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";

pub fn provider_snapshot(
    id: &str,
    name: &str,
    auth_token: &str,
    account_id: Option<&str>,
    proxy_url: Option<&str>,
    timeout_ms: u64,
    config_dir: &Path,
    window_label_overrides: &HashMap<String, String>,
    visible_window_ids: &[String],
) -> ProviderSnapshot {
    let started = std::time::Instant::now();
    match fetch_codex_usage(auth_token, account_id, proxy_url, timeout_ms, config_dir) {
        Ok(value) => {
            let mut provider = provider_from_usage_response(id, name, &value);
            provider.diagnostics = merge_diagnostics(
                provider.diagnostics,
                Some(ProviderDiagnostics {
                    checked_at: Utc::now().to_rfc3339(),
                    messages: Vec::new(),
                    command_path: Some(CODEX_USAGE_URL.to_string()),
                    exit_code: None,
                    duration_ms: Some(started.elapsed().as_millis() as u64),
                    timed_out: None,
                    stderr: None,
                }),
            );
            apply_visible_windows(&mut provider, visible_window_ids);
            apply_window_label_overrides(&mut provider, window_label_overrides);
            provider
        }
        Err(error) => error_provider(id, name, error, started.elapsed().as_millis() as u64),
    }
}

fn fetch_codex_usage(
    auth_token: &str,
    account_id: Option<&str>,
    proxy_url: Option<&str>,
    timeout_ms: u64,
    config_dir: &Path,
) -> Result<Value, String> {
    let token = resolve_secret_value(auth_token, config_dir)?;
    let token = token
        .trim()
        .strip_prefix("Bearer ")
        .unwrap_or(token.trim())
        .trim();
    if token.is_empty() {
        return Err("Codex access token is empty".to_string());
    }

    let client = build_codex_client(timeout_ms, proxy_url, config_dir)?;
    let mut request = client
        .get(CODEX_USAGE_URL)
        .header("Authorization", format!("Bearer {token}"))
        .header("User-Agent", "QuotaBarWin/0.0");

    if let Some(account_id) = account_id.map(str::trim).filter(|value| !value.is_empty()) {
        request = request.header("ChatGPT-Account-Id", account_id);
    }

    let response = request.send().map_err(|error| error.to_string())?;
    let status = response.status();
    if !status.is_success() {
        let text = response.text().unwrap_or_default();
        return Err(format!(
            "Codex usage API returned {status}: {}",
            redact_sensitive(&text)
                .chars()
                .take(160)
                .collect::<String>()
        ));
    }

    response.json::<Value>().map_err(|error| error.to_string())
}

fn build_codex_client(
    timeout_ms: u64,
    proxy_url: Option<&str>,
    config_dir: &Path,
) -> Result<reqwest::blocking::Client, String> {
    let mut builder =
        reqwest::blocking::Client::builder().timeout(Duration::from_millis(timeout_ms.max(1)));

    if let Some(proxy_url) = proxy_url.map(str::trim).filter(|value| !value.is_empty()) {
        let proxy_url = resolve_secret_value(proxy_url, config_dir)?;
        let proxy_url = proxy_url.trim();
        let proxy = reqwest::Proxy::all(proxy_url).map_err(|error| {
            format!(
                "Invalid Codex proxy URL: {}",
                redact_sensitive(&error.to_string())
            )
        })?;
        builder = builder.proxy(proxy);
    }

    builder.build().map_err(|error| error.to_string())
}

fn provider_from_usage_response(id: &str, name: &str, value: &Value) -> ProviderSnapshot {
    let mut diagnostics = Vec::new();
    let mut metadata = Map::new();

    let plan_type = value.get("plan_type").and_then(Value::as_str);
    if let Some(plan_type) = plan_type {
        metadata.insert("planType".to_string(), Value::String(plan_type.to_string()));
    }
    if let Some(credits) = value.get("credits") {
        metadata.insert("credits".to_string(), credits.clone());
    }

    let mut windows = Vec::new();
    let primary = value.pointer("/rate_limit/primary_window");
    match primary.and_then(|window| quota_window_from_usage_window("5h", "5h", window)) {
        Some(window) => windows.push(window),
        None => diagnostics.push("rate_limit.primary_window missing or invalid".to_string()),
    }

    let secondary = value.pointer("/rate_limit/secondary_window");
    match secondary
        .and_then(|window| quota_window_from_usage_window("weekly", "Weekly limit", window))
    {
        Some(window) => windows.push(window),
        None => diagnostics.push("rate_limit.secondary_window missing or invalid".to_string()),
    }

    ProviderSnapshot {
        id: id.to_string(),
        name: name.to_string(),
        status: if diagnostics.is_empty() {
            "ok"
        } else {
            "warning"
        }
        .to_string(),
        source: "native".to_string(),
        updated_at: Some(Utc::now().to_rfc3339()),
        windows,
        error: None,
        diagnostics: if diagnostics.is_empty() {
            None
        } else {
            Some(ProviderDiagnostics {
                checked_at: Utc::now().to_rfc3339(),
                messages: diagnostics,
                command_path: None,
                exit_code: None,
                duration_ms: None,
                timed_out: None,
                stderr: None,
            })
        },
        metadata: Some(Value::Object(metadata)),
    }
}

fn quota_window_from_usage_window(id: &str, label: &str, value: &Value) -> Option<QuotaWindow> {
    let used_percent = value
        .get("used_percent")
        .and_then(number_to_f64)
        .map(clamp_percent)?;
    Some(QuotaWindow {
        id: id.to_string(),
        label: label.to_string(),
        remaining: None,
        used: None,
        limit: None,
        unit: Some("percent".to_string()),
        used_percent: Some(used_percent),
        remaining_percent: Some(clamp_percent(100.0 - used_percent)),
        warning_remaining: None,
        reset_at: reset_iso_from_window(value),
        reset_text: None,
        confidence: "exact".to_string(),
    })
}

fn reset_iso_from_window(value: &Value) -> Option<String> {
    value
        .get("reset_at")
        .and_then(number_to_f64)
        .and_then(reset_iso_from_epoch_seconds)
        .or_else(|| {
            value
                .get("reset_after_seconds")
                .and_then(number_to_f64)
                .and_then(reset_iso_from_now_seconds)
        })
}

fn reset_iso_from_epoch_seconds(seconds: f64) -> Option<String> {
    if !seconds.is_finite() || seconds <= 0.0 {
        return None;
    }
    let millis = (seconds * 1000.0).round() as i64;
    chrono::DateTime::<Utc>::from_timestamp_millis(millis)
        .map(|datetime| datetime.to_rfc3339_opts(SecondsFormat::Micros, true))
}

fn reset_iso_from_now_seconds(seconds: f64) -> Option<String> {
    if !seconds.is_finite() || seconds <= 0.0 {
        return None;
    }
    let millis = (seconds * 1000.0).round() as i64;
    Utc::now()
        .checked_add_signed(TimeDelta::milliseconds(millis))
        .map(|datetime| datetime.to_rfc3339_opts(SecondsFormat::Micros, true))
}

fn apply_window_label_overrides(
    provider: &mut ProviderSnapshot,
    overrides: &HashMap<String, String>,
) {
    if overrides.is_empty() {
        return;
    }

    for window in &mut provider.windows {
        if let Some(label) = overrides
            .get(&window.id)
            .or_else(|| overrides.get(&window.label))
            .filter(|label| !label.trim().is_empty())
        {
            window.label = label.trim().to_string();
        }
    }
}

fn apply_visible_windows(provider: &mut ProviderSnapshot, visible_window_ids: &[String]) {
    if visible_window_ids.is_empty() {
        return;
    }

    let original_windows = provider.windows.clone();
    let mut selected_indexes = HashSet::new();
    let mut visible_windows = Vec::new();

    for visible in visible_window_ids {
        let visible = visible.trim();
        if visible.is_empty() {
            continue;
        }

        for (index, window) in original_windows.iter().enumerate() {
            if selected_indexes.contains(&index) || visible != window.id {
                continue;
            }
            visible_windows.push(window.clone());
            selected_indexes.insert(index);
        }

        for (index, window) in original_windows.iter().enumerate() {
            if selected_indexes.contains(&index) || visible != window.label {
                continue;
            }
            visible_windows.push(window.clone());
            selected_indexes.insert(index);
        }
    }

    provider.windows = visible_windows;
}

fn merge_diagnostics(
    existing: Option<ProviderDiagnostics>,
    runtime: Option<ProviderDiagnostics>,
) -> Option<ProviderDiagnostics> {
    match (existing, runtime) {
        (None, None) => None,
        (Some(existing), None) => Some(existing),
        (None, Some(runtime)) if runtime.messages.is_empty() => Some(runtime),
        (None, Some(runtime)) => Some(runtime),
        (Some(mut existing), Some(runtime)) => {
            existing.command_path = runtime.command_path;
            existing.duration_ms = runtime.duration_ms;
            Some(existing)
        }
    }
}

fn error_provider(id: &str, name: &str, error: String, duration_ms: u64) -> ProviderSnapshot {
    ProviderSnapshot {
        id: id.to_string(),
        name: name.to_string(),
        status: "error".to_string(),
        source: "native".to_string(),
        updated_at: Some(Utc::now().to_rfc3339()),
        windows: Vec::new(),
        error: Some(error.clone()),
        diagnostics: Some(ProviderDiagnostics {
            checked_at: Utc::now().to_rfc3339(),
            messages: vec![error],
            command_path: Some(CODEX_USAGE_URL.to_string()),
            exit_code: None,
            duration_ms: Some(duration_ms),
            timed_out: None,
            stderr: None,
        }),
        metadata: Some(json!({
            "api": CODEX_USAGE_URL
        })),
    }
}

fn number_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_response_maps_5h_and_weekly_windows() {
        let input = json!({
            "plan_type": "pro",
            "rate_limit": {
                "primary_window": {
                    "used_percent": 36,
                    "reset_at": 1780981200
                },
                "secondary_window": {
                    "used_percent": 12.5,
                    "reset_after_seconds": 3600
                }
            }
        });

        let provider = provider_from_usage_response("codex", "Codex", &input);

        assert_eq!(provider.status, "ok");
        assert_eq!(provider.source, "native");
        assert_eq!(provider.windows.len(), 2);
        assert_eq!(provider.windows[0].id, "5h");
        assert_eq!(provider.windows[0].label, "5h");
        assert_eq!(provider.windows[0].used_percent, Some(36.0));
        assert_eq!(provider.windows[0].remaining_percent, Some(64.0));
        assert_eq!(provider.windows[1].id, "weekly");
        assert_eq!(provider.windows[1].label, "Weekly limit");
        assert_eq!(provider.windows[1].remaining_percent, Some(87.5));
    }

    #[test]
    fn secret_file_source_accepts_quoted_paths_with_spaces() {
        let temp = tempfile::tempdir().expect("temp dir");
        let secret_dir = temp.path().join("codex secrets");
        std::fs::create_dir(&secret_dir).expect("create secret dir");
        let secret_path = secret_dir.join("access token.txt");
        std::fs::write(&secret_path, "token-from-file\n").expect("write secret");

        let actual = resolve_secret_value(
            &format!("${{file:\"{}\"}}", secret_path.display()),
            temp.path(),
        )
        .unwrap();

        assert_eq!(actual, "token-from-file");
    }

    #[test]
    fn codex_client_accepts_http_and_socks_proxy_urls() {
        let temp = tempfile::tempdir().expect("temp dir");
        build_codex_client(1000, Some("http://127.0.0.1:7890"), temp.path())
            .expect("http proxy client");
        build_codex_client(1000, Some("socks5h://127.0.0.1:7890"), temp.path())
            .expect("socks proxy client");
    }

    #[test]
    fn visible_windows_follow_config_order_before_label_overrides() {
        let input = json!({
            "rate_limit": {
                "primary_window": {
                    "used_percent": 36,
                    "reset_at": 1780981200
                },
                "secondary_window": {
                    "used_percent": 12.5,
                    "reset_after_seconds": 3600
                }
            }
        });
        let mut provider = provider_from_usage_response("codex", "Codex", &input);
        let overrides = HashMap::from([
            ("5h".to_string(), "Five hour".to_string()),
            ("Weekly limit".to_string(), "Team weekly".to_string()),
        ]);

        apply_visible_windows(
            &mut provider,
            &[
                "Weekly limit".to_string(),
                "5h".to_string(),
                "Weekly limit".to_string(),
            ],
        );
        apply_window_label_overrides(&mut provider, &overrides);

        assert_eq!(provider.windows.len(), 2);
        assert_eq!(provider.windows[0].id, "weekly");
        assert_eq!(provider.windows[0].label, "Team weekly");
        assert_eq!(provider.windows[1].id, "5h");
        assert_eq!(provider.windows[1].label, "Five hour");
    }
}
