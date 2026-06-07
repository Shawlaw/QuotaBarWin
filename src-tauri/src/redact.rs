pub fn redact_sensitive(input: &str) -> String {
    input
        .split_whitespace()
        .map(redact_word)
        .collect::<Vec<_>>()
        .join(" ")
}

fn redact_word(word: &str) -> String {
    let upper = word.to_ascii_uppercase();
    if upper.contains("AUTHORIZATION:")
        || upper == "BEARER"
        || upper.contains("API_KEY")
        || upper.contains("TOKEN")
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
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.'))
        .count();
    token_chars >= 32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_authorization_and_tokens_from_logs() {
        let input = "Authorization: Bearer abcdefghijklmnopqrstuvwxyz123456 API_KEY=secret Cookie=session";
        let output = redact_sensitive(input);

        assert!(!output.contains("abcdefghijklmnopqrstuvwxyz123456"));
        assert!(!output.contains("API_KEY=secret"));
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
}
