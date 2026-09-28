//! Windows DPAPI encryption for application-managed Provider secrets.
//!
//! Encrypted payloads are bound to the current Windows user through DPAPI and
//! to their managed secret reference through the optional entropy parameter,
//! so a ciphertext file cannot be swapped between Provider instances. The
//! magic prefix lets readers distinguish encrypted payloads from legacy
//! plaintext files during and after migration.

use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};

/// Prefix that marks a managed secret file as an encrypted DPAPI payload.
const ENCRYPTED_PAYLOAD_PREFIX: &[u8] = b"QBWSEC1";

/// Scope label mixed into the DPAPI entropy so managed secret ciphertexts
/// cannot be confused with DPAPI blobs from other data.
const ENTROPY_SCOPE: &str = "QuotaBarWin:managed-secret:";

pub fn is_encrypted_payload(bytes: &[u8]) -> bool {
    bytes.starts_with(ENCRYPTED_PAYLOAD_PREFIX)
}

/// Encrypts a managed secret value for `reference` (`providers/INSTANCE/PARAM`).
///
/// The result is the magic prefix followed by the raw DPAPI blob. Errors never
/// contain the plaintext value.
pub fn encrypt_managed_secret(value: &str, reference: &str) -> Result<Vec<u8>, String> {
    let entropy = entropy_bytes(reference);
    let blob = crypt_data(true, value.as_bytes(), &entropy)
        .map_err(|error| format!("Unable to encrypt managed Provider secret {reference}: {error}"))?;
    let mut payload = ENCRYPTED_PAYLOAD_PREFIX.to_vec();
    payload.extend_from_slice(&blob);
    Ok(payload)
}

/// Decrypts a managed secret payload previously produced for `reference`.
///
/// Legacy plaintext bytes (no magic prefix) are rejected here; callers decide
/// how to treat them.
pub fn decrypt_managed_secret(payload: &[u8], reference: &str) -> Result<String, String> {
    let Some(blob) = payload.strip_prefix(ENCRYPTED_PAYLOAD_PREFIX) else {
        return Err(format!(
            "Managed Provider secret {reference} is not an encrypted payload"
        ));
    };
    let entropy = entropy_bytes(reference);
    let value = crypt_data(false, blob, &entropy).map_err(|error| {
        format!(
            "Unable to decrypt managed Provider secret {reference}: {error}; it may have been \
             encrypted for a different Windows account or copied from another machine, so \
             re-enter the secret in Provider settings"
        )
    })?;
    String::from_utf8(value)
        .map_err(|_| format!("Managed Provider secret {reference} is not valid UTF-8"))
}

/// Decodes managed secret file bytes: encrypted payloads are decrypted, legacy
/// plaintext bytes are returned as-is.
pub fn decode_managed_secret_payload(bytes: &[u8], reference: &str) -> Result<String, String> {
    if is_encrypted_payload(bytes) {
        decrypt_managed_secret(bytes, reference)
    } else {
        String::from_utf8(bytes.to_vec())
            .map_err(|_| format!("Managed Provider secret {reference} is not valid UTF-8"))
    }
}

fn entropy_bytes(reference: &str) -> Vec<u8> {
    format!("{ENTROPY_SCOPE}{reference}").into_bytes()
}

fn crypt_data(protect: bool, data: &[u8], entropy: &[u8]) -> Result<Vec<u8>, String> {
    if data.is_empty() {
        return Err("empty payload".to_string());
    }
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let optional_entropy = CRYPT_INTEGER_BLOB {
            cbData: entropy.len() as u32,
            pbData: entropy.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        let result = if protect {
            CryptProtectData(
                &input,
                std::ptr::null(),
                &optional_entropy,
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                &optional_entropy,
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if result == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let decrypted = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData.cast());
        Ok(decrypted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REFERENCE: &str = "providers/kimi-coding/KIMI_API_KEY";

    #[test]
    fn encrypts_and_decrypts_roundtrip() {
        let payload = encrypt_managed_secret("secret-value-123", REFERENCE).expect("encrypt");

        assert!(is_encrypted_payload(&payload));
        assert_eq!(
            decrypt_managed_secret(&payload, REFERENCE).expect("decrypt"),
            "secret-value-123"
        );
        // The plaintext never appears in the ciphertext bytes.
        let payload_str = String::from_utf8_lossy(&payload);
        assert!(!payload_str.contains("secret-value-123"));
    }

    #[test]
    fn rejects_tampered_payload() {
        let mut payload = encrypt_managed_secret("secret-value", REFERENCE).expect("encrypt");
        let last = payload.len() - 1;
        payload[last] ^= 0xff;

        let error = decrypt_managed_secret(&payload, REFERENCE).expect_err("tampered");
        assert!(error.contains("Unable to decrypt"));
        assert!(!error.contains("secret-value"));
    }

    #[test]
    fn rejects_decryption_with_wrong_reference() {
        let payload = encrypt_managed_secret("secret-value", REFERENCE).expect("encrypt");

        let error = decrypt_managed_secret(&payload, "providers/other/KEY").expect_err("reference");
        assert!(error.contains("Unable to decrypt"));
    }

    #[test]
    fn decodes_legacy_plaintext_as_is() {
        assert_eq!(
            decode_managed_secret_payload(b"legacy-secret", REFERENCE).expect("decode"),
            "legacy-secret"
        );
        assert_eq!(
            decode_managed_secret_payload(
                &encrypt_managed_secret("v", REFERENCE).expect("encrypt"),
                REFERENCE
            )
            .expect("decode"),
            "v"
        );
    }

    #[test]
    fn empty_payload_is_rejected_without_calling_dpapi() {
        let error =
            decrypt_managed_secret(b"QBWSEC1", REFERENCE).expect_err("empty blob");
        assert!(error.contains("Unable to decrypt"));
    }
}
