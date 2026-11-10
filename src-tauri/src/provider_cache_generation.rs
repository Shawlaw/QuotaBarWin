//! Spike for the P0-03 cache protocol: immutable generation directories plus an
//! atomically replaced pointer file.
//!
//! Design: `private-doc/p0/03-provider-resource-transactions.md`.
//! Not wired into the provider runtime yet; CP0b-2 only needs the publish /
//! resolve primitives and the R-series tests so the layout can be frozen
//! before the runner and updater are migrated.
#![allow(dead_code)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const POINTER_FILE: &str = "current.json";
const GENERATIONS_DIR: &str = "generations";

static GENERATION_COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
thread_local! {
    static FAIL_POINTER_WRITE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Test-only fault injection for the atomic pointer write.
#[cfg(test)]
pub(crate) fn set_pointer_write_failure(fail: bool) {
    FAIL_POINTER_WRITE.with(|flag| flag.set(fail));
}

#[cfg(test)]
fn fail_pointer_write() -> bool {
    FAIL_POINTER_WRITE.with(std::cell::Cell::get)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedGeneration {
    pub generation: String,
    pub dir: PathBuf,
}

/// A generation directory that exists on disk but is not referenced yet.
///
/// Fields are private: only [`prepare_generation`] can mint one, so a caller
/// cannot point activation/discard at an arbitrary directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedGeneration {
    generation: String,
    dir: PathBuf,
}

impl PreparedGeneration {
    pub fn generation(&self) -> &str {
        &self.generation
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedGeneration {
    pub generation: String,
    pub dir: PathBuf,
    /// Fingerprint of the provider config entry that this generation matches.
    /// `None` for pointers written before pairing was introduced.
    pub config_fingerprint: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CurrentPointer {
    generation: String,
    #[serde(default, rename = "configFingerprint")]
    config_fingerprint: Option<String>,
}

pub fn generations_dir(provider_dir: &Path) -> PathBuf {
    provider_dir.join(GENERATIONS_DIR)
}

pub fn current_pointer_path(provider_dir: &Path) -> PathBuf {
    provider_dir.join(POINTER_FILE)
}

/// Writes `files` into a fresh immutable generation directory **without**
/// changing the pointer. Two-resource transactions (cache + config) prepare
/// first, commit config, then call [`activate_generation`].
pub fn prepare_generation(
    provider_dir: &Path,
    files: &[(&str, &[u8])],
) -> Result<PreparedGeneration, String> {
    if files.is_empty() {
        return Err("a cache generation needs at least one file".to_string());
    }
    for (name, _) in files {
        validate_file_name(name)?;
    }

    let generations = generations_dir(provider_dir);
    fs::create_dir_all(&generations)
        .map_err(|error| format!("Unable to create generations dir: {error}"))?;

    let generation = next_generation_id();
    let staging = generations.join(format!(".staging-{generation}"));
    fs::create_dir(&staging).map_err(|error| format!("Unable to create staging dir: {error}"))?;

    let write_result = (|| -> Result<(), String> {
        for (name, bytes) in files {
            let path = staging.join(name);
            let mut file = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)
                .map_err(|error| format!("Unable to create cache file {name}: {error}"))?;
            file.write_all(bytes)
                .map_err(|error| format!("Unable to write cache file {name}: {error}"))?;
            file.sync_all()
                .map_err(|error| format!("Unable to flush cache file {name}: {error}"))?;
        }
        Ok(())
    })();

    if let Err(error) = write_result {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }

    let published_dir = generations.join(&generation);
    if let Err(error) = fs::rename(&staging, &published_dir) {
        let _ = fs::remove_dir_all(&staging);
        return Err(format!("Unable to publish cache generation: {error}"));
    }

    Ok(PreparedGeneration {
        generation,
        dir: published_dir,
    })
}

/// Atomically flips `current.json` to a prepared generation. On failure the
/// prepared directory is left in place so the caller can decide between
/// retrying and [`discard_generation`] (the previous pointer is untouched).
pub fn activate_generation(
    provider_dir: &Path,
    prepared: &PreparedGeneration,
) -> Result<PublishedGeneration, String> {
    activate_generation_with_fingerprint(provider_dir, prepared, None)
}

/// Activates a prepared generation and records the provider config fingerprint
/// it matches. Readers can then reject a cache/config pair that was never
/// committed together.
pub fn activate_generation_with_fingerprint(
    provider_dir: &Path,
    prepared: &PreparedGeneration,
    config_fingerprint: Option<&str>,
) -> Result<PublishedGeneration, String> {
    validate_generation_id(&prepared.generation)?;
    let expected = generations_dir(provider_dir).join(&prepared.generation);
    if prepared.dir != expected {
        return Err(format!(
            "Prepared generation directory does not match the provider generations root: {}",
            prepared.dir.display()
        ));
    }
    if !prepared.dir().is_dir() {
        return Err(format!(
            "Prepared cache generation is missing: {}",
            prepared.dir.display()
        ));
    }
    write_pointer(provider_dir, &prepared.generation, config_fingerprint)?;
    Ok(PublishedGeneration {
        generation: prepared.generation.clone(),
        dir: prepared.dir().to_path_buf(),
    })
}

/// Removes a prepared generation that never got activated.
///
/// Refuses to delete the generation the pointer currently references, even if
/// a caller somehow holds a stale `PreparedGeneration` for it.
pub fn discard_generation(
    provider_dir: &Path,
    prepared: &PreparedGeneration,
) -> Result<(), String> {
    validate_generation_id(&prepared.generation)?;
    let dir = generations_dir(provider_dir).join(&prepared.generation);
    if let Some(current) = resolve_current(provider_dir)? {
        if current.generation == prepared.generation {
            return Err(format!(
                "Refusing to discard the active cache generation {}",
                prepared.generation
            ));
        }
    }
    match fs::remove_dir_all(&dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Unable to discard cache generation: {error}")),
    }
}

/// Convenience wrapper for callers that do not need a separate config commit:
/// prepare + activate, removing the prepared directory when activation fails.
pub fn publish_generation(
    provider_dir: &Path,
    files: &[(&str, &[u8])],
) -> Result<PublishedGeneration, String> {
    let prepared = prepare_generation(provider_dir, files)?;
    match activate_generation(provider_dir, &prepared) {
        Ok(published) => Ok(published),
        Err(error) => {
            let _ = discard_generation(provider_dir, &prepared);
            Err(error)
        }
    }
}

/// Which step of the cache+config transaction failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheCommitStage {
    Prepare,
    Config,
    Activate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheCommitError {
    pub stage: CacheCommitStage,
    pub message: String,
}

/// Runs the ordered two-resource commit: prepare the generation, commit the
/// config entry, then activate the pointer.
///
/// - prepare failure: nothing changed;
/// - config failure: prepared directory is discarded, pointer unchanged;
/// - activation failure: the prepared directory is **kept** because the config
///   commit may already reference it; the caller must either retry activation
///   or restore the config bytes it captured.
pub fn commit_generation(
    provider_dir: &Path,
    files: &[(&str, &[u8])],
    commit_config: impl FnOnce(&PreparedGeneration) -> Result<(), String>,
) -> Result<PublishedGeneration, CacheCommitError> {
    let prepared = prepare_generation(provider_dir, files).map_err(|message| CacheCommitError {
        stage: CacheCommitStage::Prepare,
        message,
    })?;

    if let Err(message) = commit_config(&prepared) {
        let _ = discard_generation(provider_dir, &prepared);
        return Err(CacheCommitError {
            stage: CacheCommitStage::Config,
            message,
        });
    }

    activate_generation(provider_dir, &prepared).map_err(|message| CacheCommitError {
        stage: CacheCommitStage::Activate,
        message,
    })
}

/// Outcome of a one-time legacy cache migration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyMigration {
    Migrated { generation: String },
    AlreadyCurrent,
    NoLegacyCache,
}

/// Migrates a legacy cache layout (`<provider_dir>/provider.json` + source +
/// `.meta.json`) into an immutable generation and flips `current.json`.
///
/// Legacy files are intentionally left in place for one release so a rollback
/// to an older build still works; readers prefer the pointer once it exists.
/// Idempotent: a second call with the pointer present returns `AlreadyCurrent`.
pub fn migrate_legacy_cache(provider_dir: &Path) -> Result<LegacyMigration, String> {
    if current_pointer_path(provider_dir).exists() {
        return Ok(LegacyMigration::AlreadyCurrent);
    }

    let manifest_path = provider_dir.join("provider.json");
    let manifest_bytes = match fs::read(&manifest_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(LegacyMigration::NoLegacyCache)
        }
        Err(error) => {
            return Err(format!(
                "Unable to read legacy provider manifest {}: {error}",
                manifest_path.display()
            ))
        }
    };

    let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("Legacy provider manifest is invalid: {error}"))?;
    let entry = manifest
        .get("entry")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "Legacy provider manifest has no entry".to_string())?;
    let source_name = Path::new(entry)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| format!("Legacy provider entry is invalid: {entry}"))?;
    let source_path = provider_dir.join(source_name);
    let source_bytes = fs::read(&source_path).map_err(|error| {
        format!(
            "Unable to read legacy provider source {}: {error}",
            source_path.display()
        )
    })?;
    let meta_bytes = fs::read(provider_dir.join(".meta.json")).ok();

    let mut files: Vec<(&str, &[u8])> = vec![
        ("provider.json", manifest_bytes.as_slice()),
        (source_name, source_bytes.as_slice()),
    ];
    if let Some(meta) = meta_bytes.as_deref() {
        files.push((".meta.json", meta));
    }

    let prepared = prepare_generation(provider_dir, &files)?;
    match activate_generation(provider_dir, &prepared) {
        Ok(published) => Ok(LegacyMigration::Migrated {
            generation: published.generation,
        }),
        Err(error) => {
            let _ = discard_generation(provider_dir, &prepared);
            Err(error)
        }
    }
}

/// Directory that currently holds `provider.json` and the entry source.
///
/// New-layout caches resolve through `current.json`; legacy caches fall back to
/// the provider directory itself.
pub fn resolve_cache_root(provider_dir: &Path) -> Result<PathBuf, String> {
    if current_pointer_path(provider_dir).exists() {
        return resolve_current(provider_dir)?
            .map(|current| current.dir)
            .ok_or_else(|| {
                format!(
                    "Cache pointer disappeared while resolving {}",
                    provider_dir.display()
                )
            });
    }
    Ok(provider_dir.to_path_buf())
}

/// Resolves the cache root and verifies that the generation matches the saved
/// provider config.
///
/// - no pointer (legacy cache): returns `provider_dir`, no pairing possible;
/// - pointer without `configFingerprint` (written before pairing): accepted;
/// - mismatch: `Err`, callers must refresh/reinstall instead of executing a
///   cache/config pair that was never committed together.
pub fn resolve_cache_root_checked(
    provider_dir: &Path,
    expected_fingerprint: &str,
) -> Result<PathBuf, String> {
    let Some(current) = resolve_current(provider_dir)? else {
        return Ok(provider_dir.to_path_buf());
    };
    if let Some(recorded) = current.config_fingerprint.as_deref() {
        if recorded != expected_fingerprint {
            return Err(format!(
                "Provider cache generation {} does not match the saved configuration",
                current.generation
            ));
        }
    }
    Ok(current.dir)
}

/// Removes leftover staging directories from interrupted publishes.
///
/// Safe at startup (single-instance mutex, no provider operation running): any
/// `.staging-*` directory under `generations/` is by definition not referenced
/// by `current.json` and can never be activated again.
pub fn cleanup_staging_generations(provider_dir: &Path) -> Result<usize, String> {
    let dir = generations_dir(provider_dir);
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(format!("Unable to list cache staging directories: {error}")),
    };
    let mut removed = 0;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(".staging-") {
            continue;
        }
        if entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            fs::remove_dir_all(entry.path())
                .map_err(|error| format!("Unable to remove staging directory {name}: {error}"))?;
            removed += 1;
        }
    }
    Ok(removed)
}

/// Reads the current pointer and verifies the referenced generation exists.
///
/// `Ok(None)` means "no cache published yet"; `Err` means the pointer exists
/// but is unreadable or dangling, which callers must treat as a repair case
/// rather than silently falling back to an older generation.
pub fn resolve_current(provider_dir: &Path) -> Result<Option<ResolvedGeneration>, String> {
    let pointer_path = current_pointer_path(provider_dir);
    let raw = match fs::read_to_string(&pointer_path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Unable to read cache pointer: {error}")),
    };
    let pointer: CurrentPointer = serde_json::from_str(&raw)
        .map_err(|error| format!("Unable to parse cache pointer: {error}"))?;
    validate_generation_id(&pointer.generation)?;
    let dir = generations_dir(provider_dir).join(&pointer.generation);
    if !dir.is_dir() {
        return Err(format!(
            "Current cache generation {} is missing: {}",
            pointer.generation,
            dir.display()
        ));
    }
    Ok(Some(ResolvedGeneration {
        generation: pointer.generation,
        dir,
        config_fingerprint: pointer.config_fingerprint,
    }))
}

/// Lists published generations in lexicographic order (staging dirs excluded).
pub fn list_generations(provider_dir: &Path) -> Result<Vec<String>, String> {
    let dir = generations_dir(provider_dir);
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("Unable to list cache generations: {error}")),
    };
    let mut generations = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        if !entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        generations.push(name);
    }
    generations.sort();
    Ok(generations)
}

fn write_pointer(
    provider_dir: &Path,
    generation: &str,
    config_fingerprint: Option<&str>,
) -> Result<(), String> {
    #[cfg(test)]
    if fail_pointer_write() {
        return Err("injected pointer write failure".to_string());
    }

    let pointer = CurrentPointer {
        generation: generation.to_string(),
        config_fingerprint: config_fingerprint.map(str::to_string),
    };
    let bytes = serde_json::to_vec_pretty(&pointer)
        .map_err(|error| format!("Unable to serialize cache pointer: {error}"))?;
    let temp_path = provider_dir.join(format!(".{POINTER_FILE}.{generation}.tmp"));

    let write_result = (|| -> Result<(), String> {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .map_err(|error| format!("Unable to create cache pointer temp file: {error}"))?;
        file.write_all(&bytes)
            .map_err(|error| format!("Unable to write cache pointer: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("Unable to flush cache pointer: {error}"))?;
        drop(file);
        fs::rename(&temp_path, current_pointer_path(provider_dir))
            .map_err(|error| format!("Unable to replace cache pointer: {error}"))
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    write_result
}

fn validate_file_name(name: &str) -> Result<(), String> {
    // `.meta.json` is the only dot-file the provider cache layout needs; every
    // other dot-prefixed name is staging/internal and must not be publishable.
    if name == ".meta.json" {
        return Ok(());
    }
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || name.contains(':')
        || name.starts_with('.')
    {
        return Err(format!("Invalid cache file name: {name}"));
    }
    Ok(())
}

/// Generation ids are used as a single path component. Only the alphabet we
/// mint ourselves is accepted, so a tampered `current.json` cannot escape the
/// provider's `generations/` directory (no separators, `..`, drive prefixes or
/// NTFS alternate-data-stream colons).
fn validate_generation_id(generation: &str) -> Result<(), String> {
    if generation.is_empty() || generation.len() > 128 {
        return Err(format!("Invalid cache generation id: {generation}"));
    }
    if !generation
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(format!("Invalid cache generation id: {generation}"));
    }
    Ok(())
}

fn next_generation_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let counter = GENERATION_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{nanos:032x}-{counter:08x}-{:08x}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn staging_entries(provider_dir: &Path) -> Vec<String> {
        let Ok(entries) = fs::read_dir(generations_dir(provider_dir)) else {
            return Vec::new();
        };
        entries
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.starts_with('.'))
            .collect()
    }

    fn provider_dir(temp: &tempfile::TempDir) -> PathBuf {
        temp.path().join("providers").join("remote").join("kimi")
    }

    #[test]
    fn publishes_and_resolves_a_complete_generation() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dir = provider_dir(&temp);
        fs::create_dir_all(&dir).expect("provider dir");

        let published = publish_generation(
            &dir,
            &[
                ("provider.json", br#"{"id":"kimi"}"#),
                ("provider.js", b"function main() {}"),
            ],
        )
        .expect("publish");

        let resolved = resolve_current(&dir).expect("resolve").expect("pointer");
        assert_eq!(resolved.generation, published.generation);
        assert_eq!(
            fs::read(resolved.dir.join("provider.js")).expect("source"),
            b"function main() {}"
        );
        assert!(staging_entries(&dir).is_empty());
        assert_eq!(list_generations(&dir).expect("list").len(), 1);
    }

    #[test]
    fn new_generation_keeps_the_previous_one_readable() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dir = provider_dir(&temp);
        fs::create_dir_all(&dir).expect("provider dir");

        let first = publish_generation(&dir, &[("provider.json", b"v1")]).expect("first");
        let second = publish_generation(&dir, &[("provider.json", b"v2")]).expect("second");

        assert_ne!(first.generation, second.generation);
        assert_eq!(
            fs::read(first.dir.join("provider.json")).expect("old readable"),
            b"v1"
        );
        let resolved = resolve_current(&dir).expect("resolve").expect("pointer");
        assert_eq!(resolved.generation, second.generation);
    }

    #[test]
    fn rollback_publishes_a_new_generation_instead_of_repointing() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dir = provider_dir(&temp);
        fs::create_dir_all(&dir).expect("provider dir");

        let first = publish_generation(&dir, &[("provider.json", b"v1")]).expect("first");
        let _second = publish_generation(&dir, &[("provider.json", b"v2")]).expect("second");
        let rollback = publish_generation(&dir, &[("provider.json", b"v1")]).expect("rollback");

        assert_ne!(rollback.generation, first.generation);
        let resolved = resolve_current(&dir).expect("resolve").expect("pointer");
        assert_eq!(resolved.generation, rollback.generation);
        assert_eq!(
            fs::read(resolved.dir.join("provider.json")).expect("content"),
            b"v1"
        );
    }

    #[test]
    fn pointer_failure_keeps_the_previous_pointer_and_removes_the_new_generation() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dir = provider_dir(&temp);
        fs::create_dir_all(&dir).expect("provider dir");

        let first = publish_generation(&dir, &[("provider.json", b"v1")]).expect("first");
        let before = list_generations(&dir).expect("list");

        set_pointer_write_failure(true);
        let result = publish_generation(&dir, &[("provider.json", b"v2")]);
        set_pointer_write_failure(false);

        assert!(result.is_err());
        let resolved = resolve_current(&dir).expect("resolve").expect("pointer");
        assert_eq!(resolved.generation, first.generation);
        assert_eq!(
            list_generations(&dir).expect("list"),
            before,
            "the failed generation must not be left behind"
        );
        assert!(staging_entries(&dir).is_empty());
    }

    #[test]
    fn concurrent_publishes_leave_a_valid_pointer_and_complete_generation() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dir = provider_dir(&temp);
        fs::create_dir_all(&dir).expect("provider dir");

        let payloads = [
            b"alpha".as_slice(),
            b"beta".as_slice(),
            b"gamma".as_slice(),
            b"delta".as_slice(),
        ];
        let mut handles = Vec::new();
        for payload in payloads {
            let dir = dir.clone();
            handles.push(thread::spawn(move || {
                publish_generation(&dir, &[("provider.json", payload)])
                    .map(|published| (published.generation, payload.to_vec()))
            }));
        }

        let mut published = Vec::new();
        for handle in handles {
            published.push(handle.join().expect("thread").expect("publish"));
        }

        let resolved = resolve_current(&dir).expect("resolve").expect("pointer");
        let content = fs::read(resolved.dir.join("provider.json")).expect("content");
        let matching = published
            .iter()
            .find(|(generation, _)| generation == &resolved.generation)
            .expect("pointer must reference a published generation");
        assert_eq!(content, matching.1);
        assert_eq!(list_generations(&dir).expect("list").len(), payloads.len());
    }

    #[test]
    fn prepared_generation_is_not_visible_until_activated() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dir = provider_dir(&temp);
        fs::create_dir_all(&dir).expect("provider dir");

        let first = publish_generation(&dir, &[("provider.json", b"v1")]).expect("first");
        let prepared = prepare_generation(&dir, &[("provider.json", b"v2")]).expect("prepare");

        // Still pointing at v1 even though v2 exists on disk.
        let resolved = resolve_current(&dir).expect("resolve").expect("pointer");
        assert_eq!(resolved.generation, first.generation);
        assert!(prepared.dir().is_dir());

        let activated = activate_generation(&dir, &prepared).expect("activate");
        let resolved = resolve_current(&dir).expect("resolve").expect("pointer");
        assert_eq!(resolved.generation, activated.generation);
        assert_eq!(
            fs::read(resolved.dir.join("provider.json")).expect("content"),
            b"v2"
        );
    }

    #[test]
    fn activation_failure_leaves_the_pointer_and_allows_discard() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dir = provider_dir(&temp);
        fs::create_dir_all(&dir).expect("provider dir");

        let first = publish_generation(&dir, &[("provider.json", b"v1")]).expect("first");
        let prepared = prepare_generation(&dir, &[("provider.json", b"v2")]).expect("prepare");

        set_pointer_write_failure(true);
        let result = activate_generation(&dir, &prepared);
        set_pointer_write_failure(false);

        assert!(result.is_err());
        let resolved = resolve_current(&dir).expect("resolve").expect("pointer");
        assert_eq!(resolved.generation, first.generation);
        assert!(prepared.dir().is_dir(), "caller decides when to discard");

        discard_generation(&dir, &prepared).expect("discard");
        assert!(!prepared.dir().exists());
        assert_eq!(list_generations(&dir).expect("list").len(), 1);
    }

    #[test]
    fn accepts_the_real_three_file_cache_layout() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dir = provider_dir(&temp);
        fs::create_dir_all(&dir).expect("provider dir");

        let published = publish_generation(
            &dir,
            &[
                ("provider.json", br#"{"id":"kimi"}"#),
                ("provider.js", b"function main() {}"),
                (".meta.json", br#"{"checksum":"sha256:aaaa"}"#),
            ],
        )
        .expect("publish");

        let resolved = resolve_current(&dir).expect("resolve").expect("pointer");
        assert_eq!(resolved.generation, published.generation);
        assert!(resolved.dir.join("provider.json").is_file());
        assert!(resolved.dir.join(".meta.json").is_file());
    }

    #[test]
    fn rejects_pointer_generation_ids_that_escape_the_generations_dir() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dir = provider_dir(&temp);
        fs::create_dir_all(&dir).expect("provider dir");

        for bad in [
            "../other",
            "..\\other",
            "/etc/passwd",
            "C:\\temp",
            "a/b",
            "a:b",
            ".hidden",
            "",
        ] {
            let pointer = serde_json::json!({ "generation": bad });
            fs::write(
                current_pointer_path(&dir),
                serde_json::to_vec(&pointer).expect("pointer json"),
            )
            .expect("write pointer");
            assert!(
                resolve_current(&dir).is_err(),
                "generation {bad:?} must be rejected"
            );
        }
    }

    #[test]
    fn discard_refuses_the_active_generation() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dir = provider_dir(&temp);
        fs::create_dir_all(&dir).expect("provider dir");

        let prepared = prepare_generation(&dir, &[("provider.json", b"v1")]).expect("prepare");
        let published = activate_generation(&dir, &prepared).expect("activate");

        let error = discard_generation(&dir, &prepared).expect_err("active generation is pinned");
        assert!(error.contains("active"));
        assert!(published.dir.is_dir());
    }

    #[test]
    fn activate_rejects_a_prepared_generation_from_another_provider() {
        let temp = tempfile::tempdir().expect("temp dir");
        let first = provider_dir(&temp);
        let other = temp.path().join("providers").join("remote").join("other");
        fs::create_dir_all(&first).expect("first dir");
        fs::create_dir_all(&other).expect("other dir");

        let prepared = prepare_generation(&first, &[("provider.json", b"v1")]).expect("prepare");
        let error = activate_generation(&other, &prepared).expect_err("wrong provider root");
        assert!(error.contains("does not match"));
        assert!(resolve_current(&other).expect("resolve").is_none());
    }

    #[test]
    fn rejects_unsafe_file_names() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dir = provider_dir(&temp);
        fs::create_dir_all(&dir).expect("provider dir");

        for name in ["../escape", "nested/file", "..", "", ".hidden"] {
            assert!(
                publish_generation(&dir, &[(name, b"x")]).is_err(),
                "{name} must be rejected"
            );
        }
    }

    #[test]
    fn missing_pointer_is_not_an_error_but_a_dangling_one_is() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dir = provider_dir(&temp);
        fs::create_dir_all(&dir).expect("provider dir");

        assert!(resolve_current(&dir).expect("resolve").is_none());

        fs::write(
            current_pointer_path(&dir),
            br#"{"generation":"does-not-exist"}"#,
        )
        .expect("write pointer");

        assert!(resolve_current(&dir).is_err());
    }

    #[test]
    fn commit_generation_orders_prepare_config_activate() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dir = provider_dir(&temp);
        fs::create_dir_all(&dir).expect("provider dir");

        let mut committed_dir: Option<PathBuf> = None;
        let published = commit_generation(&dir, &[("provider.json", b"v1")], |prepared| {
            // During the config commit the pointer must still be absent.
            assert!(resolve_current(&dir).expect("resolve").is_none());
            assert!(prepared.dir().is_dir());
            committed_dir = Some(prepared.dir().to_path_buf());
            Ok(())
        })
        .expect("commit");

        assert_eq!(committed_dir.as_deref(), Some(published.dir.as_path()));
        let resolved = resolve_current(&dir).expect("resolve").expect("pointer");
        assert_eq!(resolved.generation, published.generation);
        assert!(staging_entries(&dir).is_empty());
    }

    #[test]
    fn commit_generation_discards_the_generation_when_config_fails() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dir = provider_dir(&temp);
        fs::create_dir_all(&dir).expect("provider dir");

        let first = publish_generation(&dir, &[("provider.json", b"v0")]).expect("first");
        let before = list_generations(&dir).expect("list");

        let error = commit_generation(&dir, &[("provider.json", b"v1")], |_| {
            Err("synthetic config failure".to_string())
        })
        .expect_err("config failure must abort");

        assert_eq!(error.stage, CacheCommitStage::Config);
        assert!(error.message.contains("synthetic config failure"));
        let resolved = resolve_current(&dir).expect("resolve").expect("pointer");
        assert_eq!(resolved.generation, first.generation);
        assert_eq!(list_generations(&dir).expect("list"), before);
        assert!(staging_entries(&dir).is_empty());
    }

    #[test]
    fn commit_generation_keeps_the_prepared_directory_when_activation_fails() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dir = provider_dir(&temp);
        fs::create_dir_all(&dir).expect("provider dir");

        let first = publish_generation(&dir, &[("provider.json", b"v0")]).expect("first");
        set_pointer_write_failure(true);

        let mut committed_dir: Option<PathBuf> = None;
        let error = commit_generation(&dir, &[("provider.json", b"v1")], |prepared| {
            committed_dir = Some(prepared.dir().to_path_buf());
            Ok(())
        })
        .expect_err("activation must fail");
        set_pointer_write_failure(false);

        assert_eq!(error.stage, CacheCommitStage::Activate);
        let resolved = resolve_current(&dir).expect("resolve").expect("pointer");
        assert_eq!(resolved.generation, first.generation);
        let committed_dir = committed_dir.expect("config commit ran");
        assert!(
            committed_dir.is_dir(),
            "the caller still needs the prepared generation to retry or restore"
        );
    }

    #[test]
    fn migrates_a_legacy_cache_and_prefers_the_pointer() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dir = provider_dir(&temp);
        fs::create_dir_all(&dir).expect("provider dir");
        fs::write(
            dir.join("provider.json"),
            br#"{"id":"kimi","entry":"provider.js"}"#,
        )
        .expect("manifest");
        fs::write(dir.join("provider.js"), b"function main() {}").expect("source");
        fs::write(dir.join(".meta.json"), br#"{"checksum":"sha256:aaaa"}"#).expect("meta");

        let migration = migrate_legacy_cache(&dir).expect("migrate");
        let LegacyMigration::Migrated { generation } = migration else {
            panic!("expected migration, got {migration:?}");
        };

        let resolved = resolve_current(&dir).expect("resolve").expect("pointer");
        assert_eq!(resolved.generation, generation);
        assert!(resolved.dir.join("provider.js").is_file());
        assert!(resolved.dir.join(".meta.json").is_file());
        // Legacy files stay for one release.
        assert!(dir.join("provider.json").is_file());
        assert!(dir.join("provider.js").is_file());

        assert_eq!(
            resolve_cache_root(&dir).expect("resolve root"),
            resolved.dir
        );
    }

    #[test]
    fn migration_is_idempotent() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dir = provider_dir(&temp);
        fs::create_dir_all(&dir).expect("provider dir");
        fs::write(
            dir.join("provider.json"),
            br#"{"id":"kimi","entry":"provider.js"}"#,
        )
        .expect("manifest");
        fs::write(dir.join("provider.js"), b"function main() {}").expect("source");

        migrate_legacy_cache(&dir).expect("first migration");
        assert_eq!(
            migrate_legacy_cache(&dir).expect("second migration"),
            LegacyMigration::AlreadyCurrent
        );
    }

    #[test]
    fn migration_reports_a_missing_legacy_cache() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dir = provider_dir(&temp);
        fs::create_dir_all(&dir).expect("provider dir");

        assert_eq!(
            migrate_legacy_cache(&dir).expect("migrate"),
            LegacyMigration::NoLegacyCache
        );
        assert!(resolve_current(&dir).expect("resolve").is_none());
        assert_eq!(resolve_cache_root(&dir).expect("root"), dir);
    }

    #[test]
    fn migration_rejects_an_invalid_legacy_manifest() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dir = provider_dir(&temp);
        fs::create_dir_all(&dir).expect("provider dir");
        fs::write(dir.join("provider.json"), b"not json").expect("manifest");

        assert!(migrate_legacy_cache(&dir).is_err());
        assert!(resolve_current(&dir).expect("resolve").is_none());
    }

    #[test]
    fn activation_records_the_config_fingerprint_and_checks_it() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dir = provider_dir(&temp);
        fs::create_dir_all(&dir).expect("provider dir");

        let prepared = prepare_generation(&dir, &[("provider.json", b"v1")]).expect("prepare");
        activate_generation_with_fingerprint(&dir, &prepared, Some("fingerprint-a"))
            .expect("activate");

        let resolved = resolve_current(&dir).expect("resolve").expect("pointer");
        assert_eq!(
            resolved.config_fingerprint.as_deref(),
            Some("fingerprint-a")
        );
        assert_eq!(
            resolve_cache_root_checked(&dir, "fingerprint-a").expect("matching pair"),
            resolved.dir
        );
        assert!(resolve_cache_root_checked(&dir, "fingerprint-b").is_err());
    }

    #[test]
    fn checked_resolution_accepts_legacy_caches_and_pointers_without_fingerprints() {
        let temp = tempfile::tempdir().expect("temp dir");
        let legacy_dir = temp.path().join("providers").join("remote").join("legacy");
        fs::create_dir_all(&legacy_dir).expect("legacy dir");
        assert_eq!(
            resolve_cache_root_checked(&legacy_dir, "anything").expect("legacy cache"),
            legacy_dir
        );

        let migrated_dir = temp
            .path()
            .join("providers")
            .join("remote")
            .join("migrated");
        fs::create_dir_all(&migrated_dir).expect("migrated dir");
        publish_generation(&migrated_dir, &[("provider.json", b"v1")]).expect("publish");
        assert!(resolve_cache_root_checked(&migrated_dir, "anything").is_ok());
    }

    #[test]
    fn cleanup_removes_staging_directories_and_keeps_published_generations() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dir = provider_dir(&temp);
        fs::create_dir_all(generations_dir(&dir).join(".staging-interrupted"))
            .expect("staging dir");
        fs::write(
            generations_dir(&dir)
                .join(".staging-interrupted")
                .join("provider.json"),
            b"partial",
        )
        .expect("staging file");
        let published = publish_generation(&dir, &[("provider.json", b"v1")]).expect("publish");

        assert_eq!(cleanup_staging_generations(&dir).expect("cleanup"), 1);
        assert!(!generations_dir(&dir).join(".staging-interrupted").exists());
        assert!(generations_dir(&dir).join(&published.generation).is_dir());
    }
}
