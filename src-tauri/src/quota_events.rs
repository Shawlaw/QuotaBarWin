use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::config::{
    config_path_for_app, NOTIFICATION_EVENT_APP_STARTED, NOTIFICATION_EVENT_APP_UPDATE_APPLIED,
    NOTIFICATION_EVENT_PROVIDER_ERROR, NOTIFICATION_EVENT_PROVIDER_RECOVERED,
    NOTIFICATION_EVENT_QUOTA_EXHAUSTED, NOTIFICATION_EVENT_QUOTA_LOW,
    NOTIFICATION_EVENT_QUOTA_RECOVERED_UNEXPECTED, NOTIFICATION_EVENT_QUOTA_RESET,
};
use crate::quota::{ProviderSnapshot, QuotaWindow};

const EVENT_HISTORY_FILE_NAME: &str = "events.quotaBarWin.json";
const EVENT_HISTORY_SCHEMA_VERSION: u8 = 1;
const EVENT_HISTORY_LIMIT: usize = 200;
// Mirrors the snapshot verification tolerance: a reset time that only drifts
// within this window is the same cycle, not a new one.
const RESET_ADVANCE_TOLERANCE_SECONDS: i64 = 2 * 60;
// Matches the optimistic-jump verification threshold in quota.rs: only a
// rebound this large is worth reporting as an unexpected recovery.
const UNEXPECTED_RECOVERY_DROP_PERCENT: f64 = 20.0;
const EXHAUSTED_REMAINING_PERCENT: f64 = 0.5;
const ERROR_DETAIL_LIMIT: usize = 400;

pub const SEVERITY_INFO: &str = "info";
pub const SEVERITY_POSITIVE: &str = "positive";
pub const SEVERITY_WARNING: &str = "warning";
pub const SEVERITY_ERROR: &str = "error";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QuotaEvent {
    pub id: u64,
    pub occurred_at: String,
    pub event_type: String,
    pub severity: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

/// An event detected by callers but not yet stored; `append_events` assigns
/// the id, timestamp, and history slot.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingQuotaEvent {
    pub event_type: String,
    pub severity: String,
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
    pub window_id: Option<String>,
    pub window_label: Option<String>,
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct EventHistoryFile {
    #[serde(default)]
    schema_version: u8,
    #[serde(default)]
    next_id: u64,
    // Stored oldest-first; the reader returns newest-first.
    events: Vec<QuotaEvent>,
}

static EVENT_HISTORY_CACHE: OnceLock<Mutex<Option<EventHistoryFile>>> = OnceLock::new();

fn event_history_cache() -> &'static Mutex<Option<EventHistoryFile>> {
    EVENT_HISTORY_CACHE.get_or_init(|| Mutex::new(None))
}

fn history_path_for_config_path(path: &Path) -> PathBuf {
    path.parent()
        .unwrap_or_else(|| Path::new("."))
        .join(EVENT_HISTORY_FILE_NAME)
}

fn parse_rfc3339(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|time| time.with_timezone(&Utc))
}

fn truncate_detail(value: &str) -> String {
    if value.chars().count() <= ERROR_DETAIL_LIMIT {
        return value.to_string();
    }
    let truncated: String = value.chars().take(ERROR_DETAIL_LIMIT).collect();
    format!("{truncated}…")
}

fn provider_healthy(status: &str) -> bool {
    status == "ok" || status == "warning"
}

fn provider_failing(status: &str) -> bool {
    status == "error" || status == "stale"
}

fn pending_event(
    event_type: &str,
    severity: &str,
    provider: &ProviderSnapshot,
    window: Option<&QuotaWindow>,
    details: serde_json::Value,
) -> PendingQuotaEvent {
    PendingQuotaEvent {
        event_type: event_type.to_string(),
        severity: severity.to_string(),
        provider_id: Some(provider.id.clone()),
        provider_name: Some(provider.name.clone()),
        window_id: window.map(|window| window.id.clone()),
        window_label: window.map(|window| window.label.clone()),
        details: Some(details),
    }
}

/// Diffs a previous refresh result against the accepted current result. Events
/// are edge-triggered: each transition is reported once, and callers should
/// compare the snapshot that was current before the refresh with the snapshot
/// the refresh accepted (post-verification), so transient provider spikes that
/// the stabilization pipeline rejects never produce events.
pub fn detect_provider_events(
    previous: &[ProviderSnapshot],
    current: &[ProviderSnapshot],
    now: DateTime<Utc>,
    low_quota_threshold: f64,
) -> Vec<PendingQuotaEvent> {
    let mut events = Vec::new();
    for current_provider in current {
        let Some(previous_provider) = previous.iter().find(|p| p.id == current_provider.id) else {
            continue;
        };

        let previous_failing = provider_failing(&previous_provider.status);
        let current_failing = provider_failing(&current_provider.status);
        let previous_healthy = provider_healthy(&previous_provider.status);
        let current_healthy = provider_healthy(&current_provider.status);

        // "unknown" counts as not-yet-failing so a first observed failure
        // still notifies, but only a genuinely healthy result counts as a
        // recovery.
        if !previous_failing && current_failing {
            let mut details = serde_json::json!({ "status": current_provider.status });
            if let Some(error) = current_provider.error.as_deref() {
                details["error"] = serde_json::json!(truncate_detail(error));
            }
            events.push(pending_event(
                NOTIFICATION_EVENT_PROVIDER_ERROR,
                SEVERITY_ERROR,
                current_provider,
                None,
                details,
            ));
        } else if previous_failing && current_healthy {
            events.push(pending_event(
                NOTIFICATION_EVENT_PROVIDER_RECOVERED,
                SEVERITY_INFO,
                current_provider,
                None,
                serde_json::json!({ "previousStatus": previous_provider.status }),
            ));
        }

        if !(previous_healthy && current_healthy) {
            continue;
        }

        for current_window in &current_provider.windows {
            let Some(previous_window) = previous_provider
                .windows
                .iter()
                .find(|window| window.id == current_window.id)
            else {
                continue;
            };
            detect_window_events(
                previous_window,
                current_provider,
                current_window,
                now,
                low_quota_threshold,
                &mut events,
            );
        }
    }
    events
}

fn detect_window_events(
    previous_window: &QuotaWindow,
    current_provider: &ProviderSnapshot,
    current_window: &QuotaWindow,
    now: DateTime<Utc>,
    low_quota_threshold: f64,
    events: &mut Vec<PendingQuotaEvent>,
) {
    let previous_used = previous_window.used_percent;
    let current_used = current_window.used_percent;
    let current_remaining = current_window.remaining_percent;

    if let (Some(previous_reset), Some(current_reset)) = (
        previous_window.reset_at.as_deref().and_then(parse_rfc3339),
        current_window.reset_at.as_deref().and_then(parse_rfc3339),
    ) {
        let advanced = current_reset.timestamp()
            > previous_reset.timestamp() + RESET_ADVANCE_TOLERANCE_SECONDS;
        if advanced && previous_reset <= now {
            // The previous cycle's boundary has passed, so this is a
            // regular reset into a new cycle.
            let mut details = serde_json::json!({
                "resetAt": current_window.reset_at,
            });
            if let (Some(previous_used), Some(current_used)) = (previous_used, current_used) {
                details["usedPercentBefore"] = serde_json::json!(previous_used);
                details["usedPercentAfter"] = serde_json::json!(current_used);
            }
            if let Some(current_remaining) = current_remaining {
                details["remainingPercent"] = serde_json::json!(current_remaining);
            }
            events.push(pending_event(
                NOTIFICATION_EVENT_QUOTA_RESET,
                SEVERITY_POSITIVE,
                current_provider,
                Some(current_window),
                details,
            ));
        } else if let (Some(previous_used), Some(current_used)) = (previous_used, current_used) {
            // The old window had not ended (or the provider has not rolled its
            // reset time yet), so a rebound this large is quota the provider
            // granted back mid-cycle. The snapshot stabilization pipeline has
            // already confirmed the jump before it can reach this diff.
            if previous_used.is_finite()
                && current_used.is_finite()
                && previous_used - current_used >= UNEXPECTED_RECOVERY_DROP_PERCENT
            {
                events.push(pending_event(
                    NOTIFICATION_EVENT_QUOTA_RECOVERED_UNEXPECTED,
                    SEVERITY_WARNING,
                    current_provider,
                    Some(current_window),
                    serde_json::json!({
                        "usedPercentBefore": previous_used,
                        "usedPercentAfter": current_used,
                        "resetAt": current_window.reset_at,
                    }),
                ));
            }
        }
    }

    let previous_remaining = previous_window.remaining_percent;
    if let (Some(previous_remaining), Some(current_remaining)) = (previous_remaining, current_remaining)
    {
        let exhausted_now = current_remaining <= EXHAUSTED_REMAINING_PERCENT;
        let was_exhausted = previous_remaining <= EXHAUSTED_REMAINING_PERCENT;
        if !was_exhausted && exhausted_now {
            let mut details = serde_json::json!({ "remainingPercent": current_remaining });
            if let Some(current_used) = current_used {
                details["usedPercent"] = serde_json::json!(current_used);
            }
            events.push(pending_event(
                NOTIFICATION_EVENT_QUOTA_EXHAUSTED,
                SEVERITY_ERROR,
                current_provider,
                Some(current_window),
                details,
            ));
        } else if low_quota_threshold > 0.0
            && previous_remaining > low_quota_threshold
            && current_remaining <= low_quota_threshold
            && !exhausted_now
        {
            events.push(pending_event(
                NOTIFICATION_EVENT_QUOTA_LOW,
                SEVERITY_WARNING,
                current_provider,
                Some(current_window),
                serde_json::json!({
                    "threshold": low_quota_threshold,
                    "remainingPercent": current_remaining,
                }),
            ));
        }
    }
}

pub fn pending_app_started(started_hidden: bool) -> PendingQuotaEvent {
    PendingQuotaEvent {
        event_type: NOTIFICATION_EVENT_APP_STARTED.to_string(),
        severity: SEVERITY_INFO.to_string(),
        provider_id: None,
        provider_name: None,
        window_id: None,
        window_label: None,
        details: Some(serde_json::json!({ "startedHidden": started_hidden })),
    }
}

pub fn pending_app_update_applied(version: &str) -> PendingQuotaEvent {
    PendingQuotaEvent {
        event_type: NOTIFICATION_EVENT_APP_UPDATE_APPLIED.to_string(),
        severity: SEVERITY_POSITIVE.to_string(),
        provider_id: None,
        provider_name: None,
        window_id: None,
        window_label: None,
        details: Some(serde_json::json!({ "version": version })),
    }
}

fn load_history_from_disk(path: &Path) -> EventHistoryFile {
    let history_path = history_path_for_config_path(path);
    let Ok(contents) = fs::read_to_string(&history_path) else {
        return EventHistoryFile::default();
    };
    match serde_json::from_str::<EventHistoryFile>(&contents) {
        Ok(history) if history.schema_version == EVENT_HISTORY_SCHEMA_VERSION => history,
        _ => EventHistoryFile::default(),
    }
}

fn persist_history(path: &Path, history: &EventHistoryFile) -> Result<(), String> {
    let history_path = history_path_for_config_path(path);
    if let Some(parent) = history_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let temporary_path = history_path.with_file_name(format!(".{EVENT_HISTORY_FILE_NAME}.tmp"));
    let contents = serde_json::to_string_pretty(history).map_err(|error| error.to_string())?;
    fs::write(&temporary_path, contents).map_err(|error| error.to_string())?;
    if history_path.exists() {
        fs::remove_file(&history_path).map_err(|error| error.to_string())?;
    }
    fs::rename(&temporary_path, &history_path).map_err(|error| {
        let _ = fs::remove_file(&temporary_path);
        error.to_string()
    })
}

/// Assigns ids and timestamps, appends to the in-memory + on-disk history,
/// prunes the oldest entries beyond the cap, and returns what was recorded.
pub fn append_events(
    path: &Path,
    pending: Vec<PendingQuotaEvent>,
) -> Result<Vec<QuotaEvent>, String> {
    if pending.is_empty() {
        return Ok(Vec::new());
    }
    let mut guard = event_history_cache()
        .lock()
        .map_err(|_| "Event history lock poisoned".to_string())?;
    let mut history = guard
        .clone()
        .unwrap_or_else(|| load_history_from_disk(path));
    if history.next_id == 0 {
        history.next_id = 1;
    }
    history.schema_version = EVENT_HISTORY_SCHEMA_VERSION;
    let now = Utc::now().to_rfc3339();
    let recorded = pending
        .into_iter()
        .map(|pending| QuotaEvent {
            id: {
                let id = history.next_id;
                history.next_id += 1;
                id
            },
            occurred_at: now.clone(),
            event_type: pending.event_type,
            severity: pending.severity,
            provider_id: pending.provider_id,
            provider_name: pending.provider_name,
            window_id: pending.window_id,
            window_label: pending.window_label,
            details: pending.details,
        })
        .collect::<Vec<_>>();
    history.events.extend(recorded.iter().cloned());
    if history.events.len() > EVENT_HISTORY_LIMIT {
        let overflow = history.events.len() - EVENT_HISTORY_LIMIT;
        history.events.drain(0..overflow);
    }
    persist_history(path, &history)?;
    *guard = Some(history);
    Ok(recorded)
}

/// Returns the recorded events newest-first.
pub fn read_events(path: &Path) -> Result<Vec<QuotaEvent>, String> {
    let guard = event_history_cache()
        .lock()
        .map_err(|_| "Event history lock poisoned".to_string())?;
    if let Some(history) = guard.as_ref() {
        return Ok(history.events.iter().rev().cloned().collect());
    }
    drop(guard);
    let history = load_history_from_disk(path);
    let events: Vec<QuotaEvent> = history.events.iter().rev().cloned().collect();
    Ok(events)
}

pub fn clear_events(path: &Path) -> Result<(), String> {
    let mut guard = event_history_cache()
        .lock()
        .map_err(|_| "Event history lock poisoned".to_string())?;
    let next_id = guard.as_ref().map(|history| history.next_id).unwrap_or(1);
    let history = EventHistoryFile {
        schema_version: EVENT_HISTORY_SCHEMA_VERSION,
        next_id,
        events: Vec::new(),
    };
    persist_history(path, &history)?;
    *guard = Some(history);
    Ok(())
}

#[tauri::command]
pub fn get_quota_event_history(app: AppHandle) -> Result<Vec<QuotaEvent>, String> {
    let path = config_path_for_app(&app)?;
    read_events(&path)
}

#[tauri::command]
pub fn clear_quota_event_history(app: AppHandle) -> Result<(), String> {
    let path = config_path_for_app(&app)?;
    clear_events(&path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quota::ProviderDiagnostics;
    use std::sync::{Mutex, MutexGuard};

    static TEST_HISTORY_LOCK: Mutex<()> = Mutex::new(());

    fn isolate_history() -> MutexGuard<'static, ()> {
        let guard = TEST_HISTORY_LOCK.lock().expect("test history lock");
        *event_history_cache().lock().expect("lock") = None;
        guard
    }

    fn window(id: &str, used_percent: f64, reset_at: Option<&str>) -> QuotaWindow {
        QuotaWindow {
            id: id.to_string(),
            label: format!("{id} window"),
            remaining: None,
            used: None,
            limit: None,
            unit: Some("percent".to_string()),
            used_percent: Some(used_percent),
            remaining_percent: Some(100.0 - used_percent),
            warning_remaining: None,
            reset_at: reset_at.map(str::to_string),
            reset_text: None,
            confidence: "exact".to_string(),
        }
    }

    fn provider(id: &str, status: &str, windows: Vec<QuotaWindow>) -> ProviderSnapshot {
        ProviderSnapshot {
            id: id.to_string(),
            name: format!("Provider {id}"),
            status: status.to_string(),
            source: "remote".to_string(),
            updated_at: Some("2026-10-05T10:00:00Z".to_string()),
            windows,
            error: None,
            diagnostics: None,
            metadata: None,
        }
    }

    fn detect_simple(
        previous: &ProviderSnapshot,
        current: &ProviderSnapshot,
    ) -> Vec<PendingQuotaEvent> {
        detect_provider_events(
            &[previous.clone()],
            &[current.clone()],
            DateTime::parse_from_rfc3339("2026-10-05T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            20.0,
        )
    }

    #[test]
    fn steady_state_detects_nothing() {
        let previous = provider("a", "ok", vec![window("5h", 40.0, Some("2026-10-05T16:00:00Z"))]);
        let current = provider("a", "ok", vec![window("5h", 42.0, Some("2026-10-05T16:00:00Z"))]);
        assert!(detect_simple(&previous, &current).is_empty());
    }

    #[test]
    fn passed_boundary_with_later_reset_records_quota_reset() {
        let previous = provider("a", "ok", vec![window("5h", 80.0, Some("2026-10-05T11:00:00Z"))]);
        let current = provider("a", "ok", vec![window("5h", 5.0, Some("2026-10-05T16:00:00Z"))]);
        let events = detect_simple(&previous, &current);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, NOTIFICATION_EVENT_QUOTA_RESET);
        assert_eq!(events[0].severity, SEVERITY_POSITIVE);
        assert_eq!(events[0].window_id.as_deref(), Some("5h"));
        assert_eq!(events[0].provider_name.as_deref(), Some("Provider a"));
    }

    #[test]
    fn reset_time_revision_without_rebound_records_nothing() {
        let previous = provider("a", "ok", vec![window("5h", 40.0, Some("2026-10-05T16:00:00Z"))]);
        let current = provider("a", "ok", vec![window("5h", 42.0, Some("2026-10-05T16:30:00Z"))]);
        assert!(detect_simple(&previous, &current).is_empty());
    }

    #[test]
    fn mid_cycle_confirmed_rebound_records_unexpected_recovery() {
        // Same cycle (reset time unchanged, boundary still in the future), but
        // the accepted snapshot rebounded by more than the verification
        // threshold: quota came back without the window ending.
        let previous = provider("a", "ok", vec![window("5h", 80.0, Some("2026-10-05T16:00:00Z"))]);
        let current = provider("a", "ok", vec![window("5h", 5.0, Some("2026-10-05T16:00:00Z"))]);
        let events = detect_simple(&previous, &current);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, NOTIFICATION_EVENT_QUOTA_RECOVERED_UNEXPECTED);
        assert_eq!(events[0].severity, SEVERITY_WARNING);
    }

    #[test]
    fn boundary_passed_with_unmoved_reset_time_is_not_a_reset_yet() {
        // The old boundary has passed but the provider has not rolled its
        // reset time yet and usage barely moved: no event until either the
        // reset time advances or the quota actually rebounds.
        let previous = provider("a", "ok", vec![window("5h", 80.0, Some("2026-10-05T11:00:00Z"))]);
        let current = provider("a", "ok", vec![window("5h", 82.0, Some("2026-10-05T11:00:30Z"))]);
        assert!(detect_simple(&previous, &current).is_empty());
    }

    #[test]
    fn exhausted_crossing_records_exhausted_without_low() {
        let previous = provider("a", "ok", vec![window("weekly", 95.0, Some("2026-10-06T00:00:00Z"))]);
        let current = provider("a", "ok", vec![window("weekly", 100.0, Some("2026-10-06T00:00:00Z"))]);
        let events = detect_simple(&previous, &current);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, NOTIFICATION_EVENT_QUOTA_EXHAUSTED);
        assert_eq!(events[0].severity, SEVERITY_ERROR);
    }

    #[test]
    fn threshold_crossing_records_low_quota() {
        let previous = provider("a", "ok", vec![window("weekly", 70.0, Some("2026-10-06T00:00:00Z"))]);
        let current = provider("a", "ok", vec![window("weekly", 85.0, Some("2026-10-06T00:00:00Z"))]);
        let events = detect_simple(&previous, &current);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, NOTIFICATION_EVENT_QUOTA_LOW);
        assert_eq!(events[0].details.as_ref().unwrap()["threshold"], serde_json::json!(20.0));
    }

    #[test]
    fn provider_failure_and_recovery_are_edge_triggered() {
        let healthy = provider("a", "ok", vec![window("5h", 40.0, Some("2026-10-05T16:00:00Z"))]);

        let mut failing = healthy.clone();
        failing.status = "error".to_string();
        failing.error = Some("Remote provider timed out".to_string());
        failing.diagnostics = Some(ProviderDiagnostics {
            checked_at: "2026-10-05T12:00:00Z".to_string(),
            messages: vec![],
            command_path: Some("node".to_string()),
            exit_code: Some(1),
            duration_ms: Some(100),
            timed_out: Some(false),
            stderr: None,
        });

        let first = detect_simple(&healthy, &failing);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].event_type, NOTIFICATION_EVENT_PROVIDER_ERROR);
        assert_eq!(first[0].details.as_ref().unwrap()["error"], serde_json::json!("Remote provider timed out"));

        // Still failing on the next refresh: no repeat event.
        assert!(detect_simple(&failing, &failing).is_empty());

        let recovered = detect_simple(&failing, &healthy);
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].event_type, NOTIFICATION_EVENT_PROVIDER_RECOVERED);
    }

    #[test]
    fn new_providers_without_history_produce_no_events() {
        let current = provider("fresh", "ok", vec![window("5h", 40.0, None)]);
        assert!(detect_provider_events(&[], &[current], Utc::now(), 20.0).is_empty());
    }

    #[test]
    fn append_assigns_ids_persists_and_reads_newest_first() {
        let _guard = isolate_history();
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");

        let recorded = append_events(
            &path,
            vec![pending_app_started(false), pending_app_update_applied("1.6.0")],
        )
        .expect("append");
        assert_eq!(recorded.len(), 2);
        assert_eq!(recorded[0].id, 1);
        assert_eq!(recorded[1].id, 2);
        assert!(!recorded[0].occurred_at.is_empty());

        let events = read_events(&path).expect("read");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].id, 2, "newest first");

        // A cold process re-reads the same history from disk.
        *event_history_cache().lock().unwrap() = None;
        let events = read_events(&path).expect("read from disk");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].id, 2);

        clear_events(&path).expect("clear");
        assert!(read_events(&path).expect("read after clear").is_empty());
    }

    #[test]
    fn append_prunes_oldest_events_beyond_the_limit() {
        let _guard = isolate_history();
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");

        for _ in 0..(EVENT_HISTORY_LIMIT + 5) {
            append_events(&path, vec![pending_app_started(false)]).expect("append");
        }

        let events = read_events(&path).expect("read");
        assert_eq!(events.len(), EVENT_HISTORY_LIMIT);
        assert_eq!(events[0].id, (EVENT_HISTORY_LIMIT + 5) as u64, "newest kept");
        assert_eq!(events[events.len() - 1].id, 6, "oldest pruned");
    }

    #[test]
    fn corrupt_history_file_is_treated_as_empty() {
        let _guard = isolate_history();
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        fs::write(
            history_path_for_config_path(&path),
            "{ not json",
        )
        .expect("write corrupt file");

        assert!(read_events(&path).expect("read").is_empty());
        let recorded = append_events(&path, vec![pending_app_started(true)]).expect("append");
        assert_eq!(recorded[0].id, 1);
    }

    #[test]
    fn quota_event_serializes_frontend_shape() {
        let event = QuotaEvent {
            id: 7,
            occurred_at: "2026-10-05T12:00:00Z".to_string(),
            event_type: NOTIFICATION_EVENT_QUOTA_RESET.to_string(),
            severity: SEVERITY_POSITIVE.to_string(),
            provider_id: Some("remote-a".to_string()),
            provider_name: Some("Remote A".to_string()),
            window_id: Some("5h".to_string()),
            window_label: Some("5h window".to_string()),
            details: Some(serde_json::json!({ "remainingPercent": 95.0 })),
        };

        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["eventType"], serde_json::json!("quota-reset"));
        assert_eq!(value["occurredAt"], serde_json::json!("2026-10-05T12:00:00Z"));
        assert_eq!(value["windowLabel"], serde_json::json!("5h window"));
        assert!(value.get("event_type").is_none());
    }
}
