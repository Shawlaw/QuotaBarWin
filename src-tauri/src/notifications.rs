use std::{path::Path, thread};

use serde::Serialize;
use tauri::AppHandle;

use crate::config::{
    config_path_for_app, load_or_create_config, resolve_secret_value, AppLanguage,
    NotificationSettings, NOTIFICATION_EVENT_APP_STARTED, NOTIFICATION_EVENT_APP_UPDATE_APPLIED,
    NOTIFICATION_EVENT_PROVIDER_ERROR, NOTIFICATION_EVENT_PROVIDER_RECOVERED,
    NOTIFICATION_EVENT_QUOTA_EXHAUSTED, NOTIFICATION_EVENT_QUOTA_LOW,
    NOTIFICATION_EVENT_QUOTA_RECOVERED_UNEXPECTED, NOTIFICATION_EVENT_QUOTA_RESET,
};
use crate::logger::{LogLevel, LogSink};
use crate::proxy::{self, ProxyConfig};
use crate::quota_events::QuotaEvent;

const NOTIFICATION_EVENT_TEST: &str = "test-notification";
const WEBHOOK_MIN_TIMEOUT_SECONDS: u64 = 1;
const WEBHOOK_MAX_TIMEOUT_SECONDS: u64 = 60;
// The toast brand icon; extracted to a per-user location so both the toast XML
// image and the AUMID registry IconUri can reference a stable file.
#[cfg(windows)]
const TOAST_ICON_PNG: &[u8] = include_bytes!("../icons/128x128.png");

#[cfg(windows)]
fn toast_icon_path() -> Option<std::path::PathBuf> {
    let local_app_data = std::env::var_os("LOCALAPPDATA")?;
    Some(
        std::path::PathBuf::from(local_app_data)
            .join("QuotaBarWin")
            .join("toast-icon.png"),
    )
}

/// Materializes the embedded brand icon for toast display. Machine-local by
/// design, like the AppUserModelID registry key itself. Returns None on any
/// failure; toasts then fall back to the generic Windows icon.
#[cfg(windows)]
fn ensure_toast_icon_file() -> Option<std::path::PathBuf> {
    let path = toast_icon_path()?;
    let is_current = std::fs::metadata(&path)
        .ok()
        .is_some_and(|metadata| metadata.len() == TOAST_ICON_PNG.len() as u64);
    if !is_current {
        std::fs::create_dir_all(path.parent()?).ok()?;
        std::fs::write(&path, TOAST_ICON_PNG).ok()?;
    }
    Some(path)
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChannelOutcome {
    // "sent" | "skipped" | "failed"
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
}

impl ChannelOutcome {
    fn sent(status_code: Option<u16>) -> Self {
        Self {
            status: "sent".to_string(),
            detail: None,
            status_code,
        }
    }

    fn skipped(detail: &str) -> Self {
        Self {
            status: "skipped".to_string(),
            detail: Some(detail.to_string()),
            status_code: None,
        }
    }

    fn failed(detail: String) -> Self {
        Self {
            status: "failed".to_string(),
            detail: Some(detail),
            status_code: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestNotificationResult {
    pub toast: ChannelOutcome,
    pub webhook: ChannelOutcome,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebhookPayload<'a> {
    schema_version: u8,
    app: &'a str,
    app_version: String,
    sent_at: String,
    events: &'a [QuotaEvent],
}

/// True when the configured notification event list includes this event type.
/// An empty list selects nothing, so clearing the list silences notifications.
pub fn event_matches_filter(settings: &NotificationSettings, event_type: &str) -> bool {
    settings.events.iter().any(|allowed| allowed == event_type)
}

/// Spawns delivery for events recorded by a refresh. Delivery must never block
/// or fail the refresh itself, so everything happens on a worker thread and
/// failures only reach the structured log.
pub fn dispatch_events(
    config_path: &Path,
    settings: NotificationSettings,
    global_proxy: Option<ProxyConfig>,
    language: AppLanguage,
    events: Vec<QuotaEvent>,
) {
    if events.is_empty() || (!settings.toast_enabled && !settings.webhook_enabled) {
        return;
    }
    let config_path = config_path.to_path_buf();
    let spawn_result = thread::Builder::new()
        .name("quotabarwin-notifications".to_string())
        .spawn(move || {
            let selected: Vec<QuotaEvent> = events
                .into_iter()
                .filter(|event| event_matches_filter(&settings, &event.event_type))
                .collect();
            if selected.is_empty() {
                return;
            }
            if settings.toast_enabled {
                if let Err(error) = show_toast_batch(&language, &selected) {
                    log_notification_failure(&config_path, "toast", &error);
                }
            }
            if settings.webhook_enabled {
                let outcome = send_webhook(&config_path, &settings, global_proxy.as_ref(), &selected);
                if outcome.status == "failed" {
                    log_notification_failure(
                        &config_path,
                        "webhook",
                        outcome.detail.as_deref().unwrap_or("unknown error"),
                    );
                }
            }
        });
    if let Err(error) = spawn_result {
        eprintln!("Failed to start notification worker: {error}");
    }
}

fn log_notification_failure(config_path: &Path, channel: &str, detail: &str) {
    let loaded = load_or_create_config(config_path);
    let log = match loaded {
        Ok(loaded) => LogSink::from_config_path(config_path, &loaded.config),
        Err(_) => return,
    };
    let _ = log.write(
        LogLevel::Warn,
        "notifications",
        &format!(
            "notification delivery failed channel={} detail={}",
            crate::redact::redact_sensitive(detail),
            channel
        ),
    );
}

pub fn send_webhook(
    config_path: &Path,
    settings: &NotificationSettings,
    global_proxy: Option<&ProxyConfig>,
    events: &[QuotaEvent],
) -> ChannelOutcome {
    if events.is_empty() {
        return ChannelOutcome::skipped("no events");
    }
    let raw_url = settings.webhook_url.as_deref().unwrap_or("").trim();
    if raw_url.is_empty() {
        return ChannelOutcome::skipped("webhook URL is not configured");
    }
    let config_dir = config_path.parent().unwrap_or_else(|| Path::new("."));
    let resolved_url = match resolve_secret_value(raw_url, config_dir) {
        Ok(url) => url,
        Err(error) => return ChannelOutcome::failed(crate::redact::redact_sensitive(&error)),
    };
    if !(resolved_url.starts_with("http://") || resolved_url.starts_with("https://")) {
        return ChannelOutcome::failed("webhook URL must start with http:// or https://".to_string());
    }

    let timeout_seconds = settings
        .webhook_timeout_seconds
        .clamp(WEBHOOK_MIN_TIMEOUT_SECONDS, WEBHOOK_MAX_TIMEOUT_SECONDS);
    let client = match proxy::build_http_client(
        None,
        global_proxy,
        std::time::Duration::from_secs(timeout_seconds),
    ) {
        Ok(client) => client,
        Err(error) => return ChannelOutcome::failed(crate::redact::redact_sensitive(&error)),
    };

    let payload = WebhookPayload {
        schema_version: 1,
        app: "QuotaBarWin",
        app_version: crate::app_info::app_display_version(),
        sent_at: chrono::Utc::now().to_rfc3339(),
        events,
    };
    match client.post(&resolved_url).json(&payload).send() {
        Ok(response) => {
            let status_code = response.status().as_u16();
            if response.status().is_success() {
                ChannelOutcome::sent(Some(status_code))
            } else {
                ChannelOutcome::failed(format!("webhook returned HTTP {status_code}"))
            }
        }
        Err(error) => {
            // The resolved URL can embed secrets from ${secret:} references;
            // only the redacted error leaves this function.
            ChannelOutcome::failed(crate::redact::redact_sensitive(&error.to_string()))
        }
    }
}

fn language_is_chinese(language: &AppLanguage) -> bool {
    matches!(language, AppLanguage::System | AppLanguage::ZhCn)
}

fn format_percent(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

fn detail_number(details: &serde_json::Value, key: &str) -> Option<f64> {
    details.get(key).and_then(|value| value.as_f64())
}

fn detail_string(details: &serde_json::Value, key: &str) -> Option<String> {
    details
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

pub fn describe_event(language: &AppLanguage, event: &QuotaEvent) -> String {
    let chinese = language_is_chinese(language);
    let details = event.details.clone().unwrap_or_default();
    let provider = event.provider_name.as_deref().unwrap_or("");
    let window = event.window_label.as_deref().unwrap_or("");
    let subject = if window.is_empty() {
        provider.to_string()
    } else {
        format!("{provider} {window}")
    };

    match event.event_type.as_str() {
        NOTIFICATION_EVENT_QUOTA_RESET => {
            if let Some(remaining) = detail_number(&details, "remainingPercent") {
                if chinese {
                    format!("{subject} 额度已重置（剩余 {}%）", format_percent(remaining))
                } else {
                    format!("{subject} quota reset ({remaining}% remaining)", remaining = format_percent(remaining))
                }
            } else if chinese {
                format!("{subject} 额度已重置")
            } else {
                format!("{subject} quota reset")
            }
        }
        NOTIFICATION_EVENT_QUOTA_RECOVERED_UNEXPECTED => {
            let before = detail_number(&details, "usedPercentBefore")
                .map(format_percent)
                .unwrap_or_else(|| "?".to_string());
            let after = detail_number(&details, "usedPercentAfter")
                .map(format_percent)
                .unwrap_or_else(|| "?".to_string());
            if chinese {
                format!("{subject} 额度异常回升（已用 {before}% → {after}%）")
            } else {
                format!("{subject} quota recovered unexpectedly (used {before}% → {after}%)")
            }
        }
        NOTIFICATION_EVENT_QUOTA_EXHAUSTED => {
            if chinese {
                format!("{subject} 额度已用尽")
            } else {
                format!("{subject} quota exhausted")
            }
        }
        NOTIFICATION_EVENT_QUOTA_LOW => {
            let remaining = detail_number(&details, "remainingPercent")
                .map(format_percent)
                .unwrap_or_else(|| "?".to_string());
            if chinese {
                format!("{subject} 额度偏低（剩余 {remaining}%）")
            } else {
                format!("{subject} quota is low ({remaining}% remaining)")
            }
        }
        NOTIFICATION_EVENT_PROVIDER_ERROR => {
            if chinese {
                format!("{provider} 刷新失败")
            } else {
                format!("{provider} refresh failed")
            }
        }
        NOTIFICATION_EVENT_PROVIDER_RECOVERED => {
            if chinese {
                format!("{provider} 已恢复正常")
            } else {
                format!("{provider} recovered")
            }
        }
        NOTIFICATION_EVENT_APP_UPDATE_APPLIED => {
            let version = detail_string(&details, "version").unwrap_or_default();
            if chinese {
                format!("已更新到 {version}")
            } else {
                format!("Updated to {version}")
            }
        }
        NOTIFICATION_EVENT_APP_STARTED => {
            if chinese {
                "应用已启动".to_string()
            } else {
                "Application started".to_string()
            }
        }
        NOTIFICATION_EVENT_TEST => {
            if chinese {
                "测试通知：通知通道工作正常".to_string()
            } else {
                "Test notification: channels are working".to_string()
            }
        }
        _ => {
            if chinese {
                format!("事件：{}", event.event_type)
            } else {
                format!("Event: {}", event.event_type)
            }
        }
    }
}

fn test_event() -> QuotaEvent {
    QuotaEvent {
        id: 0,
        occurred_at: chrono::Utc::now().to_rfc3339(),
        event_type: NOTIFICATION_EVENT_TEST.to_string(),
        severity: crate::quota_events::SEVERITY_INFO.to_string(),
        provider_id: None,
        provider_name: None,
        window_id: None,
        window_label: None,
        details: None,
    }
}

#[cfg(windows)]
pub fn show_toast_batch(
    language: &AppLanguage,
    events: &[QuotaEvent],
) -> Result<(), String> {
    if events.is_empty() {
        return Ok(());
    }
    use crate::app_identity::APP_USER_MODEL_ID;
    use windows_sys::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

    // RPC_E_CHANGED_MODE as an unsigned HRESULT: the thread already joined a
    // different apartment model, which WinRT tolerates for toasts.
    const RPC_E_CHANGED_MODE: u32 = 0x8001_0106;

    // WinRT toast calls need an apartment on the calling thread. S_FALSE means
    // the apartment already existed.
    let coinit = unsafe { CoInitializeEx(std::ptr::null(), COINIT_MULTITHREADED as u32) };
    let need_uninitialize = coinit == 0 || coinit == 1;
    if coinit < 0 && (coinit as u32) != RPC_E_CHANGED_MODE {
        let hresult = coinit as u32;
        return Err(format!("CoInitializeEx failed: HRESULT 0x{hresult:08X}"));
    }

    let result = (|| {
        // The icon is only registered for the notification center's app
        // header (IconUri); the toast body itself stays text-only.
        let icon_path = ensure_toast_icon_file();
        ensure_app_identity_registration(icon_path.as_deref())?;
        let chinese = language_is_chinese(language);
        let first = describe_event(language, &events[0]);
        let mut toast = winrt_notification::Toast::new(APP_USER_MODEL_ID).title("QuotaBarWin");
        if events.len() > 1 {
            let more = if chinese {
                format!("以及另外 {} 个事件", events.len() - 1)
            } else {
                format!("+ {} more events", events.len() - 1)
            };
            toast = toast.text1(&first).text2(&more);
        } else {
            toast = toast.text1(&first);
        }
        toast.show().map_err(|error| format!("toast failed: {error}"))
    })();

    if need_uninitialize {
        unsafe { CoUninitialize() };
    }
    result
}

#[cfg(not(windows))]
pub fn show_toast_batch(_language: &AppLanguage, _events: &[QuotaEvent]) -> Result<(), String> {
    Err("toast notifications are only supported on Windows".to_string())
}

// A WinRT toast is only displayed when its AppUserModelID is known to the
// shell. Bundled installers create a Start Menu shortcut for this; the
// portable build instead registers the AppUserModelID key in HKCU (the same
// mechanism Firefox/Chrome and PowerShell's BurntToast use), including an
// IconUri pointing at the extracted brand icon so toasts are branded.
#[cfg(windows)]
fn ensure_app_identity_registration(icon_path: Option<&Path>) -> Result<(), String> {
    use crate::app_identity::APP_USER_MODEL_ID;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegSetValueExW, HKEY_CURRENT_USER, KEY_SET_VALUE,
        REG_OPTION_NON_VOLATILE, REG_EXPAND_SZ,
    };

    fn set_registry_value(
        hkey: windows_sys::Win32::Foundation::HANDLE,
        name: &str,
        value: &str,
    ) -> Result<(), u32> {
        let name: Vec<u16> = name.encode_utf16().chain([0]).collect();
        let value: Vec<u16> = value.encode_utf16().chain([0]).collect();
        let result = unsafe {
            RegSetValueExW(
                hkey,
                name.as_ptr(),
                0,
                REG_EXPAND_SZ,
                value.as_ptr().cast(),
                (value.len() * 2) as u32,
            )
        };
        if result == 0 {
            Ok(())
        } else {
            Err(result)
        }
    }

    let subkey: Vec<u16> = "Software\\Classes\\AppUserModelId\\"
        .encode_utf16()
        .chain(APP_USER_MODEL_ID.encode_utf16())
        .chain([0])
        .collect();

    let mut hkey = std::ptr::null_mut();
    let create_result = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut hkey,
            std::ptr::null_mut(),
        )
    };
    if create_result != 0 {
        return Err(format!(
            "failed to register AppUserModelID key: error {create_result}"
        ));
    }
    let set_results = [
        set_registry_value(hkey, "DisplayName", "QuotaBarWin"),
        match icon_path {
            Some(icon_path) => set_registry_value(
                hkey,
                "IconUri",
                &icon_path.to_string_lossy(),
            ),
            None => Ok(()),
        },
    ];
    unsafe { RegCloseKey(hkey) };
    for result in set_results {
        if let Err(error) = result {
            return Err(format!("failed to set AppUserModelID value: error {error}"));
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn send_test_notification(
    app: AppHandle,
    settings: NotificationSettings,
) -> Result<TestNotificationResult, String> {
    // The caller passes the settings form's current draft, so testing does not
    // require saving first; language and proxy still follow the saved config.
    let path = config_path_for_app(&app)?;
    let loaded = load_or_create_config(&path)?;
    let language = loaded.config.language.clone();
    let global_proxy = loaded.config.network_proxy.clone();
    let event = test_event();

    let toast = if settings.toast_enabled {
        match show_toast_batch(&language, std::slice::from_ref(&event)) {
            Ok(()) => ChannelOutcome::sent(None),
            Err(error) => ChannelOutcome::failed(error),
        }
    } else {
        ChannelOutcome::skipped("toast notifications are disabled")
    };

    let webhook = if settings.webhook_enabled {
        send_webhook(&path, &settings, global_proxy.as_ref(), &[event])
    } else {
        ChannelOutcome::skipped("webhook is disabled")
    };

    Ok(TestNotificationResult { toast, webhook })
}

// --- Temporary self-test helpers for the 1.6.0 manual QA pass. Remove these
// (plus their lib.rs registrations, api.ts bindings, and settings UI block)
// after verification.

#[tauri::command]
pub async fn debug_show_test_toast(app: AppHandle) -> Result<ChannelOutcome, String> {
    let path = config_path_for_app(&app)?;
    let loaded = load_or_create_config(&path)?;
    let event = test_event();
    match show_toast_batch(&loaded.config.language, std::slice::from_ref(&event)) {
        Ok(()) => Ok(ChannelOutcome::sent(None)),
        Err(error) => Ok(ChannelOutcome::failed(error)),
    }
}

#[tauri::command]
pub async fn debug_remove_toast_registration() -> Result<ChannelOutcome, String> {
    remove_app_identity_registration()
}

#[cfg(windows)]
fn remove_app_identity_registration() -> Result<ChannelOutcome, String> {
    use crate::app_identity::APP_USER_MODEL_ID;
    use windows_sys::Win32::System::Registry::{RegDeleteTreeW, HKEY_CURRENT_USER};

    let subkey: Vec<u16> = "Software\\Classes\\AppUserModelId\\"
        .encode_utf16()
        .chain(APP_USER_MODEL_ID.encode_utf16())
        .chain([0])
        .collect();
    const ERROR_FILE_NOT_FOUND: u32 = 2;
    let result = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, subkey.as_ptr()) };
    // The extracted icon is part of the same registration footprint.
    if let Some(icon_path) = toast_icon_path() {
        let _ = std::fs::remove_file(icon_path);
    }
    match result {
        0 => Ok(ChannelOutcome {
            status: "sent".to_string(),
            detail: Some("app user model id registry key removed".to_string()),
            status_code: None,
        }),
        ERROR_FILE_NOT_FOUND => Ok(ChannelOutcome::skipped(
            "app user model id registry key was not present",
        )),
        code => Ok(ChannelOutcome::failed(format!(
            "RegDeleteTreeW failed: error {code}"
        ))),
    }
}

#[cfg(not(windows))]
fn remove_app_identity_registration() -> Result<ChannelOutcome, String> {
    Ok(ChannelOutcome::skipped(
        "toast registration is only managed on Windows",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::default_notification_settings;
    use crate::quota_events::{
        pending_app_started, pending_app_update_applied, SEVERITY_POSITIVE,
    };
    use std::io::Read;

    fn recorded_event(event_type: &str) -> QuotaEvent {
        QuotaEvent {
            id: 1,
            occurred_at: "2026-10-05T12:00:00Z".to_string(),
            event_type: event_type.to_string(),
            severity: SEVERITY_POSITIVE.to_string(),
            provider_id: Some("remote-a".to_string()),
            provider_name: Some("Remote A".to_string()),
            window_id: Some("5h".to_string()),
            window_label: Some("5h window".to_string()),
            details: Some(serde_json::json!({
                "remainingPercent": 95.0,
                "usedPercentBefore": 80.0,
                "usedPercentAfter": 5.0,
                "version": "1.6.0"
            })),
        }
    }

    #[test]
    fn event_filter_follows_the_configured_list() {
        let mut settings = default_notification_settings();
        assert!(event_matches_filter(&settings, NOTIFICATION_EVENT_QUOTA_RESET));
        settings.events = Vec::new();
        assert!(!event_matches_filter(&settings, NOTIFICATION_EVENT_QUOTA_RESET));
    }

    #[test]
    fn describe_event_renders_both_languages() {
        let event = recorded_event(NOTIFICATION_EVENT_QUOTA_RESET);
        let chinese = describe_event(&AppLanguage::ZhCn, &event);
        let english = describe_event(&AppLanguage::En, &event);
        assert!(chinese.contains("额度已重置"));
        assert!(chinese.contains("95%"));
        assert!(english.contains("quota reset"));
        assert!(english.contains("95% remaining"));

        let update = recorded_event(NOTIFICATION_EVENT_APP_UPDATE_APPLIED);
        assert!(describe_event(&AppLanguage::ZhCn, &update).contains("1.6.0"));
    }

    #[test]
    fn webhook_skips_without_url_and_rejects_non_http_schemes() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        let mut settings = default_notification_settings();
        settings.webhook_url = None;
        let outcome = send_webhook(&path, &settings, None, &[recorded_event(NOTIFICATION_EVENT_QUOTA_RESET)]);
        assert_eq!(outcome.status, "skipped");

        settings.webhook_url = Some("ftp://example.com/hook".to_string());
        let outcome = send_webhook(&path, &settings, None, &[recorded_event(NOTIFICATION_EVENT_QUOTA_RESET)]);
        assert_eq!(outcome.status, "failed");
        assert!(outcome.detail.unwrap().contains("http"));
    }

    #[test]
    fn webhook_posts_the_event_batch_as_json() {
        let server = tiny_http::Server::http("127.0.0.1:0").expect("test server");
        let addr = server.server_addr();
        let url = format!("http://{addr}/hook");

        let handler = thread::spawn(move || {
            let mut request = server.recv().expect("request");
            let mut body = String::new();
            request.as_reader().read_to_string(&mut body).expect("body");
            let content_type = request
                .headers()
                .iter()
                .find(|header| header.field.equiv("Content-Type"))
                .map(|header| header.value.as_str().to_string());
            let response = tiny_http::Response::from_string("ok").with_status_code(200);
            let _ = request.respond(response);
            (body, content_type)
        });

        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        let mut settings = default_notification_settings();
        settings.webhook_url = Some(url);
        let events = vec![recorded_event(NOTIFICATION_EVENT_QUOTA_RESET)];
        let outcome = send_webhook(&path, &settings, None, &events);

        assert_eq!(outcome.status, "sent");
        assert_eq!(outcome.status_code, Some(200));
        let (body, content_type) = handler.join().expect("handler");
        assert!(content_type.unwrap().starts_with("application/json"));
        let payload: serde_json::Value = serde_json::from_str(&body).expect("payload json");
        assert_eq!(payload["app"], serde_json::json!("QuotaBarWin"));
        assert_eq!(payload["schemaVersion"], serde_json::json!(1));
        assert_eq!(payload["events"][0]["eventType"], serde_json::json!("quota-reset"));
        assert_eq!(payload["events"][0]["providerName"], serde_json::json!("Remote A"));
    }

    #[test]
    fn webhook_resolves_secret_placeholders_in_the_url() {
        let server = tiny_http::Server::http("127.0.0.1:0").expect("test server");
        let addr = server.server_addr();
        let direct_url = format!("http://{addr}/hook");

        let handler = thread::spawn(move || {
            let request = server.recv().expect("request");
            let _ = request.respond(tiny_http::Response::from_string("ok").with_status_code(200));
        });

        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        std::fs::write(temp.path().join("webhook-url.txt"), &direct_url).expect("write url file");
        let mut settings = default_notification_settings();
        settings.webhook_url = Some(format!(
            "${{file:{}}}",
            temp.path().join("webhook-url.txt").display()
        ));

        let outcome = send_webhook(&path, &settings, None, &[recorded_event(NOTIFICATION_EVENT_QUOTA_RESET)]);
        assert_eq!(outcome.status, "sent");
        handler.join().expect("handler");
    }

    #[test]
    fn test_notification_event_is_describable() {
        let event = test_event();
        assert!(describe_event(&AppLanguage::En, &event).contains("Test notification"));
    }

    #[test]
    fn app_event_constructors_use_expected_types() {
        let started = pending_app_started(false);
        assert_eq!(started.event_type, NOTIFICATION_EVENT_APP_STARTED);
        let started_details = started.details.unwrap();
        assert_eq!(started_details["startedHidden"], serde_json::json!(false));
        // The recorded startup version embeds the short commit when built from
        // git; at minimum it carries the crate version.
        assert!(started_details["version"]
            .as_str()
            .is_some_and(|version| version.starts_with(env!("CARGO_PKG_VERSION"))));
        let updated = pending_app_update_applied("1.6.0");
        assert_eq!(updated.event_type, NOTIFICATION_EVENT_APP_UPDATE_APPLIED);
        assert_eq!(
            updated.details.as_ref().unwrap()["version"],
            serde_json::json!("1.6.0")
        );
    }
}
