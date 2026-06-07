use std::{fs::{self, File}, io::Write, path::{Path, PathBuf}};

use serde::Serialize;
use tauri::{command, AppHandle};
use zip::{write::FileOptions, ZipWriter};

use crate::{
    config::{config_path_for_app, load_or_create_config},
    quota::get_cached_snapshot,
    redact::redact_sensitive,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticsManifest {
    app_version: String,
    platform: String,
    arch: String,
}

#[command]
pub async fn export_diagnostics(app: AppHandle, output_path: String) -> Result<(), String> {
    let config_path = config_path_for_app(&app)?;
    let snapshot = get_cached_snapshot().ok().flatten();
    let log_path = config_path.with_file_name("quotabarwin.log");
    let app_version = env!("CARGO_PKG_VERSION").to_string();
    let output_path = PathBuf::from(output_path);

    tauri::async_runtime::spawn_blocking(move || {
        let config = load_or_create_config(&config_path).map(|loaded| loaded.config).ok();
        let config_value = config
            .as_ref()
            .and_then(|config| serde_json::to_value(config).ok());
        let snapshot_value = snapshot
            .as_ref()
            .and_then(|snapshot| serde_json::to_value(snapshot).ok());

        export_diagnostics_zip_to_path(
            &output_path,
            &app_version,
            config_value.as_ref(),
            snapshot_value.as_ref(),
            &log_path,
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

pub fn export_diagnostics_zip_to_path(
    output_path: &Path,
    app_version: &str,
    config: Option<&serde_json::Value>,
    snapshot: Option<&serde_json::Value>,
    log_path: &Path,
) -> Result<(), String> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let file = File::create(output_path).map_err(|error| error.to_string())?;
    write_diagnostics_zip(file, app_version, config, snapshot, log_path)
}

fn write_diagnostics_zip<W: Write + std::io::Seek>(
    writer: W,
    app_version: &str,
    config: Option<&serde_json::Value>,
    snapshot: Option<&serde_json::Value>,
    log_path: &Path,
) -> Result<(), String> {
    let mut zip = ZipWriter::new(writer);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let manifest = DiagnosticsManifest {
        app_version: app_version.to_string(),
        platform: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
    };

    add_json(&mut zip, options, "manifest.json", &manifest)?;
    add_redacted_json(&mut zip, options, "config.redacted.json", config)?;
    add_redacted_json(&mut zip, options, "last_snapshot.redacted.json", snapshot)?;
    add_text(
        &mut zip,
        options,
        "recent_logs.redacted.log",
        &read_redacted_log(log_path),
    )?;
    zip.finish().map_err(|error| error.to_string())?;
    Ok(())
}

fn add_json<T: Serialize, W: Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    options: FileOptions,
    name: &str,
    value: &T,
) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    add_text(zip, options, name, &text)
}

fn add_redacted_json<W: Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    options: FileOptions,
    name: &str,
    value: Option<&serde_json::Value>,
) -> Result<(), String> {
    let text = value
        .map(|value| serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string()))
        .unwrap_or_else(|| "{}".to_string());
    add_text(zip, options, name, &redact_sensitive(&text))
}

fn add_text<W: Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    options: FileOptions,
    name: &str,
    text: &str,
) -> Result<(), String> {
    zip.start_file(name, options)
        .map_err(|error| error.to_string())?;
    zip.write_all(text.as_bytes())
        .map_err(|error| error.to_string())
}

fn read_redacted_log(path: &Path) -> String {
    fs::read_to_string(path)
        .map(|contents| redact_sensitive(&contents))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::{Cursor, Read}, path::PathBuf};

    #[test]
    fn diagnostics_export_redacts_secrets() {
        let temp = tempfile::tempdir().expect("temp dir");
        let output = temp.path().join("diagnostics.zip");
        let log_path = temp.path().join("app.log");
        fs::write(
            &log_path,
            "Authorization: Bearer abcdefghijklmnopqrstuvwxyz123456",
        )
        .expect("write log");
        let config = serde_json::json!({
            "env": {"KIMI_API_KEY": "abcdefghijklmnopqrstuvwxyz123456"},
            "args": ["Authorization: Bearer abcdefghijklmnopqrstuvwxyz123456"]
        });
        let snapshot = serde_json::json!({
            "diagnostics": {"stderr": "Cookie=session-secret-token-abcdefghijklmnopqrstuvwxyz"}
        });

        export_diagnostics_zip_to_path(&output, "1.0.0", Some(&config), Some(&snapshot), &log_path)
            .expect("export diagnostics");

        let file = File::open(output).expect("open zip");
        let mut archive = zip::ZipArchive::new(file).expect("read zip");
        let mut combined = String::new();
        for index in 0..archive.len() {
            let mut file = archive.by_index(index).expect("zip file");
            let mut text = String::new();
            file.read_to_string(&mut text).expect("read file");
            combined.push_str(&text);
        }

        assert!(!combined.contains("abcdefghijklmnopqrstuvwxyz123456"));
        assert!(!combined.contains("session-secret-token"));
        assert!(combined.contains("[REDACTED]"));
    }

    #[test]
    fn diagnostics_zip_can_be_written_to_memory() {
        let cursor = Cursor::new(Vec::new());
        write_diagnostics_zip(cursor, "1.0.0", None, None, &PathBuf::from("missing.log"))
            .expect("zip");
    }
}
