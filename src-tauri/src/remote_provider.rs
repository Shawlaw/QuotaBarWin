use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::proxy::{build_http_client, ProxyConfig};
use crate::redact::redact_sensitive;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderManifest {
    pub schema_version: u8,
    pub id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub runtime: String,
    pub entry: String,
    #[serde(default, rename = "requiredEnvVars")]
    pub required_env_vars: Vec<String>,
    pub output: String,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub checksums: Checksums,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Checksums {
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteProviderMeta {
    #[serde(default)]
    pub etag: Option<String>,
    #[serde(default)]
    pub last_check_at: Option<String>,
    #[serde(default)]
    pub checksum: Option<String>,
    pub source_url: String,
    #[serde(default)]
    pub resolved_runtime: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub id: String,
    pub available: bool,
    pub new_checksum: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteProviderPreview {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub runtime: String,
    pub source_url: String,
    pub required_env_vars: Vec<String>,
    pub checksum: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RemoteProviderError {
    Network(String),
    Http(u16),
    InvalidManifest(String),
    ChecksumMismatch { expected: String, actual: String },
    UnsupportedSchemaVersion(u8),
    RuntimeNotFound(String),
    Io(String),
}

impl std::fmt::Display for RemoteProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RemoteProviderError::Network(msg) => write!(f, "Network error: {msg}"),
            RemoteProviderError::Http(status) => write!(f, "HTTP error: {status}"),
            RemoteProviderError::InvalidManifest(msg) => write!(f, "Invalid manifest: {msg}"),
            RemoteProviderError::ChecksumMismatch { expected, actual } => {
                write!(f, "Checksum mismatch: expected {expected}, got {actual}")
            }
            RemoteProviderError::UnsupportedSchemaVersion(version) => {
                write!(f, "Unsupported schema version: {version}")
            }
            RemoteProviderError::RuntimeNotFound(runtime) => {
                write!(f, "Runtime not found: {runtime}")
            }
            RemoteProviderError::Io(msg) => write!(f, "IO error: {msg}"),
        }
    }
}

impl From<std::io::Error> for RemoteProviderError {
    fn from(error: std::io::Error) -> Self {
        RemoteProviderError::Io(error.to_string())
    }
}

fn validate_manifest(manifest: &ProviderManifest) -> Result<(), RemoteProviderError> {
    if manifest.schema_version != 1 {
        return Err(RemoteProviderError::UnsupportedSchemaVersion(
            manifest.schema_version,
        ));
    }
    if manifest.id.trim().is_empty() {
        return Err(RemoteProviderError::InvalidManifest(
            "missing id".to_string(),
        ));
    }
    if manifest.runtime.trim().is_empty() {
        return Err(RemoteProviderError::InvalidManifest(
            "missing runtime".to_string(),
        ));
    }
    if manifest.entry.trim().is_empty() {
        return Err(RemoteProviderError::InvalidManifest(
            "missing entry".to_string(),
        ));
    }
    if manifest.output.trim().is_empty() {
        return Err(RemoteProviderError::InvalidManifest(
            "missing output".to_string(),
        ));
    }
    Ok(())
}

fn fetch_text(
    url: &str,
    per_provider_proxy: Option<&str>,
    global_proxy: Option<&ProxyConfig>,
    timeout: Duration,
) -> Result<String, RemoteProviderError> {
    let url = url.trim();

    if url.starts_with("http://") || url.starts_with("https://") {
        let client = build_http_client(per_provider_proxy, global_proxy, timeout)
            .map_err(RemoteProviderError::Network)?;
        let response = client
            .get(url)
            .send()
            .map_err(|error| RemoteProviderError::Network(redact_sensitive(&error.to_string())))?;
        let status = response.status();
        if !status.is_success() {
            return Err(RemoteProviderError::Http(status.as_u16()));
        }
        return response
            .text()
            .map_err(|error| RemoteProviderError::Network(error.to_string()));
    }

    if url.starts_with("file://") {
        let path = Path::new(&url[7..]);
        return fs::read_to_string(path)
            .map_err(|error| RemoteProviderError::Io(error.to_string()));
    }

    let path = Path::new(url);
    if path.is_file() {
        return fs::read_to_string(path)
            .map_err(|error| RemoteProviderError::Io(error.to_string()));
    }

    Err(RemoteProviderError::Network(format!(
        "unsupported URL or file path: {url}"
    )))
}

pub fn fetch_manifest(
    url: &str,
    per_provider_proxy: Option<&str>,
    global_proxy: Option<&ProxyConfig>,
    timeout: Duration,
) -> Result<ProviderManifest, RemoteProviderError> {
    let text = fetch_text(url, per_provider_proxy, global_proxy, timeout)?;
    let manifest: ProviderManifest = serde_json::from_str(&text)
        .map_err(|error| RemoteProviderError::InvalidManifest(error.to_string()))?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

pub fn fetch_source(
    url: &str,
    per_provider_proxy: Option<&str>,
    global_proxy: Option<&ProxyConfig>,
    timeout: Duration,
) -> Result<String, RemoteProviderError> {
    fetch_text(url, per_provider_proxy, global_proxy, timeout)
}

pub fn resolve_source_url(manifest_url: &str, entry: &str) -> String {
    let entry = entry.trim();
    if entry.starts_with("http://")
        || entry.starts_with("https://")
        || entry.starts_with("file://")
    {
        return entry.to_string();
    }

    if manifest_url.starts_with("http://") || manifest_url.starts_with("https://") {
        let base = manifest_url
            .rfind('/')
            .map(|index| &manifest_url[..index + 1])
            .unwrap_or(manifest_url);
        return format!("{base}{entry}");
    }

    let manifest_path = if manifest_url.starts_with("file://") {
        Path::new(&manifest_url[7..])
    } else {
        Path::new(manifest_url)
    };

    manifest_path
        .parent()
        .map(|parent| parent.join(entry))
        .unwrap_or_else(|| PathBuf::from(entry))
        .to_string_lossy()
        .to_string()
}

fn source_file_name(entry: &str) -> String {
    entry
        .rsplit('/')
        .next()
        .unwrap_or(entry)
        .rsplit('\\')
        .next()
        .unwrap_or(entry)
        .to_string()
}

pub fn compute_checksum(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

pub fn verify_checksum(source: &str, expected: &str) -> Result<(), RemoteProviderError> {
    let actual = compute_checksum(source);
    if actual != expected {
        return Err(RemoteProviderError::ChecksumMismatch {
            expected: expected.to_string(),
            actual,
        });
    }
    Ok(())
}

pub fn cache_remote_provider(
    cache_dir: &Path,
    id: &str,
    manifest_url: &str,
    manifest: &ProviderManifest,
    source: &str,
    resolved_runtime: Option<&Path>,
) -> Result<PathBuf, RemoteProviderError> {
    let provider_dir = cache_dir.join(id);
    fs::create_dir_all(&provider_dir)?;

    let manifest_path = provider_dir.join("provider.json");
    let source_name = source_file_name(&manifest.entry);
    let source_path = provider_dir.join(&source_name);

    if source_path.exists() {
        let backup_path = provider_dir.join(format!("{source_name}.bak"));
        fs::copy(&source_path, &backup_path)?;
    }

    let manifest_json =
        serde_json::to_string_pretty(manifest).map_err(|error| RemoteProviderError::Io(error.to_string()))?;
    fs::write(&manifest_path, manifest_json)?;
    fs::write(&source_path, source)?;

    let meta = RemoteProviderMeta {
        etag: None,
        last_check_at: Some(chrono::Utc::now().to_rfc3339()),
        checksum: Some(compute_checksum(source)),
        source_url: resolve_source_url(manifest_url, &manifest.entry),
        resolved_runtime: resolved_runtime.map(|path| path.display().to_string()),
    };
    let meta_path = provider_dir.join(".meta.json");
    let meta_json =
        serde_json::to_string_pretty(&meta).map_err(|error| RemoteProviderError::Io(error.to_string()))?;
    fs::write(&meta_path, meta_json)?;

    Ok(provider_dir)
}

pub fn load_cached_manifest(provider_dir: &Path) -> Result<ProviderManifest, RemoteProviderError> {
    let manifest_path = provider_dir.join("provider.json");
    let contents = fs::read_to_string(&manifest_path)?;
    let manifest = serde_json::from_str(&contents)
        .map_err(|error| RemoteProviderError::InvalidManifest(error.to_string()))?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

pub fn load_cached_meta(provider_dir: &Path) -> Result<Option<RemoteProviderMeta>, RemoteProviderError> {
    let meta_path = provider_dir.join(".meta.json");
    if !meta_path.exists() {
        return Ok(None);
    }
    let contents = fs::read_to_string(&meta_path)?;
    let meta = serde_json::from_str(&contents)
        .map_err(|error| RemoteProviderError::InvalidManifest(error.to_string()))?;
    Ok(Some(meta))
}

pub fn resolve_runtime(runtime: &str) -> Result<PathBuf, RemoteProviderError> {
    let candidate = if looks_like_absolute_path(runtime) {
        PathBuf::from(runtime)
    } else {
        find_executable_in_path(runtime)
            .ok_or_else(|| RemoteProviderError::RuntimeNotFound(runtime.to_string()))?
    };

    if !candidate.exists() {
        return Err(RemoteProviderError::RuntimeNotFound(runtime.to_string()));
    }

    Ok(candidate)
}

fn looks_like_absolute_path(runtime: &str) -> bool {
    runtime.contains('/') || runtime.contains('\\') || runtime.ends_with(".exe")
}

fn find_executable_in_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        for extension in ["", ".exe", ".cmd", ".bat"] {
            let candidate = dir.join(format!("{name}{extension}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

pub fn validate_runtime_executable(path: &Path) -> Result<(), RemoteProviderError> {
    for flag in ["--version", "--help"] {
        match run_runtime_check(path, flag) {
            Ok(true) => return Ok(()),
            Ok(false) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(RemoteProviderError::RuntimeNotFound(
        path.display().to_string(),
    ))
}

fn run_runtime_check(path: &Path, flag: &str) -> Result<bool, RemoteProviderError> {
    let mut command = Command::new(path);
    command
        .arg(flag)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = command
        .spawn()
        .map_err(|error| RemoteProviderError::RuntimeNotFound(error.to_string()))?;

    let timeout = Duration::from_secs(5);
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status.success()),
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    return Ok(false);
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(error) => return Err(RemoteProviderError::RuntimeNotFound(error.to_string())),
        }
    }
}

#[allow(dead_code)]
pub fn ensure_runtime_resolved(
    runtime: &str,
    resolved_runtime: Option<&str>,
) -> Result<PathBuf, RemoteProviderError> {
    if let Some(path) = resolved_runtime {
        let path = PathBuf::from(path);
        if path.exists() {
            return Ok(path);
        }
    }
    resolve_runtime(runtime)
}

pub fn check_update(
    provider_dir: &Path,
    manifest_url: &str,
    per_provider_proxy: Option<&str>,
    global_proxy: Option<&ProxyConfig>,
    trusted_checksum: Option<&str>,
    timeout: Duration,
) -> Result<UpdateInfo, RemoteProviderError> {
    let client = build_http_client(per_provider_proxy, global_proxy, timeout)
        .map_err(RemoteProviderError::Network)?;

    let meta_path = provider_dir.join(".meta.json");
    let existing_meta: Option<RemoteProviderMeta> = if meta_path.exists() {
        let contents = fs::read_to_string(&meta_path)?;
        serde_json::from_str(&contents)
            .map_err(|error| RemoteProviderError::InvalidManifest(error.to_string()))?
    } else {
        None
    };

    let mut request = client.get(manifest_url);
    if let Some(etag) = existing_meta.as_ref().and_then(|meta| meta.etag.as_ref()) {
        request = request.header("If-None-Match", etag.clone());
    }

    let response = request
        .send()
        .map_err(|error| RemoteProviderError::Network(redact_sensitive(&error.to_string())))?;
    let status = response.status();

    if status.as_u16() == 304 {
        return Ok(UpdateInfo {
            id: provider_dir
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            available: false,
            new_checksum: None,
        });
    }

    if !status.is_success() {
        return Err(RemoteProviderError::Http(status.as_u16()));
    }

    let new_etag = response
        .headers()
        .get("etag")
        .and_then(|value| value.to_str().ok().map(|s| s.to_string()));

    let text = response
        .text()
        .map_err(|error| RemoteProviderError::Network(error.to_string()))?;
    let manifest: ProviderManifest = serde_json::from_str(&text)
        .map_err(|error| RemoteProviderError::InvalidManifest(error.to_string()))?;
    validate_manifest(&manifest)?;

    let new_checksum = manifest.checksums.source.clone();
    let available = match (&new_checksum, trusted_checksum) {
        (Some(new), Some(trusted)) => new != trusted,
        (Some(_), None) => true,
        (None, _) => false,
    };

    let source_url = resolve_source_url(manifest_url, &manifest.entry);
    let mut meta = existing_meta.unwrap_or_else(|| RemoteProviderMeta {
        etag: None,
        last_check_at: None,
        checksum: trusted_checksum.map(|value| value.to_string()),
        source_url: source_url.clone(),
        resolved_runtime: None,
    });
    meta.etag = new_etag;
    meta.last_check_at = Some(chrono::Utc::now().to_rfc3339());
    meta.source_url = source_url;

    let meta_json =
        serde_json::to_string_pretty(&meta).map_err(|error| RemoteProviderError::Io(error.to_string()))?;
    fs::write(&meta_path, meta_json)?;

    Ok(UpdateInfo {
        id: manifest.id,
        available,
        new_checksum,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_source_url_with_relative_entry() {
        let manifest =
            "https://example.com/providers/kimi-coding/provider.json";
        assert_eq!(
            resolve_source_url(manifest, "provider.cjs"),
            "https://example.com/providers/kimi-coding/provider.cjs"
        );
    }

    #[test]
    fn resolve_source_url_with_absolute_entry() {
        let manifest =
            "https://example.com/providers/kimi-coding/provider.json";
        assert_eq!(
            resolve_source_url(
                manifest,
                "https://other.example.com/script.js"
            ),
            "https://other.example.com/script.js"
        );
    }

    #[test]
    fn source_file_name_extracts_last_segment() {
        assert_eq!(source_file_name("provider.cjs"), "provider.cjs");
        assert_eq!(
            source_file_name("https://example.com/dir/provider.cjs"),
            "provider.cjs"
        );
    }

    #[test]
    fn compute_checksum_is_stable() {
        let source = "console.log('hello');";
        let checksum = compute_checksum(source);
        assert!(checksum.starts_with("sha256:"));
        assert_eq!(checksum.len(), 7 + 64);
    }

    #[test]
    fn verify_checksum_matches() {
        let source = "console.log('hello');";
        let checksum = compute_checksum(source);
        assert!(verify_checksum(source, &checksum).is_ok());
    }

    #[test]
    fn verify_checksum_mismatch() {
        let result = verify_checksum("console.log('hello');", "sha256:deadbeef");
        assert!(matches!(result, Err(RemoteProviderError::ChecksumMismatch { .. })));
    }

    #[test]
    fn validate_manifest_rejects_bad_schema_version() {
        let manifest = ProviderManifest {
            schema_version: 2,
            id: "x".to_string(),
            display_name: "X".to_string(),
            description: None,
            runtime: "node".to_string(),
            entry: "x.cjs".to_string(),
            required_env_vars: vec![],
            output: "provider-snapshot-v1".to_string(),
            permissions: vec![],
            checksums: Checksums::default(),
        };
        assert!(matches!(
            validate_manifest(&manifest),
            Err(RemoteProviderError::UnsupportedSchemaVersion(2))
        ));
    }

    #[test]
    fn validate_manifest_rejects_empty_id() {
        let manifest = ProviderManifest {
            schema_version: 1,
            id: "   ".to_string(),
            display_name: "X".to_string(),
            description: None,
            runtime: "node".to_string(),
            entry: "x.cjs".to_string(),
            required_env_vars: vec![],
            output: "provider-snapshot-v1".to_string(),
            permissions: vec![],
            checksums: Checksums::default(),
        };
        assert!(matches!(
            validate_manifest(&manifest),
            Err(RemoteProviderError::InvalidManifest(_))
        ));
    }

    #[test]
    fn cache_round_trip() {
        let temp = tempfile::tempdir().expect("temp dir");
        let manifest = ProviderManifest {
            schema_version: 1,
            id: "kimi-coding".to_string(),
            display_name: "Kimi Coding".to_string(),
            description: None,
            runtime: "node".to_string(),
            entry: "provider.cjs".to_string(),
            required_env_vars: vec!["KIMI_API_KEY".to_string()],
            output: "provider-snapshot-v1".to_string(),
            permissions: vec![],
            checksums: Checksums {
                source: Some(compute_checksum("// source")),
            },
        };
        let dir = cache_remote_provider(
            temp.path(),
            "kimi-coding",
            "https://example.com/provider.json",
            &manifest,
            "// source",
            None,
        )
        .expect("cache provider");

        let loaded = load_cached_manifest(&dir).expect("load manifest");
        assert_eq!(loaded.id, "kimi-coding");

        let meta = load_cached_meta(&dir).expect("load meta").expect("meta exists");
        assert_eq!(meta.source_url, "https://example.com/provider.cjs");
        assert!(meta.checksum.is_some());
    }

    #[test]
    fn resolve_runtime_finds_executable_in_path() {
        let temp = tempfile::tempdir().expect("temp dir");
        let runtime_name = "quotabarwin_test_runtime";
        let script_path = temp.path().join(format!("{runtime_name}.cmd"));
        fs::write(
            &script_path,
            "@echo off\necho 1.0.0\n",
        )
        .expect("write test runtime");

        let original_path = std::env::var_os("PATH");
        let mut paths: Vec<PathBuf> =
            std::env::split_paths(&original_path.clone().unwrap_or_default()).collect();
        paths.push(temp.path().to_path_buf());
        let new_path = std::env::join_paths(paths).expect("join paths");
        std::env::set_var("PATH", &new_path);

        let resolved = resolve_runtime(runtime_name).expect("resolve runtime");
        assert_eq!(resolved, script_path);

        if let Some(path) = original_path {
            std::env::set_var("PATH", path);
        } else {
            std::env::remove_var("PATH");
        }
    }

    #[test]
    fn ensure_runtime_resolved_uses_existing_path() {
        let temp = tempfile::tempdir().expect("temp dir");
        let fake = temp.path().join("node.exe");
        fs::write(&fake, "").expect("write fake");
        let path_str = fake.display().to_string();
        let resolved = ensure_runtime_resolved("node", Some(&path_str)).expect("resolved");
        assert_eq!(resolved, fake);
    }

    #[test]
    fn ensure_runtime_resolved_falls_back_when_missing() {
        let temp = tempfile::tempdir().expect("temp dir");
        let runtime_name = "quotabarwin_fallback_runtime";
        let script_path = temp.path().join(format!("{runtime_name}.cmd"));
        fs::write(&script_path, "@echo off\n").expect("write test runtime");

        let original_path = std::env::var_os("PATH");
        let mut paths: Vec<PathBuf> =
            std::env::split_paths(&original_path.clone().unwrap_or_default()).collect();
        paths.push(temp.path().to_path_buf());
        let new_path = std::env::join_paths(paths).expect("join paths");
        std::env::set_var("PATH", &new_path);

        let missing = temp.path().join("missing.exe");
        let resolved =
            ensure_runtime_resolved(runtime_name, Some(&missing.display().to_string())).expect("fallback");
        assert_eq!(resolved, script_path);

        if let Some(path) = original_path {
            std::env::set_var("PATH", path);
        } else {
            std::env::remove_var("PATH");
        }
    }
}

    #[test]
    fn example_remote_provider_manifests_are_valid() {
        let cargo_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo_root = cargo_dir.parent().expect("repo root");
        let examples_dir = repo_root.join("examples").join("remote-providers");

        for provider_id in ["kimi-coding", "bigmodel-coding-plan", "codex-usage"] {
            let dir = examples_dir.join(provider_id);
            let manifest_path = dir.join("provider.json");
            let source_path = dir.join("provider.cjs");

            let manifest_json = fs::read_to_string(&manifest_path)
                .unwrap_or_else(|error| panic!("failed to read {} manifest: {error}", provider_id));
            let manifest: ProviderManifest = serde_json::from_str(&manifest_json)
                .unwrap_or_else(|error| panic!("failed to parse {} manifest: {error}", provider_id));
            validate_manifest(&manifest)
                .unwrap_or_else(|error| panic!("{} manifest invalid: {error}", provider_id));

            let source = fs::read_to_string(&source_path)
                .unwrap_or_else(|error| panic!("failed to read {} source: {error}", provider_id));
            let expected_checksum = manifest.checksums.source.as_deref().unwrap_or_else(|| {
                panic!("{} manifest is missing checksums.source", provider_id)
            });
            verify_checksum(&source, expected_checksum)
                .unwrap_or_else(|error| panic!("{} checksum mismatch: {error}", provider_id));
        }
    }

#[test]
fn fetch_manifest_reads_local_file_url() {
    let temp = tempfile::tempdir().expect("temp dir");
    let manifest_path = temp.path().join("provider.json");
    fs::write(
        &manifest_path,
        r#"{"schemaVersion":1,"id":"local","displayName":"Local","runtime":"node","entry":"provider.cjs","output":"provider-snapshot-v1"}"#,
    )
    .expect("write manifest");

    let file_url = format!("file://{}", manifest_path.to_string_lossy().replace('\\', "/"));
    let manifest = fetch_manifest(&file_url, None, None, Duration::from_secs(1))
        .expect("fetch manifest from file URL");
    assert_eq!(manifest.id, "local");
}

#[test]
fn fetch_source_reads_local_file_path() {
    let temp = tempfile::tempdir().expect("temp dir");
    let source_path = temp.path().join("provider.cjs");
    fs::write(&source_path, "// local source").expect("write source");

    let source =
        fetch_source(&source_path.to_string_lossy(), None, None, Duration::from_secs(1))
            .expect("fetch source from local path");
    assert_eq!(source, "// local source");
}

#[test]
fn resolve_source_url_with_local_file_manifest_and_relative_entry() {
    let temp = tempfile::tempdir().expect("temp dir");
    let manifest_path = temp.path().join("provider.json");
    let file_url = format!("file://{}", manifest_path.to_string_lossy().replace('\\', "/"));

    let resolved = resolve_source_url(&file_url, "provider.cjs");
    assert!(
        resolved.ends_with("provider.cjs"),
        "resolved should end with provider.cjs: {resolved}"
    );
    assert!(
        resolved.contains(&temp.path().to_string_lossy().replace('\\', "/")),
        "resolved should be under temp dir: {resolved}"
    );
}
