use std::{path::Path, thread};

use serde::Serialize;
use tauri::AppHandle;

use crate::config::{
    config_path_for_app, load_or_create_config, resolve_secret_value, AppLanguage,
    NotificationSettings, WebhookEndpointSettings, NOTIFICATION_EVENT_APP_STARTED,
    NOTIFICATION_EVENT_APP_UPDATE_APPLIED, NOTIFICATION_EVENT_PROVIDER_ERROR,
    NOTIFICATION_EVENT_PROVIDER_RECOVERED, NOTIFICATION_EVENT_QUOTA_EXHAUSTED,
    NOTIFICATION_EVENT_QUOTA_LOW, NOTIFICATION_EVENT_QUOTA_RECOVERED_UNEXPECTED,
    NOTIFICATION_EVENT_QUOTA_RESET, NOTIFICATION_EVENT_QUOTA_RESET_TIME_CHANGED,
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
    // One entry per configured webhook endpoint, in configuration order, so
    // the test report shows exactly which target succeeded or failed.
    pub webhooks: Vec<WebhookEndpointTestOutcome>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WebhookEndpointTestOutcome {
    pub id: String,
    // Configured name, or "#<index>" when unnamed; shown to the user.
    pub label: String,
    // "sent" | "skipped" | "failed" — same vocabulary as ChannelOutcome.
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
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

/// True when the event's provider participates in notifications. Events with
/// no provider (app-level events, the test notification) always pass; the
/// local event history is unaffected by this opt-out.
pub fn event_provider_notifies(
    event: &QuotaEvent,
    muted_provider_ids: &std::collections::HashSet<String>,
) -> bool {
    event
        .provider_id
        .as_deref()
        .map_or(true, |id| !muted_provider_ids.contains(id))
}

/// The endpoints delivery fans out to: enabled and carrying a URL. Delivery
/// itself inlines the same predicate (it also needs each endpoint index),
/// so this lives as the tested contract of that filter.
#[cfg_attr(not(test), allow(dead_code))]
pub fn enabled_webhook_endpoints(
    settings: &NotificationSettings,
) -> Vec<&WebhookEndpointSettings> {
    settings
        .webhooks
        .iter()
        .filter(|endpoint| endpoint.enabled && !endpoint.url.trim().is_empty())
        .collect()
}

/// Display name for one endpoint in logs and test results.
fn webhook_endpoint_label(endpoint: &WebhookEndpointSettings, index: usize) -> String {
    endpoint
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("#{}", index + 1))
}

/// Spawns delivery for events recorded by a refresh. Delivery must never block
/// or fail the refresh itself, so everything happens on a worker thread and
/// failures only reach the structured log. `muted_provider_ids` carries the
/// providers that opted out of notifications; their events are dropped here
/// and never reach toast or webhook delivery.
pub fn dispatch_events(
    config_path: &Path,
    settings: NotificationSettings,
    muted_provider_ids: std::collections::HashSet<String>,
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
                .filter(|event| event_provider_notifies(event, &muted_provider_ids))
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
                for (index, endpoint) in settings.webhooks.iter().enumerate() {
                    if !endpoint.enabled || endpoint.url.trim().is_empty() {
                        continue;
                    }
                    // One endpoint failing never blocks the others.
                    let outcome = send_webhook(
                        &config_path,
                        endpoint,
                        global_proxy.as_ref(),
                        &language,
                        &selected,
                    );
                    if outcome.status == "failed" {
                        log_notification_failure(
                            &config_path,
                            &format!("webhook[{}]", webhook_endpoint_label(endpoint, index)),
                            outcome.detail.as_deref().unwrap_or("unknown error"),
                        );
                    }
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

/// Renders a user-configured webhook body template for one event. Unknown
/// placeholders are left untouched so templates stay predictable; values are
/// event data only (no secret resolution happens in the body).
pub fn render_webhook_template(template: &str, event: &QuotaEvent, language: &AppLanguage) -> String {
    template
        .replace("{{message}}", &describe_event(language, event))
        .replace("{{eventType}}", &event.event_type)
        .replace("{{severity}}", &event.severity)
        .replace("{{providerId}}", event.provider_id.as_deref().unwrap_or(""))
        .replace("{{providerName}}", event.provider_name.as_deref().unwrap_or(""))
        .replace("{{windowId}}", event.window_id.as_deref().unwrap_or(""))
        .replace("{{windowLabel}}", event.window_label.as_deref().unwrap_or(""))
        .replace("{{occurredAt}}", &event.occurred_at)
        .replace("{{app}}", "QuotaBarWin")
        .replace(
            "{{appVersion}}",
            &crate::app_info::app_display_version(),
        )
        .replace(
            "{{eventJson}}",
            &serde_json::to_string(event).unwrap_or_else(|_| "{}".to_string()),
        )
}

pub fn send_webhook(
    config_path: &Path,
    endpoint: &WebhookEndpointSettings,
    global_proxy: Option<&ProxyConfig>,
    language: &AppLanguage,
    events: &[QuotaEvent],
) -> ChannelOutcome {
    if events.is_empty() {
        return ChannelOutcome::skipped("no events");
    }
    let raw_url = endpoint.url.trim();
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

    let timeout_seconds = endpoint
        .timeout_seconds
        .clamp(WEBHOOK_MIN_TIMEOUT_SECONDS, WEBHOOK_MAX_TIMEOUT_SECONDS);
    let client = match proxy::build_http_client(
        None,
        global_proxy,
        std::time::Duration::from_secs(timeout_seconds),
    ) {
        Ok(client) => client,
        Err(error) => return ChannelOutcome::failed(crate::redact::redact_sensitive(&error)),
    };

    let template = endpoint
        .template
        .as_deref()
        .map(str::trim)
        .filter(|template| !template.is_empty());
    match template {
        // Without a template the delivery stays the documented JSON batch.
        None => {
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
        // A template describes a single event, so every selected event is
        // rendered and delivered as its own request. The rendered body is
        // sent as JSON when it parses, plain text otherwise.
        Some(template) => {
            let mut delivered = 0usize;
            let mut last_status_code = None;
            let mut failures: Vec<String> = Vec::new();
            for event in events {
                let body = render_webhook_template(template, event, language);
                let content_type = if serde_json::from_str::<serde_json::Value>(&body).is_ok() {
                    "application/json"
                } else {
                    "text/plain; charset=utf-8"
                };
                match client
                    .post(&resolved_url)
                    .header("Content-Type", content_type)
                    .body(body)
                    .send()
                {
                    Ok(response) if response.status().is_success() => {
                        delivered += 1;
                        last_status_code = Some(response.status().as_u16());
                    }
                    Ok(response) => {
                        failures.push(format!("webhook returned HTTP {}", response.status().as_u16()))
                    }
                    Err(error) => {
                        failures.push(crate::redact::redact_sensitive(&error.to_string()))
                    }
                }
            }
            if failures.is_empty() {
                ChannelOutcome::sent(last_status_code)
            } else {
                let detail = if delivered == 0 {
                    failures.join("; ")
                } else {
                    format!(
                        "{}/{} deliveries failed: {}",
                        failures.len(),
                        events.len(),
                        failures.join("; ")
                    )
                };
                ChannelOutcome::failed(detail)
            }
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

fn format_local_time(rfc3339: &str) -> Option<String> {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .ok()
        .map(|time| time.with_timezone(&chrono::Local).format("%m-%d %H:%M").to_string())
}

fn local_event_time_suffix(event: &QuotaEvent) -> String {
    format_local_time(&event.occurred_at)
        .map(|time| format!(" · {time}"))
        .unwrap_or_default()
}

/// The localized, single-line description of an event, ending with the local
/// time the event occurred so toast and webhook texts carry a concrete time.
pub fn describe_event(language: &AppLanguage, event: &QuotaEvent) -> String {
    format!(
        "{}{}",
        describe_event_text(language, event),
        local_event_time_suffix(event)
    )
}

fn describe_event_text(language: &AppLanguage, event: &QuotaEvent) -> String {
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
        NOTIFICATION_EVENT_QUOTA_RESET_TIME_CHANGED => {
            let next = detail_string(&details, "resetAt")
                .as_deref()
                .and_then(format_local_time);
            match (chinese, next) {
                (true, Some(next)) => format!("{subject} 额度到期时间变更（下次 {next}）"),
                (true, None) => format!("{subject} 额度到期时间变更"),
                (false, Some(next)) => format!("{subject} quota expiry time changed (next {next})"),
                (false, None) => format!("{subject} quota expiry time changed"),
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
    use crate::app_identity::{active_app_display_name, active_app_user_model_id};
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
        // The active identity follows the running build: a dev exe toasts as
        // "QuotaBarWin Dev" under its own AppUserModelID so its notifications
        // group separately from the release app.
        let app_user_model_id = active_app_user_model_id();
        let display_name = active_app_display_name();
        // The icon is only registered for the notification center's app
        // header (IconUri); the toast body itself stays text-only.
        let icon_path = ensure_toast_icon_file();
        ensure_app_identity_registration(icon_path.as_deref(), app_user_model_id, display_name)?;
        let chinese = language_is_chinese(language);
        let first = describe_event(language, &events[0]);
        let mut toast = winrt_notification::Toast::new(app_user_model_id).title(display_name);
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

// The HKCU subkey holding one build's AppUserModelID registration. Null-
// terminated UTF-16 as the registry APIs expect.
#[cfg(windows)]
fn app_user_model_id_registry_subkey(app_user_model_id: &str) -> Vec<u16> {
    "Software\\Classes\\AppUserModelId\\"
        .encode_utf16()
        .chain(app_user_model_id.encode_utf16())
        .chain([0])
        .collect()
}

// A WinRT toast is only displayed when its AppUserModelID is known to the
// shell. Bundled installers create a Start Menu shortcut for this; the
// portable build instead registers the AppUserModelID key in HKCU (the same
// mechanism Firefox/Chrome and PowerShell's BurntToast use), including an
// IconUri pointing at the extracted brand icon so toasts are branded. The dev
// build registers under its own AppUserModelID with its own display name, so
// its toasts never merge into the release app's group.
#[cfg(windows)]
fn ensure_app_identity_registration(
    icon_path: Option<&Path>,
    app_user_model_id: &str,
    display_name: &str,
) -> Result<(), String> {
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

    let subkey = app_user_model_id_registry_subkey(app_user_model_id);

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
        set_registry_value(hkey, "DisplayName", display_name),
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

    let mut webhooks = Vec::new();
    if settings.webhook_enabled {
        for (index, endpoint) in settings.webhooks.iter().enumerate() {
            let label = webhook_endpoint_label(endpoint, index);
            let outcome = if endpoint.enabled {
                send_webhook(&path, endpoint, global_proxy.as_ref(), &language, &[event.clone()])
            } else {
                ChannelOutcome::skipped("webhook endpoint is disabled")
            };
            webhooks.push(WebhookEndpointTestOutcome {
                id: endpoint.id.clone(),
                label,
                status: outcome.status,
                detail: outcome.detail,
                status_code: outcome.status_code,
            });
        }
    }

    Ok(TestNotificationResult { toast, webhooks })
}

// Counterpart of ensure_app_identity_registration: removes the HKCU
// AppUserModelID key and the extracted toast icon so a user who turns Windows
// notifications off leaves no registration footprint behind. A missing key
// counts as success; this never needs to fail the surrounding config save.
#[cfg(windows)]
pub fn remove_toast_registration() -> Result<(), String> {
    use crate::app_identity::active_app_user_model_id;
    use windows_sys::Win32::System::Registry::{RegDeleteTreeW, HKEY_CURRENT_USER};

    let subkey = app_user_model_id_registry_subkey(active_app_user_model_id());
    const ERROR_FILE_NOT_FOUND: u32 = 2;
    let result = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, subkey.as_ptr()) };
    // The extracted icon is part of the same registration footprint.
    if let Some(icon_path) = toast_icon_path() {
        let _ = std::fs::remove_file(icon_path);
    }
    match result {
        0 | ERROR_FILE_NOT_FOUND => Ok(()),
        code => Err(format!("RegDeleteTreeW failed: error {code}")),
    }
}

// Nothing is registered outside Windows, so removal trivially succeeds.
#[cfg(not(windows))]
pub fn remove_toast_registration() -> Result<(), String> {
    Ok(())
}

/// True when saving transitions Windows toast notifications from enabled to
/// disabled and the local toast registration should therefore be removed.
/// `None` (config predating the notifications field) counts as disabled.
pub fn should_remove_toast_registration(
    previous: Option<&NotificationSettings>,
    next: &NotificationSettings,
) -> bool {
    previous.is_some_and(|settings| settings.toast_enabled) && !next.toast_enabled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{default_notification_settings, DEFAULT_WEBHOOK_TIMEOUT_SECONDS};
    use crate::quota_events::{
        pending_app_started, pending_app_update_applied, SEVERITY_POSITIVE,
    };
    use std::io::Read;

    fn endpoint(url: &str) -> WebhookEndpointSettings {
        WebhookEndpointSettings {
            id: "webhook-1".to_string(),
            name: None,
            url: url.to_string(),
            timeout_seconds: DEFAULT_WEBHOOK_TIMEOUT_SECONDS,
            template: None,
            enabled: true,
        }
    }

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
    fn provider_opt_out_mutes_only_that_providers_events() {
        let muted = std::collections::HashSet::from(["remote-time".to_string()]);
        let mut muted_event = recorded_event(NOTIFICATION_EVENT_QUOTA_RESET);
        muted_event.provider_id = Some("remote-time".to_string());
        assert!(!event_provider_notifies(&muted_event, &muted));

        let mut other_event = recorded_event(NOTIFICATION_EVENT_QUOTA_RESET);
        other_event.provider_id = Some("remote-kimi".to_string());
        assert!(event_provider_notifies(&other_event, &muted));

        // App-level events (startup, applied update, test notifications) have
        // no provider and always pass.
        let mut app_event = recorded_event(NOTIFICATION_EVENT_APP_STARTED);
        app_event.provider_id = None;
        assert!(event_provider_notifies(&app_event, &muted));
        assert!(event_provider_notifies(&app_event, &std::collections::HashSet::new()));
    }

    #[cfg(windows)]
    #[test]
    fn app_user_model_id_registry_subkey_separates_dev_from_release() {
        let release = app_user_model_id_registry_subkey(crate::app_identity::APP_USER_MODEL_ID);
        let dev = app_user_model_id_registry_subkey(crate::app_identity::DEV_APP_USER_MODEL_ID);
        assert_ne!(release, dev, "dev and release must not share a registration key");
        let as_path = |value: &[u16]| String::from_utf16_lossy(&value[..value.len() - 1]);
        assert_eq!(
            as_path(&release),
            r"Software\Classes\AppUserModelId\com.quotabarwin.app"
        );
        assert_eq!(
            as_path(&dev),
            r"Software\Classes\AppUserModelId\com.quotabarwin.app.dev"
        );
    }

    #[test]
    fn toast_registration_removal_is_only_required_when_toast_turns_off() {
        let mut enabled = default_notification_settings();
        enabled.toast_enabled = true;
        let mut disabled = default_notification_settings();
        disabled.toast_enabled = false;

        assert!(should_remove_toast_registration(Some(&enabled), &disabled));
        assert!(!should_remove_toast_registration(Some(&disabled), &disabled));
        assert!(!should_remove_toast_registration(Some(&enabled), &enabled));
        assert!(!should_remove_toast_registration(Some(&disabled), &enabled));
        // Configs saved before the notifications field existed count as
        // disabled, so they never require cleanup.
        assert!(!should_remove_toast_registration(None, &disabled));
        assert!(!should_remove_toast_registration(None, &enabled));
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
    fn describe_event_appends_the_local_event_time() {
        let event = recorded_event(NOTIFICATION_EVENT_QUOTA_RESET);
        let described = describe_event(&AppLanguage::ZhCn, &event);
        // The concrete local time is machine-dependent; assert the MM-DD HH:MM
        // shape rather than a fixed value.
        let time_suffix = described.rsplit(" · ").next().unwrap_or_default();
        let mut parts = time_suffix.split(' ');
        let date = parts.next().unwrap_or_default();
        let time = parts.next().unwrap_or_default();
        assert_eq!(date.split('-').count(), 2, "date part looks like MM-DD: {date}");
        assert_eq!(time.len(), 5, "time part looks like HH:MM: {time}");
        let time_digits: String = time.chars().filter(|c| *c != ':').collect();
        assert!(time_digits.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn describe_reset_time_change_event_renders_next_expiry() {
        let mut event = recorded_event(NOTIFICATION_EVENT_QUOTA_RESET_TIME_CHANGED);
        event.details = Some(serde_json::json!({
            "resetAtBefore": "2026-10-05T11:00:00Z",
            "resetAt": "2026-10-05T16:00:00Z",
            "usedPercentBefore": 0.0,
            "usedPercentAfter": 0.0
        }));

        let chinese = describe_event(&AppLanguage::ZhCn, &event);
        assert!(chinese.contains("额度到期时间变更（下次 "), "zh: {chinese}");

        let english = describe_event(&AppLanguage::En, &event);
        assert!(english.contains("quota expiry time changed (next "), "en: {english}");

        // Without a parseable next reset time the text still describes the
        // change; the vanished-boundary variant records resetAt as null.
        event.details = Some(serde_json::json!({ "resetAt": null }));
        assert!(describe_event(&AppLanguage::ZhCn, &event).contains("额度到期时间变更"));
        assert!(describe_event(&AppLanguage::En, &event).contains("quota expiry time changed"));
    }

    #[test]
    fn render_webhook_template_replaces_known_placeholders() {
        let event = recorded_event(NOTIFICATION_EVENT_QUOTA_RESET);
        let rendered = render_webhook_template(
            "{{message}} | {{eventType}} | {{providerName}} | {{windowLabel}} | {{occurredAt}} | {{unknown}}",
            &event,
            &AppLanguage::ZhCn,
        );
        assert!(rendered.contains("额度已重置"));
        assert!(rendered.contains("quota-reset"));
        assert!(rendered.contains("Remote A"));
        assert!(rendered.contains("5h window"));
        assert!(rendered.contains("2026-10-05T12:00:00Z"));
        assert!(rendered.contains("{{unknown}}"), "unknown placeholders stay untouched");
    }

    #[test]
    fn webhook_template_posts_one_rendered_request_per_event() {
        let server = tiny_http::Server::http("127.0.0.1:0").expect("test server");
        let addr = server.server_addr();
        let url = format!("http://{addr}/hook");

        let handler = thread::spawn(move || {
            let mut received = Vec::new();
            for _ in 0..2 {
                let mut request = server.recv().expect("request");
                let mut body = String::new();
                request.as_reader().read_to_string(&mut body).expect("body");
                let content_type = request
                    .headers()
                    .iter()
                    .find(|header| header.field.equiv("Content-Type"))
                    .map(|header| header.value.as_str().to_string());
                let _ = request.respond(tiny_http::Response::from_string("ok").with_status_code(200));
                received.push((body, content_type));
            }
            received
        });

        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        let endpoint = WebhookEndpointSettings {
            template: Some("{\"text\":\"{{message}}\"}".to_string()),
            ..endpoint(&url)
        };
        let events = vec![
            recorded_event(NOTIFICATION_EVENT_QUOTA_RESET),
            recorded_event(NOTIFICATION_EVENT_QUOTA_LOW),
        ];
        let outcome = send_webhook(&path, &endpoint, None, &AppLanguage::En, &events);

        assert_eq!(outcome.status, "sent");
        let received = handler.join().expect("handler");
        assert_eq!(received.len(), 2, "one request per event");
        for (body, content_type) in received {
            assert!(content_type.unwrap().starts_with("application/json"));
            let payload: serde_json::Value = serde_json::from_str(&body).expect("rendered json");
            assert!(payload["text"].as_str().unwrap().contains("quota"));
        }
    }

    #[test]
    fn webhook_template_non_json_body_is_sent_as_plain_text() {
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
            let _ = request.respond(tiny_http::Response::from_string("ok").with_status_code(200));
            (body, content_type)
        });

        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        let endpoint = WebhookEndpointSettings {
            template: Some("[QuotaBarWin] {{message}}".to_string()),
            ..endpoint(&url)
        };
        let outcome = send_webhook(
            &path,
            &endpoint,
            None,
            &AppLanguage::En,
            &[recorded_event(NOTIFICATION_EVENT_QUOTA_RESET)],
        );

        assert_eq!(outcome.status, "sent");
        let (body, content_type) = handler.join().expect("handler");
        assert!(content_type.unwrap().starts_with("text/plain"));
        assert!(body.starts_with("[QuotaBarWin] "));
    }

    #[test]
    fn enabled_webhook_endpoints_filters_disabled_and_unconfigured() {
        let mut settings = default_notification_settings();
        settings.webhooks = vec![
            WebhookEndpointSettings {
                name: Some("DingTalk".to_string()),
                url: "https://example.com/a".to_string(),
                ..endpoint("")
            },
            endpoint("   "),
            WebhookEndpointSettings {
                enabled: false,
                url: "https://example.com/b".to_string(),
                ..endpoint("")
            },
        ];

        let endpoints = enabled_webhook_endpoints(&settings);
        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].name.as_deref(), Some("DingTalk"));
    }

    #[test]
    fn webhook_endpoint_label_prefers_the_name_then_falls_back_to_index() {
        let named = WebhookEndpointSettings {
            name: Some(" 日志 ".to_string()),
            ..endpoint("")
        };
        assert_eq!(webhook_endpoint_label(&named, 3), "日志");

        assert_eq!(webhook_endpoint_label(&endpoint(""), 1), "#2");
    }

    #[test]
    fn webhook_skips_without_url_and_rejects_non_http_schemes() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config.json");
        let outcome = send_webhook(&path, &endpoint(""), None, &AppLanguage::En, &[recorded_event(NOTIFICATION_EVENT_QUOTA_RESET)]);
        assert_eq!(outcome.status, "skipped");

        let endpoint = endpoint("ftp://example.com/hook");
        let outcome = send_webhook(&path, &endpoint, None, &AppLanguage::En, &[recorded_event(NOTIFICATION_EVENT_QUOTA_RESET)]);
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
        let events = vec![recorded_event(NOTIFICATION_EVENT_QUOTA_RESET)];
        let outcome = send_webhook(&path, &endpoint(&url), None, &AppLanguage::En, &events);

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
        let endpoint = endpoint(&format!(
            "${{file:{}}}",
            temp.path().join("webhook-url.txt").display()
        ));

        let outcome = send_webhook(&path, &endpoint, None, &AppLanguage::En, &[recorded_event(NOTIFICATION_EVENT_QUOTA_RESET)]);
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
