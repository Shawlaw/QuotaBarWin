use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use crate::config::SecretStorageMode;
use crate::secret_encryption;

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
/// The store intentionally provides only instance-scoped writes; it is not a
/// system credential store. When the configured storage mode is `Encrypted`,
/// values are sealed with Windows DPAPI bound to the current user and to the
/// secret reference; in `Plaintext` mode values are written as-is for
/// installations that have not opted in. Reads always accept both formats so
/// mixed state during migration keeps resolving. Batch mode migrations also
/// cover user-managed `secrets/NAME.txt` files so the whole secrets directory
/// follows one storage mode.
#[derive(Debug, Clone)]
pub struct ManagedSecretStore {
    config_dir: PathBuf,
    storage_mode: SecretStorageMode,
}

impl ManagedSecretStore {
    pub fn new(config_dir: impl Into<PathBuf>, storage_mode: SecretStorageMode) -> Self {
        Self {
            config_dir: config_dir.into(),
            storage_mode,
        }
    }

    pub fn write(
        &self,
        provider_id: &str,
        parameter_name: &str,
        value: &str,
    ) -> Result<ManagedSecretRef, String> {
        let path = managed_secret_path(&self.config_dir, provider_id, parameter_name)?;
        let reference = managed_secret_reference(provider_id, parameter_name);
        let payload = match self.storage_mode {
            SecretStorageMode::Encrypted => {
                secret_encryption::encrypt_managed_secret(value, &reference)?
            }
            SecretStorageMode::Plaintext => value.as_bytes().to_vec(),
        };
        write_payload_atomic(&path, &payload)?;

        Ok(ManagedSecretRef {
            provider_id: provider_id.to_string(),
            parameter_name: parameter_name.to_string(),
        })
    }

    pub fn exists(&self, provider_id: &str, parameter_name: &str) -> Result<bool, String> {
        Ok(managed_secret_path(&self.config_dir, provider_id, parameter_name)?.exists())
    }

    /// Reads the exact existing bytes only for an in-process config-save
    /// rollback, so restoring reproduces the on-disk format (encrypted or
    /// plaintext) without re-encrypting. Callers must never serialize or log
    /// the returned bytes.
    pub(crate) fn read_for_rollback(
        &self,
        provider_id: &str,
        parameter_name: &str,
    ) -> Result<Option<Vec<u8>>, String> {
        let path = managed_secret_path(&self.config_dir, provider_id, parameter_name)?;
        match fs::read(path) {
            Ok(value) => Ok(Some(value)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(format!("Unable to read managed secret file: {error}")),
        }
    }

    /// Restores bytes captured by `read_for_rollback` after a config write
    /// failure. The bytes remain process-local and are never included in
    /// errors.
    pub(crate) fn restore_for_rollback(
        &self,
        provider_id: &str,
        parameter_name: &str,
        previous_bytes: Option<&[u8]>,
    ) -> Result<(), String> {
        match previous_bytes {
            Some(bytes) => {
                let path = managed_secret_path(&self.config_dir, provider_id, parameter_name)?;
                ensure_parent(&path)?;
                write_payload_atomic(&path, bytes)
            }
            None => self.delete(provider_id, parameter_name),
        }
    }

    pub fn delete(&self, provider_id: &str, parameter_name: &str) -> Result<(), String> {
        let path = managed_secret_path(&self.config_dir, provider_id, parameter_name)?;
        match fs::remove_file(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            result => result.map_err(|error| format!("Unable to delete managed secret file: {error}")),
        }
    }

    pub fn delete_provider_secrets(&self, provider_id: &str) -> Result<(), String> {
        let provider_dir = managed_provider_secret_dir(&self.config_dir, provider_id)?;
        match fs::remove_dir_all(&provider_dir) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            result => result.map_err(|error| {
                format!("Unable to delete managed Provider secrets: {error}")
            }),
        }
    }

    /// Enumerates every managed Provider secret file currently on disk in a
    /// deterministic order. Temporary files and names that could not have been
    /// produced by this store are ignored.
    pub fn list_provider_secret_entries(&self) -> Result<Vec<ManagedSecretEntry>, String> {
        let root = self
            .config_dir
            .join(SECRET_DIRECTORY)
            .join(PROVIDER_SECRET_DIRECTORY);
        let mut entries = Vec::new();
        let provider_dirs = match fs::read_dir(&root) {
            Ok(dirs) => dirs,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(entries),
            Err(error) => {
                return Err(format!("Unable to list managed Provider secrets: {error}"))
            }
        };
        for provider_dir in provider_dirs {
            let provider_dir = provider_dir
                .map_err(|error| format!("Unable to list managed Provider secrets: {error}"))?;
            if !provider_dir
                .file_type()
                .map_err(|error| error.to_string())?
                .is_dir()
            {
                continue;
            }
            let provider_id = provider_dir.file_name().to_string_lossy().to_string();
            if validate_path_component(&provider_id, "Provider instance id").is_err() {
                continue;
            }
            let files = fs::read_dir(provider_dir.path())
                .map_err(|error| format!("Unable to list managed Provider secrets: {error}"))?;
            for file in files {
                let file = file
                    .map_err(|error| format!("Unable to list managed Provider secrets: {error}"))?;
                if !file.file_type().map_err(|error| error.to_string())?.is_file() {
                    continue;
                }
                let file_name = file.file_name().to_string_lossy().to_string();
                let Some(parameter_name) = file_name.strip_suffix(".txt") else {
                    continue;
                };
                if parameter_name.is_empty() || file_name.starts_with('.') {
                    continue;
                }
                if validate_path_component(parameter_name, "Provider parameter name").is_err() {
                    continue;
                }
                entries.push(ManagedSecretEntry {
                    path: file.path(),
                    provider_id: provider_id.clone(),
                    parameter_name: parameter_name.to_string(),
                });
            }
        }
        entries.sort_by(|left, right| {
            (&left.provider_id, &left.parameter_name).cmp(&(&right.provider_id, &right.parameter_name))
        });
        Ok(entries)
    }

    /// Enumerates every user-managed secret file (`secrets/NAME.txt`) on disk
    /// in a deterministic order. Only names that the secret resolver could
    /// actually reference are returned, so unrelated files users parked in the
    /// secrets directory are left alone.
    pub fn list_user_secret_entries(&self) -> Result<Vec<UserSecretEntry>, String> {
        let root = self.config_dir.join(SECRET_DIRECTORY);
        let mut entries = Vec::new();
        let files = match fs::read_dir(&root) {
            Ok(files) => files,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(entries),
            Err(error) => return Err(format!("Unable to list user secret files: {error}")),
        };
        for file in files {
            let file =
                file.map_err(|error| format!("Unable to list user secret files: {error}"))?;
            if !file.file_type().map_err(|error| error.to_string())?.is_file() {
                continue;
            }
            let file_name = file.file_name().to_string_lossy().to_string();
            let Some(name) = file_name.strip_suffix(".txt") else {
                continue;
            };
            // `${secret:providers}` is parsed as an invalid managed Provider
            // reference, so a file with that exact name is unreachable.
            if name.is_empty()
                || file_name.starts_with('.')
                || name == PROVIDER_SECRET_DIRECTORY
                || !crate::config::is_valid_secret_name(name)
            {
                continue;
            }
            entries.push(UserSecretEntry {
                name: name.to_string(),
                path: file.path(),
            });
        }
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(entries)
    }

    /// Re-encrypts every plaintext secret file on disk, covering both
    /// application-managed Provider secrets and user-managed
    /// `secrets/NAME.txt` files. Returns the number of migrated files.
    /// Already-encrypted files are skipped, so partial runs can be retried.
    pub fn encrypt_plaintext_secrets(&self) -> Result<u32, String> {
        let mut migrated = 0;
        for entry in self.list_provider_secret_entries()? {
            migrated += u32::from(encrypt_entry_if_plaintext(
                &entry.path,
                &entry.reference_name(),
                "managed Provider secret",
            )?);
        }
        for entry in self.list_user_secret_entries()? {
            migrated += u32::from(encrypt_entry_if_plaintext(
                &entry.path,
                &entry.name,
                "user secret file",
            )?);
        }
        Ok(migrated)
    }

    /// Decrypts every encrypted secret file back to plaintext files for
    /// installations that opt out of encryption, covering both managed
    /// Provider secrets and user-managed files. Returns the number of
    /// decrypted files.
    pub fn decrypt_encrypted_secrets(&self) -> Result<u32, String> {
        let mut decrypted = 0;
        for entry in self.list_provider_secret_entries()? {
            decrypted += u32::from(decrypt_entry_if_encrypted(
                &entry.path,
                &entry.reference_name(),
                "managed Provider secret",
            )?);
        }
        for entry in self.list_user_secret_entries()? {
            decrypted += u32::from(decrypt_entry_if_encrypted(
                &entry.path,
                &entry.name,
                "user secret file",
            )?);
        }
        Ok(decrypted)
    }
}

/// A discovered managed Provider secret file on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedSecretEntry {
    pub provider_id: String,
    pub parameter_name: String,
    pub path: PathBuf,
}

impl ManagedSecretEntry {
    pub fn reference_name(&self) -> String {
        managed_secret_reference(&self.provider_id, &self.parameter_name)
    }
}

/// A discovered user-managed secret file (`secrets/NAME.txt`) on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserSecretEntry {
    pub name: String,
    pub path: PathBuf,
}

pub fn managed_secret_reference(provider_id: &str, parameter_name: &str) -> String {
    format!("{PROVIDER_SECRET_DIRECTORY}/{provider_id}/{parameter_name}")
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

/// Encrypts one secret file in place if it currently holds plaintext bytes.
/// Returns whether the file was migrated. Empty files are left alone because
/// they resolve as "missing" anyway.
fn encrypt_entry_if_plaintext(path: &Path, reference: &str, label: &str) -> Result<bool, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("Unable to read {label} {reference}: {error}"))?;
    if secret_encryption::is_encrypted_payload(&bytes) || bytes.is_empty() {
        return Ok(false);
    }
    let value = String::from_utf8(bytes)
        .map_err(|_| format!("Unable to encrypt {label} {reference}: value is not valid UTF-8"))?;
    let payload = secret_encryption::encrypt_managed_secret(&value, reference)?;
    write_payload_atomic(path, &payload)?;
    Ok(true)
}

/// Decrypts one secret file in place if it currently holds an encrypted
/// payload. Returns whether the file was restored to plaintext.
fn decrypt_entry_if_encrypted(path: &Path, reference: &str, label: &str) -> Result<bool, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("Unable to read {label} {reference}: {error}"))?;
    if !secret_encryption::is_encrypted_payload(&bytes) {
        return Ok(false);
    }
    let value = secret_encryption::decrypt_managed_secret(&bytes, reference)?;
    write_payload_atomic(path, value.as_bytes())?;
    Ok(true)
}

fn ensure_parent(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Unable to resolve managed secret directory".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Unable to create managed secret directory: {error}"))
}

fn write_payload_atomic(path: &Path, payload: &[u8]) -> Result<(), String> {
    ensure_parent(path)?;
    let temp_path = temporary_path(path);
    let write_result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|error| format!("Unable to create managed secret file: {error}"))?;
        file.write_all(payload)
            .map_err(|error| format!("Unable to write managed secret file: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("Unable to finalize managed secret file: {error}"))?;
        drop(file);
        // `rename` replaces an existing target atomically on the supported
        // platform filesystems. The temporary file prevents partial values
        // from becoming visible to the Provider runner.
        fs::rename(&temp_path, path)
            .map_err(|error| format!("Unable to replace managed secret file: {error}"))
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    write_result
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
    use crate::secret_encryption::decode_managed_secret_payload;

    const PLAINTEXT: SecretStorageMode = SecretStorageMode::Plaintext;
    const ENCRYPTED: SecretStorageMode = SecretStorageMode::Encrypted;

    fn secret_path(temp: &tempfile::TempDir, provider: &str, parameter: &str) -> PathBuf {
        temp.path()
            .join(SECRET_DIRECTORY)
            .join(PROVIDER_SECRET_DIRECTORY)
            .join(provider)
            .join(format!("{parameter}.txt"))
    }

    #[test]
    fn writes_isolated_provider_secret_and_returns_resolver_reference() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = ManagedSecretStore::new(temp.path(), PLAINTEXT);

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
            fs::read_to_string(secret_path(&temp, "kimi-coding", "KIMI_API_KEY"))
                .expect("read first secret"),
            "first-secret"
        );
        assert_eq!(
            fs::read_to_string(secret_path(&temp, "kimi-coding-2", "KIMI_API_KEY"))
                .expect("read second secret"),
            "second-secret"
        );
    }

    #[test]
    fn writes_dpapi_encrypted_payload_when_encryption_enabled() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = ManagedSecretStore::new(temp.path(), ENCRYPTED);

        store
            .write("kimi-coding", "KIMI_API_KEY", "secret-value")
            .expect("write encrypted secret");

        let bytes = fs::read(secret_path(&temp, "kimi-coding", "KIMI_API_KEY")).expect("ciphertext");
        assert!(secret_encryption::is_encrypted_payload(&bytes));
        assert!(!bytes.windows(12).any(|w| *w == b"secret-value"[..]));
        assert_eq!(
            decode_managed_secret_payload(&bytes, "providers/kimi-coding/KIMI_API_KEY")
                .expect("decrypt"),
            "secret-value"
        );
    }

    #[test]
    fn encrypted_ciphertext_cannot_be_swapped_between_instances() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = ManagedSecretStore::new(temp.path(), ENCRYPTED);
        store
            .write("provider-a", "API_KEY", "secret-value")
            .expect("write secret");

        let bytes = fs::read(secret_path(&temp, "provider-a", "API_KEY")).expect("ciphertext");
        fs::create_dir_all(
            temp.path()
                .join(SECRET_DIRECTORY)
                .join(PROVIDER_SECRET_DIRECTORY)
                .join("provider-b"),
        )
        .expect("other instance dir");
        fs::write(secret_path(&temp, "provider-b", "API_KEY"), &bytes).expect("copy ciphertext");

        assert!(secret_encryption::decrypt_managed_secret(
            &fs::read(secret_path(&temp, "provider-b", "API_KEY")).expect("copy"),
            "providers/provider-b/API_KEY"
        )
        .is_err());
    }

    #[test]
    fn overwrites_secret_without_appending_newline() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = ManagedSecretStore::new(temp.path(), PLAINTEXT);
        store
            .write("provider-a", "API_KEY", "old")
            .expect("write old secret");
        store
            .write("provider-a", "API_KEY", "replacement")
            .expect("replace secret");

        assert_eq!(
            fs::read_to_string(secret_path(&temp, "provider-a", "API_KEY"))
                .expect("read replacement"),
            "replacement"
        );
    }

    #[test]
    fn rejects_path_injection_without_echoing_secret_value() {
        let temp = tempfile::tempdir().expect("temp dir");
        let error = ManagedSecretStore::new(temp.path(), PLAINTEXT)
            .write("../outside", "API_KEY", "secret-value-that-must-not-leak")
            .expect_err("path injection must fail");

        assert!(error.contains("Invalid Provider instance id"));
        assert!(!error.contains("secret-value-that-must-not-leak"));
        assert!(!temp.path().join("outside").exists());
    }

    #[test]
    fn deletes_single_secret_or_the_whole_provider_directory() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = ManagedSecretStore::new(temp.path(), PLAINTEXT);
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

    #[test]
    fn rollback_captures_and_restores_exact_bytes_across_formats() {
        let temp = tempfile::tempdir().expect("temp dir");
        let encrypted_store = ManagedSecretStore::new(temp.path(), ENCRYPTED);
        encrypted_store
            .write("provider-a", "API_KEY", "encrypted-secret")
            .expect("write encrypted");

        let captured = encrypted_store
            .read_for_rollback("provider-a", "API_KEY")
            .expect("capture")
            .expect("present");
        assert_eq!(
            captured,
            fs::read(secret_path(&temp, "provider-a", "API_KEY")).expect("on-disk bytes")
        );

        encrypted_store
            .delete("provider-a", "API_KEY")
            .expect("delete");
        encrypted_store
            .restore_for_rollback("provider-a", "API_KEY", Some(&captured))
            .expect("restore");
        assert_eq!(
            fs::read(secret_path(&temp, "provider-a", "API_KEY")).expect("restored bytes"),
            captured
        );
    }

    #[test]
    fn encrypt_plaintext_secrets_migrates_files_and_is_idempotent() {
        let temp = tempfile::tempdir().expect("temp dir");
        let plaintext_store = ManagedSecretStore::new(temp.path(), PLAINTEXT);
        plaintext_store
            .write("provider-a", "API_KEY", "one")
            .expect("write one");
        plaintext_store
            .write("provider-b", "TOKEN", "two")
            .expect("write two");

        let migrated = ManagedSecretStore::new(temp.path(), ENCRYPTED)
            .encrypt_plaintext_secrets()
            .expect("migrate");
        assert_eq!(migrated, 2);

        for (provider, parameter, value) in
            [("provider-a", "API_KEY", "one"), ("provider-b", "TOKEN", "two")]
        {
            let bytes = fs::read(secret_path(&temp, provider, parameter)).expect("payload");
            assert!(secret_encryption::is_encrypted_payload(&bytes));
            assert_eq!(
                decode_managed_secret_payload(
                    &bytes,
                    &managed_secret_reference(provider, parameter)
                )
                .expect("decrypt"),
                value
            );
        }

        let migrated_again = ManagedSecretStore::new(temp.path(), ENCRYPTED)
            .encrypt_plaintext_secrets()
            .expect("migrate again");
        assert_eq!(migrated_again, 0);
    }

    #[test]
    fn decrypt_encrypted_secrets_restores_plaintext_files() {
        let temp = tempfile::tempdir().expect("temp dir");
        let encrypted_store = ManagedSecretStore::new(temp.path(), ENCRYPTED);
        encrypted_store
            .write("provider-a", "API_KEY", "one")
            .expect("write one");

        let decrypted = ManagedSecretStore::new(temp.path(), PLAINTEXT)
            .decrypt_encrypted_secrets()
            .expect("decrypt all");
        assert_eq!(decrypted, 1);
        assert_eq!(
            fs::read_to_string(secret_path(&temp, "provider-a", "API_KEY")).expect("plaintext"),
            "one"
        );

        let decrypted_again = ManagedSecretStore::new(temp.path(), PLAINTEXT)
            .decrypt_encrypted_secrets()
            .expect("decrypt again");
        assert_eq!(decrypted_again, 0);
    }

    #[test]
    fn list_provider_secret_entries_ignores_temp_files_and_invalid_names() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = ManagedSecretStore::new(temp.path(), PLAINTEXT);
        store
            .write("provider-a", "API_KEY", "one")
            .expect("write one");
        let path = secret_path(&temp, "provider-a", "API_KEY");
        fs::write(
            path.with_file_name(".API_KEY.123.0.tmp"),
            "partial",
        )
        .expect("temp file");
        fs::create_dir_all(
            temp.path()
                .join(SECRET_DIRECTORY)
                .join(PROVIDER_SECRET_DIRECTORY)
                .join("provider-a")
                .join("nested"),
        )
        .expect("nested dir");

        let entries = store.list_provider_secret_entries().expect("entries");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].provider_id, "provider-a");
        assert_eq!(entries[0].parameter_name, "API_KEY");
        assert_eq!(entries[0].path, path);
    }

    #[test]
    fn list_provider_secret_entries_handles_missing_store() {
        let temp = tempfile::tempdir().expect("temp dir");
        assert!(ManagedSecretStore::new(temp.path(), ENCRYPTED)
            .list_provider_secret_entries()
            .expect("entries")
            .is_empty());
    }

    fn user_secret_path(temp: &tempfile::TempDir, name: &str) -> PathBuf {
        temp.path().join(SECRET_DIRECTORY).join(format!("{name}.txt"))
    }

    #[test]
    fn list_user_secret_entries_returns_only_referencable_files() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = ManagedSecretStore::new(temp.path(), PLAINTEXT);
        store
            .write("provider-a", "API_KEY", "one")
            .expect("write managed secret");
        let secret_dir = temp.path().join(SECRET_DIRECTORY);
        fs::write(secret_dir.join("USER_KEY.txt"), "value").expect("valid user file");
        fs::write(secret_dir.join(".USER_KEY.123.0.tmp"), "partial").expect("temp file");
        fs::write(secret_dir.join("dashed-name.txt"), "value").expect("unreachable name");
        fs::write(secret_dir.join("providers.txt"), "value").expect("reserved name");
        fs::write(secret_dir.join("notes.md"), "value").expect("non-secret file");

        let entries = store.list_user_secret_entries().expect("entries");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "USER_KEY");
        assert_eq!(entries[0].path, user_secret_path(&temp, "USER_KEY"));
    }

    #[test]
    fn list_user_secret_entries_handles_missing_store() {
        let temp = tempfile::tempdir().expect("temp dir");
        assert!(ManagedSecretStore::new(temp.path(), ENCRYPTED)
            .list_user_secret_entries()
            .expect("entries")
            .is_empty());
    }

    #[test]
    fn encrypt_plaintext_secrets_migrates_user_files_bound_to_name() {
        let temp = tempfile::tempdir().expect("temp dir");
        fs::create_dir_all(temp.path().join(SECRET_DIRECTORY)).expect("secrets dir");
        fs::write(user_secret_path(&temp, "USER_KEY"), "user-value").expect("user file");
        ManagedSecretStore::new(temp.path(), PLAINTEXT)
            .write("provider-a", "API_KEY", "managed-value")
            .expect("managed file");

        let migrated = ManagedSecretStore::new(temp.path(), ENCRYPTED)
            .encrypt_plaintext_secrets()
            .expect("migrate");
        assert_eq!(migrated, 2);

        let bytes = fs::read(user_secret_path(&temp, "USER_KEY")).expect("user payload");
        assert!(secret_encryption::is_encrypted_payload(&bytes));
        assert!(!bytes.windows(11).any(|w| *w == b"user-value"[..]));
        assert_eq!(
            decode_managed_secret_payload(&bytes, "USER_KEY").expect("decrypt by name"),
            "user-value"
        );
        assert!(secret_encryption::decrypt_managed_secret(&bytes, "OTHER_NAME").is_err());

        let migrated_again = ManagedSecretStore::new(temp.path(), ENCRYPTED)
            .encrypt_plaintext_secrets()
            .expect("migrate again");
        assert_eq!(migrated_again, 0);
    }

    #[test]
    fn encrypt_plaintext_secrets_leaves_empty_user_files_untouched() {
        let temp = tempfile::tempdir().expect("temp dir");
        fs::create_dir_all(temp.path().join(SECRET_DIRECTORY)).expect("secrets dir");
        fs::write(user_secret_path(&temp, "EMPTY_KEY"), "").expect("empty user file");

        let migrated = ManagedSecretStore::new(temp.path(), ENCRYPTED)
            .encrypt_plaintext_secrets()
            .expect("migrate");

        assert_eq!(migrated, 0);
        assert_eq!(
            fs::read_to_string(user_secret_path(&temp, "EMPTY_KEY")).expect("empty file"),
            ""
        );
    }

    #[test]
    fn decrypt_encrypted_secrets_restores_user_files() {
        let temp = tempfile::tempdir().expect("temp dir");
        fs::create_dir_all(temp.path().join(SECRET_DIRECTORY)).expect("secrets dir");
        let encrypted = secret_encryption::encrypt_managed_secret("user-value", "USER_KEY")
            .expect("encrypt");
        fs::write(user_secret_path(&temp, "USER_KEY"), &encrypted).expect("user file");

        let decrypted = ManagedSecretStore::new(temp.path(), PLAINTEXT)
            .decrypt_encrypted_secrets()
            .expect("decrypt all");
        assert_eq!(decrypted, 1);
        assert_eq!(
            fs::read_to_string(user_secret_path(&temp, "USER_KEY")).expect("plaintext"),
            "user-value"
        );

        let decrypted_again = ManagedSecretStore::new(temp.path(), PLAINTEXT)
            .decrypt_encrypted_secrets()
            .expect("decrypt again");
        assert_eq!(decrypted_again, 0);
    }
}
