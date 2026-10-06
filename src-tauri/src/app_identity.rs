use tauri::AppHandle;

pub const APP_USER_MODEL_ID: &str = "com.quotabarwin.app";

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

pub fn configure_process_identity() -> Result<(), String> {
    #[cfg(windows)]
    {
        set_windows_app_user_model_id()
    }

    #[cfg(not(windows))]
    {
        Ok(())
    }
}

#[cfg(windows)]
fn set_windows_app_user_model_id() -> Result<(), String> {
    let app_id: Vec<u16> = APP_USER_MODEL_ID.encode_utf16().chain([0]).collect();
    let result = unsafe { SetCurrentProcessExplicitAppUserModelID(app_id.as_ptr()) };
    if result < 0 {
        let hresult = result as u32;
        Err(format!(
            "failed to set Windows AppUserModelID {APP_USER_MODEL_ID}: HRESULT 0x{hresult:08X}"
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
    fn dev_identity_is_detected_exactly() {
        assert!(is_dev_build_identifier(DEV_APP_IDENTIFIER));
        assert!(!is_dev_build_identifier(RELEASE_APP_IDENTIFIER));
        // Whitelist semantics: anything unrecognized is treated as a dev
        // build so unknown variants cannot enable release-only features.
        assert!(is_dev_build_identifier("com.quotabarwin.app.debug"));
        assert!(is_dev_build_identifier(""));
    }
}
