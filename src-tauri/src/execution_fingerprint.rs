//! Spike for P0-02: execution fingerprint used to invalidate a provider test
//! when the execution environment changes while the test runs.
//!
//! Design: `private-doc/p0/02-execution-fingerprint.md`.
//! This module is a pure helper: callers supply already-read manifest/source
//! bytes and the **final environment the runner would inject** (including the
//! resolved `QBWIN_PROXY_URL`). It never stores plaintext secrets, never
//! derives `Debug`, and does not touch the filesystem.
#![allow(dead_code)]

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

/// Raw inputs for one fingerprint.
///
/// `effective_env` must be the same map `remote_provider_runner` hands to the
/// child process:
///
/// - every configured `envVars` entry resolved through the secret resolver;
/// - `requiredEnvVars` fallbacks resolved too;
/// - the final `QBWIN_PROXY_URL` after `resolve_secret_value` + trim (explicit
///   provider value wins; global/system proxy is the fallback).
///
/// Because the proxy is part of this map there is no separate static proxy
/// digest: a system-proxy change or a `${file:...}` proxy target change shows up
/// as a different effective environment.
///
/// `resolved_runtime` must be the executable actually resolved for this run
/// (not just the persisted field), so a PATH resolution change invalidates the
/// fingerprint as well.
pub struct ExecutionParts<'a> {
    pub provider_id: &'a str,
    pub name: &'a str,
    pub cache_generation: &'a str,
    pub manifest: &'a [u8],
    pub source: &'a [u8],
    pub effective_env: &'a BTreeMap<String, String>,
    pub provider_dir: &'a str,
    pub runtime: &'a str,
    pub resolved_runtime: Option<&'a str>,
    pub timeout_seconds: u64,
    pub source_url: &'a str,
    pub trusted_checksum: Option<&'a str>,
}

/// Digest-only view of the execution environment.
///
/// Deliberately does not implement `Debug` (or `Serialize`): printing it must
/// never expose literal secrets or proxy credentials.
#[derive(Clone, PartialEq, Eq)]
pub struct ExecutionFingerprint {
    pub provider_id: String,
    /// Hash of `QBWIN_PROVIDER_NAME` (the display name is injected into the
    /// runner environment and can influence Provider behavior).
    pub name_digest: String,
    pub cache_generation: String,
    pub manifest_sha256: String,
    pub source_sha256: String,
    /// Hash of the **resolved** environment values, including the final
    /// `QBWIN_PROXY_URL`, not the binding expressions.
    pub effective_env_digest: String,
    pub provider_dir: String,
    pub runtime: String,
    pub resolved_runtime: Option<String>,
    pub timeout_seconds: u64,
    pub source_url: String,
    pub trusted_checksum: Option<String>,
}

pub fn compute(parts: &ExecutionParts<'_>) -> ExecutionFingerprint {
    ExecutionFingerprint {
        provider_id: parts.provider_id.to_string(),
        name_digest: sha256_hex(parts.name.as_bytes()),
        cache_generation: parts.cache_generation.to_string(),
        manifest_sha256: sha256_hex(parts.manifest),
        source_sha256: sha256_hex(parts.source),
        effective_env_digest: digest_effective_env(parts.effective_env),
        provider_dir: parts.provider_dir.to_string(),
        runtime: parts.runtime.to_string(),
        resolved_runtime: parts.resolved_runtime.map(str::to_string),
        timeout_seconds: parts.timeout_seconds,
        source_url: parts.source_url.to_string(),
        trusted_checksum: parts.trusted_checksum.map(str::to_string),
    }
}

/// Hashes the resolved environment as `sha256(name, sha256(value))` pairs, so
/// the output is stable and cannot be reversed into a secret.
pub fn digest_effective_env(env: &BTreeMap<String, String>) -> String {
    let mut hasher = Sha256::new();
    for (name, value) in env {
        hasher.update(name.as_bytes());
        hasher.update([0u8]);
        hasher.update(Sha256::digest(value.as_bytes()));
        hasher.update([0xffu8]);
    }
    hex_encode(hasher.finalize().as_slice())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex_encode(Sha256::digest(bytes).as_slice())
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    fn parts<'a>(env: &'a BTreeMap<String, String>) -> ExecutionParts<'a> {
        ExecutionParts {
            provider_id: "kimi",
            name: "Kimi account",
            cache_generation: "gen-1",
            manifest: br#"{"id":"kimi"}"#,
            source: b"function main() {}",
            effective_env: env,
            provider_dir: "C:/config/providers/remote/kimi",
            runtime: "builtin-js",
            resolved_runtime: None,
            timeout_seconds: 30,
            source_url: "https://example.com/provider.js",
            trusted_checksum: Some("sha256:aaaa"),
        }
    }

    #[test]
    fn f1_each_execution_input_changes_the_fingerprint() {
        let env = env_map(&[("API_KEY", "sk-live-secret")]);
        let baseline = compute(&parts(&env));

        let other_env = env_map(&[("API_KEY", "sk-live-secret"), ("REGION", "CN")]);

        let mut variants = Vec::new();
        variants.push(compute(&ExecutionParts {
            name: "Other name",
            ..parts(&env)
        }));
        variants.push(compute(&ExecutionParts {
            cache_generation: "gen-2",
            ..parts(&env)
        }));
        variants.push(compute(&ExecutionParts {
            manifest: br#"{"id":"kimi","version":"2"}"#,
            ..parts(&env)
        }));
        variants.push(compute(&ExecutionParts {
            source: b"function main() { return 1; }",
            ..parts(&env)
        }));
        variants.push(compute(&ExecutionParts {
            provider_dir: "C:/config/providers/remote/other",
            ..parts(&env)
        }));
        variants.push(compute(&ExecutionParts {
            runtime: "node",
            ..parts(&env)
        }));
        variants.push(compute(&ExecutionParts {
            resolved_runtime: Some("C:/Program Files/nodejs/node.exe"),
            ..parts(&env)
        }));
        variants.push(compute(&ExecutionParts {
            timeout_seconds: 60,
            ..parts(&env)
        }));
        variants.push(compute(&ExecutionParts {
            source_url: "https://example.com/v2.js",
            ..parts(&env)
        }));
        variants.push(compute(&ExecutionParts {
            trusted_checksum: Some("sha256:bbbb"),
            ..parts(&env)
        }));
        variants.push(compute(&parts(&other_env)));

        for variant in variants {
            assert!(
                variant != baseline,
                "every input must affect the fingerprint"
            );
        }

        // Stable: same inputs, same fingerprint.
        assert!(baseline == compute(&parts(&env)));
    }

    #[test]
    fn f3_proxy_changes_are_covered_by_the_effective_environment() {
        // The runner resolves the final proxy value into QBWIN_PROXY_URL, so a
        // proxy change must appear here rather than in a separate static digest.
        let direct = env_map(&[
            ("API_KEY", "value"),
            ("QBWIN_PROXY_URL", "http://127.0.0.1:7890"),
        ]);
        let switched = env_map(&[
            ("API_KEY", "value"),
            ("QBWIN_PROXY_URL", "http://127.0.0.1:7891"),
        ]);
        let cleared = env_map(&[("API_KEY", "value")]);

        let first = compute(&parts(&direct));
        let second = compute(&parts(&switched));
        let none = compute(&parts(&cleared));

        assert!(first != second);
        assert!(first != none);
        assert!(second != none);
    }

    #[test]
    fn f14_same_bindings_with_a_replaced_secret_produce_a_new_fingerprint() {
        // The envVars expression is identical; only the resolved value changed
        // because the managed secret file was replaced.
        let before = env_map(&[("API_KEY", "sk-old-secret-value")]);
        let after = env_map(&[("API_KEY", "sk-new-secret-value")]);

        let before_print = compute(&parts(&before));
        let after_print = compute(&parts(&after));

        assert_ne!(
            before_print.effective_env_digest,
            after_print.effective_env_digest
        );
        assert!(before_print != after_print);
    }

    #[test]
    fn fingerprints_never_embed_plaintext_secrets_or_proxy_credentials() {
        let env = env_map(&[
            ("API_KEY", "sk-super-secret-token"),
            ("QBWIN_PROXY_URL", "http://user:password@127.0.0.1:7890"),
        ]);
        let fingerprint = compute(&parts(&env));

        let rendered = format!(
            "{}{}{}{}{}",
            fingerprint.provider_id,
            fingerprint.name_digest,
            fingerprint.manifest_sha256,
            fingerprint.source_sha256,
            fingerprint.effective_env_digest
        );
        assert!(!rendered.contains("sk-super-secret-token"));
        assert!(!rendered.contains("password"));
    }

    #[test]
    fn f14b_managed_secret_replacement_through_the_real_resolver_changes_the_digest() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = crate::managed_secret_store::ManagedSecretStore::new(
            temp.path(),
            crate::config::SecretStorageMode::Plaintext,
        );
        store
            .write("kimi", "API_KEY", "old-secret")
            .expect("write old secret");

        let resolve_env = || {
            let value = crate::config::resolve_secret_value(
                "${secret:providers/kimi/API_KEY}",
                temp.path(),
            )
            .expect("resolve managed secret");
            let mut map = BTreeMap::new();
            map.insert("API_KEY".to_string(), value);
            map
        };

        let before = compute(&parts(&resolve_env()));
        store
            .write("kimi", "API_KEY", "new-secret")
            .expect("replace secret");
        let after = compute(&parts(&resolve_env()));

        assert_ne!(
            before.effective_env_digest, after.effective_env_digest,
            "replacing the managed secret file must invalidate the fingerprint"
        );
    }

    #[test]
    fn f14c_file_secret_replacement_through_the_real_resolver_changes_the_digest() {
        let temp = tempfile::tempdir().expect("temp dir");
        let secret_path = temp.path().join("token.txt");
        std::fs::write(&secret_path, "one").expect("write secret file");

        let placeholder = format!("${{file:{}}}", secret_path.display());
        let resolve_env = || {
            let value =
                crate::config::resolve_secret_value(&placeholder, temp.path()).expect("resolve");
            let mut map = BTreeMap::new();
            map.insert("TOKEN".to_string(), value);
            map
        };

        let before = compute(&parts(&resolve_env()));
        std::fs::write(&secret_path, "two").expect("replace secret file");
        let after = compute(&parts(&resolve_env()));

        assert_ne!(before.effective_env_digest, after.effective_env_digest);
    }
}
