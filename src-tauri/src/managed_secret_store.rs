use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

const SECRET_DIRECTORY: &str = "secrets";
const PROVIDER_SECRET_DIRECTORY: &str = "providers";

/// A non-sensitive reference to an application-managed Provider secret.
///
/// This type deliberately stores only the stable instance and parameter names.
/// It must never contain the secret value itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedSecretRef {
    provider_id: String,
    parameter_name: String,
}

impl ManagedSecretRef {
    pub fn placeholder(&self) -> String {
        format!(
            "${{secret:{}/{}/{}}}",
            PROVIDER_SECRET_DIRECTORY, self.provider_id, self.parameter_name
        )
    }
}

/// File-backed storage for secrets entered through the Provider setup UI.
///
/// The store intentionally provides only instance-scoped files. It is not a
/// system credential store and does not encrypt values; it merely keeps them
/// separate from the main config and makes resolver-compatible references.
#[derive(Debug, Clone)]
pub struct ManagedSecretStore {
    config_dir: PathBuf,
}

impl ManagedSecretStore {
    pub fn new(config_dir: impl Into<PathBuf>) -> Self {
        Self {
            config_dir: config_dir.into(),
        }
    }

    pub fn write(
        &self,
        provider_id: &str,
        parameter_name: &str,
        value: &str,
    ) -> Result<ManagedSecretRef, String> {
        let path = managed_secret_path(&self.config_dir, provider_id, parameter_name)?;
        let parent = path
            .parent()
            .ok_or_else(|| "Unable to resolve managed secret directory".to_string())?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("Unable to create managed secret directory: {error}"))?;

        let temp_path = temporary_path(&path);
        let write_result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)
                .map_err(|error| format!("Unable to create managed secret file: {error}"))?;
            file.write_all(value.as_bytes())
                .map_err(|error| format!("Unable to write managed secret file: {error}"))?;
            file.sync_all()
                .map_err(|error| format!("Unable to finalize managed secret file: {error}"))?;
            drop(file);
            // `rename` replaces an existing target atomically on the supported
            // platform filesystems. The temporary file prevents partial values
            // from becoming visible to the Provider runner.
            fs::rename(&temp_path, &path)
                .map_err(|error| format!("Unable to replace managed secret file: {error}"))
        })();

        if write_result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        write_result?;

        Ok(ManagedSecretRef {
            provider_id: provider_id.to_string(),
            parameter_name: parameter_name.to_string(),
        })
    }

    pub fn exists(&self, provider_id: &str, parameter_name: &str) -> Result<bool, String> {
        Ok(managed_secret_path(&self.config_dir, provider_id, parameter_name)?.exists())
    }

    /// Reads the exact existing value only for an in-process config-save rollback.
    /// Callers must never serialize or log the returned value.
    pub(crate) fn read_for_rollback(
        &self,
        provider_id: &str,
        parameter_name: &str,
    ) -> Result<Option<String>, String> {
        let path = managed_secret_path(&self.config_dir, provider_id, parameter_name)?;
        match fs::read_to_string(path) {
            Ok(value) => Ok(Some(value)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(format!("Unable to read managed secret file: {error}")),
        }
    }

    /// Restores a value captured by `read_for_rollback` after a config write
    /// failure. The value remains process-local and is never included in errors.
    pub(crate) fn restore_for_rollback(
        &self,
        provider_id: &str,
        parameter_name: &str,
        previous_value: Option<&str>,
    ) -> Result<(), String> {
        match previous_value {
            Some(value) => self.write(provider_id, parameter_name, value).map(|_| ()),
            None => self.delete(provider_id, parameter_name),
        }
    }

    pub fn delete(&self, provider_id: &str, parameter_name: &str) -> Result<(), String> {
        let path = managed_secret_path(&self.config_dir, provider_id, parameter_name)?;
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("Unable to delete managed secret file: {error}")),
        }
    }

    pub fn delete_provider_secrets(&self, provider_id: &str) -> Result<(), String> {
        let provider_dir = managed_provider_secret_dir(&self.config_dir, provider_id)?;
        match fs::remove_dir_all(&provider_dir) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!(
                "Unable to delete managed Provider secrets: {error}"
            )),
        }
    }
}

pub fn managed_provider_secret_dir(
    config_dir: &Path,
    provider_id: &str,
) -> Result<PathBuf, String> {
    validate_path_component(provider_id, "Provider instance id")?;
    Ok(config_dir
        .join(SECRET_DIRECTORY)
        .join(PROVIDER_SECRET_DIRECTORY)
        .join(provider_id))
}

pub fn managed_secret_path(
    config_dir: &Path,
    provider_id: &str,
    parameter_name: &str,
) -> Result<PathBuf, String> {
    validate_path_component(provider_id, "Provider instance id")?;
    validate_path_component(parameter_name, "Provider parameter name")?;
    Ok(managed_provider_secret_dir(config_dir, provider_id)?.join(format!("{parameter_name}.txt")))
}

pub fn managed_secret_path_from_reference(
    config_dir: &Path,
    name: &str,
) -> Option<Result<PathBuf, String>> {
    let mut parts = name.split('/');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(PROVIDER_SECRET_DIRECTORY), Some(provider_id), Some(parameter_name), None) => {
            Some(managed_secret_path(config_dir, provider_id, parameter_name))
        }
        (Some(PROVIDER_SECRET_DIRECTORY), ..) => {
            Some(Err("Invalid managed Provider secret reference".to_string()))
        }
        _ => None,
    }
}

fn validate_path_component(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        return Err(format!("Invalid {label}"));
    }
    Ok(())
}

fn temporary_path(path: &Path) -> PathBuf {
    let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("secret.txt");
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
    fn writes_isolated_provider_secret_and_returns_resolver_reference() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = ManagedSecretStore::new(temp.path());

        let first = store
            .write("kimi-coding", "KIMI_API_KEY", "first-secret")
            .expect("write first secret");
        store
            .write("kimi-coding-2", "KIMI_API_KEY", "second-secret")
            .expect("write second secret");

        assert_eq!(
            first.placeholder(),
            "${secret:providers/kimi-coding/KIMI_API_KEY}"
        );
        assert_eq!(
            fs::read_to_string(
                temp.path()
                    .join("secrets/providers/kimi-coding/KIMI_API_KEY.txt")
            )
            .expect("read first secret"),
            "first-secret"
        );
        assert_eq!(
            fs::read_to_string(
                temp.path()
                    .join("secrets/providers/kimi-coding-2/KIMI_API_KEY.txt")
            )
            .expect("read second secret"),
            "second-secret"
        );
    }

    #[test]
    fn overwrites_secret_without_appending_newline() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = ManagedSecretStore::new(temp.path());
        store
            .write("provider-a", "API_KEY", "old")
            .expect("write old secret");
        store
            .write("provider-a", "API_KEY", "replacement")
            .expect("replace secret");

        assert_eq!(
            fs::read_to_string(temp.path().join("secrets/providers/provider-a/API_KEY.txt"))
                .expect("read replacement"),
            "replacement"
        );
    }

    #[test]
    fn rejects_path_injection_without_echoing_secret_value() {
        let temp = tempfile::tempdir().expect("temp dir");
        let error = ManagedSecretStore::new(temp.path())
            .write("../outside", "API_KEY", "secret-value-that-must-not-leak")
            .expect_err("path injection must fail");

        assert!(error.contains("Invalid Provider instance id"));
        assert!(!error.contains("secret-value-that-must-not-leak"));
        assert!(!temp.path().join("outside").exists());
    }

    #[test]
    fn deletes_single_secret_or_the_whole_provider_directory() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = ManagedSecretStore::new(temp.path());
        store
            .write("provider-a", "TOKEN", "one")
            .expect("write token");
        store
            .write("provider-a", "ACCOUNT", "two")
            .expect("write account");

        store.delete("provider-a", "TOKEN").expect("delete token");
        assert!(!store.exists("provider-a", "TOKEN").expect("token exists"));
        assert!(store
            .exists("provider-a", "ACCOUNT")
            .expect("account exists"));

        store
            .delete_provider_secrets("provider-a")
            .expect("delete provider secrets");
        assert!(!temp.path().join("secrets/providers/provider-a").exists());
    }
}
