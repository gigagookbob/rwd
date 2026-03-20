// Detects and masks sensitive data before sending to LLM APIs.

pub mod patterns;

use std::collections::BTreeMap;

/// Masking result summary. BTreeMap ensures alphabetical ordering by type name.
pub struct RedactResult {
    pub total_count: usize,
    pub by_type: BTreeMap<String, usize>,
}

impl RedactResult {
    /// Empty result for when redactor is disabled.
    pub fn empty() -> Self {
        Self {
            total_count: 0,
            by_type: BTreeMap::new(),
        }
    }

    /// Merges another result into this one (e.g., Claude + Codex).
    pub fn merge(&mut self, other: RedactResult) {
        self.total_count += other.total_count;
        for (key, count) in other.by_type {
            *self.by_type.entry(key).or_insert(0) += count;
        }
    }

    /// Formats summary like "API_KEY: 3, BEARER_TOKEN: 1".
    pub fn format_summary(&self) -> String {
        self.by_type
            .iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Detects sensitive data in text and replaces with [REDACTED:TYPE].
/// Rules are applied in order — earlier replacements must not match later patterns.
pub fn redact_text(text: &str) -> (String, RedactResult) {
    let rules = patterns::builtin_rules();
    let mut result_text = text.to_string();
    let mut by_type: BTreeMap<String, usize> = BTreeMap::new();
    let mut total_count: usize = 0;

    for rule in rules {
        let count = rule.pattern.find_iter(&result_text).count();
        if count > 0 {
            let replacement = format!("[REDACTED:{}]", rule.name);
            result_text = rule
                .pattern
                .replace_all(&result_text, replacement.as_str())
                .into_owned();
            *by_type.entry(rule.name.to_string()).or_insert(0) += count;
            total_count += count;
        }
    }

    (result_text, RedactResult { total_count, by_type })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_key_masking() {
        let input = "키는 sk-abcdefghijklmnopqrstuvwxyz1234 입니다";
        let (output, result) = redact_text(input);
        assert!(output.contains("[REDACTED:API_KEY]"));
        assert!(!output.contains("sk-abcdefghijklmnopqrstuvwxyz1234"));
        assert_eq!(result.total_count, 1);
        assert_eq!(result.by_type["API_KEY"], 1);
    }

    #[test]
    fn test_aws_key_masking() {
        let input = "AWS 키: AKIAIOSFODNN7EXAMPLE";
        let (output, result) = redact_text(input);
        assert!(output.contains("[REDACTED:AWS_KEY]"));
        assert_eq!(result.total_count, 1);
    }

    #[test]
    fn test_github_token_masking() {
        let input = "ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmn";
        let (output, result) = redact_text(input);
        assert!(output.contains("[REDACTED:GITHUB_TOKEN]"));
        assert_eq!(result.total_count, 1);
    }

    #[test]
    fn test_slack_token_masking() {
        let input = "토큰: xoxb-123456-abcdef";
        let (output, result) = redact_text(input);
        assert!(output.contains("[REDACTED:SLACK_TOKEN]"));
        assert_eq!(result.total_count, 1);
    }

    #[test]
    fn test_bearer_token_masking() {
        let input = "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.test";
        let (output, result) = redact_text(input);
        assert!(output.contains("[REDACTED:BEARER_TOKEN]"));
        assert_eq!(result.total_count, 1);
    }

    #[test]
    fn test_env_secret_matches_quoted_values_only() {
        let input = r#"password = "my_secret_pass""#;
        let (output, result) = redact_text(input);
        assert!(output.contains("[REDACTED:ENV_SECRET]"));
        assert_eq!(result.total_count, 1);
    }

    #[test]
    fn test_env_secret_no_match_without_quotes() {
        let input = "password = some_value";
        let (_, result) = redact_text(input);
        assert_eq!(result.total_count, 0);
    }

    #[test]
    fn test_private_ip_masking() {
        let input = "서버 주소: 192.168.1.100";
        let (output, result) = redact_text(input);
        assert!(output.contains("[REDACTED:PRIVATE_IP]"));
        assert_eq!(result.total_count, 1);
    }

    #[test]
    fn test_private_key_header_masking() {
        let input = "-----BEGIN RSA PRIVATE KEY-----";
        let (output, result) = redact_text(input);
        assert!(output.contains("[REDACTED:PRIVATE_KEY]"));
        assert_eq!(result.total_count, 1);
    }

    #[test]
    fn test_no_sensitive_data_preserves_original() {
        let input = "일반 텍스트입니다. 아무 민감 정보 없음.";
        let (output, result) = redact_text(input);
        assert_eq!(output, input);
        assert_eq!(result.total_count, 0);
        assert!(result.by_type.is_empty());
    }

    #[test]
    fn test_multiple_patterns_matched() {
        let input = "키: sk-abcdefghijklmnopqrstuvwxyz1234\n주소: 10.0.0.1";
        let (output, result) = redact_text(input);
        assert!(output.contains("[REDACTED:API_KEY]"));
        assert!(output.contains("[REDACTED:PRIVATE_IP]"));
        assert_eq!(result.total_count, 2);
        assert_eq!(result.by_type.len(), 2);
    }

    #[test]
    fn test_same_pattern_matched_multiple_times() {
        let input = "10.0.0.1 그리고 192.168.0.1";
        let (_, result) = redact_text(input);
        assert_eq!(result.total_count, 2);
        assert_eq!(result.by_type["PRIVATE_IP"], 2);
    }

    #[test]
    fn test_empty_result_defaults() {
        let result = RedactResult::empty();
        assert_eq!(result.total_count, 0);
        assert!(result.by_type.is_empty());
    }

    #[test]
    fn test_merge_two_results() {
        let mut a = RedactResult {
            total_count: 2,
            by_type: BTreeMap::from([("API_KEY".to_string(), 2)]),
        };
        let b = RedactResult {
            total_count: 1,
            by_type: BTreeMap::from([("API_KEY".to_string(), 1)]),
        };
        a.merge(b);
        assert_eq!(a.total_count, 3);
        assert_eq!(a.by_type["API_KEY"], 3);
    }

    #[test]
    fn test_format_summary_alphabetical() {
        let result = RedactResult {
            total_count: 4,
            by_type: BTreeMap::from([
                ("PRIVATE_IP".to_string(), 1),
                ("API_KEY".to_string(), 3),
            ]),
        };
        assert_eq!(result.format_summary(), "API_KEY: 3, PRIVATE_IP: 1");
    }

    #[test]
    fn test_api_key_short_no_match() {
        let input = "sk-short123";
        let (_, result) = redact_text(input);
        assert_eq!(result.total_count, 0);
    }

    #[test]
    fn test_public_ip_no_match() {
        let input = "서버: 8.8.8.8";
        let (output, result) = redact_text(input);
        assert_eq!(output, input);
        assert_eq!(result.total_count, 0);
    }

    #[test]
    fn test_env_secret_matches_single_quotes() {
        let input = "secret = 'my_secret_value'";
        let (output, result) = redact_text(input);
        assert!(output.contains("[REDACTED:ENV_SECRET]"));
        assert_eq!(result.total_count, 1);
    }

    /// Integration test with realistic session log content.
    #[test]
    fn test_realistic_prompt_combined_masking() {
        let prompt = r#"[Session: abc123]
[USER] .env 파일에 api_key = "sk-proj-abcdefghijklmnopqrstuvwxyz1234" 넣었는데 작동 안 해
[ASSISTANT] 키 형식을 확인해보겠습니다. Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.payload 헤더로 테스트하세요
[USER] AWS 키도 설정했어 AKIAIOSFODNN7EXAMPLE 이거
[ASSISTANT] 사설 서버 10.0.1.50 에 배포하려면 ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmn 토큰이 필요합니다
[USER] -----BEGIN RSA PRIVATE KEY----- 이 키 파일도 필요한가요?"#;

        let (output, result) = redact_text(prompt);

        // Verify all sensitive data is masked
        assert!(output.contains("[REDACTED:ENV_SECRET]"));
        assert!(output.contains("[REDACTED:BEARER_TOKEN]"));
        assert!(output.contains("[REDACTED:AWS_KEY]"));
        assert!(output.contains("[REDACTED:PRIVATE_IP]"));
        assert!(output.contains("[REDACTED:GITHUB_TOKEN]"));
        assert!(output.contains("[REDACTED:PRIVATE_KEY]"));

        // Verify original sensitive data is removed
        assert!(!output.contains("sk-proj-abcdefghijklmnopqrstuvwxyz1234"));
        assert!(!output.contains("AKIAIOSFODNN7EXAMPLE"));
        assert!(!output.contains("10.0.1.50"));

        // Verify non-sensitive text is preserved
        assert!(output.contains("[Session: abc123]"));
        assert!(output.contains("[USER]"));
        assert!(output.contains("작동 안 해"));

        assert_eq!(result.total_count, 6);
        assert_eq!(result.by_type.len(), 6);
    }
}
