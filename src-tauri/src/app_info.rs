#[tauri::command]
pub fn get_app_version() -> Result<String, String> {
    Ok(app_display_version())
}

pub fn app_display_version() -> String {
    format_app_display_version(
        env!("CARGO_PKG_VERSION"),
        option_env!("QUOTABARWIN_GIT_SHORT_HASH"),
    )
}

fn format_app_display_version(version: &str, git_short_hash: Option<&str>) -> String {
    match git_short_hash
        .map(str::trim)
        .filter(|hash| !hash.is_empty())
    {
        Some(hash) => format!("{version}({hash})"),
        None => version.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_version_includes_short_commit_when_available() {
        assert_eq!(
            format_app_display_version("1.2.3", Some("abc1234")),
            "1.2.3(abc1234)"
        );
    }

    #[test]
    fn display_version_omits_empty_commit() {
        assert_eq!(format_app_display_version("1.2.3", Some(" ")), "1.2.3");
        assert_eq!(format_app_display_version("1.2.3", None), "1.2.3");
    }
}
