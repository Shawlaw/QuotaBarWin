use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Duration as ChronoDuration, Local, Timelike};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use desktop_updater::{
    ApplyRequest, CheckResult, DownloadedUpdate, PortableLayout, SignedCheck, UpdateCandidate,
    UpdateConfig,
};

use crate::logger::{LogLevel, LogSink};

const APP_UPDATE_MANIFEST_URL: &str =
    "https://raw.githubusercontent.com/Shawlaw/QuotaBarWin/main/updates/stable.json";
const APP_UPDATE_SIGNATURE_URL: &str =
    "https://raw.githubusercontent.com/Shawlaw/QuotaBarWin/main/updates/stable.json.sig";
const APP_UPDATE_APP_ID: &str = "com.quotabarwin.app";
const APP_UPDATE_CHANNEL: &str = "stable";
const APP_UPDATE_HELPER_NAME: &str = "QuotaBarWin.Updater.exe";
const APP_UPDATE_MAIN_EXE_NAME: &str = "QuotaBarWin.exe";
const APP_UPDATE_STATUS_CACHE_FILE: &str = "app_update_status.quotaBarWin.json";
const APP_UPDATE_STATUS_CACHE_SCHEMA_VERSION: u8 = 2;
const AUTOMATIC_CHECK_START_HOUR: u32 = 8;
const AUTOMATIC_CHECK_RETRY_DELAY: ChronoDuration = ChronoDuration::minutes(15);
const APP_UPDATE_HELPER_COPY_PREFIX: &str = "helper-";
const APP_UPDATE_HELPER_COPY_SUFFIX: &str = ".exe";
const APP_UPDATE_HELPER_CLEANUP_RETRY_DELAY: Duration = Duration::from_millis(250);
const APP_UPDATE_HELPER_CLEANUP_MAX_RETRIES: usize = 40;
const MAIN_WINDOW_LABEL: &str = "main";
#[cfg(any(debug_assertions, feature = "update-preview"))]
const APP_UPDATE_DEMO_ENV: &str = "QBWIN_DEMO_APP_UPDATE";
#[cfg(any(debug_assertions, feature = "update-preview"))]
const APP_UPDATE_DEMO_DELAY: Duration = Duration::from_millis(700);
pub const APP_UPDATE_STATUS_CHANGED_EVENT: &str = "app-update-status-changed";
pub const OPEN_APP_UPDATE_EVENT: &str = "open-app-update";

pub struct AppUpdateState {
    inner: Arc<Mutex<AppUpdateStateInner>>,
}

#[derive(Default)]
pub struct AppUpdateNavigationState {
    request_id: AtomicU64,
}

#[tauri::command]
pub fn get_app_update_navigation_request(state: State<'_, AppUpdateNavigationState>) -> u64 {
    state.request_id.load(Ordering::SeqCst)
}

/// Stores an application-update navigation request before the main window receives focus. The
/// renderer also reads this value on focus, which covers the short interval before its event
/// listener is ready (or a previously hidden window is resuming).
pub fn begin_app_update_navigation(app: &AppHandle) -> u64 {
    app.state::<AppUpdateNavigationState>()
        .request_id
        .fetch_add(1, Ordering::SeqCst)
        + 1
}

/// Notifies a ready main-window renderer about a request already recorded by
/// [`begin_app_update_navigation`]. The stored request id remains the recovery path if this
/// transient event is missed.
pub fn emit_app_update_navigation(app: &AppHandle, request_id: u64) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit(OPEN_APP_UPDATE_EVENT, request_id);
    }
}

impl Default for AppUpdateState {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(AppUpdateStateInner::default())),
        }
    }
}

#[derive(Default)]
struct AppUpdateStateInner {
    candidate: Option<UpdateCandidate>,
    downloaded: Option<DownloadedUpdate>,
    check_in_flight: bool,
    #[cfg(any(debug_assertions, feature = "update-preview"))]
    demo_notice_emitted: bool,
    #[cfg(any(debug_assertions, feature = "update-preview"))]
    demo_info: Option<AppUpdateInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateInfo {
    pub configured: bool,
    pub current_version: String,
    pub available: bool,
    pub version: Option<String>,
    pub notes_url: Option<String>,
    pub downloaded: bool,
    pub checked_at: Option<String>,
    pub error: Option<String>,
    pub dismissed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateStatusEvent {
    pub info: AppUpdateInfo,
    pub animate: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CachedAppUpdateStatus {
    #[serde(default)]
    schema_version: u8,
    #[serde(default)]
    last_automatic_check_date: Option<String>,
    #[serde(default)]
    next_automatic_retry_at: Option<String>,
    #[serde(default)]
    checked_at: Option<String>,
    #[serde(default)]
    current_version: Option<String>,
    #[serde(default)]
    available: bool,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    notes_url: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    dismissed_version: Option<String>,
    /// Base64 of the exact manifest bytes that were signature-verified when
    /// `available` was recorded. Enables an offline candidate restore through
    /// `UpdateCandidate::from_persisted_bytes` after a restart.
    #[serde(default)]
    verified_manifest_base64: Option<String>,
    #[serde(default)]
    verified_signature_base64: Option<String>,
}

#[tauri::command]
pub async fn get_app_update_status(
    app: AppHandle,
    state: State<'_, AppUpdateState>,
) -> Result<AppUpdateInfo, String> {
    #[cfg(any(debug_assertions, feature = "update-preview"))]
    {
        let demo_mode = demo_update_preview_mode();
        log_demo_preview(&app, &format!("status requested demoMode={demo_mode:?}"));
        if let Some(mode) = demo_mode {
            let demo_state = state.inner.clone();
            if mode == DemoUpdatePreviewMode::Automatic {
                request_demo_update_preview(app.clone(), demo_state.clone());
            }
            let info = {
                let mut state = demo_state
                    .lock()
                    .map_err(|_| "Application update state is unavailable")?;
                if let Some(info) = state.demo_info.clone() {
                    info
                } else if mode == DemoUpdatePreviewMode::Manual {
                    pending_demo_update_info()
                } else {
                    let info = demo_update_info();
                    state.demo_info = Some(info.clone());
                    info
                }
            };
            log_demo_preview(
                &app,
                &format!(
                    "status returned available={} dismissed={} checkedAtPresent={}",
                    info.available,
                    info.dismissed,
                    info.checked_at.is_some()
                ),
            );
            return Ok(info);
        }
    }

    let app_for_cache = app.clone();
    let cache =
        tauri::async_runtime::spawn_blocking(move || load_status_cache_for_app(&app_for_cache))
            .await
            .map_err(redacted_error)??;
    let downloaded = state
        .inner
        .lock()
        .map_err(|_| "Application update state is unavailable")?
        .downloaded
        .is_some();
    Ok(info_from_cache(
        &cache,
        app_updates_are_configured(),
        downloaded,
    ))
}

#[tauri::command]
pub async fn check_app_update(
    app: AppHandle,
    state: State<'_, AppUpdateState>,
) -> Result<AppUpdateInfo, String> {
    // Dev builds never talk to the update feed: a downloaded release zip
    // must not overwrite a dev exe. The settings UI disables the entry
    // points; this guard covers every other caller.
    if crate::app_identity::is_dev_build(&app) {
        return Err("Application updates are unavailable in development builds".to_string());
    }

    #[cfg(any(debug_assertions, feature = "update-preview"))]
    if let Some(mode) = demo_update_preview_mode() {
        let info = match mode {
            DemoUpdatePreviewMode::Automatic => {
                request_demo_update_preview(app.clone(), state.inner.clone());
                state
                    .inner
                    .lock()
                    .map_err(|_| "Application update state is unavailable")?
                    .demo_info
                    .clone()
                    .unwrap_or_else(demo_update_info)
            }
            DemoUpdatePreviewMode::Manual => {
                let info = demo_update_info();
                state
                    .inner
                    .lock()
                    .map_err(|_| "Application update state is unavailable")?
                    .demo_info = Some(info.clone());
                info
            }
        };
        log_demo_preview(
            &app,
            &format!(
                "manual check returned available={} version={}",
                info.available,
                info.version.as_deref().unwrap_or("none")
            ),
        );
        emit_update_status(&app, &info, info.available && !info.dismissed);
        return Ok(info);
    }

    if !app_updates_are_configured() {
        let app_for_cache = app.clone();
        let cache =
            tauri::async_runtime::spawn_blocking(move || load_status_cache_for_app(&app_for_cache))
                .await
                .map_err(redacted_error)??;
        return Ok(info_from_cache(&cache, false, false));
    }

    log_app_update_event(&app, LogLevel::Info, "manual check started");
    begin_check(&state.inner)?;
    let result = run_update_check(app.clone()).await;
    match result {
        Ok(result) => {
            let app_for_cache = app.clone();
            let result_for_cache = result.clone();
            let cache = tauri::async_runtime::spawn_blocking(move || {
                persist_check_result(&app_for_cache, &result_for_cache, false)
            })
            .await
            .map_err(redacted_error)?;
            match cache {
                Ok(cache) => {
                    let downloaded =
                        finish_check_success(&state.inner, candidate_for_result(result.result))?;
                    let info = info_from_cache(&cache, true, downloaded);
                    log_app_update_event(
                        &app,
                        LogLevel::Info,
                        &format!(
                            "manual check finished available={} version={} checkedAt={}",
                            info.available,
                            info.version.as_deref().unwrap_or("none"),
                            info.checked_at.as_deref().unwrap_or("none")
                        ),
                    );
                    emit_update_status(&app, &info, info.available && !info.dismissed);
                    Ok(info)
                }
                Err(error) => {
                    // The signed check itself succeeded, so keep its candidate
                    // downloadable even though the status could not be persisted.
                    let _ = finish_check_success(&state.inner, candidate_for_result(result.result));
                    log_app_update_event(
                        &app,
                        LogLevel::Warn,
                        &format!(
                            "manual check persistence failed error={}",
                            redacted_error(&error)
                        ),
                    );
                    Err(redacted_error(error))
                }
            }
        }
        Err(error) => {
            let _ = finish_check_failure(&state.inner);
            log_app_update_event(
                &app,
                LogLevel::Warn,
                &format!("manual check failed error={}", redacted_error(&error)),
            );
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn dismiss_app_update_notice(
    app: AppHandle,
    state: State<'_, AppUpdateState>,
) -> Result<AppUpdateInfo, String> {
    #[cfg(any(debug_assertions, feature = "update-preview"))]
    if demo_update_preview_mode().is_some() {
        let info = state
            .inner
            .lock()
            .map_err(|_| "Application update state is unavailable")?
            .demo_info
            .as_mut()
            .map(|info| {
                info.dismissed = true;
                info.clone()
            })
            .unwrap_or_else(pending_demo_update_info);
        emit_update_status(&app, &info, false);
        return Ok(info);
    }

    let app_for_cache = app.clone();
    let cache = tauri::async_runtime::spawn_blocking(move || dismiss_cached_notice(&app_for_cache))
        .await
        .map_err(redacted_error)??;
    let downloaded = state
        .inner
        .lock()
        .map_err(|_| "Application update state is unavailable")?
        .downloaded
        .is_some();
    let info = info_from_cache(&cache, app_updates_are_configured(), downloaded);
    emit_update_status(&app, &info, false);
    Ok(info)
}

#[tauri::command]
pub async fn download_app_update(
    app: AppHandle,
    state: State<'_, AppUpdateState>,
) -> Result<AppUpdateInfo, String> {
    if crate::app_identity::is_dev_build(&app) {
        return Err("Application updates are unavailable in development builds".to_string());
    }
    let existing_candidate = {
        let state = state
            .inner
            .lock()
            .map_err(|_| "Application update state is unavailable")?;
        state.candidate.clone()
    };
    let candidate = match existing_candidate {
        Some(candidate) => candidate,
        None => {
            // Fresh session (for example after a restart): rebuild the
            // candidate from the persisted signed check without network I/O.
            let app_for_restore = app.clone();
            let restored = tauri::async_runtime::spawn_blocking(move || {
                restore_candidate_from_disk(&app_for_restore)
            })
            .await
            .map_err(redacted_error)??;
            let Some(candidate) = restored else {
                return Err("Check for an application update before downloading it".to_string());
            };
            log_app_update_event(
                &app,
                LogLevel::Info,
                &format!(
                    "candidate restored from persisted signed check version={}",
                    candidate.version()
                ),
            );
            if let Ok(mut inner) = state.inner.lock() {
                inner.candidate = Some(candidate.clone());
            }
            candidate
        }
    };
    let version = candidate.version().to_string();
    let (config, updates_dir) = update_context(&app)?;
    log_app_update_event(
        &app,
        LogLevel::Info,
        &format!("download started version={version}"),
    );
    let downloaded = match tauri::async_runtime::spawn_blocking(move || {
        desktop_updater::download(&config, candidate, &updates_dir, |_, _| {})
    })
    .await
    {
        Ok(Ok(downloaded)) => downloaded,
        Ok(Err(error)) => {
            let error = redacted_error(error);
            log_app_update_event(
                &app,
                LogLevel::Warn,
                &format!("download failed version={version} error={error}"),
            );
            return Err(error);
        }
        Err(error) => {
            let error = redacted_error(error);
            log_app_update_event(
                &app,
                LogLevel::Warn,
                &format!("download task failed version={version} error={error}"),
            );
            return Err(error);
        }
    };
    let package_bytes = fs::metadata(&downloaded.package_path)
        .map(|metadata| metadata.len())
        .unwrap_or_default();
    let downloaded = {
        let mut state = state
            .inner
            .lock()
            .map_err(|_| "Application update state is unavailable")?;
        state.downloaded = Some(downloaded);
        state.downloaded.is_some()
    };
    log_app_update_event(
        &app,
        LogLevel::Info,
        &format!("download finished version={version} bytes={package_bytes}"),
    );
    let app_for_cache = app.clone();
    let cache =
        tauri::async_runtime::spawn_blocking(move || load_status_cache_for_app(&app_for_cache))
            .await
            .map_err(redacted_error)??;
    Ok(info_from_cache(&cache, true, downloaded))
}

#[tauri::command]
pub async fn apply_app_update(
    app: AppHandle,
    state: State<'_, AppUpdateState>,
) -> Result<(), String> {
    if crate::app_identity::is_dev_build(&app) {
        return Err("Application updates are unavailable in development builds".to_string());
    }
    let downloaded = {
        let state = state
            .inner
            .lock()
            .map_err(|_| "Application update state is unavailable")?;
        state
            .downloaded
            .clone()
            .ok_or_else(|| "Download an application update before applying it".to_string())?
    };
    let version = downloaded.candidate.version().to_string();
    let request = update_apply_request()?;
    log_app_update_event(
        &app,
        LogLevel::Info,
        &format!("apply started version={version}"),
    );
    let pending = match tauri::async_runtime::spawn_blocking(move || {
        desktop_updater::apply_and_restart(&downloaded, &request)
    })
    .await
    {
        Ok(Ok(pending)) => pending,
        Ok(Err(error)) => {
            let error = redacted_error(error);
            log_app_update_event(
                &app,
                LogLevel::Warn,
                &format!("apply failed version={version} error={error}"),
            );
            return Err(error);
        }
        Err(error) => {
            let error = redacted_error(error);
            log_app_update_event(
                &app,
                LogLevel::Warn,
                &format!("apply task failed version={version} error={error}"),
            );
            return Err(error);
        }
    };
    log_app_update_event(
        &app,
        LogLevel::Info,
        &format!(
            "apply scheduled version={} pendingVersion={}",
            version, pending.version
        ),
    );
    app.exit(0);
    Ok(())
}

/// Starts an automatic update check if the active local day is eligible. This is intentionally
/// called from window-focus handling rather than a timer: a hidden tray application should not
/// perform update traffic until the user next opens one of its windows.
pub fn request_automatic_update_check(app: AppHandle) {
    let state = app.state::<AppUpdateState>().inner.clone();
    #[cfg(any(debug_assertions, feature = "update-preview"))]
    {
        let demo_mode = demo_update_preview_mode();
        log_demo_preview(&app, &format!("focus trigger demoMode={demo_mode:?}"));
        if let Some(mode) = demo_mode {
            if mode == DemoUpdatePreviewMode::Automatic {
                request_demo_update_preview(app, state);
            }
            return;
        }
    }

    tauri::async_runtime::spawn(async move {
        let app_for_start = app.clone();
        let state_for_start = state.clone();
        let should_check = match tauri::async_runtime::spawn_blocking(move || {
            begin_automatic_check(&app_for_start, &state_for_start)
        })
        .await
        {
            Ok(Ok(should_check)) => should_check,
            Ok(Err(error)) => {
                log_app_update_event(
                    &app,
                    LogLevel::Warn,
                    &format!(
                        "automatic check scheduling failed error={}",
                        redacted_error(error)
                    ),
                );
                false
            }
            Err(error) => {
                log_app_update_event(
                    &app,
                    LogLevel::Warn,
                    &format!(
                        "automatic check scheduling task failed error={}",
                        redacted_error(error)
                    ),
                );
                false
            }
        };

        if !should_check {
            return;
        }

        if !app_updates_are_configured() {
            let app_for_cache = app.clone();
            if let Ok(Ok(cache)) = tauri::async_runtime::spawn_blocking(move || {
                persist_automatic_unconfigured(&app_for_cache)
            })
            .await
            {
                if let Ok(downloaded) = finish_check_success(&state, None) {
                    let info = info_from_cache(&cache, false, downloaded);
                    emit_update_status(&app, &info, false);
                }
            } else {
                let _ = finish_check_failure(&state);
            }
            return;
        }

        log_app_update_event(&app, LogLevel::Info, "automatic check started");
        match run_update_check(app.clone()).await {
            Ok(result) => {
                let app_for_cache = app.clone();
                let result_for_cache = result.clone();
                let cache = tauri::async_runtime::spawn_blocking(move || {
                    persist_check_result(&app_for_cache, &result_for_cache, true)
                })
                .await;
                match cache {
                    Ok(Ok(cache)) => {
                        if let Ok(downloaded) =
                            finish_check_success(&state, candidate_for_result(result.result))
                        {
                            let info = info_from_cache(&cache, true, downloaded);
                            log_app_update_event(
                                &app,
                                LogLevel::Info,
                                &format!(
                                    "automatic check finished available={} version={} checkedAt={}",
                                    info.available,
                                    info.version.as_deref().unwrap_or("none"),
                                    info.checked_at.as_deref().unwrap_or("none")
                                ),
                            );
                            emit_update_status(&app, &info, info.available && !info.dismissed);
                        }
                    }
                    _ => {
                        // The signed check itself succeeded, so keep its
                        // candidate downloadable even though the automatic
                        // status could not be persisted.
                        let _ = finish_check_success(&state, candidate_for_result(result.result));
                        log_app_update_event(
                            &app,
                            LogLevel::Warn,
                            "automatic check result could not be persisted",
                        );
                    }
                }
            }
            Err(error) => {
                let redacted = redacted_error(&error);
                let app_for_cache = app.clone();
                let cache = tauri::async_runtime::spawn_blocking(move || {
                    persist_automatic_failure(&app_for_cache, &error)
                })
                .await;
                if let Ok(downloaded) = finish_check_failure(&state) {
                    if let Ok(Ok(cache)) = cache {
                        let info = info_from_cache(&cache, true, downloaded);
                        log_app_update_event(
                            &app,
                            LogLevel::Warn,
                            &format!(
                                "automatic check failed error={} retryNotBefore={}",
                                redacted,
                                cache.next_automatic_retry_at.as_deref().unwrap_or("none")
                            ),
                        );
                        emit_update_status(&app, &info, false);
                    } else {
                        log_app_update_event(
                            &app,
                            LogLevel::Warn,
                            &format!(
                                "automatic check failed and could not persist error={redacted}"
                            ),
                        );
                    }
                }
            }
        }
    });
}

#[cfg(any(debug_assertions, feature = "update-preview"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DemoUpdatePreviewMode {
    Automatic,
    Manual,
}

#[cfg(any(debug_assertions, feature = "update-preview"))]
fn demo_update_preview_mode() -> Option<DemoUpdatePreviewMode> {
    std::env::var(APP_UPDATE_DEMO_ENV)
        .ok()
        .as_deref()
        .and_then(demo_update_preview_mode_from_value)
}

#[cfg(any(debug_assertions, feature = "update-preview"))]
fn demo_update_preview_mode_from_value(value: &str) -> Option<DemoUpdatePreviewMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "automatic" => Some(DemoUpdatePreviewMode::Automatic),
        "manual" => Some(DemoUpdatePreviewMode::Manual),
        _ => None,
    }
}

#[cfg(any(debug_assertions, feature = "update-preview"))]
fn request_demo_update_preview(app: AppHandle, state: Arc<Mutex<AppUpdateStateInner>>) {
    let should_emit = state
        .lock()
        .map(|mut state| {
            if state.demo_notice_emitted {
                false
            } else {
                state.demo_notice_emitted = true;
                true
            }
        })
        .unwrap_or(false);
    log_demo_preview(
        &app,
        &format!("demo notice scheduling accepted={should_emit}"),
    );
    if !should_emit {
        return;
    }

    tauri::async_runtime::spawn(async move {
        let _ = tauri::async_runtime::spawn_blocking(|| std::thread::sleep(APP_UPDATE_DEMO_DELAY))
            .await;
        let info = demo_update_info();
        if let Ok(mut state) = state.lock() {
            state.demo_info = Some(info.clone());
        }
        log_demo_preview(
            &app,
            "demo notice emitted available=true version=1.2.3-demo",
        );
        emit_update_status(&app, &info, true);
    });
}

#[cfg(any(debug_assertions, feature = "update-preview"))]
fn log_demo_preview(app: &AppHandle, message: &str) {
    let Ok(path) = crate::config::config_path_for_app(app) else {
        return;
    };
    let Ok(loaded) = crate::config::load_or_create_config(&path) else {
        return;
    };
    let log = crate::logger::LogSink::from_config_path(&path, &loaded.config);
    let _ = log.write_unfiltered(crate::logger::LogLevel::Info, "app_update_preview", message);
}

#[cfg(any(debug_assertions, feature = "update-preview"))]
fn pending_demo_update_info() -> AppUpdateInfo {
    AppUpdateInfo {
        configured: true,
        current_version: current_version(),
        available: false,
        version: None,
        notes_url: None,
        downloaded: false,
        checked_at: None,
        error: None,
        dismissed: false,
    }
}

#[cfg(any(debug_assertions, feature = "update-preview"))]
fn demo_update_info() -> AppUpdateInfo {
    AppUpdateInfo {
        configured: true,
        current_version: current_version(),
        available: true,
        version: Some("1.2.3-demo".to_string()),
        notes_url: Some("https://github.com/Shawlaw/QuotaBarWin/releases".to_string()),
        downloaded: false,
        checked_at: Some(Local::now().to_rfc3339()),
        error: None,
        dismissed: false,
    }
}

pub fn acknowledge_applied_update(app: &AppHandle) -> Result<bool, String> {
    let acknowledged = desktop_updater::acknowledge_if_requested().map_err(redacted_error)?;
    if acknowledged {
        // The helper is still running when the new app acknowledges its startup, so it cannot
        // delete its own copied executable on Windows. Retry in the new process until it exits.
        if let Ok(updates_dir) = updates_dir_for_app(app) {
            thread::spawn(move || cleanup_update_helper_copies(&updates_dir));
        }
    }
    Ok(acknowledged)
}

fn app_updates_are_configured() -> bool {
    option_env!("QUOTABARWIN_UPDATE_PUBLIC_KEY")
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

fn public_key() -> Result<&'static str, String> {
    option_env!("QUOTABARWIN_UPDATE_PUBLIC_KEY")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            "Application updates are not configured in this build. The maintainer must set QUOTABARWIN_UPDATE_PUBLIC_KEY when building a release."
                .to_string()
        })
}

fn begin_check(state: &Arc<Mutex<AppUpdateStateInner>>) -> Result<(), String> {
    let mut state = state
        .lock()
        .map_err(|_| "Application update state is unavailable")?;
    if state.check_in_flight {
        return Err("An application update check is already running".to_string());
    }
    state.check_in_flight = true;
    Ok(())
}

/// Ends an in-flight check that completed. The candidate is replaced with the
/// fresh signed result, and any previous download is dropped: the next download
/// re-verifies and reuses the cached package file when it still matches.
fn finish_check_success(
    state: &Arc<Mutex<AppUpdateStateInner>>,
    candidate: Option<UpdateCandidate>,
) -> Result<bool, String> {
    let mut state = state
        .lock()
        .map_err(|_| "Application update state is unavailable")?;
    state.candidate = candidate;
    state.downloaded = None;
    state.check_in_flight = false;
    Ok(false)
}

/// Ends an in-flight check that failed. A transport failure carries no
/// information about the signed manifest, so a previously verified candidate
/// and its downloaded package must stay usable. Clearing them here used to
/// deadlock the UI: the persisted status cache kept advertising an available
/// update while every download failed with "check first" because the network
/// also kept failing the checks.
fn finish_check_failure(state: &Arc<Mutex<AppUpdateStateInner>>) -> Result<bool, String> {
    let mut state = state
        .lock()
        .map_err(|_| "Application update state is unavailable")?;
    state.check_in_flight = false;
    Ok(state.downloaded.is_some())
}

fn begin_automatic_check(
    app: &AppHandle,
    state: &Arc<Mutex<AppUpdateStateInner>>,
) -> Result<bool, String> {
    if crate::app_identity::is_dev_build(app) {
        // Dev builds must not discover or download release updates.
        return Ok(false);
    }
    let config_path = crate::config::config_path_for_app(app)?;
    let config = crate::config::load_or_create_config(&config_path)?.config;
    let now = Local::now();
    let cache = load_status_cache_from_path(&status_cache_path_for_config_path(&config_path)?)?;
    let app_version = current_version();
    let cache_matches_current_version =
        cache.current_version.as_deref() == Some(app_version.as_str());
    let last_automatic_check_date = cache_matches_current_version
        .then(|| cache.last_automatic_check_date.as_deref())
        .flatten();
    let next_automatic_retry_at = cache_matches_current_version
        .then(|| cache.next_automatic_retry_at.as_deref())
        .flatten();
    if !automatic_check_is_due(
        config.app_update.auto_check,
        now,
        last_automatic_check_date,
        next_automatic_retry_at,
    ) {
        log_app_update_event(
            app,
            LogLevel::Debug,
            &format!(
                "automatic check skipped enabled={} lastAutomaticCheckDate={} retryNotBefore={}",
                config.app_update.auto_check,
                last_automatic_check_date.unwrap_or("none"),
                next_automatic_retry_at.unwrap_or("none")
            ),
        );
        return Ok(false);
    }

    let mut state = state
        .lock()
        .map_err(|_| "Application update state is unavailable")?;
    if state.check_in_flight {
        log_app_update_event(
            app,
            LogLevel::Debug,
            "automatic check skipped inFlight=true",
        );
        return Ok(false);
    }
    state.check_in_flight = true;
    Ok(true)
}

fn automatic_check_is_due(
    auto_check_enabled: bool,
    now: DateTime<Local>,
    last_automatic_check_date: Option<&str>,
    next_automatic_retry_at: Option<&str>,
) -> bool {
    let today = now.date_naive().to_string();
    auto_check_enabled
        && now.hour() >= AUTOMATIC_CHECK_START_HOUR
        && last_automatic_check_date != Some(today.as_str())
        && automatic_retry_is_ready(now, next_automatic_retry_at)
}

fn automatic_retry_is_ready(now: DateTime<Local>, next_automatic_retry_at: Option<&str>) -> bool {
    let Some(retry_at) = next_automatic_retry_at
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Local))
    else {
        return true;
    };
    now >= retry_at
}

async fn run_update_check(app: AppHandle) -> Result<SignedCheck, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (config, _) = update_context(&app)?;
        desktop_updater::check_signed(&config).map_err(redacted_error)
    })
    .await
    .map_err(redacted_error)?
}

fn candidate_for_result(result: CheckResult) -> Option<UpdateCandidate> {
    match result {
        CheckResult::UpToDate => None,
        CheckResult::UpdateAvailable(candidate) => Some(candidate),
    }
}

fn persist_check_result(
    app: &AppHandle,
    signed: &SignedCheck,
    automatic: bool,
) -> Result<CachedAppUpdateStatus, String> {
    let mut cache = load_status_cache_for_app(app)?;
    apply_signed_check_to_cache(&mut cache, signed, automatic, Local::now());
    write_status_cache_for_app(app, &cache)?;
    Ok(cache)
}

/// Pure core of [`persist_check_result`], separated so the persistence
/// semantics stay testable without a Tauri app handle. The exact verified
/// bytes are persisted so a later session can restore the candidate offline
/// through [`restore_persisted_candidate`].
fn apply_signed_check_to_cache(
    cache: &mut CachedAppUpdateStatus,
    signed: &SignedCheck,
    automatic: bool,
    now: DateTime<Local>,
) {
    let current_version = current_version();
    if cache.current_version.as_deref() != Some(current_version.as_str()) {
        *cache = CachedAppUpdateStatus::default();
    }

    cache.schema_version = APP_UPDATE_STATUS_CACHE_SCHEMA_VERSION;
    cache.current_version = Some(current_version);
    cache.checked_at = Some(now.to_rfc3339());
    cache.error = None;
    cache.next_automatic_retry_at = None;
    match &signed.result {
        CheckResult::UpToDate => {
            cache.available = false;
            cache.version = None;
            cache.notes_url = None;
            cache.dismissed_version = None;
            cache.verified_manifest_base64 = None;
            cache.verified_signature_base64 = None;
        }
        CheckResult::UpdateAvailable(candidate) => {
            let version = candidate.version().to_string();
            if cache.version.as_deref() != Some(version.as_str()) {
                cache.dismissed_version = None;
            }
            cache.available = true;
            cache.version = Some(version);
            cache.notes_url = candidate.notes_url().map(str::to_string);
            cache.verified_manifest_base64 = Some(STANDARD.encode(&signed.manifest_bytes));
            cache.verified_signature_base64 = Some(STANDARD.encode(&signed.signature_bytes));
        }
    }
    if automatic {
        cache.last_automatic_check_date = Some(now.date_naive().to_string());
    }
}

/// Rebuilds a download candidate from the persisted signed check without
/// network access. The updater library re-verifies the detached signature
/// against the pinned public key, so a tampered cache file fails closed and
/// yields no candidate instead of redirecting the download.
fn restore_persisted_candidate(
    cache: &CachedAppUpdateStatus,
    config: &UpdateConfig,
) -> Option<UpdateCandidate> {
    if !cache.available || cache.current_version.as_deref() != Some(current_version().as_str()) {
        return None;
    }
    let manifest_bytes = decode_persisted_bytes(cache.verified_manifest_base64.as_deref()?)?;
    let signature_bytes = decode_persisted_bytes(cache.verified_signature_base64.as_deref()?)?;
    UpdateCandidate::from_persisted_bytes(config, &manifest_bytes, &signature_bytes)
        .ok()
        .flatten()
}

fn decode_persisted_bytes(value: &str) -> Option<Vec<u8>> {
    STANDARD.decode(value.trim()).ok()
}

fn restore_candidate_from_disk(app: &AppHandle) -> Result<Option<UpdateCandidate>, String> {
    let (config, _) = update_context(app)?;
    let cache = load_status_cache_for_app(app)?;
    if cache.available
        && cache.current_version.as_deref() == Some(current_version().as_str())
        && cache.verified_manifest_base64.is_some()
        && cache.verified_signature_base64.is_some()
    {
        match restore_persisted_candidate(&cache, &config) {
            Some(candidate) => Ok(Some(candidate)),
            None => {
                log_app_update_event(
                    app,
                    LogLevel::Warn,
                    "persisted update candidate failed offline verification",
                );
                Ok(None)
            }
        }
    } else {
        Ok(None)
    }
}

fn persist_automatic_failure(
    app: &AppHandle,
    error: &str,
) -> Result<CachedAppUpdateStatus, String> {
    let mut cache = load_status_cache_for_app(app)?;
    let current_version = current_version();
    if cache.current_version.as_deref() != Some(current_version.as_str()) {
        cache = CachedAppUpdateStatus::default();
    }
    cache.schema_version = APP_UPDATE_STATUS_CACHE_SCHEMA_VERSION;
    cache.current_version = Some(current_version);
    let now = Local::now();
    cache.checked_at = Some(now.to_rfc3339());
    cache.error = Some(redacted_error(error));
    cache.next_automatic_retry_at = Some((now + AUTOMATIC_CHECK_RETRY_DELAY).to_rfc3339());
    write_status_cache_for_app(app, &cache)?;
    Ok(cache)
}

fn persist_automatic_unconfigured(app: &AppHandle) -> Result<CachedAppUpdateStatus, String> {
    let cache = CachedAppUpdateStatus {
        schema_version: APP_UPDATE_STATUS_CACHE_SCHEMA_VERSION,
        last_automatic_check_date: Some(Local::now().date_naive().to_string()),
        next_automatic_retry_at: None,
        checked_at: Some(Local::now().to_rfc3339()),
        current_version: Some(current_version()),
        ..Default::default()
    };
    write_status_cache_for_app(app, &cache)?;
    Ok(cache)
}

fn dismiss_cached_notice(app: &AppHandle) -> Result<CachedAppUpdateStatus, String> {
    let mut cache = load_status_cache_for_app(app)?;
    if cache.current_version.as_deref() == Some(current_version().as_str()) && cache.available {
        cache.dismissed_version = cache.version.clone();
        write_status_cache_for_app(app, &cache)?;
    }
    Ok(cache)
}

fn load_status_cache_for_app(app: &AppHandle) -> Result<CachedAppUpdateStatus, String> {
    let config_path = crate::config::config_path_for_app(app)?;
    load_status_cache_from_path(&status_cache_path_for_config_path(&config_path)?)
}

fn write_status_cache_for_app(
    app: &AppHandle,
    cache: &CachedAppUpdateStatus,
) -> Result<(), String> {
    let config_path = crate::config::config_path_for_app(app)?;
    write_status_cache_to_path(&status_cache_path_for_config_path(&config_path)?, cache)
}

fn status_cache_path_for_config_path(config_path: &Path) -> Result<PathBuf, String> {
    config_path
        .parent()
        .map(|parent| parent.join(APP_UPDATE_STATUS_CACHE_FILE))
        .ok_or_else(|| "Unable to resolve application update status cache directory".to_string())
}

fn load_status_cache_from_path(path: &Path) -> Result<CachedAppUpdateStatus, String> {
    match fs::read(path) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes).unwrap_or_default()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(CachedAppUpdateStatus::default())
        }
        Err(error) => Err(error.to_string()),
    }
}

fn write_status_cache_to_path(path: &Path, cache: &CachedAppUpdateStatus) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(cache).map_err(|error| error.to_string())?;
    fs::write(path, bytes).map_err(|error| error.to_string())
}

fn info_from_cache(
    cache: &CachedAppUpdateStatus,
    configured: bool,
    downloaded: bool,
) -> AppUpdateInfo {
    let is_current_version = cache.current_version.as_deref() == Some(current_version().as_str());
    let available = configured && is_current_version && cache.available;
    let version = available.then(|| cache.version.clone()).flatten();
    let dismissed = available && cache.dismissed_version == version;
    AppUpdateInfo {
        configured,
        current_version: current_version(),
        available,
        version,
        notes_url: available.then(|| cache.notes_url.clone()).flatten(),
        downloaded,
        checked_at: is_current_version
            .then(|| cache.checked_at.clone())
            .flatten(),
        error: is_current_version.then(|| cache.error.clone()).flatten(),
        dismissed,
    }
}

fn current_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn log_app_update_event(app: &AppHandle, level: LogLevel, message: &str) {
    let Ok(path) = crate::config::config_path_for_app(app) else {
        return;
    };
    let Ok(loaded) = crate::config::load_or_create_config(&path) else {
        return;
    };
    let log = LogSink::from_config_path(&path, &loaded.config);
    let _ = log.write(level, "app_update", message);
}

fn emit_update_status(app: &AppHandle, info: &AppUpdateInfo, animate: bool) {
    let status = AppUpdateStatusEvent {
        info: info.clone(),
        animate,
    };
    let targets = [MAIN_WINDOW_LABEL, crate::tray::TRAY_POPUP_LABEL];

    for target in targets {
        if app.get_webview_window(target).is_none() {
            continue;
        }
        if let Err(error) = app.emit_to(target, APP_UPDATE_STATUS_CHANGED_EVENT, status.clone()) {
            log_app_update_event(
                app,
                LogLevel::Warn,
                &format!(
                    "status event emission failed target={target} error={}",
                    redacted_error(error)
                ),
            );
        }
    }
}

fn redacted_error(error: impl ToString) -> String {
    crate::redact::redact_sensitive(&error.to_string())
}

fn update_context(app: &AppHandle) -> Result<(UpdateConfig, PathBuf), String> {
    let public_key = public_key()?;
    let config_path = crate::config::config_path_for_app(app)?;
    let config = crate::config::load_or_create_config(&config_path)?.config;
    let mut update_config = UpdateConfig::new(
        APP_UPDATE_APP_ID,
        APP_UPDATE_CHANNEL,
        env!("CARGO_PKG_VERSION"),
        APP_UPDATE_MANIFEST_URL,
        APP_UPDATE_SIGNATURE_URL,
        public_key,
    );
    update_config.proxy_url = crate::proxy::select_proxy_url(None, config.network_proxy.as_ref());
    let updates_dir = updates_dir_for_config_path(&config_path)?;
    Ok((update_config, updates_dir))
}

fn updates_dir_for_app(app: &AppHandle) -> Result<PathBuf, String> {
    let config_path = crate::config::config_path_for_app(app)?;
    updates_dir_for_config_path(&config_path)
}

fn updates_dir_for_config_path(config_path: &Path) -> Result<PathBuf, String> {
    Ok(config_path
        .parent()
        .ok_or_else(|| "Unable to resolve application update directory".to_string())?
        .join("updates"))
}

fn cleanup_update_helper_copies(updates_dir: &Path) {
    for attempt in 0..=APP_UPDATE_HELPER_CLEANUP_MAX_RETRIES {
        match cleanup_update_helper_copies_once(updates_dir) {
            Ok(false) | Err(_) => return,
            Ok(true) if attempt == APP_UPDATE_HELPER_CLEANUP_MAX_RETRIES => return,
            Ok(true) => thread::sleep(APP_UPDATE_HELPER_CLEANUP_RETRY_DELAY),
        }
    }
}

/// Removes only helper executables copied by `desktop-updater` into the active update directory.
/// Returns whether a currently locked helper should be retried after it exits.
fn cleanup_update_helper_copies_once(updates_dir: &Path) -> std::io::Result<bool> {
    let entries = match fs::read_dir(updates_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    let mut retry = false;
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if !file_name.starts_with(APP_UPDATE_HELPER_COPY_PREFIX)
            || !file_name.ends_with(APP_UPDATE_HELPER_COPY_SUFFIX)
        {
            continue;
        }
        match fs::remove_file(entry.path()) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => retry = true,
            Err(_) => {}
        }
    }
    Ok(retry)
}

fn update_apply_request() -> Result<ApplyRequest, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let install_dir = executable
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| "Unable to resolve application installation directory".to_string())?;
    Ok(ApplyRequest {
        helper_path: install_dir.join(APP_UPDATE_HELPER_NAME),
        restart_executable: install_dir.join(APP_UPDATE_MAIN_EXE_NAME),
        install_dir,
        layout: PortableLayout::flat([
            APP_UPDATE_MAIN_EXE_NAME,
            "QuotaBarWin.Cli.exe",
            APP_UPDATE_HELPER_NAME,
        ])
        .with_preserved_files(["quotabarwin.portable"]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_check_only_runs_once_after_eight_am() {
        let now = Local::now();
        let after_eight = now
            .with_hour(8)
            .and_then(|value| value.with_minute(0))
            .and_then(|value| value.with_second(0))
            .expect("a local time at 08:00 should exist");
        let before_eight = after_eight - ChronoDuration::hours(1);
        let today = after_eight.date_naive().to_string();

        assert!(!automatic_check_is_due(true, before_eight, None, None));
        assert!(automatic_check_is_due(true, after_eight, None, None));
        assert!(!automatic_check_is_due(
            true,
            after_eight,
            Some(today.as_str()),
            None
        ));
        assert!(!automatic_check_is_due(false, after_eight, None, None));
    }

    #[test]
    fn automatic_check_retries_after_the_retry_delay_when_a_previous_attempt_failed() {
        let now = Local::now();
        let after_eight = now
            .with_hour(8)
            .and_then(|value| value.with_minute(0))
            .and_then(|value| value.with_second(0))
            .expect("a local time at 08:00 should exist");
        let retry_at = (after_eight + AUTOMATIC_CHECK_RETRY_DELAY).to_rfc3339();

        assert!(!automatic_check_is_due(
            true,
            after_eight,
            None,
            Some(retry_at.as_str())
        ));
        assert!(automatic_check_is_due(
            true,
            after_eight + AUTOMATIC_CHECK_RETRY_DELAY,
            None,
            Some(retry_at.as_str())
        ));
    }

    #[test]
    fn corrupt_status_cache_is_ignored() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join(APP_UPDATE_STATUS_CACHE_FILE);
        fs::write(&path, b"not json").expect("write corrupt cache");
        assert_eq!(
            load_status_cache_from_path(&path)
                .expect("read status cache")
                .schema_version,
            0
        );
    }

    fn update_candidate_fixture(version: &str) -> UpdateCandidate {
        UpdateCandidate {
            manifest: desktop_updater::UpdateManifest {
                schema_version: 1,
                app_id: APP_UPDATE_APP_ID.to_string(),
                channel: APP_UPDATE_CHANNEL.to_string(),
                version: version.to_string(),
                published_at: "2026-09-20T00:00:00Z".to_string(),
                target: "windows-x64".to_string(),
                asset: desktop_updater::UpdateAsset {
                    url: "https://example.test/package.zip".to_string(),
                    sha256: "a".repeat(64),
                    size: 123,
                },
                notes_url: None,
            },
        }
    }

    fn downloaded_fixture(version: &str) -> DownloadedUpdate {
        DownloadedUpdate {
            candidate: update_candidate_fixture(version),
            package_path: PathBuf::from("updates").join(format!("{version}.zip")),
        }
    }

    /// Builds a real Ed25519-signed manifest/signature pair plus the matching
    /// verifier config, so persistence and offline-restore tests exercise the
    /// actual signature path.
    fn signed_bytes_fixture(manifest_version: &str) -> (UpdateConfig, Vec<u8>, Vec<u8>) {
        use ed25519_dalek::{Signer, SigningKey};
        let signing_key = SigningKey::from_bytes(&[7; 32]);
        let public_key = STANDARD.encode(signing_key.verifying_key().as_bytes());
        let config = UpdateConfig::new(
            APP_UPDATE_APP_ID,
            APP_UPDATE_CHANNEL,
            current_version(),
            "https://example.test/stable.json",
            "https://example.test/stable.json.sig",
            public_key,
        );
        let manifest_bytes =
            serde_json::to_vec(&update_candidate_fixture(manifest_version).manifest)
                .expect("manifest JSON");
        let signature_bytes = format!(
            "{}\n",
            STANDARD.encode(signing_key.sign(&manifest_bytes).to_bytes())
        )
        .into_bytes();
        (config, manifest_bytes, signature_bytes)
    }

    fn signed_check_fixture(manifest_version: &str) -> (UpdateConfig, SignedCheck) {
        let (config, manifest_bytes, signature_bytes) = signed_bytes_fixture(manifest_version);
        let candidate =
            UpdateCandidate::from_persisted_bytes(&config, &manifest_bytes, &signature_bytes)
                .expect("offline verification")
                .unwrap_or_else(|| {
                    panic!(
                        "{manifest_version} must be newer than {}",
                        current_version()
                    )
                });
        (
            config,
            SignedCheck {
                result: CheckResult::UpdateAvailable(candidate),
                manifest_bytes,
                signature_bytes,
            },
        )
    }

    #[test]
    fn persisted_signed_check_round_trips_into_a_restorable_candidate() {
        let (config, signed) = signed_check_fixture("9.9.9");
        let mut cache = CachedAppUpdateStatus::default();
        apply_signed_check_to_cache(&mut cache, &signed, false, Local::now());

        assert!(cache.available);
        assert_eq!(cache.version.as_deref(), Some("9.9.9"));

        let restored = restore_persisted_candidate(&cache, &config).expect("restored candidate");
        assert_eq!(restored.version(), "9.9.9");
    }

    #[test]
    fn restore_rejects_a_tampered_persisted_manifest() {
        let (config, signed) = signed_check_fixture("9.9.9");
        let mut cache = CachedAppUpdateStatus::default();
        apply_signed_check_to_cache(&mut cache, &signed, false, Local::now());

        let mut tampered = signed.manifest_bytes.clone();
        tampered[0] = b' ';
        cache.verified_manifest_base64 = Some(STANDARD.encode(&tampered));

        assert!(restore_persisted_candidate(&cache, &config).is_none());
    }

    #[test]
    fn restore_requires_a_matching_app_version_and_available_flag() {
        let (config, signed) = signed_check_fixture("9.9.9");
        let mut cache = CachedAppUpdateStatus::default();
        apply_signed_check_to_cache(&mut cache, &signed, false, Local::now());

        cache.available = false;
        assert!(restore_persisted_candidate(&cache, &config).is_none());

        cache.available = true;
        cache.current_version = Some("0.0.1".to_string());
        assert!(restore_persisted_candidate(&cache, &config).is_none());
    }

    #[test]
    fn an_up_to_date_check_clears_the_persisted_signed_bytes() {
        let (config, signed) = signed_check_fixture("9.9.9");
        let mut cache = CachedAppUpdateStatus::default();
        apply_signed_check_to_cache(&mut cache, &signed, false, Local::now());
        assert!(cache.verified_manifest_base64.is_some());

        // Any validly signed manifest that is not newer must clear the bytes.
        let (_, older_bytes, older_signature) = signed_bytes_fixture("1.0.0");
        let older = SignedCheck {
            result: CheckResult::UpToDate,
            manifest_bytes: older_bytes,
            signature_bytes: older_signature,
        };
        apply_signed_check_to_cache(&mut cache, &older, false, Local::now());

        assert!(!cache.available);
        assert!(cache.verified_manifest_base64.is_none());
        assert!(cache.verified_signature_base64.is_none());
        assert!(restore_persisted_candidate(&cache, &config).is_none());
    }

    #[test]
    fn a_failed_check_keeps_the_verified_candidate_and_download() {
        let state = Arc::new(Mutex::new(AppUpdateStateInner::default()));
        {
            let mut inner = state.lock().expect("state lock");
            inner.candidate = Some(update_candidate_fixture("1.4.2"));
            inner.downloaded = Some(downloaded_fixture("1.4.2"));
            inner.check_in_flight = true;
        }

        let still_downloaded = finish_check_failure(&state).expect("finish failed check");

        assert!(still_downloaded);
        let inner = state.lock().expect("state lock");
        assert_eq!(
            inner
                .candidate
                .as_ref()
                .map(|candidate| candidate.version().to_string()),
            Some("1.4.2".to_string())
        );
        assert!(inner.downloaded.is_some());
        assert!(!inner.check_in_flight);
    }

    #[test]
    fn a_successful_check_replaces_the_candidate_and_drops_the_previous_download() {
        let state = Arc::new(Mutex::new(AppUpdateStateInner::default()));
        {
            let mut inner = state.lock().expect("state lock");
            inner.candidate = Some(update_candidate_fixture("1.4.2"));
            inner.downloaded = Some(downloaded_fixture("1.4.2"));
            inner.check_in_flight = true;
        }

        let still_downloaded =
            finish_check_success(&state, Some(update_candidate_fixture("1.4.3")))
                .expect("finish successful check");

        assert!(!still_downloaded);
        {
            // Scoped so the guard is released before the second finish call below.
            let inner = state.lock().expect("state lock");
            assert_eq!(
                inner
                    .candidate
                    .as_ref()
                    .map(|candidate| candidate.version().to_string()),
                Some("1.4.3".to_string())
            );
            assert!(inner.downloaded.is_none());
            assert!(!inner.check_in_flight);
        }

        finish_check_success(&state, None).expect("finish up-to-date check");
        assert!(state.lock().expect("state lock").candidate.is_none());
    }

    #[test]
    fn status_hides_a_dismissed_version() {
        let cache = CachedAppUpdateStatus {
            current_version: Some(current_version()),
            available: true,
            version: Some("9.9.9".to_string()),
            dismissed_version: Some("9.9.9".to_string()),
            ..Default::default()
        };
        let status = info_from_cache(&cache, true, false);
        assert!(status.available);
        assert!(status.dismissed);
    }

    #[cfg(any(debug_assertions, feature = "update-preview"))]
    #[test]
    fn debug_update_preview_has_an_available_demo_version() {
        let info = demo_update_info();
        assert!(info.available);
        assert_eq!(info.version.as_deref(), Some("1.2.3-demo"));
        assert_eq!(
            demo_update_preview_mode_from_value("manual"),
            Some(DemoUpdatePreviewMode::Manual)
        );
    }

    #[test]
    fn update_apply_layout_only_allows_release_files() {
        let request = update_apply_request();
        if let Ok(request) = request {
            assert_eq!(request.layout.replace_files.len(), 3);
            assert_eq!(request.layout.preserve_files, ["quotabarwin.portable"]);
            assert!(request
                .layout
                .replace_files
                .contains(&"QuotaBarWin.exe".to_string()));
        }
    }

    #[test]
    fn cleanup_only_removes_copied_update_helpers() {
        let temp = tempfile::tempdir().expect("temp dir");
        let updates = temp.path().join("updates");
        fs::create_dir_all(&updates).expect("create updates dir");
        let copied_helper = updates.join("helper-123.exe");
        let another_copied_helper = updates.join("helper-456.exe");
        let official_helper = updates.join(APP_UPDATE_HELPER_NAME);
        let package = updates.join("QuotaBarWin_1.2.3.zip");
        let helper_named_directory = updates.join("helper-directory.exe");
        fs::write(&copied_helper, b"helper").expect("write copied helper");
        fs::write(&another_copied_helper, b"helper").expect("write second copied helper");
        fs::write(&official_helper, b"official helper").expect("write official helper");
        fs::write(&package, b"package").expect("write package");
        fs::create_dir(&helper_named_directory).expect("create helper-named directory");

        assert!(!cleanup_update_helper_copies_once(&updates).expect("clean helpers"));
        assert!(!copied_helper.exists());
        assert!(!another_copied_helper.exists());
        assert!(official_helper.exists());
        assert!(package.exists());
        assert!(helper_named_directory.is_dir());
    }
}
