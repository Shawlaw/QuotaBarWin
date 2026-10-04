use std::path::Path;

use serde::Serialize;
use tauri::AppHandle;

use crate::config::{
    config_path_for_app, load_or_create_config, save_config_to_path, SecretStorageMode,
};
use crate::managed_secret_store::ManagedSecretStore;
use crate::secret_encryption;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ManagedSecretsEncryptionStatus {
    pub storage: SecretStorageMode,
    pub prompt_pending: bool,
    pub plaintext_count: u32,
    pub encrypted_count: u32,
}

pub fn managed_secrets_encryption_status_for_path(
    config_path: &Path,
) -> Result<ManagedSecretsEncryptionStatus, String> {
    let config = load_or_create_config(config_path)?.config;
    let config_dir = config_dir_for(config_path)?;
    let (plaintext_count, encrypted_count) = classify_provider_secrets(config_dir)?;

    Ok(ManagedSecretsEncryptionStatus {
        storage: config.secrets_storage,
        prompt_pending: config.secrets_encryption_prompt_pending,
        plaintext_count,
        encrypted_count,
    })
}

pub fn enable_managed_secrets_encryption_for_path(config_path: &Path) -> Result<u32, String> {
    let config_dir = config_dir_for(config_path)?;
    let store = ManagedSecretStore::new(config_dir, SecretStorageMode::Encrypted);
    let migrated = store.encrypt_plaintext_secrets()?;
    update_config(config_path, |config| {
        config.secrets_storage = SecretStorageMode::Encrypted;
        config.secrets_encryption_prompt_pending = false;
    })?;
    Ok(migrated)
}

pub fn disable_managed_secrets_encryption_for_path(config_path: &Path) -> Result<u32, String> {
    let config_dir = config_dir_for(config_path)?;
    let store = ManagedSecretStore::new(config_dir, SecretStorageMode::Plaintext);
    let decrypted = store.decrypt_encrypted_secrets()?;
    update_config(config_path, |config| {
        config.secrets_storage = SecretStorageMode::Plaintext;
        config.secrets_encryption_prompt_pending = false;
    })?;
    Ok(decrypted)
}

pub fn dismiss_managed_secrets_encryption_prompt_for_path(
    config_path: &Path,
) -> Result<(), String> {
    update_config(config_path, |config| {
        config.secrets_encryption_prompt_pending = false;
    })
}

#[tauri::command]
pub async fn get_managed_secrets_encryption_status(
    app: AppHandle,
) -> Result<ManagedSecretsEncryptionStatus, String> {
    let path = config_path_for_app(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        managed_secrets_encryption_status_for_path(&path)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn enable_managed_secrets_encryption(app: AppHandle) -> Result<u32, String> {
    let path = config_path_for_app(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        enable_managed_secrets_encryption_for_path(&path)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn disable_managed_secrets_encryption(app: AppHandle) -> Result<u32, String> {
    let path = config_path_for_app(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        disable_managed_secrets_encryption_for_path(&path)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn dismiss_managed_secrets_encryption_prompt(app: AppHandle) -> Result<(), String> {
    let path = config_path_for_app(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        dismiss_managed_secrets_encryption_prompt_for_path(&path)
    })
    .await
    .map_err(|error| error.to_string())?
}

fn config_dir_for(config_path: &Path) -> Result<&Path, String> {
    config_path
        .parent()
        .ok_or_else(|| "Unable to resolve config directory".to_string())
}

fn classify_provider_secrets(config_dir: &Path) -> Result<(u32, u32), String> {
    let store = ManagedSecretStore::new(config_dir, SecretStorageMode::Encrypted);
    let mut plaintext_count = 0;
    let mut encrypted_count = 0;
    let mut classify = |path: &std::path::Path, label: &str| -> Result<(), String> {
        let bytes = std::fs::read(path)
            .map_err(|error| format!("Unable to read {label}: {error}"))?;
        if secret_encryption::is_encrypted_payload(&bytes) {
            encrypted_count += 1;
        } else {
            plaintext_count += 1;
        }
        Ok(())
    };
    for entry in store.list_provider_secret_entries()? {
        classify(&entry.path, &format!("managed Provider secret {}", entry.reference_name()))?;
    }
    for entry in store.list_user_secret_entries()? {
        classify(&entry.path, &format!("user secret file {}", entry.name))?;
    }
    Ok((plaintext_count, encrypted_count))
}

fn update_config(
    config_path: &Path,
    apply: impl FnOnce(&mut crate::config::AppConfig),
) -> Result<(), String> {
    let mut config = load_or_create_config(config_path)?.config;
    apply(&mut config);
    save_config_to_path(config_path, &config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::default_config;

    fn plaintext_upgrade_config(temp: &tempfile::TempDir) -> std::path::PathBuf {
        let path = temp.path().join("config.quotaBarWin.json");
        let mut config = default_config();
        config.secrets_storage = SecretStorageMode::Plaintext;
        config.secrets_encryption_prompt_pending = true;
        save_config_to_path(&path, &config).expect("save config");
        path
    }

    fn read_storage(config_path: &std::path::Path) -> (SecretStorageMode, bool) {
        let config = load_or_create_config(config_path).expect("load config").config;
        (config.secrets_storage, config.secrets_encryption_prompt_pending)
    }

    fn secret_bytes(temp: &tempfile::TempDir, provider: &str, parameter: &str) -> Vec<u8> {
        std::fs::read(
            temp.path()
                .join("secrets")
                .join("providers")
                .join(provider)
                .join(format!("{parameter}.txt")),
        )
        .expect("secret file")
    }

    #[test]
    fn status_counts_plaintext_and_encrypted_secrets() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = plaintext_upgrade_config(&temp);
        let plaintext_store =
            ManagedSecretStore::new(temp.path(), SecretStorageMode::Plaintext);
        plaintext_store
            .write("provider-a", "API_KEY", "one")
            .expect("plaintext secret");
        ManagedSecretStore::new(temp.path(), SecretStorageMode::Encrypted)
            .write("provider-b", "TOKEN", "two")
            .expect("encrypted secret");
        std::fs::create_dir_all(temp.path().join("secrets")).expect("secrets dir");
        std::fs::write(temp.path().join("secrets").join("USER_KEY.txt"), "user-one")
            .expect("plaintext user secret");

        let status =
            managed_secrets_encryption_status_for_path(&path).expect("status");

        assert_eq!(status.storage, SecretStorageMode::Plaintext);
        assert!(status.prompt_pending);
        assert_eq!(status.plaintext_count, 2);
        assert_eq!(status.encrypted_count, 1);
    }

    #[test]
    fn enable_migrates_files_and_updates_config() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = plaintext_upgrade_config(&temp);
        ManagedSecretStore::new(temp.path(), SecretStorageMode::Plaintext)
            .write("provider-a", "API_KEY", "one")
            .expect("plaintext secret");
        std::fs::create_dir_all(temp.path().join("secrets")).expect("secrets dir");
        let user_secret = temp.path().join("secrets").join("USER_KEY.txt");
        std::fs::write(&user_secret, "user-one").expect("plaintext user secret");

        let migrated =
            enable_managed_secrets_encryption_for_path(&path).expect("enable");

        assert_eq!(migrated, 2);
        assert!(secret_encryption::is_encrypted_payload(&secret_bytes(
            &temp, "provider-a", "API_KEY"
        )));
        assert!(secret_encryption::is_encrypted_payload(
            &std::fs::read(&user_secret).expect("user secret file")
        ));
        assert_eq!(
            read_storage(&path),
            (SecretStorageMode::Encrypted, false)
        );
    }

    #[test]
    fn disable_decrypts_files_and_updates_config() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut config = default_config();
        let path = temp.path().join("config.quotaBarWin.json");
        config.secrets_storage = SecretStorageMode::Encrypted;
        config.secrets_encryption_prompt_pending = false;
        save_config_to_path(&path, &config).expect("save config");
        ManagedSecretStore::new(temp.path(), SecretStorageMode::Encrypted)
            .write("provider-a", "API_KEY", "one")
            .expect("encrypted secret");

        let decrypted =
            disable_managed_secrets_encryption_for_path(&path).expect("disable");

        assert_eq!(decrypted, 1);
        assert!(!secret_encryption::is_encrypted_payload(&secret_bytes(
            &temp, "provider-a", "API_KEY"
        )));
        assert_eq!(
            String::from_utf8(secret_bytes(&temp, "provider-a", "API_KEY")).expect("utf8"),
            "one"
        );
        assert_eq!(
            read_storage(&path),
            (SecretStorageMode::Plaintext, false)
        );
    }

    #[test]
    fn dismiss_clears_prompt_without_touching_files_or_storage_mode() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = plaintext_upgrade_config(&temp);
        ManagedSecretStore::new(temp.path(), SecretStorageMode::Plaintext)
            .write("provider-a", "API_KEY", "one")
            .expect("plaintext secret");

        dismiss_managed_secrets_encryption_prompt_for_path(&path).expect("dismiss");

        assert_eq!(
            read_storage(&path),
            (SecretStorageMode::Plaintext, false)
        );
        assert!(!secret_encryption::is_encrypted_payload(&secret_bytes(
            &temp, "provider-a", "API_KEY"
        )));
    }
}
