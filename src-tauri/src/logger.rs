use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use chrono::Utc;
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

    let entry = LogEntry {
        timestamp: Utc::now().to_rfc3339(),
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
