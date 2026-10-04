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

// Opens Provider help links supplied by remote manifests. The scheme is
// restricted to http/https because manifest content is third-party data and
// must not reach other protocol handlers or local files.
#[tauri::command]
pub fn open_external_link(url: String) -> Result<(), String> {
    let url = url.trim();
    if !is_allowed_external_link_url(url) {
        return Err("External link must be a non-empty http(s) URL".to_string());
    }

    open_url_external(url)
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

pub(crate) fn is_allowed_external_link_url(url: &str) -> bool {
    for prefix in ["http://", "https://"] {
        if let Some(rest) = url.strip_prefix(prefix) {
            return !rest.is_empty();
        }
    }
    let lowered = url.to_ascii_lowercase();
    for prefix in ["http://", "https://"] {
        if let Some(rest) = lowered.strip_prefix(prefix) {
            return !rest.is_empty();
        }
    }
    false
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
    use super::*;

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

    #[test]
    fn external_links_allow_only_non_empty_http_urls() {
        assert!(is_allowed_external_link_url("https://platform.example.com/docs"));
        assert!(is_allowed_external_link_url("http://example.com"));
        assert!(is_allowed_external_link_url("HTTPS://EXAMPLE.COM/KEYS"));

        assert!(!is_allowed_external_link_url("https://"));
        assert!(!is_allowed_external_link_url(""));
        assert!(!is_allowed_external_link_url("file:///C:/Windows/System32/calc.exe"));
        assert!(!is_allowed_external_link_url("ms-settings:windows-defender"));
        assert!(!is_allowed_external_link_url("javascript:alert(1)"));
        assert!(!is_allowed_external_link_url("\\\\localhost\\share"));
    }
}
