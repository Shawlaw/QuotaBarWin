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

use chrono::{Local, Timelike};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use desktop_updater::{
    ApplyRequest, CheckResult, DownloadedUpdate, PortableLayout, UpdateCandidate, UpdateConfig,
};

const APP_UPDATE_MANIFEST_URL: &str =
    "https://raw.githubusercontent.com/Shawlaw/QuotaBarWin/main/updates/stable.json";
const APP_UPDATE_SIGNATURE_URL: &str =
    "https://raw.githubusercontent.com/Shawlaw/QuotaBarWin/main/updates/stable.json.sig";
const APP_UPDATE_APP_ID: &str = "com.quotabarwin.app";
const APP_UPDATE_CHANNEL: &str = "stable";
const APP_UPDATE_HELPER_NAME: &str = "QuotaBarWin.Updater.exe";
const APP_UPDATE_MAIN_EXE_NAME: &str = "QuotaBarWin.exe";
const APP_UPDATE_STATUS_CACHE_FILE: &str = "app_update_status.quotaBarWin.json";
const APP_UPDATE_STATUS_CACHE_SCHEMA_VERSION: u8 = 1;
const AUTOMATIC_CHECK_START_HOUR: u32 = 8;
const APP_UPDATE_HELPER_COPY_PREFIX: &str = "helper-";
const APP_UPDATE_HELPER_COPY_SUFFIX: &str = ".exe";
const APP_UPDATE_HELPER_CLEANUP_RETRY_DELAY: Duration = Duration::from_millis(250);
const APP_UPDATE_HELPER_CLEANUP_MAX_RETRIES: usize = 40;
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
}

#[tauri::command]
pub async fn get_app_update_status(
    app: AppHandle,
    state: State<'_, AppUpdateState>,
) -> Result<AppUpdateInfo, String> {
    #[cfg(any(debug_assertions, feature = "update-preview"))]
    {
        let demo_requested = demo_update_preview_requested();
        log_demo_preview(
            &app,
            &format!("status requested demoRequested={demo_requested}"),
        );
        if demo_requested {
            let demo_state = state.inner.clone();
            request_demo_update_preview(app.clone(), demo_state.clone());
            let info = {
                let mut state = demo_state
                    .lock()
                    .map_err(|_| "Application update state is unavailable")?;
                if let Some(info) = state.demo_info.clone() {
                    info
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
    if !app_updates_are_configured() {
        let app_for_cache = app.clone();
        let cache =
            tauri::async_runtime::spawn_blocking(move || load_status_cache_for_app(&app_for_cache))
                .await
                .map_err(redacted_error)??;
        return Ok(info_from_cache(&cache, false, false));
    }

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
                    let downloaded = finish_check(&state.inner, candidate_for_result(result))?;
                    let info = info_from_cache(&cache, true, downloaded);
                    emit_update_status(&app, &info, false);
                    Ok(info)
                }
                Err(error) => {
                    let _ = finish_check(&state.inner, None);
                    Err(redacted_error(error))
                }
            }
        }
        Err(error) => {
            let _ = finish_check(&state.inner, None);
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
    if demo_update_preview_requested() {
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
    let candidate = {
        let state = state
            .inner
            .lock()
            .map_err(|_| "Application update state is unavailable")?;
        state
            .candidate
            .clone()
            .ok_or_else(|| "Check for an application update before downloading it".to_string())?
    };
    let (config, updates_dir) = update_context(&app)?;
    let downloaded = tauri::async_runtime::spawn_blocking(move || {
        desktop_updater::download(&config, candidate, &updates_dir, |_, _| {})
    })
    .await
    .map_err(redacted_error)?
    .map_err(redacted_error)?;
    let downloaded = {
        let mut state = state
            .inner
            .lock()
            .map_err(|_| "Application update state is unavailable")?;
        state.downloaded = Some(downloaded);
        state.downloaded.is_some()
    };
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
    let request = update_apply_request()?;
    tauri::async_runtime::spawn_blocking(move || {
        desktop_updater::apply_and_restart(&downloaded, &request)
    })
    .await
    .map_err(redacted_error)?
    .map_err(redacted_error)?;
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
        let demo_requested = demo_update_preview_requested();
        log_demo_preview(
            &app,
            &format!("focus trigger demoRequested={demo_requested}"),
        );
        if demo_requested {
            request_demo_update_preview(app, state);
            return;
        }
    }

    tauri::async_runtime::spawn(async move {
        let app_for_start = app.clone();
        let state_for_start = state.clone();
        let should_check = tauri::async_runtime::spawn_blocking(move || {
            begin_automatic_check(&app_for_start, &state_for_start)
        })
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or(false);

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
                if let Ok(downloaded) = finish_check(&state, None) {
                    let info = info_from_cache(&cache, false, downloaded);
                    emit_update_status(&app, &info, false);
                }
            } else {
                let _ = finish_check(&state, None);
            }
            return;
        }

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
                        if let Ok(downloaded) = finish_check(&state, candidate_for_result(result)) {
                            let info = info_from_cache(&cache, true, downloaded);
                            emit_update_status(&app, &info, info.available && !info.dismissed);
                        }
                    }
                    _ => {
                        let _ = finish_check(&state, None);
                    }
                }
            }
            Err(error) => {
                let app_for_cache = app.clone();
                let cache = tauri::async_runtime::spawn_blocking(move || {
                    persist_automatic_failure(&app_for_cache, &error)
                })
                .await;
                if let Ok(downloaded) = finish_check(&state, None) {
                    if let Ok(Ok(cache)) = cache {
                        let info = info_from_cache(&cache, true, downloaded);
                        emit_update_status(&app, &info, false);
                    }
                }
            }
        }
    });
}

#[cfg(any(debug_assertions, feature = "update-preview"))]
fn demo_update_preview_requested() -> bool {
    std::env::var(APP_UPDATE_DEMO_ENV)
        .ok()
        .is_some_and(|value| matches!(value.trim(), "1" | "true" | "TRUE"))
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

fn finish_check(
    state: &Arc<Mutex<AppUpdateStateInner>>,
    candidate: Option<UpdateCandidate>,
) -> Result<bool, String> {
    let mut state = state
        .lock()
        .map_err(|_| "Application update state is unavailable")?;
    state.candidate = candidate;
    state.downloaded = None;
    state.check_in_flight = false;
    Ok(state.downloaded.is_some())
}

fn begin_automatic_check(
    app: &AppHandle,
    state: &Arc<Mutex<AppUpdateStateInner>>,
) -> Result<bool, String> {
    let config_path = crate::config::config_path_for_app(app)?;
    let config = crate::config::load_or_create_config(&config_path)?.config;
    let now = Local::now();
    let today = now.date_naive().to_string();
    let cache = load_status_cache_from_path(&status_cache_path_for_config_path(&config_path)?)?;
    if !automatic_check_is_due(
        config.app_update.auto_check,
        now.hour(),
        &today,
        cache.last_automatic_check_date.as_deref(),
    ) {
        return Ok(false);
    }

    let mut state = state
        .lock()
        .map_err(|_| "Application update state is unavailable")?;
    if state.check_in_flight {
        return Ok(false);
    }
    state.check_in_flight = true;
    Ok(true)
}

fn automatic_check_is_due(
    auto_check_enabled: bool,
    local_hour: u32,
    today: &str,
    last_automatic_check_date: Option<&str>,
) -> bool {
    auto_check_enabled
        && local_hour >= AUTOMATIC_CHECK_START_HOUR
        && last_automatic_check_date != Some(today)
}

async fn run_update_check(app: AppHandle) -> Result<CheckResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (config, _) = update_context(&app)?;
        desktop_updater::check(&config).map_err(redacted_error)
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
    result: &CheckResult,
    automatic: bool,
) -> Result<CachedAppUpdateStatus, String> {
    let mut cache = load_status_cache_for_app(app)?;
    let current_version = current_version();
    if cache.current_version.as_deref() != Some(current_version.as_str()) {
        cache = CachedAppUpdateStatus::default();
    }

    cache.schema_version = APP_UPDATE_STATUS_CACHE_SCHEMA_VERSION;
    cache.current_version = Some(current_version);
    cache.checked_at = Some(Local::now().to_rfc3339());
    cache.error = None;
    match result {
        CheckResult::UpToDate => {
            cache.available = false;
            cache.version = None;
            cache.notes_url = None;
            cache.dismissed_version = None;
        }
        CheckResult::UpdateAvailable(candidate) => {
            let version = candidate.version().to_string();
            if cache.version.as_deref() != Some(version.as_str()) {
                cache.dismissed_version = None;
            }
            cache.available = true;
            cache.version = Some(version);
            cache.notes_url = candidate.notes_url().map(str::to_string);
        }
    }
    if automatic {
        cache.last_automatic_check_date = Some(Local::now().date_naive().to_string());
    }
    write_status_cache_for_app(app, &cache)?;
    Ok(cache)
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
    cache.checked_at = Some(Local::now().to_rfc3339());
    cache.error = Some(redacted_error(error));
    cache.last_automatic_check_date = Some(Local::now().date_naive().to_string());
    write_status_cache_for_app(app, &cache)?;
    Ok(cache)
}

fn persist_automatic_unconfigured(app: &AppHandle) -> Result<CachedAppUpdateStatus, String> {
    let cache = CachedAppUpdateStatus {
        schema_version: APP_UPDATE_STATUS_CACHE_SCHEMA_VERSION,
        last_automatic_check_date: Some(Local::now().date_naive().to_string()),
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

fn emit_update_status(app: &AppHandle, info: &AppUpdateInfo, animate: bool) {
    let _ = app.emit(
        APP_UPDATE_STATUS_CHANGED_EVENT,
        AppUpdateStatusEvent {
            info: info.clone(),
            animate,
        },
    );
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
        assert!(!automatic_check_is_due(true, 7, "2026-08-14", None));
        assert!(automatic_check_is_due(true, 8, "2026-08-14", None));
        assert!(!automatic_check_is_due(
            true,
            9,
            "2026-08-14",
            Some("2026-08-14")
        ));
        assert!(!automatic_check_is_due(false, 9, "2026-08-14", None));
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
