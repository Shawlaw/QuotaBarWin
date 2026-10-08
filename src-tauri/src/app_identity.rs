use std::sync::OnceLock;
use tauri::AppHandle;

// Never change the release AUMID: existing installations already registered it
// in HKCU and users' Windows notification settings hang off it.
pub const APP_USER_MODEL_ID: &str = "com.quotabarwin.app";
// Dev builds toast under their own identity so a dev exe running alongside the
// release app groups separately in Action Center and can be muted
// independently in Windows notification settings.
pub const DEV_APP_USER_MODEL_ID: &str = "com.quotabarwin.app.dev";
pub const APP_DISPLAY_NAME: &str = "QuotaBarWin";
pub const DEV_APP_DISPLAY_NAME: &str = "QuotaBarWin Dev";

// The release identifier from tauri.conf.json. Local development builds pass
// src-tauri/tauri.dev.conf.json via `npm run tauri:preview`, which swaps the
// identifier to the dev variant so a dev exe never shares the release
// single-instance mutex. Features that must not run from a dev exe (autostart,
// application updates) key off this predicate instead of trusting callers.
pub const RELEASE_APP_IDENTIFIER: &str = "com.quotabarwin.app";
// Pinned by the productization test against src-tauri/tauri.dev.conf.json;
// runtime checks compare against the release identifier instead.
#[allow(dead_code)]
pub const DEV_APP_IDENTIFIER: &str = "com.quotabarwin.app.dev";

pub fn is_dev_build_identifier(identifier: &str) -> bool {
    // Whitelist: only the exact release identifier gets full release
    // behavior. Any other identifier — the dev variant now, and any
    // staging-style variant someone adds later — is treated as a
    // development build.
    identifier != RELEASE_APP_IDENTIFIER
}

pub fn is_dev_build(app: &AppHandle) -> bool {
    is_dev_build_identifier(&app.config().identifier)
}

pub fn app_user_model_id_for_identifier(identifier: &str) -> &'static str {
    if is_dev_build_identifier(identifier) {
        DEV_APP_USER_MODEL_ID
    } else {
        APP_USER_MODEL_ID
    }
}

pub fn app_display_name_for_identifier(identifier: &str) -> &'static str {
    if is_dev_build_identifier(identifier) {
        DEV_APP_DISPLAY_NAME
    } else {
        APP_DISPLAY_NAME
    }
}

// The toast pipeline runs on worker threads without an AppHandle, so the
// per-build identity chosen at startup is parked here. Both getters fall back
// to the release identity so tests and non-app entry points keep the
// historical behavior.
static ACTIVE_APP_USER_MODEL_ID: OnceLock<&'static str> = OnceLock::new();
static ACTIVE_APP_DISPLAY_NAME: OnceLock<&'static str> = OnceLock::new();

pub fn active_app_user_model_id() -> &'static str {
    ACTIVE_APP_USER_MODEL_ID
        .get()
        .copied()
        .unwrap_or(APP_USER_MODEL_ID)
}

pub fn active_app_display_name() -> &'static str {
    ACTIVE_APP_DISPLAY_NAME
        .get()
        .copied()
        .unwrap_or(APP_DISPLAY_NAME)
}

pub fn configure_process_identity(identifier: &str) -> Result<(), String> {
    let app_user_model_id = app_user_model_id_for_identifier(identifier);
    let _ = ACTIVE_APP_USER_MODEL_ID.set(app_user_model_id);
    let _ = ACTIVE_APP_DISPLAY_NAME.set(app_display_name_for_identifier(identifier));
    #[cfg(windows)]
    {
        set_windows_app_user_model_id(app_user_model_id)
    }

    #[cfg(not(windows))]
    {
        Ok(())
    }
}

#[cfg(windows)]
fn set_windows_app_user_model_id(app_user_model_id: &str) -> Result<(), String> {
    let app_id: Vec<u16> = app_user_model_id.encode_utf16().chain([0]).collect();
    let result = unsafe { SetCurrentProcessExplicitAppUserModelID(app_id.as_ptr()) };
    if result < 0 {
        let hresult = result as u32;
        Err(format!(
            "failed to set Windows AppUserModelID {app_user_model_id}: HRESULT 0x{hresult:08X}"
        ))
    } else {
        Ok(())
    }
}

#[cfg(windows)]
#[link(name = "shell32")]
unsafe extern "system" {
    fn SetCurrentProcessExplicitAppUserModelID(app_id: *const u16) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_user_model_id_matches_tauri_identifier() {
        assert_eq!(APP_USER_MODEL_ID, "com.quotabarwin.app");
    }

    #[test]
    fn dev_builds_derive_their_own_toast_identity() {
        // The dev exe must not toast as the release app: a separate AUMID and
        // display name give it its own Action Center group and its own entry
        // in Windows notification settings.
        assert_eq!(
            app_user_model_id_for_identifier(DEV_APP_IDENTIFIER),
            DEV_APP_USER_MODEL_ID
        );
        assert_eq!(
            app_display_name_for_identifier(DEV_APP_IDENTIFIER),
            DEV_APP_DISPLAY_NAME
        );
        // Any unrecognized identifier is a dev build (whitelist semantics),
        // so it also gets the dev toast identity.
        assert_eq!(
            app_user_model_id_for_identifier("com.quotabarwin.app.debug"),
            DEV_APP_USER_MODEL_ID
        );
    }

    #[test]
    fn release_identifier_keeps_the_registered_toast_identity() {
        assert_eq!(
            app_user_model_id_for_identifier(RELEASE_APP_IDENTIFIER),
            APP_USER_MODEL_ID
        );
        assert_eq!(
            app_display_name_for_identifier(RELEASE_APP_IDENTIFIER),
            APP_DISPLAY_NAME
        );
    }

    #[test]
    fn active_identity_falls_back_to_release_before_startup_configures_it() {
        assert_eq!(active_app_user_model_id(), APP_USER_MODEL_ID);
        assert_eq!(active_app_display_name(), APP_DISPLAY_NAME);
    }

    #[test]
    fn dev_identity_is_detected_exactly() {
        assert!(is_dev_build_identifier(DEV_APP_IDENTIFIER));
        assert!(!is_dev_build_identifier(RELEASE_APP_IDENTIFIER));
        // Whitelist semantics: anything unrecognized is treated as a dev
        // build so unknown variants cannot enable release-only features.
        assert!(is_dev_build_identifier("com.quotabarwin.app.debug"));
        assert!(is_dev_build_identifier(""));
    }
}
