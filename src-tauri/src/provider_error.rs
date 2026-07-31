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
}
