use std::{path::PathBuf, sync::Mutex};

use serde::Serialize;
use tauri::{AppHandle, State};

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

#[derive(Default)]
pub struct AppUpdateState {
    inner: Mutex<AppUpdateStateInner>,
}

#[derive(Default)]
struct AppUpdateStateInner {
    candidate: Option<UpdateCandidate>,
    downloaded: Option<DownloadedUpdate>,
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
}

#[tauri::command]
pub async fn check_app_update(
    app: AppHandle,
    state: State<'_, AppUpdateState>,
) -> Result<AppUpdateInfo, String> {
    let (config, _) = update_context(&app)?;
    let result = tauri::async_runtime::spawn_blocking(move || desktop_updater::check(&config))
        .await
        .map_err(redacted_error)?
        .map_err(redacted_error)?;
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "Application update state is unavailable")?;
    match result {
        CheckResult::UpToDate => {
            state.candidate = None;
            state.downloaded = None;
            Ok(info_for_state(&state))
        }
        CheckResult::UpdateAvailable(candidate) => {
            state.candidate = Some(candidate);
            state.downloaded = None;
            Ok(info_for_state(&state))
        }
    }
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
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "Application update state is unavailable")?;
    state.downloaded = Some(downloaded);
    Ok(info_for_state(&state))
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

pub fn acknowledge_applied_update() -> Result<bool, String> {
    desktop_updater::acknowledge_if_requested().map_err(redacted_error)
}

fn redacted_error(error: impl ToString) -> String {
    crate::redact::redact_sensitive(&error.to_string())
}

fn update_context(app: &AppHandle) -> Result<(UpdateConfig, PathBuf), String> {
    let public_key = option_env!("QUOTABARWIN_UPDATE_PUBLIC_KEY")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            "Application updates are not configured in this build. The maintainer must set QUOTABARWIN_UPDATE_PUBLIC_KEY when building a release."
                .to_string()
        })?;
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
    let updates_dir = config_path
        .parent()
        .ok_or_else(|| "Unable to resolve application update directory".to_string())?
        .join("updates");
    Ok((update_config, updates_dir))
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

fn info_for_state(state: &AppUpdateStateInner) -> AppUpdateInfo {
    let candidate = state.candidate.as_ref();
    AppUpdateInfo {
        configured: true,
        current_version: env!("CARGO_PKG_VERSION").to_string(),
        available: candidate.is_some(),
        version: candidate.map(|candidate| candidate.version().to_string()),
        notes_url: candidate.and_then(|candidate| candidate.notes_url().map(str::to_string)),
        downloaded: state.downloaded.is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
