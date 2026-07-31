use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::redact::redact_sensitive;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProxyConfig {
    pub kind: ProxyKind,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ProxyKind {
    None,
    Http,
    Socks5,
    System,
}

const PROXY_TEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProxyTestResult {
    pub success: bool,
    pub status_code: Option<u16>,
    pub elapsed_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<ProxyTestErrorKind>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ProxyTestErrorKind {
    NoProxy,
    InvalidTarget,
    InvalidProxy,
    RequestFailed,
    HttpStatus,
}

/// Build a `reqwest::blocking::Client` honoring the proxy priority:
///
/// 1. per-provider proxy URL (non-empty)
/// 2. global config proxy (Http / Socks5)
/// 3. system proxy (read from `HTTP_PROXY` / `HTTPS_PROXY` env vars)
/// 4. no proxy
pub fn build_http_client(
    per_provider_proxy: Option<&str>,
    global_proxy: Option<&ProxyConfig>,
    timeout: Duration,
) -> Result<reqwest::blocking::Client, String> {
    let mut builder = reqwest::blocking::Client::builder().timeout(timeout);

    if let Some(proxy_url) = select_proxy_url(per_provider_proxy, global_proxy) {
        let proxy = reqwest::Proxy::all(&proxy_url).map_err(|error| {
            format!(
                "Invalid proxy URL: {}",
                redact_sensitive(&error.to_string())
            )
        })?;
        builder = builder.proxy(proxy);
    } else {
        // reqwest otherwise reads HTTP(S)_PROXY on its own. Keep the documented
        // policy explicit: system environment proxies apply only when the
        // project proxy is configured as System.
        builder = builder.no_proxy();
    }

    builder.build().map_err(|error| error.to_string())
}

pub fn select_proxy_url(
    per_provider_proxy: Option<&str>,
    global_proxy: Option<&ProxyConfig>,
) -> Option<String> {
    if let Some(url) = per_provider_proxy
        .map(str::trim)
        .filter(|url| !url.is_empty())
    {
        return Some(url.to_string());
    }

    if let Some(config) = global_proxy {
        match config.kind {
            ProxyKind::None => return None,
            ProxyKind::Http | ProxyKind::Socks5 => {
                let url = config.url.trim();
                if !url.is_empty() {
                    return Some(url.to_string());
                }
            }
            ProxyKind::System => {
                if let Some(url) = system_proxy_url() {
                    return Some(url);
                }
            }
        }
    }

    None
}

fn elapsed_millis(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

fn proxy_test_failure(started: Instant, error_kind: ProxyTestErrorKind) -> ProxyTestResult {
    ProxyTestResult {
        success: false,
        status_code: None,
        elapsed_ms: elapsed_millis(started),
        error_kind: Some(error_kind),
    }
}

/// Tests a configured proxy without exposing request URLs, response bodies, or
/// transport error details to the frontend. The caller should use an HTTPS
/// target so a proxy cannot silently alter the request destination.
pub fn test_proxy_connection(proxy: &ProxyConfig, target_url: &str) -> ProxyTestResult {
    let started = Instant::now();
    let target = match reqwest::Url::parse(target_url.trim()) {
        Ok(target) if target.scheme() == "https" => target,
        _ => return proxy_test_failure(started, ProxyTestErrorKind::InvalidTarget),
    };

    if select_proxy_url(None, Some(proxy)).is_none() {
        return proxy_test_failure(started, ProxyTestErrorKind::NoProxy);
    }

    let client = match build_http_client(None, Some(proxy), PROXY_TEST_TIMEOUT) {
        Ok(client) => client,
        Err(_) => return proxy_test_failure(started, ProxyTestErrorKind::InvalidProxy),
    };

    match client.get(target).send() {
        Ok(response) => {
            let status = response.status();
            let success = status.is_success() || status.is_redirection();
            ProxyTestResult {
                success,
                status_code: Some(status.as_u16()),
                elapsed_ms: elapsed_millis(started),
                error_kind: (!success).then_some(ProxyTestErrorKind::HttpStatus),
            }
        }
        Err(_) => proxy_test_failure(started, ProxyTestErrorKind::RequestFailed),
    }
}

#[tauri::command]
pub async fn test_network_proxy(
    proxy: ProxyConfig,
    target_url: String,
) -> Result<ProxyTestResult, String> {
    tauri::async_runtime::spawn_blocking(move || test_proxy_connection(&proxy, &target_url))
        .await
        .map_err(|_| "Proxy test could not be started".to_string())
}

fn system_proxy_url() -> Option<String> {
    std::env::var("HTTPS_PROXY")
        .or_else(|_| std::env::var("https_proxy"))
        .or_else(|_| std::env::var("HTTP_PROXY"))
        .or_else(|_| std::env::var("http_proxy"))
        .ok()
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn per_provider_proxy_wins_over_global() {
        let global = ProxyConfig {
            kind: ProxyKind::Http,
            url: "http://global:8080".to_string(),
        };
        let url = select_proxy_url(Some("http://provider:9090"), Some(&global));
        assert_eq!(url, Some("http://provider:9090".to_string()));
    }

    #[test]
    fn empty_per_provider_falls_back_to_global() {
        let global = ProxyConfig {
            kind: ProxyKind::Socks5,
            url: "socks5h://global:1080".to_string(),
        };
        let url = select_proxy_url(Some("   "), Some(&global));
        assert_eq!(url, Some("socks5h://global:1080".to_string()));
    }

    #[test]
    fn none_global_disables_proxy() {
        let global = ProxyConfig {
            kind: ProxyKind::None,
            url: "".to_string(),
        };
        let url = select_proxy_url(None, Some(&global));
        assert_eq!(url, None);
    }

    #[test]
    fn system_global_reads_env_var() {
        let _lock = TEST_ENV_LOCK.lock().expect("test environment lock");
        let previous = std::env::var_os("HTTPS_PROXY");
        std::env::set_var("HTTPS_PROXY", "http://system:3128");
        let global = ProxyConfig {
            kind: ProxyKind::System,
            url: "".to_string(),
        };
        let url = select_proxy_url(None, Some(&global));
        assert_eq!(url, Some("http://system:3128".to_string()));
        if let Some(previous) = previous {
            std::env::set_var("HTTPS_PROXY", previous);
        } else {
            std::env::remove_var("HTTPS_PROXY");
        }
    }

    #[test]
    fn build_client_accepts_http_and_socks_proxy_urls() {
        let global = ProxyConfig {
            kind: ProxyKind::Http,
            url: "http://127.0.0.1:7890".to_string(),
        };
        build_http_client(None, Some(&global), Duration::from_secs(1)).expect("http proxy client");

        let global = ProxyConfig {
            kind: ProxyKind::Socks5,
            url: "socks5h://127.0.0.1:7890".to_string(),
        };
        build_http_client(None, Some(&global), Duration::from_secs(1)).expect("socks proxy client");
    }

    #[test]
    fn proxy_test_rejects_non_https_targets() {
        let proxy = ProxyConfig {
            kind: ProxyKind::Http,
            url: "http://127.0.0.1:7890".to_string(),
        };

        let result = test_proxy_connection(&proxy, "http://github.com/");

        assert!(!result.success);
        assert_eq!(result.error_kind, Some(ProxyTestErrorKind::InvalidTarget));
        assert_eq!(result.status_code, None);
    }

    #[test]
    fn proxy_test_requires_a_configured_proxy() {
        let proxy = ProxyConfig {
            kind: ProxyKind::None,
            url: "".to_string(),
        };

        let result = test_proxy_connection(&proxy, "https://github.com/");

        assert!(!result.success);
        assert_eq!(result.error_kind, Some(ProxyTestErrorKind::NoProxy));
        assert_eq!(result.status_code, None);
    }
}
