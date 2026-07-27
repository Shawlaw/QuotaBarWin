use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    io::Read,
    path::PathBuf,
    time::{Duration, Instant},
};

use chrono::{Local, Utc};
use reqwest::{header, Method, Url};
use rquickjs::{context::intrinsic, CatchResultExt, Context, Error, Function, Runtime};
use serde::{Deserialize, Serialize};

use crate::{
    logger::{LogLevel, LogSink},
    proxy::build_http_client,
    redact::redact_sensitive,
    remote_provider::ProviderManifest,
};

pub const MAX_BUILTIN_JS_MEMORY_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_BUILTIN_JS_STACK_BYTES: usize = 512 * 1024;
const MAX_HTTP_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_HTTP_REQUEST_BODY_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct BuiltinJsRun<'a> {
    pub provider_id: &'a str,
    pub provider_name: &'a str,
    pub manifest: &'a ProviderManifest,
    pub source: &'a str,
    pub env: &'a HashMap<String, String>,
    pub timeout: Duration,
    pub proxy_url: Option<&'a str>,
    pub log: Option<&'a LogSink>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltinJsResult {
    pub json: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Default)]
pub struct BuiltinJsCapabilities {
    exact_env: HashSet<String>,
    env_prefixes: Vec<String>,
    file_permissions: Vec<FilePermission>,
    network_permissions: Vec<NetworkPermission>,
}

#[derive(Debug, Clone)]
enum FilePermission {
    Path(String),
    EnvironmentPath(String),
}

#[derive(Debug, Clone)]
enum NetworkPermission {
    Scheme(String),
    Origin {
        scheme: String,
        host: String,
        port: Option<u16>,
    },
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct JsHttpOptions {
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    headers: BTreeMap<String, String>,
    #[serde(default)]
    body: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsHttpResponse {
    status: u16,
    ok: bool,
    body: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BuiltinJsMeta<'a> {
    provider_id: &'a str,
    manifest_id: &'a str,
    name: &'a str,
    version: Option<&'a str>,
    source_checksum: Option<&'a str>,
    timeout_seconds: u64,
}

#[derive(Clone)]
struct BuiltinJsHost {
    provider_id: String,
    environment: HashMap<String, String>,
    capabilities: BuiltinJsCapabilities,
    home_dir: Option<PathBuf>,
    deadline: Instant,
    proxy_url: Option<String>,
    log: Option<LogSink>,
}

pub fn validate_builtin_js_manifest(manifest: &ProviderManifest) -> Result<(), String> {
    if manifest.output != "provider-snapshot-v1" {
        return Err("builtin-js requires output 'provider-snapshot-v1'".to_string());
    }
    if !manifest.entry.to_ascii_lowercase().ends_with(".js") {
        return Err("builtin-js entry must use a .js file".to_string());
    }

    let capabilities = BuiltinJsCapabilities::from_permissions(&manifest.permissions)?;
    for name in &manifest.required_env_vars {
        if !capabilities.allows_env(name) {
            return Err(format!(
                "builtin-js requiredEnvVars entry '{name}' needs matching permission 'env:{name}' or 'env-prefix:<PREFIX>'"
            ));
        }
    }
    Ok(())
}

impl BuiltinJsCapabilities {
    pub fn from_permissions(permissions: &[String]) -> Result<Self, String> {
        let mut result = Self::default();
        for raw in permissions {
            let permission = raw.trim();
            if permission.is_empty() {
                return Err("builtin-js permissions cannot contain an empty value".to_string());
            }
            if let Some(name) = permission.strip_prefix("env:") {
                validate_environment_name(name, "env")?;
                result.exact_env.insert(name.to_string());
                continue;
            }
            if let Some(prefix) = permission.strip_prefix("env-prefix:") {
                validate_environment_name(prefix, "env-prefix")?;
                result.env_prefixes.push(prefix.to_string());
                continue;
            }
            if let Some(value) = permission.strip_prefix("fs:env:") {
                validate_environment_name(value, "fs:env")?;
                result
                    .file_permissions
                    .push(FilePermission::EnvironmentPath(value.to_string()));
                continue;
            }
            if let Some(path) = permission.strip_prefix("fs:") {
                if path.trim().is_empty() {
                    return Err("builtin-js fs permission needs a path".to_string());
                }
                result
                    .file_permissions
                    .push(FilePermission::Path(path.to_string()));
                continue;
            }
            if let Some(value) = permission.strip_prefix("net:") {
                result
                    .network_permissions
                    .push(parse_network_permission(value)?);
                continue;
            }
            return Err(format!(
                "unsupported builtin-js permission '{permission}'; supported prefixes are env:, env-prefix:, fs:, fs:env:, and net:"
            ));
        }
        Ok(result)
    }

    fn allows_env(&self, name: &str) -> bool {
        self.exact_env.contains(name)
            || self
                .env_prefixes
                .iter()
                .any(|prefix| name.starts_with(prefix))
    }

    fn allows_url(&self, url: &Url) -> bool {
        self.network_permissions
            .iter()
            .any(|permission| match permission {
                NetworkPermission::Scheme(scheme) => url.scheme() == scheme,
                NetworkPermission::Origin { scheme, host, port } => {
                    url.scheme() == scheme
                        && url
                            .host_str()
                            .is_some_and(|value| value.eq_ignore_ascii_case(host))
                        && url.port_or_known_default() == *port
                }
            })
    }

    fn allowed_file_paths(
        &self,
        environment: &HashMap<String, String>,
        home_dir: Option<&std::path::Path>,
    ) -> Vec<PathBuf> {
        self.file_permissions
            .iter()
            .filter_map(|permission| match permission {
                FilePermission::Path(path) => Some(path.as_str()),
                FilePermission::EnvironmentPath(name) => environment.get(name).map(String::as_str),
            })
            .map(|path| expand_home_path_with(path, home_dir))
            .filter_map(|path| fs::canonicalize(path).ok())
            .collect()
    }
}

impl BuiltinJsHost {
    fn read_env(&self, name: &str) -> Result<String, String> {
        if is_reserved_host_env(name) {
            return Err(format!(
                "builtin-js environment access is not available for reserved host variable '{name}'"
            ));
        }
        if !self.capabilities.allows_env(name) {
            return Err(format!(
                "builtin-js environment access is not permitted for '{name}'"
            ));
        }
        self.environment
            .get(name)
            .cloned()
            .ok_or_else(|| format!("builtin-js environment value '{name}' is not configured"))
    }

    fn read_optional_env(&self, name: &str) -> Result<Option<String>, String> {
        if is_reserved_host_env(name) {
            return Err(format!(
                "builtin-js environment access is not available for reserved host variable '{name}'"
            ));
        }
        if !self.capabilities.allows_env(name) {
            return Err(format!(
                "builtin-js environment access is not permitted for '{name}'"
            ));
        }
        Ok(self.environment.get(name).cloned())
    }

    fn read_text_file(&self, requested_path: &str) -> Result<String, String> {
        let actual_path = fs::canonicalize(expand_home_path_with(
            requested_path,
            self.home_dir.as_deref(),
        ))
        .map_err(|error| {
            format!(
                "builtin-js cannot read requested file '{}': {}",
                requested_path,
                redact_sensitive(&error.to_string())
            )
        })?;
        if !self
            .capabilities
            .allowed_file_paths(&self.environment, self.home_dir.as_deref())
            .iter()
            .any(|allowed| allowed == &actual_path)
        {
            return Err(format!(
                "builtin-js file access is not permitted for '{}'",
                actual_path.display()
            ));
        }
        fs::read_to_string(&actual_path).map_err(|error| {
            format!(
                "builtin-js cannot read permitted file '{}': {}",
                actual_path.display(),
                redact_sensitive(&error.to_string())
            )
        })
    }

    fn http_request(&self, raw_url: &str, raw_options: &str) -> Result<String, String> {
        if self.deadline <= Instant::now() {
            return Err("builtin-js provider timed out".to_string());
        }
        let url = Url::parse(raw_url)
            .map_err(|error| format!("builtin-js http URL is invalid: {error}"))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err("builtin-js http only supports http and https URLs".to_string());
        }
        if !self.capabilities.allows_url(&url) {
            return Err(format!(
                "builtin-js network access is not permitted for origin '{}'",
                url.origin().ascii_serialization()
            ));
        }

        let options: JsHttpOptions = serde_json::from_str(raw_options)
            .map_err(|error| format!("builtin-js http options must be JSON: {error}"))?;
        let method = options
            .method
            .as_deref()
            .unwrap_or("GET")
            .to_ascii_uppercase();
        let method = Method::from_bytes(method.as_bytes())
            .map_err(|_| "builtin-js http method is invalid".to_string())?;
        let body = options.body.unwrap_or_default();
        if body.len() > MAX_HTTP_REQUEST_BODY_BYTES {
            return Err(format!(
                "builtin-js http request body exceeds {MAX_HTTP_REQUEST_BODY_BYTES} bytes"
            ));
        }
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("builtin-js provider timed out".to_string());
        }
        let client = build_http_client(self.proxy_url.as_deref(), None, remaining)
            .map_err(|error| format!("builtin-js could not create HTTP client: {error}"))?;
        let mut request = client.request(method, url);
        for (name, value) in options.headers {
            let header_name = header::HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| format!("builtin-js http header name is invalid: {name}"))?;
            let header_value = header::HeaderValue::from_str(&value)
                .map_err(|_| format!("builtin-js http header value is invalid: {name}"))?;
            request = request.header(header_name, header_value);
        }
        let response = request.body(body).send().map_err(|error| {
            format!(
                "builtin-js HTTP request failed: {}",
                redact_sensitive(&error.to_string())
            )
        })?;
        let status = response.status();
        let mut bounded = response.take((MAX_HTTP_RESPONSE_BYTES + 1) as u64);
        let mut bytes = Vec::new();
        bounded.read_to_end(&mut bytes).map_err(|error| {
            format!(
                "builtin-js could not read HTTP response: {}",
                redact_sensitive(&error.to_string())
            )
        })?;
        if bytes.len() > MAX_HTTP_RESPONSE_BYTES {
            return Err(format!(
                "builtin-js HTTP response exceeds {MAX_HTTP_RESPONSE_BYTES} bytes"
            ));
        }
        let body = String::from_utf8(bytes)
            .map_err(|_| "builtin-js HTTP response is not UTF-8 text".to_string())?;
        serde_json::to_string(&JsHttpResponse {
            status: status.as_u16(),
            ok: status.is_success(),
            body,
        })
        .map_err(|error| format!("builtin-js could not encode HTTP response: {error}"))
    }

    fn write_log(&self, raw_entry: &str) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(raw_entry) else {
            return;
        };
        let level = value
            .get("level")
            .and_then(serde_json::Value::as_str)
            .map(LogLevel::parse)
            .unwrap_or(LogLevel::Info);
        let stage = value
            .get("stage")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown");
        let message = value
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if let Some(log) = &self.log {
            let _ = log.write(
                level,
                "builtin_js",
                &format!(
                    "providerId={} providerLog stage={} message={}",
                    self.provider_id,
                    redact_sensitive(stage),
                    redact_sensitive(message)
                ),
            );
        }
    }
}

pub fn run_builtin_js_provider(run: BuiltinJsRun<'_>) -> Result<BuiltinJsResult, String> {
    validate_builtin_js_manifest(run.manifest)?;
    let started = Instant::now();
    let timeout = run.timeout.max(Duration::from_millis(1));
    let deadline = started + timeout;
    let runtime = Runtime::new().map_err(|error| format!("failed to start builtin-js: {error}"))?;
    runtime.set_memory_limit(MAX_BUILTIN_JS_MEMORY_BYTES);
    runtime.set_max_stack_size(MAX_BUILTIN_JS_STACK_BYTES);
    runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));
    let context = Context::builder()
        .with::<intrinsic::Eval>()
        .with::<intrinsic::Date>()
        .with::<intrinsic::Json>()
        .with::<intrinsic::RegExpCompiler>()
        .with::<intrinsic::RegExp>()
        .with::<intrinsic::MapSet>()
        .build(&runtime)
        .map_err(|error| format!("failed to create builtin-js context: {error}"))?;
    let meta = BuiltinJsMeta {
        provider_id: run.provider_id,
        manifest_id: &run.manifest.id,
        name: run.provider_name,
        version: run.manifest.version.as_deref(),
        source_checksum: run.manifest.checksums.source.as_deref(),
        timeout_seconds: timeout.as_secs().max(1),
    };
    let meta_json = serde_json::to_string(&meta)
        .map_err(|error| format!("could not prepare builtin-js metadata: {error}"))?;
    let host = BuiltinJsHost {
        provider_id: run.provider_id.to_string(),
        environment: run.env.clone(),
        capabilities: BuiltinJsCapabilities::from_permissions(&run.manifest.permissions)?,
        home_dir: current_home_dir(),
        deadline,
        proxy_url: run.proxy_url.map(str::to_string),
        log: run.log.cloned(),
    };
    let source = run.source;
    let result = context.with(|ctx| {
        let execute = || -> rquickjs::Result<String> {
            install_host_functions(&ctx, host, &meta_json)?;
            ctx.eval::<(), _>(BUILTIN_JS_BOOTSTRAP)?;
            // The Eval intrinsic is needed by the embedding API, but scripts do not receive the
            // JavaScript eval global as a supported capability.
            ctx.eval::<(), _>("globalThis.eval = undefined;")?;
            ctx.eval::<(), _>(source)?;
            ctx.eval::<String, _>(
                "const __qb_result = main(qb); if (__qb_result && typeof __qb_result.then === 'function') { throw new Error('builtin-js main(qb) must return synchronously'); } const __qb_json = JSON.stringify(__qb_result); if (typeof __qb_json !== 'string') { throw new Error('builtin-js main(qb) must return a JSON-serializable object'); } __qb_json;",
            )
        };
        execute().catch(&ctx).map_err(|error| error.to_string())
    });
    result.map(|json| BuiltinJsResult {
        json,
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

fn install_host_functions(
    ctx: &rquickjs::Ctx<'_>,
    host: BuiltinJsHost,
    meta_json: &str,
) -> rquickjs::Result<()> {
    let globals = ctx.globals();
    let env_host = host.clone();
    globals.set(
        "__qb_env",
        Function::new(ctx.clone(), move |name: String| {
            env_host
                .read_env(&name)
                .map_err(|error| Error::new_from_js_message("builtin-js", "environment", error))
        })?,
    )?;
    let optional_env_host = host.clone();
    globals.set(
        "__qb_optional_env",
        Function::new(ctx.clone(), move |name: String| {
            optional_env_host
                .read_optional_env(&name)
                .map_err(|error| Error::new_from_js_message("builtin-js", "environment", error))
        })?,
    )?;
    let fs_host = host.clone();
    globals.set(
        "__qb_read_text",
        Function::new(ctx.clone(), move |path: String| {
            fs_host
                .read_text_file(&path)
                .map_err(|error| Error::new_from_js_message("builtin-js", "file", error))
        })?,
    )?;
    let http_host = host.clone();
    globals.set(
        "__qb_http_request",
        Function::new(ctx.clone(), move |url: String, options: String| {
            http_host
                .http_request(&url, &options)
                .map_err(|error| Error::new_from_js_message("builtin-js", "http", error))
        })?,
    )?;
    let log_host = host.clone();
    globals.set(
        "__qb_log",
        Function::new(ctx.clone(), move |entry: String| {
            log_host.write_log(&entry);
        })?,
    )?;
    globals.set(
        "__qb_now",
        Function::new(ctx.clone(), || {
            Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        })?,
    )?;
    globals.set(
        "__qb_timezone",
        Function::new(ctx.clone(), || format!("UTC{}", Local::now().format("%:z")))?,
    )?;
    globals.set("__qb_meta_json", meta_json)?;
    Ok(())
}

fn validate_environment_name(value: &str, capability: &str) -> Result<(), String> {
    let valid = !value.trim().is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_');
    if !valid {
        Err(format!(
            "builtin-js {capability} permission must name an environment variable using letters, numbers, or underscores"
        ))
    } else if is_reserved_host_env(value) {
        Err(format!(
            "builtin-js {capability} permission cannot grant reserved QBWIN_ host variables; use qb.meta and qb.http instead"
        ))
    } else {
        Ok(())
    }
}

fn is_reserved_host_env(name: &str) -> bool {
    name.trim().to_ascii_uppercase().starts_with("QBWIN_")
}

fn parse_network_permission(value: &str) -> Result<NetworkPermission, String> {
    match value {
        "http" | "https" => return Ok(NetworkPermission::Scheme(value.to_string())),
        _ => {}
    }
    let url = Url::parse(value).map_err(|_| {
        format!(
            "builtin-js net permission '{value}' must be net:http, net:https, or an http(s) origin"
        )
    })?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(format!(
            "builtin-js net permission '{value}' must use an http(s) origin"
        ));
    }
    if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
        return Err(format!(
            "builtin-js net permission '{value}' must not include a path, query, or fragment"
        ));
    }
    Ok(NetworkPermission::Origin {
        scheme: url.scheme().to_string(),
        host: url.host_str().unwrap_or_default().to_string(),
        port: url.port_or_known_default(),
    })
}

fn current_home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

fn expand_home_path_with(raw: &str, home: Option<&std::path::Path>) -> PathBuf {
    let trimmed = raw.trim();
    if let Some(suffix) = trimmed
        .strip_prefix("~/")
        .or_else(|| trimmed.strip_prefix("~\\"))
    {
        if let Some(home) = home {
            return home.join(suffix);
        }
    }
    PathBuf::from(trimmed)
}

const BUILTIN_JS_BOOTSTRAP: &str = r#"
const qb = Object.freeze({
  env: Object.freeze({
    get: (name) => __qb_env(String(name)),
    getOptional: (name) => __qb_optional_env(String(name))
  }),
  fs: Object.freeze({ readText: (path) => __qb_read_text(String(path)) }),
  http: Object.freeze({
    request: (url, options = {}) => JSON.parse(__qb_http_request(String(url), JSON.stringify(options)))
  }),
  now: () => __qb_now(),
  timezone: () => __qb_timezone(),
  log: (entry) => __qb_log(JSON.stringify(entry)),
  meta: Object.freeze(JSON.parse(__qb_meta_json))
});
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote_provider::{Checksums, ManifestDefaultConfig, ProviderParameter};
    use std::{io::Write, net::TcpListener, thread};

    fn manifest(permissions: &[&str]) -> ProviderManifest {
        ProviderManifest {
            schema_version: 1,
            id: "builtin-test".to_string(),
            display_name: "Builtin Test".to_string(),
            version: Some("1.0.0".to_string()),
            description: None,
            min_app_version: Some("1.1.0".to_string()),
            runtime: "builtin-js".to_string(),
            entry: "provider.js".to_string(),
            required_env_vars: vec![],
            output: "provider-snapshot-v1".to_string(),
            permissions: permissions.iter().map(|value| value.to_string()).collect(),
            default_config: ManifestDefaultConfig::default(),
            parameters: Vec::<ProviderParameter>::new(),
            checksums: Checksums::default(),
        }
    }

    #[test]
    fn parses_capabilities_and_rejects_unknown_permissions() {
        let capabilities = BuiltinJsCapabilities::from_permissions(&[
            "env:API_TOKEN".to_string(),
            "env-prefix:OPTION_".to_string(),
            "fs:env:AUTH_FILE".to_string(),
            "net:https://api.example.test".to_string(),
        ])
        .expect("capabilities");
        assert!(capabilities.allows_env("API_TOKEN"));
        assert!(capabilities.allows_env("OPTION_SECOND"));
        assert!(!capabilities.allows_env("UNDECLARED"));
        assert!(capabilities.allows_url(&Url::parse("https://api.example.test/v1").unwrap()));
        assert!(!capabilities.allows_url(&Url::parse("https://other.example.test/v1").unwrap()));
        assert!(BuiltinJsCapabilities::from_permissions(&["process".to_string()]).is_err());
        assert!(
            BuiltinJsCapabilities::from_permissions(&["env:QBWIN_PROXY_URL".to_string()]).is_err()
        );
    }

    #[test]
    fn manifest_requires_public_output_and_declared_environment_access() {
        let mut manifest = manifest(&[]);
        manifest.required_env_vars = vec!["API_TOKEN".to_string()];
        assert!(validate_builtin_js_manifest(&manifest).is_err());
        manifest.permissions = vec!["env:API_TOKEN".to_string()];
        assert!(validate_builtin_js_manifest(&manifest).is_ok());
        manifest.output = "app-snapshot-v1".to_string();
        assert!(validate_builtin_js_manifest(&manifest).is_err());
    }

    #[test]
    fn runs_sync_script_with_declared_environment_and_metadata() {
        let manifest = manifest(&["env:API_TOKEN"]);
        let env = HashMap::from([("API_TOKEN".to_string(), "configured".to_string())]);
        let result = run_builtin_js_provider(BuiltinJsRun {
            provider_id: "instance-id",
            provider_name: "Instance Name",
            manifest: &manifest,
            source: r#"
                function main(qb) {
                  qb.log({ level: 'info', stage: 'test.run', message: 'started' });
                  return { id: qb.meta.providerId, token: qb.env.get('API_TOKEN'), windows: [] };
                }
            "#,
            env: &env,
            timeout: Duration::from_secs(1),
            proxy_url: None,
            log: None,
        })
        .expect("builtin result");
        let value: serde_json::Value = serde_json::from_str(&result.json).expect("JSON result");
        assert_eq!(value["id"], "instance-id");
        assert_eq!(value["token"], "configured");
    }

    #[test]
    fn rejects_undeclared_environment_and_outside_file_reads() {
        let manifest = manifest(&[]);
        let env = HashMap::new();
        let denied_env = run_builtin_js_provider(BuiltinJsRun {
            provider_id: "builtin-test",
            provider_name: "Builtin Test",
            manifest: &manifest,
            source: "function main(qb) { return { value: qb.env.get('SECRET') }; }",
            env: &env,
            timeout: Duration::from_secs(1),
            proxy_url: None,
            log: None,
        });
        assert!(denied_env
            .unwrap_err()
            .contains("environment access is not permitted"));

        let temp = tempfile::tempdir().expect("temp dir");
        let denied_file = run_builtin_js_provider(BuiltinJsRun {
            provider_id: "builtin-test",
            provider_name: "Builtin Test",
            manifest: &manifest,
            source: &format!(
                "function main(qb) {{ return {{ value: qb.fs.readText({:?}) }}; }}",
                temp.path().join("not-allowed.txt").display().to_string()
            ),
            env: &env,
            timeout: Duration::from_secs(1),
            proxy_url: None,
            log: None,
        });
        assert!(denied_file
            .unwrap_err()
            .contains("cannot read requested file"));
    }

    #[test]
    fn optional_environment_returns_null_without_granting_undeclared_access() {
        let manifest = manifest(&["env:OPTIONAL_VALUE"]);
        let env = HashMap::new();
        let result = run_builtin_js_provider(BuiltinJsRun {
            provider_id: "builtin-test",
            provider_name: "Builtin Test",
            manifest: &manifest,
            source: "function main(qb) { return { configured: qb.env.getOptional('OPTIONAL_VALUE') }; }",
            env: &env,
            timeout: Duration::from_secs(1),
            proxy_url: None,
            log: None,
        })
        .expect("optional environment result");
        let value: serde_json::Value = serde_json::from_str(&result.json).expect("JSON result");
        assert!(value["configured"].is_null());
    }

    #[test]
    fn reads_a_tilde_path_when_the_matching_file_permission_is_granted() {
        let temp = tempfile::tempdir().expect("temp dir");
        let auth_file = temp.path().join(".codex").join("auth.json");
        std::fs::create_dir(auth_file.parent().expect("auth parent")).expect("auth parent");
        std::fs::write(&auth_file, "token-from-auth-file").expect("auth file");

        let manifest = manifest(&["fs:~/.codex/auth.json"]);
        let capabilities = BuiltinJsCapabilities::from_permissions(&manifest.permissions)
            .expect("capabilities");
        let host = BuiltinJsHost {
            provider_id: "builtin-test".to_string(),
            environment: HashMap::new(),
            capabilities,
            home_dir: Some(temp.path().to_path_buf()),
            deadline: Instant::now() + Duration::from_secs(1),
            proxy_url: None,
            log: None,
        };

        assert_eq!(
            host.read_text_file("~/.codex/auth.json").unwrap(),
            "token-from-auth-file"
        );
    }

    #[test]
    fn expands_tilde_paths_with_the_same_home_directory_for_permission_and_reading() {
        let temp = tempfile::tempdir().expect("temp dir");
        let expanded = expand_home_path_with("~\\.codex\\auth.json", Some(temp.path()));
        assert_eq!(expanded, temp.path().join(".codex").join("auth.json"));
    }

    #[test]
    fn http_capability_uses_only_the_declared_origin() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("connection");
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request).expect("request");
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}",
                )
                .expect("response");
        });
        let origin = format!("http://{address}");
        let manifest = manifest(&[&format!("net:{origin}")]);
        let env = HashMap::new();
        let result = run_builtin_js_provider(BuiltinJsRun {
            provider_id: "builtin-test",
            provider_name: "Builtin Test",
            manifest: &manifest,
            source: &format!(
                "function main(qb) {{ const response = qb.http.request('{origin}/usage'); return {{ status: response.status, body: JSON.parse(response.body) }}; }}"
            ),
            env: &env,
            timeout: Duration::from_secs(2),
            proxy_url: None,
            log: None,
        })
        .expect("HTTP result");
        server.join().expect("server");
        let value: serde_json::Value = serde_json::from_str(&result.json).expect("JSON result");
        assert_eq!(value["status"], 200);
        assert_eq!(value["body"]["ok"], true);
    }

    #[test]
    fn interrupts_unbounded_javascript() {
        let manifest = manifest(&[]);
        let env = HashMap::new();
        let error = run_builtin_js_provider(BuiltinJsRun {
            provider_id: "builtin-test",
            provider_name: "Builtin Test",
            manifest: &manifest,
            source: "function main(qb) { while (true) {} }",
            env: &env,
            timeout: Duration::from_millis(20),
            proxy_url: None,
            log: None,
        })
        .unwrap_err();
        assert!(!error.is_empty());
    }
}
