use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use chrono::Utc;
use serde::Serialize;

use crate::redact::redact_sensitive;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogEntry<'a> {
    timestamp: String,
    level: LogLevel,
    target: &'a str,
    message: String,
}

pub fn write_structured_log(
    path: &Path,
    level: LogLevel,
    target: &str,
    message: &str,
    max_bytes: u64,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    rotate_if_needed(path, max_bytes)?;

    let entry = LogEntry {
        timestamp: Utc::now().to_rfc3339(),
        level,
        target,
        message: redact_sensitive(message),
    };
    let line = serde_json::to_string(&entry).map_err(|error| error.to_string())?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    writeln!(file, "{line}").map_err(|error| error.to_string())
}

fn rotate_if_needed(path: &Path, max_bytes: u64) -> Result<(), String> {
    if max_bytes == 0 || !path.exists() {
        return Ok(());
    }

    let length = fs::metadata(path).map_err(|error| error.to_string())?.len();
    if length <= max_bytes {
        return Ok(());
    }

    let rotated = path.with_extension("log.1");
    if rotated.exists() {
        fs::remove_file(&rotated).map_err(|error| error.to_string())?;
    }
    fs::rename(path, rotated).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logs_are_redacted_before_write() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("app.log");

        write_structured_log(
            &path,
            LogLevel::Info,
            "test",
            "Authorization: Bearer abcdefghijklmnopqrstuvwxyz123456",
            1024,
        )
        .expect("write log");

        let contents = fs::read_to_string(path).expect("read log");
        assert!(!contents.contains("abcdefghijklmnopqrstuvwxyz123456"));
        assert!(contents.contains("[REDACTED]"));
    }
}
