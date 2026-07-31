pub fn redact_sensitive(input: &str) -> String {
    input
        .split_whitespace()
        .map(redact_word)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Redacts exact credential values known only for the lifetime of a Provider
/// run, then applies the normal shape- and key-based redaction.
///
/// Sorting longest-first prevents a shorter value from exposing the suffix of
/// a longer value. Callers must not persist the value list.
pub fn redact_sensitive_values(input: &str, sensitive_values: &[String]) -> String {
    redact_sensitive(&redact_exact_sensitive_values(input, sensitive_values))
}

/// Replaces exact runtime credential values without changing unrelated
/// whitespace or formatting in Provider-supplied display data.
pub fn redact_exact_sensitive_values(input: &str, sensitive_values: &[String]) -> String {
    let mut values = sensitive_values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    values.sort_unstable_by_key(|value| std::cmp::Reverse(value.len()));
    values.dedup();

    let mut redacted = input.to_string();
    for value in values {
        redacted = redacted.replace(value, "[REDACTED]");
    }
    redacted
}

fn redact_word(word: &str) -> String {
    let upper = word.to_ascii_uppercase();
    if let Some((key, _value)) = word.split_once('=') {
        let upper_key = key.to_ascii_uppercase();
        if upper_key.contains("API_KEY")
            || upper_key.contains("TOKEN")
            || upper_key.contains("COOKIE")
            || upper_key.contains("SECRET")
        {
            return format!("{key}=[REDACTED]");
        }
    }

    if upper.contains("AUTHORIZATION:")
        || upper == "BEARER"
        || upper.contains("COOKIE")
        || upper.contains("SET-COOKIE")
        || looks_like_long_token(word)
    {
        return "[REDACTED]".to_string();
    }

    if word.contains("Bearer") {
        return "[REDACTED]".to_string();
    }

    word.to_string()
}

fn looks_like_long_token(word: &str) -> bool {
    let token_chars = word
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        })
        .count();
    token_chars >= 32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_authorization_and_tokens_from_logs() {
        let input =
            "Authorization: Bearer abcdefghijklmnopqrstuvwxyz123456 API_KEY=secret Cookie=session";
        let output = redact_sensitive(input);

        assert!(!output.contains("abcdefghijklmnopqrstuvwxyz123456"));
        assert!(!output.contains("API_KEY=secret"));
        assert!(output.contains("API_KEY=[REDACTED]"));
        assert!(!output.contains("Cookie=session"));
        assert!(output.contains("[REDACTED]"));
    }

    #[test]
    fn redacts_authorization_header() {
        let output = redact_sensitive("Authorization: Bearer super-secret-token-value-1234567890");

        assert!(!output.contains("Bearer"));
        assert!(!output.contains("super-secret-token"));
    }

    #[test]
    fn redaction_hides_authorization_bearer() {
        let output = redact_sensitive("Authorization: Bearer real-secret-value-123456789012345");

        assert!(!output.contains("real-secret-value"));
        assert!(output.contains("[REDACTED]"));
    }

    #[test]
    fn redaction_keeps_secret_names_actionable() {
        let output = redact_sensitive(
            "Unable to resolve remote provider environment variable DEEPSEEK_API_KEY: Missing secret DEEPSEEK_API_KEY; create secrets/DEEPSEEK_API_KEY.txt",
        );

        assert!(output.contains("DEEPSEEK_API_KEY"));
        assert!(output.contains("secrets/DEEPSEEK_API_KEY.txt"));
        assert!(!output.contains("[REDACTED]"));
    }

    #[test]
    fn redaction_hides_sensitive_assignment_values() {
        let output = redact_sensitive("DEEPSEEK_API_KEY=super-secret-value");

        assert_eq!(output, "DEEPSEEK_API_KEY=[REDACTED]");
    }

    #[test]
    fn redaction_hides_short_exact_runtime_credentials() {
        let values = vec!["tiny".to_string(), "tiny-secret".to_string()];
        let output =
            redact_sensitive_values("message=tiny-secret raw tiny and tiny-secret", &values);

        assert!(!output.contains("tiny"));
        assert!(!output.contains("tiny-secret"));
        assert_eq!(output, "message=[REDACTED] raw [REDACTED] and [REDACTED]");
    }
}
