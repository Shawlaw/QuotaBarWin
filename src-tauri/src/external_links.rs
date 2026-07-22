use std::process::Command;

const PROJECT_GITHUB_URL: &str = "https://github.com/Shawlaw/QuotaBarWin";
const RELEASE_NOTES_URL_PREFIX: &str = "https://github.com/Shawlaw/QuotaBarWin/releases/tag/";

#[tauri::command]
pub fn open_project_github() -> Result<(), String> {
    open_url_external(PROJECT_GITHUB_URL)
}

#[tauri::command]
pub fn open_app_update_notes(notes_url: String) -> Result<(), String> {
    let notes_url = notes_url.trim();
    if !is_allowed_release_notes_url(notes_url) {
        return Err("Release notes URL is not an allowed QuotaBarWin release page".to_string());
    }

    open_url_external(notes_url)
}

pub(crate) fn is_allowed_release_notes_url(url: &str) -> bool {
    let Some(tag) = url.strip_prefix(RELEASE_NOTES_URL_PREFIX) else {
        return false;
    };

    !tag.is_empty()
        && tag
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn open_url_external(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        Command::new("rundll32")
            .arg("url.dll,FileProtocolHandler")
            .arg(url)
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(());
    }
}

#[cfg(test)]
mod tests {
    use super::is_allowed_release_notes_url;

    #[test]
    fn release_notes_are_limited_to_project_release_tags() {
        assert!(is_allowed_release_notes_url(
            "https://github.com/Shawlaw/QuotaBarWin/releases/tag/v1.0.5"
        ));
        assert!(is_allowed_release_notes_url(
            "https://github.com/Shawlaw/QuotaBarWin/releases/tag/v1.0.6-rc.1"
        ));
        assert!(!is_allowed_release_notes_url(
            "https://github.com/Shawlaw/QuotaBarWin/releases/latest"
        ));
        assert!(!is_allowed_release_notes_url(
            "https://example.com/releases/tag/v1.0.5"
        ));
        assert!(!is_allowed_release_notes_url(
            "https://github.com/Shawlaw/QuotaBarWin/releases/tag/v1.0.5?download=1"
        ));
    }
}
