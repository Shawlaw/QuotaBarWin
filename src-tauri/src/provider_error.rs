use serde::{Deserialize, Serialize};

use crate::quota::ProviderSnapshot;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProviderErrorCategory {
    MissingCredential,
    Authentication,
    Network,
    Proxy,
    Timeout,
    Runtime,
    Permission,
    ProviderOutput,
    Unknown,
}

pub fn classify_provider_error(provider: &ProviderSnapshot) -> Option<ProviderErrorCategory> {
    if provider.status != "error" {
        return None;
    }
    let mut detail = provider.error.clone().unwrap_or_default();
    if let Some(diagnostics) = &provider.diagnostics {
        detail.push(' ');
        detail.push_str(&diagnostics.messages.join(" "));
    }
    let detail = detail.to_ascii_lowercase();

    let category = if contains_any(
        &detail,
        &[
            "missing secret",
            "missing credential",
            "missing environment variable",
        ],
    ) {
        ProviderErrorCategory::MissingCredential
    } else if contains_any(
        &detail,
        &[
            "unauthorized",
            "authentication",
            "invalid token",
            "forbidden",
            "401",
            "403",
        ],
    ) {
        ProviderErrorCategory::Authentication
    } else if contains_any(&detail, &["proxy", "socks"]) {
        ProviderErrorCategory::Proxy
    } else if contains_any(&detail, &["timed out", "timeout"]) {
        ProviderErrorCategory::Timeout
    } else if contains_any(&detail, &["permission", "access denied", "not permitted"]) {
        ProviderErrorCategory::Permission
    } else if contains_any(
        &detail,
        &["runtime", "executable", "node.js", "python", "pwsh"],
    ) {
        ProviderErrorCategory::Runtime
    } else if contains_any(&detail, &["output", "json", "protocol", "snapshot"]) {
        ProviderErrorCategory::ProviderOutput
    } else if contains_any(
        &detail,
        &[
            "network",
            "connection",
            "could not connect",
            "connect error",
            "dns",
            "http error",
            "request failed",
        ],
    ) {
        ProviderErrorCategory::Network
    } else {
        ProviderErrorCategory::Unknown
    };
    Some(category)
}

fn contains_any(detail: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| detail.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quota::ProviderSnapshot;

    fn error_provider(message: &str) -> ProviderSnapshot {
        ProviderSnapshot {
            id: "provider".to_string(),
            name: "Provider".to_string(),
            status: "error".to_string(),
            source: "remote".to_string(),
            updated_at: None,
            windows: Vec::new(),
            error: Some(message.to_string()),
            diagnostics: None,
            metadata: None,
        }
    }

    #[test]
    fn classifies_actionable_provider_errors() {
        assert_eq!(
            classify_provider_error(&error_provider("Missing secret API_KEY")),
            Some(ProviderErrorCategory::MissingCredential)
        );
        assert_eq!(
            classify_provider_error(&error_provider("Request timed out")),
            Some(ProviderErrorCategory::Timeout)
        );
        assert_eq!(
            classify_provider_error(&error_provider("Unable to parse provider output JSON")),
            Some(ProviderErrorCategory::ProviderOutput)
        );
    }

    #[test]
    fn classifies_http_transport_failures_by_kind_not_by_os_locale() {
        assert_eq!(
            classify_provider_error(&error_provider(
                "HTTP request timed out after 30.0s with no response from chatgpt.com"
            )),
            Some(ProviderErrorCategory::Timeout)
        );
        // The OS error text is localized on some Windows systems, so the
        // classification must not depend on words like "Connection refused".
        assert_eq!(
            classify_provider_error(&error_provider(
                "Could not connect to chatgpt.com: client error (Connect): tcp connect error: 在其上下文中，该请求的地址无效。 (os error 10049)"
            )),
            Some(ProviderErrorCategory::Network)
        );
        assert_eq!(
            classify_provider_error(&error_provider(
                "Could not connect to chatgpt.com (via the configured proxy): client error (Connect): socks connect error"
            )),
            Some(ProviderErrorCategory::Proxy)
        );
    }
}
