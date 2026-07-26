use std::time::Duration;

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
}
