use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngExt;

const LOCAL_API_TOKEN_FILE_NAME: &str = "local-api-token.txt";
const MINIMUM_TOKEN_LENGTH: usize = 32;

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn token_path_for_config_path(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("secrets")
        .join(LOCAL_API_TOKEN_FILE_NAME)
}

pub fn read_token(config_path: &Path) -> Result<Option<String>, String> {
    let path = token_path_for_config_path(config_path);
    match fs::read_to_string(path) {
        Ok(token) => {
            let token = token.trim().to_string();
            if token.is_empty() {
                Ok(None)
            } else {
                validate_token(&token)?;
                Ok(Some(token))
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "Unable to read local integration API token: {error}"
        )),
    }
}

pub fn write_token(config_path: &Path, token: &str) -> Result<(), String> {
    validate_token(token)?;

    let path = token_path_for_config_path(config_path);
    let parent = path
        .parent()
        .ok_or_else(|| "Unable to resolve local integration API token directory".to_string())?;
    fs::create_dir_all(parent).map_err(|error| {
        format!("Unable to create local integration API token directory: {error}")
    })?;

    let temporary_path = temporary_path(&path);
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
            .map_err(|error| format!("Unable to write local integration API token: {error}"))?;
        file.write_all(token.as_bytes())
            .map_err(|error| format!("Unable to write local integration API token: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("Unable to finalize local integration API token: {error}"))?;
        drop(file);
        fs::rename(&temporary_path, &path)
            .map_err(|error| format!("Unable to replace local integration API token: {error}"))
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}

pub fn delete_token(config_path: &Path) -> Result<(), String> {
    let path = token_path_for_config_path(config_path);
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "Unable to remove local integration API token: {error}"
        )),
    }
}

pub fn generate_token() -> String {
    let mut bytes = [0_u8; 32];
    rand::rng().fill(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn validate_token(token: &str) -> Result<(), String> {
    if token.len() < MINIMUM_TOKEN_LENGTH || token.chars().any(char::is_whitespace) {
        return Err(format!(
            "Local integration API token must contain at least {MINIMUM_TOKEN_LENGTH} non-whitespace characters"
        ));
    }
    Ok(())
}

fn temporary_path(path: &Path) -> PathBuf {
    let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(LOCAL_API_TOKEN_FILE_NAME);
    path.with_file_name(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        counter
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_and_reads_a_valid_token_without_a_newline() {
        let temp = tempfile::tempdir().expect("temp dir");
        let config_path = temp.path().join("config.json");
        let token = "x".repeat(MINIMUM_TOKEN_LENGTH);

        write_token(&config_path, &token).expect("write token");

        assert_eq!(read_token(&config_path).expect("read token"), Some(token));
    }

    #[test]
    fn rejects_short_or_whitespace_tokens() {
        let temp = tempfile::tempdir().expect("temp dir");
        let config_path = temp.path().join("config.json");

        assert!(write_token(&config_path, "short").is_err());
        assert!(write_token(
            &config_path,
            &format!("{} ", "x".repeat(MINIMUM_TOKEN_LENGTH))
        )
        .is_err());
    }

    #[test]
    fn generated_tokens_have_the_required_length() {
        assert!(generate_token().len() >= MINIMUM_TOKEN_LENGTH);
    }
}
