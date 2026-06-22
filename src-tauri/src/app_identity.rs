pub const APP_USER_MODEL_ID: &str = "com.quotabarwin.app";

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
}
