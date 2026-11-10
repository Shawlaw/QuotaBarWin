//! Secret clipboard writes that stay out of Windows clipboard history
//! (Win+V) and cloud clipboard sync.
//!
//! `arboard` cannot attach custom clipboard formats, so the initial copy goes
//! through Win32 directly: besides `CF_UNICODETEXT` the write also places the
//! two exclusion formats documented by Microsoft
//! (`CanIncludeInClipboardHistory`, `CanUploadToCloudClipboard`, each a single
//! `DWORD 0`). Without them a copied credential would linger in clipboard
//! history and roam through the user's cloud clipboard — the largest hidden
//! leak surface for a "copy secret" feature.

/// Copies `value` to the clipboard with the privacy exclusion formats.
pub fn copy_secret(value: &str) -> Result<(), String> {
    set_text_with_privacy_formats(value)
}

#[cfg(windows)]
fn set_text_with_privacy_formats(value: &str) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use std::thread;
    use std::time::Duration;
    use windows_sys::Win32::Foundation::{GlobalFree, HANDLE};
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE, GMEM_ZEROINIT,
    };

    const CF_UNICODETEXT: u32 = 13;
    const CLIPBOARD_OPEN_RETRIES: usize = 8;
    const CLIPBOARD_OPEN_RETRY_DELAY: Duration = Duration::from_millis(15);
    const EXCLUDE_HISTORY_FORMAT: &str = "CanIncludeInClipboardHistory";
    const EXCLUDE_CLOUD_FORMAT: &str = "CanUploadToCloudClipboard";

    fn to_wide(text: &str) -> Vec<u16> {
        let mut wide: Vec<u16> = std::ffi::OsStr::new(text).encode_wide().collect();
        wide.push(0);
        wide
    }

    /// Allocates a movable global block with `bytes.len()` zero-initialized
    /// bytes and copies the payload in. Returns the `HGLOBAL`; on failure the
    /// allocation is freed and `Err` is returned.
    fn alloc_global(bytes: &[u8]) -> Result<*mut core::ffi::c_void, String> {
        let handle = unsafe { GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, bytes.len()) };
        if handle.is_null() {
            return Err("Unable to allocate clipboard memory".to_string());
        }
        let target = unsafe { GlobalLock(handle) };
        if target.is_null() {
            unsafe { GlobalFree(handle) };
            return Err("Unable to lock clipboard memory".to_string());
        }
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), target as *mut u8, bytes.len());
            GlobalUnlock(handle);
        }
        Ok(handle)
    }

    unsafe {
        // The clipboard may be briefly held by another process; retry a few
        // times before giving up.
        let mut opened = 0;
        for _ in 0..CLIPBOARD_OPEN_RETRIES {
            opened = OpenClipboard(std::ptr::null_mut());
            if opened != 0 {
                break;
            }
            thread::sleep(CLIPBOARD_OPEN_RETRY_DELAY);
        }
        if opened == 0 {
            return Err("Unable to open the Windows clipboard".to_string());
        }

        let result = (|| -> Result<(), String> {
            if EmptyClipboard() == 0 {
                return Err("Unable to empty the Windows clipboard".to_string());
            }

            let wide = to_wide(value);
            let byte_len = wide.len() * 2;
            let text_handle =
                alloc_global(std::slice::from_raw_parts(wide.as_ptr().cast(), byte_len))?;
            if SetClipboardData(CF_UNICODETEXT, text_handle as HANDLE).is_null() {
                // Ownership was not transferred, so the allocation is still ours.
                GlobalFree(text_handle);
                return Err("Unable to place the secret on the clipboard".to_string());
            }

            // Both exclusion formats are a single DWORD 0. Best-effort: an
            // older Windows build that rejects the registration still gets the
            // text.
            for format_name in [EXCLUDE_HISTORY_FORMAT, EXCLUDE_CLOUD_FORMAT] {
                let wide_name = to_wide(format_name);
                let format = RegisterClipboardFormatW(wide_name.as_ptr());
                if format == 0 {
                    continue;
                }
                let Ok(marker) = alloc_global(&[0u8; 4]) else {
                    continue;
                };
                if SetClipboardData(format, marker as HANDLE).is_null() {
                    GlobalFree(marker);
                }
            }
            Ok(())
        })();

        CloseClipboard();
        result
    }
}

#[cfg(not(windows))]
fn set_text_with_privacy_formats(_value: &str) -> Result<(), String> {
    Err("Secret clipboard copy is only supported on Windows".to_string())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    // Touches the real clipboard of the interactive session; run manually via
    // `cargo test -- --ignored` and check Win+V history afterwards.
    #[test]
    #[ignore = "uses the real interactive clipboard"]
    fn copies_text_with_exclusion_formats() {
        let sample = "qbwin-clipboard-test-secret";
        copy_secret(sample).expect("copy");

        let mut clipboard = arboard::Clipboard::new().expect("clipboard");
        assert_eq!(clipboard.get_text().expect("text"), sample);

        use windows_sys::Win32::System::DataExchange::{
            CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
            RegisterClipboardFormatW,
        };
        unsafe {
            assert_ne!(OpenClipboard(std::ptr::null_mut()), 0);
            for name in ["CanIncludeInClipboardHistory", "CanUploadToCloudClipboard"] {
                let mut wide: Vec<u16> = name.encode_utf16().collect();
                wide.push(0);
                let format = RegisterClipboardFormatW(wide.as_ptr());
                assert_ne!(format, 0, "{name} must be registrable");
                assert_ne!(
                    IsClipboardFormatAvailable(format),
                    0,
                    "{name} must be present after the copy"
                );
                if IsClipboardFormatAvailable(format) != 0 {
                    // The marker payload is a single DWORD 0.
                    let handle = GetClipboardData(format) as *mut u32;
                    assert_eq!(*handle, 0);
                }
            }
            CloseClipboard();
        }
    }
}
