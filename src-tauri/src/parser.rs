use chrono::{DateTime, SecondsFormat, TimeZone, Utc};
use serde_json::{json, Map, Value};

use crate::quota::{clamp_percent, ProviderDiagnostics, ProviderSnapshot, QuotaWindow};

pub fn parse_kimi_coding_usage(id: &str, name: &str, stdout: &str) -> ProviderSnapshot {
    let value = match serde_json::from_str::<Value>(stdout) {
        Ok(value) => value,
        Err(error) => return parser_error(id, name, format!("Invalid Kimi JSON: {error}")),
    };
    let mut diagnostics = Vec::new();
    let mut windows = Vec::new();

    if let Some(usage) = value.get("usage") {
        if let Some(window) = kimi_detail_window("usage", "Weekly limit", usage, &mut diagnostics) {
            windows.push(window);
        }
    } else {
        diagnostics.push("usage missing".to_string());
    }

    if let Some(limits) = value.get("limits").and_then(Value::as_array) {
        for limit in limits {
            let Some(detail) = limit.get("detail") else {
                diagnostics.push("limits[*].detail missing".to_string());
                continue;
            };
            let duration = limit.pointer("/window/duration").and_then(number_to_i64);
            let time_unit = limit
                .pointer("/window/timeUnit")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let id = match (duration, time_unit) {
                (Some(duration), "TIME_UNIT_MINUTE") => format!("{duration}-minute"),
                (Some(duration), unit) => format!("{duration}-{}", unit.to_ascii_lowercase()),
                _ => "unknown-window".to_string(),
            };
            let label = kimi_window_label(duration, time_unit);
            if let Some(window) = kimi_detail_window(&id, &label, detail, &mut diagnostics) {
                windows.push(window);
            }
        }
    } else {
        diagnostics.push("limits missing".to_string());
    }

    if let Some(total_quota) = value.get("totalQuota") {
        let limit = total_quota.get("limit").and_then(number_to_f64);
        let remaining = total_quota.get("remaining").and_then(number_to_f64);
        let used = match (limit, remaining) {
            (Some(limit), Some(remaining)) => Some(limit - remaining),
            _ => None,
        };
        windows.push(window_from_numbers(
            "total-quota",
            "Total quota",
            used,
            limit,
            remaining,
            None,
            None,
            &mut diagnostics,
        ));
    } else {
        diagnostics.push("totalQuota missing".to_string());
    }

    sort_quota_windows_for_display(&mut windows);

    let mut metadata = Map::new();
    insert_string(&mut metadata, "region", value.pointer("/user/region"));
    insert_string(
        &mut metadata,
        "membershipLevel",
        value.pointer("/user/membership/level"),
    );
    insert_string(
        &mut metadata,
        "authMethod",
        value.pointer("/authentication/method"),
    );
    insert_string(
        &mut metadata,
        "authScope",
        value.pointer("/authentication/scope"),
    );
    insert_string(&mut metadata, "subType", value.get("subType"));
    if let Some(parallel_limit) = value.pointer("/parallel/limit").and_then(number_to_json) {
        metadata.insert("parallelLimit".to_string(), parallel_limit);
    } else {
        diagnostics.push("parallel.limit missing".to_string());
    }

    provider_from_parts(
        id,
        name,
        windows,
        diagnostics,
        Some(Value::Object(metadata)),
    )
}

pub fn parse_bigmodel_quota_limit_json(id: &str, name: &str, stdout: &str) -> ProviderSnapshot {
    let value = match serde_json::from_str::<Value>(stdout) {
        Ok(value) => value,
        Err(error) => return parser_error(id, name, format!("Invalid BigModel JSON: {error}")),
    };
    let mut diagnostics = Vec::new();
    let mut windows = Vec::new();
    let mut metadata = Map::new();

    insert_value(&mut metadata, "rawCode", value.get("code"));
    insert_value(&mut metadata, "rawMsg", value.get("msg"));

    if value.get("success").and_then(Value::as_bool) != Some(true)
        || value.get("code").and_then(number_to_i64) != Some(200)
    {
        let mut provider = provider_from_parts(
            id,
            name,
            windows,
            diagnostics,
            Some(Value::Object(metadata)),
        );
        provider.status = "error".to_string();
        provider.error = Some("BigModel response was unsuccessful".to_string());
        return provider;
    }

    let Some(data) = value.get("data") else {
        diagnostics.push("data missing".to_string());
        return provider_from_parts(
            id,
            name,
            windows,
            diagnostics,
            Some(Value::Object(metadata)),
        );
    };

    insert_string(&mut metadata, "level", data.get("level"));

    if let Some(limits) = data.get("limits").and_then(Value::as_array) {
        if limits.is_empty() {
            diagnostics.push("limits empty".to_string());
        }

        for limit in limits {
            let type_name = limit
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("UNKNOWN");
            if type_name == "UNKNOWN" {
                diagnostics.push("limit item missing type".to_string());
            }
            let unit_number = limit.get("unit").and_then(number_to_i64);
            let number = limit.get("number").and_then(number_to_i64);
            let percentage = limit.get("percentage").and_then(number_to_f64);
            if percentage.is_none() {
                diagnostics.push(format!("{type_name} percentage missing"));
            }
            let used_percent = percentage.map(clamp_percent);
            let remaining_percent = used_percent.map(|percent| clamp_percent(100.0 - percent));
            let used = limit.get("currentValue").and_then(number_to_f64);
            let quota_limit = limit.get("usage").and_then(number_to_f64);
            let reset_at = limit
                .get("nextResetTime")
                .and_then(number_to_i64)
                .and_then(epoch_ms_to_iso_utc);
            let type_id = type_name.to_ascii_lowercase().replace('_', "-");
            windows.push(QuotaWindow {
                id: format!(
                    "{}-{}-{}",
                    type_id,
                    unit_number.map_or_else(|| "unknown".to_string(), |value| value.to_string()),
                    number.map_or_else(|| "unknown".to_string(), |value| value.to_string())
                ),
                label: bigmodel_window_label(type_name, unit_number, number),
                used,
                limit: quota_limit,
                unit: if type_name == "TOKENS_LIMIT" {
                    Some("tokens".to_string())
                } else {
                    None
                },
                used_percent,
                remaining_percent,
                reset_at,
                reset_text: None,
                confidence: "exact".to_string(),
            });

            if let Some(details) = limit.get("usageDetails").and_then(Value::as_array) {
                let usage_details = metadata
                    .entry("usageDetails".to_string())
                    .or_insert_with(|| Value::Object(Map::new()));
                if let Value::Object(object) = usage_details {
                    for detail in details {
                        if let (Some(model), Some(usage)) = (
                            detail.get("modelCode").and_then(Value::as_str),
                            detail.get("usage").and_then(number_to_json),
                        ) {
                            object.insert(model.to_string(), usage);
                        }
                    }
                }
            }
        }
    } else {
        diagnostics.push("limits missing".to_string());
    }

    sort_quota_windows_for_display(&mut windows);

    provider_from_parts(
        id,
        name,
        windows,
        diagnostics,
        Some(Value::Object(metadata)),
    )
}

pub fn epoch_ms_to_iso_utc(value: i64) -> Option<String> {
    let millis = if value.abs() < 10_000_000_000 {
        value.checked_mul(1000)?
    } else {
        value
    };
    Utc.timestamp_millis_opt(millis)
        .single()
        .map(|datetime| datetime.to_rfc3339_opts(SecondsFormat::Micros, true))
}

fn kimi_detail_window(
    id: &str,
    label: &str,
    detail: &Value,
    diagnostics: &mut Vec<String>,
) -> Option<QuotaWindow> {
    let used = detail.get("used").and_then(number_to_f64);
    let limit = detail.get("limit").and_then(number_to_f64);
    let remaining = detail.get("remaining").and_then(number_to_f64);
    let reset_at = detail
        .get("resetTime")
        .and_then(Value::as_str)
        .and_then(|reset| normalize_reset(reset, diagnostics));

    if used.is_none() && limit.is_none() && remaining.is_none() {
        diagnostics.push(format!("{id} has no numeric quota fields"));
        return None;
    }

    Some(window_from_numbers(
        id,
        label,
        used,
        limit,
        remaining,
        reset_at,
        None,
        diagnostics,
    ))
}

fn window_from_numbers(
    id: &str,
    label: &str,
    used: Option<f64>,
    limit: Option<f64>,
    remaining: Option<f64>,
    reset_at: Option<String>,
    unit: Option<String>,
    diagnostics: &mut Vec<String>,
) -> QuotaWindow {
    let (used_percent, remaining_percent) = if let Some(limit) = limit {
        if limit <= 0.0 {
            diagnostics.push(format!("{id} limit <= 0"));
            (None, None)
        } else {
            let used_percent = used.map(|used| clamp_percent((used / limit) * 100.0));
            let remaining_percent = remaining
                .map(|remaining| clamp_percent((remaining / limit) * 100.0))
                .or_else(|| used_percent.map(|percent| clamp_percent(100.0 - percent)));
            (used_percent, remaining_percent)
        }
    } else {
        diagnostics.push(format!("{id} limit missing"));
        (None, None)
    };

    QuotaWindow {
        id: id.to_string(),
        label: label.to_string(),
        used,
        limit,
        unit,
        used_percent,
        remaining_percent,
        reset_at,
        reset_text: None,
        confidence: "exact".to_string(),
    }
}

fn provider_from_parts(
    id: &str,
    name: &str,
    windows: Vec<QuotaWindow>,
    diagnostics: Vec<String>,
    metadata: Option<Value>,
) -> ProviderSnapshot {
    let status = if diagnostics.is_empty() {
        "ok"
    } else {
        "warning"
    };
    ProviderSnapshot {
        id: id.to_string(),
        name: name.to_string(),
        status: status.to_string(),
        source: "command".to_string(),
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
        metadata,
    }
}

fn parser_error(id: &str, name: &str, error: String) -> ProviderSnapshot {
    let mut provider = provider_from_parts(id, name, Vec::new(), vec![error.clone()], None);
    provider.status = "error".to_string();
    provider.error = Some(error);
    provider
}

fn kimi_window_label(duration: Option<i64>, time_unit: &str) -> String {
    match (duration, time_unit) {
        (Some(300), "TIME_UNIT_MINUTE") => "5h".to_string(),
        (Some(60), "TIME_UNIT_MINUTE") => "1h".to_string(),
        (Some(duration), unit) => format!("{duration} {unit}"),
        _ => time_unit.to_string(),
    }
}

fn bigmodel_window_label(type_name: &str, unit: Option<i64>, number: Option<i64>) -> String {
    let type_label = type_name
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.to_ascii_lowercase().chars().collect::<Vec<_>>();
            if let Some(first) = chars.first_mut() {
                first.make_ascii_uppercase();
            }
            chars.into_iter().collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(" ");

    match (unit, number) {
        (Some(5), Some(number)) => format!("{} · {type_label}", pluralize_period(number, "month")),
        (Some(3), Some(number)) => format!("{} · {type_label}", pluralize_period(number, "hour")),
        (Some(6), Some(number)) => format!("{} · {type_label}", pluralize_period(number, "week")),
        _ => format!(
            "{type_name} unit={} number={}",
            unit.map_or_else(|| "unknown".to_string(), |value| value.to_string()),
            number.map_or_else(|| "unknown".to_string(), |value| value.to_string())
        ),
    }
}

fn pluralize_period(number: i64, unit: &str) -> String {
    if number == 1 {
        format!("1 {unit}")
    } else {
        format!("{number} {unit}s")
    }
}

fn sort_quota_windows_for_display(windows: &mut [QuotaWindow]) {
    windows.sort_by(|left, right| {
        quota_window_display_rank(left).cmp(&quota_window_display_rank(right))
    });
}

fn quota_window_display_rank(window: &QuotaWindow) -> u8 {
    if is_five_hour_window(window) {
        return 0;
    }
    if is_weekly_window(window) {
        return 1;
    }
    2
}

fn is_five_hour_window(window: &QuotaWindow) -> bool {
    window.id == "300-minute"
        || window.id.ends_with("-3-5")
        || window.label == "5h"
        || window.label.starts_with("5 hours")
}

fn is_weekly_window(window: &QuotaWindow) -> bool {
    window.id == "usage"
        || window.id.ends_with("-6-1")
        || window.label == "Weekly limit"
        || window.label.starts_with("1 week")
}

fn normalize_reset(reset: &str, diagnostics: &mut Vec<String>) -> Option<String> {
    match DateTime::parse_from_rfc3339(reset) {
        Ok(_) => Some(reset.to_string()),
        Err(_) => {
            diagnostics.push(format!("invalid resetTime: {reset}"));
            None
        }
    }
}

fn number_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn number_to_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn number_to_json(value: &Value) -> Option<Value> {
    match value {
        Value::Number(number) => Some(Value::Number(number.clone())),
        Value::String(text) => {
            let trimmed = text.trim();
            if let Ok(integer) = trimmed.parse::<i64>() {
                Some(json!(integer))
            } else {
                trimmed.parse::<f64>().ok().map(|number| json!(number))
            }
        }
        _ => None,
    }
}

fn insert_string(metadata: &mut Map<String, Value>, key: &str, value: Option<&Value>) {
    if let Some(text) = value.and_then(Value::as_str) {
        metadata.insert(key.to_string(), Value::String(text.to_string()));
    }
}

fn insert_value(metadata: &mut Map<String, Value>, key: &str, value: Option<&Value>) {
    if let Some(value) = value {
        metadata.insert(key.to_string(), value.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_fixture(path: &str) -> String {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repo root")
            .to_path_buf();
        std::fs::read_to_string(root.join(path)).expect(path)
    }

    fn assert_percent(actual: Option<f64>, expected: f64) {
        let actual = actual.expect("percent");
        assert!((actual - expected).abs() < 0.01, "{actual} != {expected}");
    }

    #[test]
    fn parse_kimi_coding_usage_fixture() {
        let input = read_fixture("docs/specs/fixtures/provider_outputs/kimi_coding_usage.json");
        let expected = read_fixture("docs/specs/fixtures/expected/kimi_provider_snapshot.json");
        let expected: ProviderSnapshot = serde_json::from_str(&expected).expect("expected");

        let actual = parse_kimi_coding_usage("kimi-coding", "Kimi Coding", &input);

        assert_eq!(actual.id, expected.id);
        assert_eq!(actual.name, expected.name);
        assert_eq!(actual.status, "ok");
        assert_eq!(actual.windows.len(), expected.windows.len());
        assert_eq!(actual.windows, expected.windows);
        assert_eq!(actual.metadata, expected.metadata);
        assert_eq!(actual.windows[0].id, "300-minute");
        assert_eq!(actual.windows[0].label, "5h");
        assert_eq!(actual.windows[1].id, "usage");
        assert_eq!(actual.windows[1].label, "Weekly limit");
        assert_eq!(actual.windows[2].id, "total-quota");
        assert_percent(actual.windows[0].remaining_percent, 90.0);
        assert_percent(actual.windows[1].remaining_percent, 85.0);
        assert_percent(actual.windows[2].remaining_percent, 99.0);
    }

    #[test]
    fn parse_bigmodel_quota_limit_json_fixture() {
        let input = read_fixture("docs/specs/fixtures/provider_outputs/bigmodel_quota_limit.json");
        let expected = read_fixture("docs/specs/fixtures/expected/bigmodel_provider_snapshot.json");
        let expected: ProviderSnapshot = serde_json::from_str(&expected).expect("expected");

        let actual = parse_bigmodel_quota_limit_json(
            "bigmodel-coding-plan",
            "BigModel / Z.ai Coding Plan",
            &input,
        );

        assert_eq!(actual.id, expected.id);
        assert_eq!(actual.name, expected.name);
        assert_eq!(actual.status, "ok");
        assert_eq!(actual.windows.len(), expected.windows.len());
        assert_eq!(actual.windows, expected.windows);
        assert_eq!(actual.metadata, expected.metadata);
        assert_eq!(actual.windows[0].id, "tokens-limit-3-5");
        assert_eq!(actual.windows[0].label, "5 hours · Tokens Limit");
        assert_eq!(actual.windows[1].id, "tokens-limit-6-1");
        assert_eq!(actual.windows[1].label, "1 week · Tokens Limit");
        assert_eq!(actual.windows[2].id, "time-limit-5-1");
        assert_eq!(actual.windows[2].label, "1 month · Time Limit");
        assert_percent(actual.windows[0].remaining_percent, 99.0);
        assert_percent(actual.windows[1].remaining_percent, 96.0);
        assert_percent(actual.windows[2].used_percent, 3.0);
    }

    #[test]
    fn bigmodel_unknown_unit_keeps_raw_label() {
        let provider = parse_bigmodel_quota_limit_json(
            "bigmodel",
            "BigModel",
            r#"{"success":true,"code":200,"data":{"limits":[{"type":"CUSTOM_LIMIT","unit":9,"number":2,"percentage":50}]}}"#,
        );

        assert_eq!(provider.windows[0].label, "CUSTOM_LIMIT unit=9 number=2");
    }

    #[test]
    fn parse_kimi_invalid_json_returns_error() {
        let provider = parse_kimi_coding_usage("kimi", "Kimi", "{bad json");

        assert_eq!(provider.status, "error");
        assert!(provider.error.unwrap().contains("Invalid Kimi JSON"));
    }

    #[test]
    fn parse_bigmodel_unsuccessful_returns_error() {
        let provider = parse_bigmodel_quota_limit_json(
            "bigmodel",
            "BigModel",
            r#"{"success":false,"code":403,"msg":"nope"}"#,
        );

        assert_eq!(provider.status, "error");
    }

    #[test]
    fn parse_percentage_clamps_to_0_100() {
        let provider = parse_bigmodel_quota_limit_json(
            "bigmodel",
            "BigModel",
            r#"{"success":true,"code":200,"data":{"limits":[{"type":"TOKENS_LIMIT","unit":1,"number":1,"percentage":140},{"type":"TOKENS_LIMIT","unit":1,"number":2,"percentage":-5}]}}"#,
        );

        assert_eq!(provider.windows[0].used_percent, Some(100.0));
        assert_eq!(provider.windows[0].remaining_percent, Some(0.0));
        assert_eq!(provider.windows[1].used_percent, Some(0.0));
        assert_eq!(provider.windows[1].remaining_percent, Some(100.0));
    }

    #[test]
    fn parse_epoch_ms_to_iso_utc() {
        assert_eq!(
            epoch_ms_to_iso_utc(1782784883993).expect("iso"),
            "2026-06-30T02:01:23.993000Z"
        );
        assert_eq!(
            epoch_ms_to_iso_utc(1782784883).expect("iso"),
            "2026-06-30T02:01:23.000000Z"
        );
    }
}
