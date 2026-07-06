use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use chrono::Local;
use serde::Serialize;

use crate::{config::AppConfig, redact::redact_sensitive};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "debug" => Self::Debug,
            "warn" | "warning" => Self::Warn,
            "error" => Self::Error,
            _ => Self::Info,
        }
    }

    fn priority(self) -> u8 {
        match self {
            Self::Debug => 10,
            Self::Info => 20,
            Self::Warn => 30,
            Self::Error => 40,
        }
    }

    fn enabled_by(self, min_level: Self) -> bool {
        self.priority() >= min_level.priority()
    }
}

#[derive(Debug, Clone)]
pub struct LogSink {
    path: PathBuf,
    min_level: LogLevel,
    max_bytes: u64,
}

impl LogSink {
    pub fn from_config_path(config_path: &Path, config: &AppConfig) -> Self {
        Self {
            path: log_path_for_config_path(config_path),
            min_level: LogLevel::parse(&config.log_level),
            max_bytes: config.log_max_bytes,
        }
    }

    pub fn write(&self, level: LogLevel, target: &str, message: &str) -> Result<(), String> {
        if !level.enabled_by(self.min_level) {
            return Ok(());
        }
        write_structured_log(&self.path, level, target, message, self.max_bytes)
    }
}

pub fn log_path_for_config_path(config_path: &Path) -> PathBuf {
    config_path.with_file_name("quotabarwin.log")
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogEntry<'a> {
    timestamp: i64,
    local_ts: String,
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

    let now = Local::now();
    let entry = LogEntry {
        timestamp: now.timestamp_millis(),
        local_ts: format!(
            "{}.{:03} {}",
            now.format("%Y-%m-%d %H:%M:%S"),
            now.timestamp_subsec_millis(),
            now.format("%:z")
        ),
        level,
        target,
        message: redact_sensitive(message),
    };
    let line = serde_json::to_string(&entry).map_err(|error| error.to_string())?;
    rotate_if_needed(path, max_bytes, line.len() as u64 + 1)?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    writeln!(file, "{line}").map_err(|error| error.to_string())
}

fn rotate_if_needed(
    path: &Path,
    max_total_bytes: u64,
    next_entry_bytes: u64,
) -> Result<(), String> {
    if max_total_bytes == 0 || !path.exists() {
        return Ok(());
    }

    let active_max_bytes = (max_total_bytes / 2).max(1);
    let length = fs::metadata(path).map_err(|error| error.to_string())?.len();
    if length.saturating_add(next_entry_bytes) <= active_max_bytes {
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

    #[test]
    fn logs_include_epoch_timestamp_and_local_display_time() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("app.log");

        write_structured_log(&path, LogLevel::Info, "test", "hello", 1024).expect("write log");

        let contents = fs::read_to_string(path).expect("read log");
        let entry: serde_json::Value =
            serde_json::from_str(contents.trim()).expect("parse structured log");
        assert!(entry["timestamp"].as_i64().is_some());

        let local_ts = entry["localTs"].as_str().expect("localTs string");
        assert_eq!(local_ts.len(), "2026-07-06 20:47:08.561 +08:00".len());
        let chars = local_ts.chars().collect::<Vec<_>>();
        assert_eq!(chars[4], '-');
        assert_eq!(chars[10], ' ');
        assert_eq!(chars[19], '.');
        assert_eq!(chars[23], ' ');
        assert!(chars[24] == '+' || chars[24] == '-');
        assert!(chars[25].is_ascii_digit());
        assert!(chars[26].is_ascii_digit());
        assert_eq!(chars[27], ':');
        assert!(chars[28].is_ascii_digit());
        assert!(chars[29].is_ascii_digit());
    }

    #[test]
    fn log_limit_is_split_across_current_and_rotated_file() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("app.log");

        for index in 0..20 {
            write_structured_log(
                &path,
                LogLevel::Info,
                "test",
                &format!("entry {index} {}", "x".repeat(40)),
                800,
            )
            .expect("write log");
        }

        let rotated = path.with_extension("log.1");
        assert!(rotated.exists());
        assert!(fs::metadata(&path).expect("current metadata").len() <= 800);
        assert!(fs::metadata(&rotated).expect("rotated metadata").len() <= 800);
        assert!(
            fs::metadata(&path).expect("current metadata").len()
                + fs::metadata(&rotated).expect("rotated metadata").len()
                <= 800
        );
    }
}
