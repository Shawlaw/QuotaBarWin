use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::proxy::{build_http_client, ProxyConfig};
use crate::redact::redact_sensitive;

pub const BUILTIN_JS_RUNTIME: &str = "builtin-js";
pub const LEGACY_PROVIDER_MANIFEST_SCHEMA_VERSION: u8 = 1;
pub const BUILTIN_JS_PROVIDER_MANIFEST_SCHEMA_VERSION: u8 = 2;

const CURRENT_APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn is_builtin_js_runtime(runtime: &str) -> bool {
    runtime.trim() == BUILTIN_JS_RUNTIME
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderManifest {
    pub schema_version: u8,
    pub id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(
        default,
        rename = "minAppVersion",
        skip_serializing_if = "Option::is_none"
    )]
    pub min_app_version: Option<String>,
    pub runtime: String,
    pub entry: String,
    #[serde(default, rename = "requiredEnvVars")]
    pub required_env_vars: Vec<String>,
    pub output: String,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default, skip_serializing_if = "ManifestDefaultConfig::is_empty")]
    #[serde(rename = "defaultConfig")]
    pub default_config: ManifestDefaultConfig,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<ProviderParameter>,
    #[serde(default)]
    pub checksums: Checksums,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestDefaultConfig {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(
        default,
        rename = "timeoutSeconds",
        alias = "timeout_seconds",
        alias = "timeout-seconds"
    )]
    pub timeout_seconds: Option<u64>,
    #[serde(
        default,
        rename = "windowLabelOverrides",
        alias = "window_label_overrides",
        alias = "window-label-overrides"
    )]
    pub window_label_overrides: HashMap<String, String>,
    #[serde(
        default,
        rename = "visibleWindowIds",
        alias = "visible_window_ids",
        alias = "visible-window-ids"
    )]
    pub visible_window_ids: Vec<String>,
    #[serde(default, rename = "envVars", alias = "env_vars", alias = "env-vars")]
    pub env_vars: HashMap<String, String>,
}

impl ManifestDefaultConfig {
    fn is_empty(&self) -> bool {
        self.name.as_deref().unwrap_or_default().trim().is_empty()
            && self.timeout_seconds.is_none()
            && self.window_label_overrides.is_empty()
            && self.visible_window_ids.is_empty()
            && self.env_vars.is_empty()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderParameter {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default, rename = "defaultValue")]
    pub default_value: Option<String>,
    #[serde(default)]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default, rename = "helpUrl")]
    pub help_url: Option<String>,
    #[serde(default)]
    pub advanced: bool,
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
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub installed_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
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
    /// The manifest selected by an enabled registry source for this specific
    /// update check. It is intentionally absent for legacy direct-manifest
    /// checks, where the installed manifest URL remains the update target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_manifest_url: Option<String>,
    #[serde(default)]
    pub current_version: Option<String>,
    #[serde(default)]
    pub new_version: Option<String>,
    #[serde(default)]
    pub checked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRegistry {
    pub schema_version: u8,
    pub providers: Vec<ProviderRegistryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRegistryEntry {
    pub id: String,
    pub provider_url: String,
    #[serde(default)]
    pub checksum: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RemoteProviderError {
    Network(String),
    Http(u16),
    InvalidManifest(String),
    ChecksumMismatch { expected: String, actual: String },
    UnsupportedSchemaVersion(u8),
    RequiresAppVersion { required: String, current: String },
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
            RemoteProviderError::RequiresAppVersion { required, current } => write!(
                f,
                "Provider requires QuotaBarWin {required} or newer; current version is {current}. Update QuotaBarWin before updating this provider."
            ),
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
    if !matches!(
        manifest.schema_version,
        LEGACY_PROVIDER_MANIFEST_SCHEMA_VERSION | BUILTIN_JS_PROVIDER_MANIFEST_SCHEMA_VERSION
    ) {
        return Err(RemoteProviderError::UnsupportedSchemaVersion(
            manifest.schema_version,
        ));
    }
    validate_minimum_app_version(manifest)?;
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
    if is_builtin_js_runtime(&manifest.runtime) {
        crate::builtin_js::validate_builtin_js_manifest(manifest)
            .map_err(RemoteProviderError::InvalidManifest)?;
    }
    Ok(())
}

fn validate_remote_manifest(manifest: &ProviderManifest) -> Result<(), RemoteProviderError> {
    validate_manifest(manifest)?;
    if is_builtin_js_runtime(&manifest.runtime)
        && manifest.schema_version != BUILTIN_JS_PROVIDER_MANIFEST_SCHEMA_VERSION
    {
        return Err(RemoteProviderError::InvalidManifest(
            "remote builtin-js manifests require schemaVersion 2 with minAppVersion so older apps reject the update before replacing their cached provider".to_string(),
        ));
    }
    Ok(())
}

fn validate_minimum_app_version(manifest: &ProviderManifest) -> Result<(), RemoteProviderError> {
    let minimum = manifest
        .min_app_version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if manifest.schema_version == LEGACY_PROVIDER_MANIFEST_SCHEMA_VERSION {
        if minimum.is_some() {
            return Err(RemoteProviderError::InvalidManifest(
                "minAppVersion requires schemaVersion 2".to_string(),
            ));
        }
        return Ok(());
    }

    let required = minimum.ok_or_else(|| {
        RemoteProviderError::InvalidManifest(
            "schemaVersion 2 requires a non-empty minAppVersion".to_string(),
        )
    })?;
    let required_version = Version::parse(required).map_err(|error| {
        RemoteProviderError::InvalidManifest(format!(
            "minAppVersion must be a SemVer version without a leading 'v': {error}"
        ))
    })?;
    let current_version = Version::parse(CURRENT_APP_VERSION).map_err(|error| {
        RemoteProviderError::InvalidManifest(format!(
            "current app version is not valid SemVer: {error}"
        ))
    })?;

    if current_version < required_version {
        return Err(RemoteProviderError::RequiresAppVersion {
            required: required.to_string(),
            current: CURRENT_APP_VERSION.to_string(),
        });
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

fn is_http_url(url: &str) -> bool {
    let url = url.trim();
    url.starts_with("http://") || url.starts_with("https://")
}

pub fn fetch_manifest_text(
    url: &str,
    per_provider_proxy: Option<&str>,
    global_proxy: Option<&ProxyConfig>,
    timeout: Duration,
) -> Result<String, RemoteProviderError> {
    fetch_text(url, per_provider_proxy, global_proxy, timeout)
}

pub fn parse_manifest(text: &str) -> Result<ProviderManifest, RemoteProviderError> {
    let manifest: ProviderManifest = serde_json::from_str(text)
        .map_err(|error| RemoteProviderError::InvalidManifest(error.to_string()))?;
    validate_remote_manifest(&manifest)?;
    Ok(manifest)
}

pub fn fetch_manifest(
    url: &str,
    per_provider_proxy: Option<&str>,
    global_proxy: Option<&ProxyConfig>,
    timeout: Duration,
) -> Result<ProviderManifest, RemoteProviderError> {
    let text = fetch_manifest_text(url, per_provider_proxy, global_proxy, timeout)?;
    parse_manifest(&text)
}

pub fn fetch_provider_registry(
    url: &str,
    per_provider_proxy: Option<&str>,
    global_proxy: Option<&ProxyConfig>,
    timeout: Duration,
) -> Result<ProviderRegistry, RemoteProviderError> {
    let text = fetch_text(url, per_provider_proxy, global_proxy, timeout)?;
    let registry: ProviderRegistry = serde_json::from_str(&text)
        .map_err(|error| RemoteProviderError::InvalidManifest(error.to_string()))?;
    if registry.schema_version != 1 {
        return Err(RemoteProviderError::UnsupportedSchemaVersion(
            registry.schema_version,
        ));
    }
    Ok(registry)
}

pub fn fetch_source(
    url: &str,
    per_provider_proxy: Option<&str>,
    global_proxy: Option<&ProxyConfig>,
    timeout: Duration,
) -> Result<String, RemoteProviderError> {
    fetch_text(url, per_provider_proxy, global_proxy, timeout)
}

fn resolve_relative_url(base_url: &str, relative: &str) -> String {
    let relative = relative.trim();
    if relative.starts_with("http://")
        || relative.starts_with("https://")
        || relative.starts_with("file://")
    {
        return relative.to_string();
    }

    if base_url.starts_with("http://") || base_url.starts_with("https://") {
        let base = base_url
            .rfind('/')
            .map(|index| &base_url[..index + 1])
            .unwrap_or(base_url);
        return format!("{base}{relative}");
    }

    let base_path = if base_url.starts_with("file://") {
        Path::new(&base_url[7..])
    } else {
        Path::new(base_url)
    };

    local_path_to_string(
        base_path
            .parent()
            .map(|parent| parent.join(relative))
            .unwrap_or_else(|| PathBuf::from(relative)),
    )
}

pub fn resolve_provider_url(registry_url: &str, provider_url: &str) -> String {
    resolve_relative_url(registry_url, provider_url)
}

pub fn resolve_source_url(manifest_url: &str, entry: &str) -> String {
    let entry = entry.trim();
    if entry.starts_with("http://") || entry.starts_with("https://") || entry.starts_with("file://")
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

    local_path_to_string(
        manifest_path
            .parent()
            .map(|parent| parent.join(entry))
            .unwrap_or_else(|| PathBuf::from(entry)),
    )
}

fn local_path_to_string(path: PathBuf) -> String {
    let path = path.to_string_lossy().to_string();
    if cfg!(windows) {
        path.replace('/', "\\")
    } else {
        path
    }
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
    let now = chrono::Utc::now().to_rfc3339();
    let existing_meta = load_cached_meta(&provider_dir).ok().flatten();

    let manifest_path = provider_dir.join("provider.json");
    let source_name = source_file_name(&manifest.entry);
    let source_path = provider_dir.join(&source_name);

    if source_path.exists() {
        let backup_path = provider_dir.join(format!("{source_name}.bak"));
        fs::copy(&source_path, &backup_path)?;
    }

    let manifest_json = serde_json::to_string_pretty(manifest)
        .map_err(|error| RemoteProviderError::Io(error.to_string()))?;
    fs::write(&manifest_path, manifest_json)?;
    fs::write(&source_path, source)?;

    let meta = RemoteProviderMeta {
        etag: existing_meta.as_ref().and_then(|meta| meta.etag.clone()),
        last_check_at: Some(now.clone()),
        checksum: Some(compute_checksum(source)),
        version: manifest.version.clone(),
        installed_at: existing_meta
            .as_ref()
            .and_then(|meta| meta.installed_at.clone())
            .or_else(|| Some(now.clone())),
        updated_at: Some(now),
        source_url: resolve_source_url(manifest_url, &manifest.entry),
        resolved_runtime: resolved_runtime.map(|path| path.display().to_string()),
    };
    let meta_path = provider_dir.join(".meta.json");
    let meta_json = serde_json::to_string_pretty(&meta)
        .map_err(|error| RemoteProviderError::Io(error.to_string()))?;
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

pub fn load_cached_meta(
    provider_dir: &Path,
) -> Result<Option<RemoteProviderMeta>, RemoteProviderError> {
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
    let meta_path = provider_dir.join(".meta.json");
    let existing_meta: Option<RemoteProviderMeta> = if meta_path.exists() {
        let contents = fs::read_to_string(&meta_path)?;
        serde_json::from_str(&contents)
            .map_err(|error| RemoteProviderError::InvalidManifest(error.to_string()))?
    } else {
        None
    };

    let (text, new_etag) = if is_http_url(manifest_url) {
        let client = build_http_client(per_provider_proxy, global_proxy, timeout)
            .map_err(RemoteProviderError::Network)?;
        let mut request = client.get(manifest_url);
        if let Some(etag) = existing_meta.as_ref().and_then(|meta| meta.etag.as_ref()) {
            request = request.header("If-None-Match", etag.clone());
        }

        let response = request
            .send()
            .map_err(|error| RemoteProviderError::Network(redact_sensitive(&error.to_string())))?;
        let status = response.status();

        if status.as_u16() == 304 {
            let checked_at = chrono::Utc::now().to_rfc3339();
            return Ok(UpdateInfo {
                id: provider_dir
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                available: false,
                new_checksum: None,
                update_manifest_url: None,
                current_version: existing_meta.as_ref().and_then(|meta| meta.version.clone()),
                new_version: existing_meta.as_ref().and_then(|meta| meta.version.clone()),
                checked_at: Some(checked_at),
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
        (text, new_etag)
    } else {
        (
            fetch_manifest_text(manifest_url, per_provider_proxy, global_proxy, timeout)?,
            None,
        )
    };
    let manifest: ProviderManifest = serde_json::from_str(&text)
        .map_err(|error| RemoteProviderError::InvalidManifest(error.to_string()))?;
    validate_remote_manifest(&manifest)?;

    let new_checksum = manifest.checksums.source.clone();
    let available = match (&new_checksum, trusted_checksum) {
        (Some(new), Some(trusted)) => new != trusted,
        (Some(_), None) => true,
        (None, _) => false,
    };

    let checked_at = chrono::Utc::now().to_rfc3339();
    let source_url = resolve_source_url(manifest_url, &manifest.entry);
    let mut meta = existing_meta.unwrap_or_else(|| RemoteProviderMeta {
        etag: None,
        last_check_at: None,
        checksum: trusted_checksum.map(|value| value.to_string()),
        version: None,
        installed_at: None,
        updated_at: None,
        source_url: source_url.clone(),
        resolved_runtime: None,
    });
    let current_version = meta.version.clone();
    meta.etag = new_etag;
    meta.last_check_at = Some(checked_at.clone());
    meta.source_url = source_url;
    if !available && new_checksum.as_deref() == trusted_checksum {
        meta.version = manifest.version.clone();
        let manifest_path = provider_dir.join("provider.json");
        let manifest_json = serde_json::to_string_pretty(&manifest)
            .map_err(|error| RemoteProviderError::Io(error.to_string()))?;
        fs::write(manifest_path, manifest_json)?;
    }

    let meta_json = serde_json::to_string_pretty(&meta)
        .map_err(|error| RemoteProviderError::Io(error.to_string()))?;
    fs::write(&meta_path, meta_json)?;

    Ok(UpdateInfo {
        id: manifest.id,
        available,
        new_checksum,
        update_manifest_url: None,
        current_version,
        new_version: manifest.version,
        checked_at: Some(checked_at),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn path_env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn resolve_source_url_with_relative_entry() {
        let manifest = "https://example.com/providers/kimi-coding/provider.json";
        assert_eq!(
            resolve_source_url(manifest, "provider.cjs"),
            "https://example.com/providers/kimi-coding/provider.cjs"
        );
    }

    #[test]
    fn resolve_source_url_with_absolute_entry() {
        let manifest = "https://example.com/providers/kimi-coding/provider.json";
        assert_eq!(
            resolve_source_url(manifest, "https://other.example.com/script.js"),
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
        assert!(matches!(
            result,
            Err(RemoteProviderError::ChecksumMismatch { .. })
        ));
    }

    #[test]
    fn validate_manifest_rejects_bad_schema_version() {
        let manifest = ProviderManifest {
            schema_version: 3,
            id: "x".to_string(),
            display_name: "X".to_string(),
            version: None,
            description: None,
            min_app_version: None,
            runtime: "node".to_string(),
            entry: "x.cjs".to_string(),
            required_env_vars: vec![],
            output: "provider-snapshot-v1".to_string(),
            permissions: vec![],
            default_config: ManifestDefaultConfig::default(),
            parameters: vec![],
            checksums: Checksums::default(),
        };
        assert!(matches!(
            validate_manifest(&manifest),
            Err(RemoteProviderError::UnsupportedSchemaVersion(3))
        ));
    }

    #[test]
    fn schema_two_requires_a_compatible_minimum_app_version() {
        let valid = parse_manifest(&format!(
            r#"{{"schemaVersion":2,"id":"current","displayName":"Current","minAppVersion":"{CURRENT_APP_VERSION}","runtime":"node","entry":"provider.cjs","output":"provider-snapshot-v1"}}"#
        ));
        assert!(valid.is_ok());

        let missing_minimum = parse_manifest(
            r#"{"schemaVersion":2,"id":"missing","displayName":"Missing","runtime":"node","entry":"provider.cjs","output":"provider-snapshot-v1"}"#,
        );
        assert!(matches!(
            missing_minimum,
            Err(RemoteProviderError::InvalidManifest(message)) if message.contains("minAppVersion")
        ));

        let too_new = parse_manifest(
            r#"{"schemaVersion":2,"id":"future","displayName":"Future","minAppVersion":"999.0.0","runtime":"node","entry":"provider.cjs","output":"provider-snapshot-v1"}"#,
        );
        assert!(matches!(
            too_new,
            Err(RemoteProviderError::RequiresAppVersion { required, current })
                if required == "999.0.0" && current == CURRENT_APP_VERSION
        ));

        let legacy_with_minimum = parse_manifest(
            r#"{"schemaVersion":1,"id":"legacy","displayName":"Legacy","minAppVersion":"1.1.0","runtime":"node","entry":"provider.cjs","output":"provider-snapshot-v1"}"#,
        );
        assert!(matches!(
            legacy_with_minimum,
            Err(RemoteProviderError::InvalidManifest(message)) if message.contains("schemaVersion 2")
        ));
    }

    #[test]
    fn validate_manifest_rejects_empty_id() {
        let manifest = ProviderManifest {
            schema_version: 1,
            id: "   ".to_string(),
            display_name: "X".to_string(),
            version: None,
            description: None,
            min_app_version: None,
            runtime: "node".to_string(),
            entry: "x.cjs".to_string(),
            required_env_vars: vec![],
            output: "provider-snapshot-v1".to_string(),
            permissions: vec![],
            default_config: ManifestDefaultConfig::default(),
            parameters: vec![],
            checksums: Checksums::default(),
        };
        assert!(matches!(
            validate_manifest(&manifest),
            Err(RemoteProviderError::InvalidManifest(_))
        ));
    }

    #[test]
    fn builtin_js_manifest_requires_declared_capabilities() {
        let legacy_builtin = parse_manifest(
            r#"{"schemaVersion":1,"id":"legacy-builtin","displayName":"Legacy builtin","runtime":"builtin-js","entry":"provider.js","output":"provider-snapshot-v1","permissions":["net:https"]}"#,
        );
        assert!(matches!(
            legacy_builtin,
            Err(RemoteProviderError::InvalidManifest(message)) if message.contains("schemaVersion 2")
        ));

        let invalid = parse_manifest(
            r#"{"schemaVersion":2,"id":"builtin","displayName":"Builtin","minAppVersion":"1.1.0","runtime":"builtin-js","entry":"provider.js","requiredEnvVars":["API_TOKEN"],"output":"provider-snapshot-v1"}"#,
        );
        assert!(matches!(
            invalid,
            Err(RemoteProviderError::InvalidManifest(message)) if message.contains("env:API_TOKEN")
        ));

        let valid = parse_manifest(
            r#"{"schemaVersion":2,"id":"builtin","displayName":"Builtin","minAppVersion":"1.1.0","runtime":"builtin-js","entry":"provider.js","requiredEnvVars":["API_TOKEN"],"output":"provider-snapshot-v1","permissions":["env:API_TOKEN","net:https://api.example.test"]}"#,
        )
        .expect("valid builtin manifest");
        assert!(is_builtin_js_runtime(&valid.runtime));
    }

    #[test]
    fn cached_legacy_builtin_manifest_remains_loadable_during_upgrade() {
        let temp = tempfile::tempdir().expect("temp dir");
        let provider_dir = temp.path().join("legacy-builtin");
        fs::create_dir_all(&provider_dir).expect("create provider dir");
        fs::write(
            provider_dir.join("provider.json"),
            r#"{"schemaVersion":1,"id":"legacy-builtin","displayName":"Legacy builtin","runtime":"builtin-js","entry":"provider.js","output":"provider-snapshot-v1","permissions":["net:https"]}"#,
        )
        .expect("write legacy manifest");

        let manifest = load_cached_manifest(&provider_dir).expect("load legacy cache");
        assert!(is_builtin_js_runtime(&manifest.runtime));
        assert!(matches!(
            parse_manifest(
                r#"{"schemaVersion":1,"id":"legacy-builtin","displayName":"Legacy builtin","runtime":"builtin-js","entry":"provider.js","output":"provider-snapshot-v1","permissions":["net:https"]}"#
            ),
            Err(RemoteProviderError::InvalidManifest(message)) if message.contains("schemaVersion 2")
        ));
    }

    #[test]
    fn cache_round_trip() {
        let temp = tempfile::tempdir().expect("temp dir");
        let manifest = ProviderManifest {
            schema_version: 1,
            id: "kimi-coding".to_string(),
            display_name: "Kimi Coding".to_string(),
            version: Some("1.0.0".to_string()),
            description: None,
            min_app_version: None,
            runtime: "node".to_string(),
            entry: "provider.cjs".to_string(),
            required_env_vars: vec!["KIMI_API_KEY".to_string()],
            output: "provider-snapshot-v1".to_string(),
            permissions: vec![],
            default_config: ManifestDefaultConfig::default(),
            parameters: vec![],
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

        let meta = load_cached_meta(&dir)
            .expect("load meta")
            .expect("meta exists");
        assert_eq!(meta.source_url, "https://example.com/provider.cjs");
        assert!(meta.checksum.is_some());
    }

    #[test]
    fn check_update_reads_local_manifest_and_refreshes_metadata_when_source_unchanged() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source = "// source";
        let source_checksum = compute_checksum(source);
        let manifest_path = temp.path().join("provider.json");

        let old_manifest = ProviderManifest {
            schema_version: 1,
            id: "local".to_string(),
            display_name: "Local".to_string(),
            version: Some("1.0.0".to_string()),
            description: None,
            min_app_version: None,
            runtime: "node".to_string(),
            entry: "provider.cjs".to_string(),
            required_env_vars: vec![],
            output: "provider-snapshot-v1".to_string(),
            permissions: vec![],
            default_config: ManifestDefaultConfig::default(),
            parameters: vec![],
            checksums: Checksums {
                source: Some(source_checksum.clone()),
            },
        };
        let provider_dir = cache_remote_provider(
            temp.path(),
            "local",
            &manifest_path.to_string_lossy(),
            &old_manifest,
            source,
            None,
        )
        .expect("cache old provider");

        let mut new_manifest = old_manifest.clone();
        new_manifest.version = Some("1.0.1".to_string());
        new_manifest.parameters = vec![ProviderParameter {
            name: "QBWIN_PROXY_URL".to_string(),
            label: Some("Provider proxy URL".to_string()),
            kind: Some("string".to_string()),
            required: false,
            default_value: None,
            placeholder: Some("http://127.0.0.1:7890".to_string()),
            description: Some("Optional proxy".to_string()),
            options: vec![],
            help_url: None,
            advanced: false,
        }];
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&new_manifest).expect("manifest json"),
        )
        .expect("write new manifest");

        let update = check_update(
            &provider_dir,
            &manifest_path.to_string_lossy(),
            None,
            None,
            Some(&source_checksum),
            Duration::from_secs(1),
        )
        .expect("check local update");

        assert!(!update.available);
        assert_eq!(update.new_version.as_deref(), Some("1.0.1"));

        let cached = load_cached_manifest(&provider_dir).expect("load refreshed manifest");
        assert_eq!(cached.parameters.len(), 1);
        assert_eq!(cached.parameters[0].name, "QBWIN_PROXY_URL");
    }

    #[test]
    fn incompatible_schema_two_update_leaves_cached_provider_untouched() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source = "// source";
        let source_checksum = compute_checksum(source);
        let manifest_path = temp.path().join("provider.json");
        let installed_manifest = ProviderManifest {
            schema_version: LEGACY_PROVIDER_MANIFEST_SCHEMA_VERSION,
            id: "local".to_string(),
            display_name: "Local".to_string(),
            version: Some("1.0.0".to_string()),
            description: None,
            min_app_version: None,
            runtime: "node".to_string(),
            entry: "provider.cjs".to_string(),
            required_env_vars: vec![],
            output: "provider-snapshot-v1".to_string(),
            permissions: vec![],
            default_config: ManifestDefaultConfig::default(),
            parameters: vec![],
            checksums: Checksums {
                source: Some(source_checksum.clone()),
            },
        };
        let provider_dir = cache_remote_provider(
            temp.path(),
            "local",
            &manifest_path.to_string_lossy(),
            &installed_manifest,
            source,
            None,
        )
        .expect("cache installed provider");

        let mut incompatible_manifest = installed_manifest.clone();
        incompatible_manifest.schema_version = BUILTIN_JS_PROVIDER_MANIFEST_SCHEMA_VERSION;
        incompatible_manifest.min_app_version = Some("999.0.0".to_string());
        incompatible_manifest.version = Some("2.0.0".to_string());
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&incompatible_manifest).expect("manifest json"),
        )
        .expect("write incompatible manifest");

        let result = check_update(
            &provider_dir,
            &manifest_path.to_string_lossy(),
            None,
            None,
            Some(&source_checksum),
            Duration::from_secs(1),
        );
        assert!(matches!(
            result,
            Err(RemoteProviderError::RequiresAppVersion { required, .. }) if required == "999.0.0"
        ));

        let cached = load_cached_manifest(&provider_dir).expect("load cached manifest");
        assert_eq!(
            cached.schema_version,
            LEGACY_PROVIDER_MANIFEST_SCHEMA_VERSION
        );
        assert_eq!(cached.version.as_deref(), Some("1.0.0"));
    }

    #[test]
    fn resolve_runtime_finds_executable_in_path() {
        let _guard = path_env_lock().lock().expect("path env lock");
        let temp = tempfile::tempdir().expect("temp dir");
        let runtime_name = "quotabarwin_test_runtime";
        let script_path = temp.path().join(format!("{runtime_name}.cmd"));
        fs::write(&script_path, "@echo off\necho 1.0.0\n").expect("write test runtime");

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
        let _guard = path_env_lock().lock().expect("path env lock");
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
        let resolved = ensure_runtime_resolved(runtime_name, Some(&missing.display().to_string()))
            .expect("fallback");
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
    let registry_json = fs::read_to_string(examples_dir.join("registry.json"))
        .expect("read example provider registry");
    let registry: ProviderRegistry =
        serde_json::from_str(&registry_json).expect("parse example provider registry");

    for provider_id in [
        "kimi-coding",
        "bigmodel-coding-plan",
        "codex-usage",
        "deepseek-balance",
        "time-flies",
    ] {
        let dir = examples_dir.join(provider_id);
        let manifest_path = dir.join("provider.json");

        let manifest_json = fs::read_to_string(&manifest_path)
            .unwrap_or_else(|error| panic!("failed to read {} manifest: {error}", provider_id));
        let manifest: ProviderManifest = serde_json::from_str(&manifest_json)
            .unwrap_or_else(|error| panic!("failed to parse {} manifest: {error}", provider_id));
        validate_remote_manifest(&manifest)
            .unwrap_or_else(|error| panic!("{} manifest invalid: {error}", provider_id));
        assert_eq!(
            manifest.schema_version, BUILTIN_JS_PROVIDER_MANIFEST_SCHEMA_VERSION,
            "{provider_id} must use the builtin-js compatibility manifest schema"
        );
        let required_version = manifest
            .min_app_version
            .as_deref()
            .unwrap_or_else(|| panic!("{provider_id} must declare minAppVersion"));
        let required_version = Version::parse(required_version)
            .unwrap_or_else(|error| panic!("{provider_id} has invalid minAppVersion: {error}"));
        let current_version =
            Version::parse(CURRENT_APP_VERSION).expect("current app version must be valid SemVer");
        assert!(
            required_version <= current_version,
            "{provider_id} must not require a newer builtin-js host than this release"
        );
        let source_path = dir.join(source_file_name(&manifest.entry));

        let source = fs::read_to_string(&source_path)
            .unwrap_or_else(|error| panic!("failed to read {} source: {error}", provider_id));
        let expected_checksum = manifest
            .checksums
            .source
            .as_deref()
            .unwrap_or_else(|| panic!("{} manifest is missing checksums.source", provider_id));
        verify_checksum(&source, expected_checksum)
            .unwrap_or_else(|error| panic!("{} checksum mismatch: {error}", provider_id));

        let registry_entry = registry
            .providers
            .iter()
            .find(|entry| entry.id == provider_id)
            .unwrap_or_else(|| panic!("registry is missing {provider_id}"));
        let expected_manifest_checksum = registry_entry
            .checksum
            .as_deref()
            .unwrap_or_else(|| panic!("registry entry for {provider_id} is missing checksum"));
        verify_checksum(&manifest_json, expected_manifest_checksum).unwrap_or_else(|error| {
            panic!(
                "{} manifest registry checksum mismatch: {error}",
                provider_id
            )
        });
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

    let file_url = format!(
        "file://{}",
        manifest_path.to_string_lossy().replace('\\', "/")
    );
    let manifest = fetch_manifest(&file_url, None, None, Duration::from_secs(1))
        .expect("fetch manifest from file URL");
    assert_eq!(manifest.id, "local");
}

#[test]
fn fetch_source_reads_local_file_path() {
    let temp = tempfile::tempdir().expect("temp dir");
    let source_path = temp.path().join("provider.cjs");
    fs::write(&source_path, "// local source").expect("write source");

    let source = fetch_source(
        &source_path.to_string_lossy(),
        None,
        None,
        Duration::from_secs(1),
    )
    .expect("fetch source from local path");
    assert_eq!(source, "// local source");
}

#[test]
fn resolve_source_url_with_local_file_manifest_and_relative_entry() {
    let temp = tempfile::tempdir().expect("temp dir");
    let manifest_path = temp.path().join("provider.json");
    let file_url = format!(
        "file://{}",
        manifest_path.to_string_lossy().replace('\\', "/")
    );

    let resolved = resolve_source_url(&file_url, "provider.cjs");
    assert!(
        resolved.ends_with("provider.cjs"),
        "resolved should end with provider.cjs: {resolved}"
    );
    assert!(
        resolved
            .replace('\\', "/")
            .contains(&temp.path().to_string_lossy().replace('\\', "/")),
        "resolved should be under temp dir: {resolved}"
    );
}

#[cfg(windows)]
#[test]
fn resolve_source_url_with_local_manifest_uses_windows_separators() {
    let resolved = resolve_source_url(
        r"D:\LocalAgentWorkspace\QuotaBarWin\examples\remote-providers\bigmodel-coding-plan\provider.json",
        "provider.cjs",
    );

    assert_eq!(
        resolved,
        r"D:\LocalAgentWorkspace\QuotaBarWin\examples\remote-providers\bigmodel-coding-plan\provider.cjs"
    );
}

#[test]
fn resolve_provider_url_with_relative_path() {
    assert_eq!(
        resolve_provider_url("https://example.com/registry.json", "kimi/provider.json"),
        "https://example.com/kimi/provider.json"
    );
}

#[test]
fn resolve_provider_url_with_local_registry_and_relative_path() {
    let temp = tempfile::tempdir().expect("temp dir");
    let registry_path = temp.path().join("registry.json");
    let resolved = resolve_provider_url(&registry_path.to_string_lossy(), "kimi/provider.json");
    let resolved_path = Path::new(&resolved);
    assert!(resolved_path.ends_with("kimi/provider.json"));
    assert_eq!(
        resolved_path.parent().unwrap().parent().unwrap(),
        temp.path()
    );
}

#[cfg(windows)]
#[test]
fn resolve_provider_url_with_local_registry_uses_windows_separators() {
    let resolved = resolve_provider_url(
        r"D:\LocalAgentWorkspace\QuotaBarWin\examples\remote-providers\registry.json",
        "bigmodel-coding-plan/provider.json",
    );

    assert_eq!(
        resolved,
        r"D:\LocalAgentWorkspace\QuotaBarWin\examples\remote-providers\bigmodel-coding-plan\provider.json"
    );
}

#[test]
fn fetch_provider_registry_reads_local_registry_file() {
    let temp = tempfile::tempdir().expect("temp dir");
    let registry_path = temp.path().join("registry.json");
    fs::write(
        &registry_path,
        r#"{"schemaVersion":1,"providers":[{"id":"local","providerUrl":"provider.json"}]}"#,
    )
    .expect("write registry");

    let registry = fetch_provider_registry(
        &registry_path.to_string_lossy(),
        None,
        None,
        Duration::from_secs(1),
    )
    .expect("fetch registry");
    assert_eq!(registry.providers.len(), 1);
    assert_eq!(registry.providers[0].id, "local");
}
