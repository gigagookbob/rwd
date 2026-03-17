// redactor 모듈은 LLM API 전송 전 민감 정보를 탐지하고 마스킹하는 역할을 합니다.
// analyzer 모듈에서 build_prompt() 결과에 적용하여, 외부 유출을 차단합니다.

pub mod patterns;

use std::collections::BTreeMap;

/// 마스킹 결과 요약.
/// BTreeMap을 사용하여 타입명 알파벳순 정렬을 보장합니다 (HashMap은 순서 비보장).
pub struct RedactResult {
    pub total_count: usize,
    pub by_type: BTreeMap<String, usize>,
}

impl RedactResult {
    /// redactor 비활성 시 사용하는 빈 결과.
    pub fn empty() -> Self {
        Self {
            total_count: 0,
            by_type: BTreeMap::new(),
        }
    }

    /// 여러 RedactResult를 합산합니다 (Claude + Codex 결과 병합).
    /// entry() API는 키가 없으면 기본값을 삽입하고, 있으면 기존 값에 접근합니다 (Rust Book Ch.8).
    pub fn merge(&mut self, other: RedactResult) {
        self.total_count += other.total_count;
        for (key, count) in other.by_type {
            *self.by_type.entry(key).or_insert(0) += count;
        }
    }

    /// "API_KEY: 3, BEARER_TOKEN: 1" 형식의 요약 문자열을 생성합니다.
    pub fn format_summary(&self) -> String {
        self.by_type
            .iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// 텍스트에서 민감 정보를 탐지하고 [REDACTED:TYPE]으로 치환합니다.
/// 패턴은 LazyLock으로 초기화되므로 이 함수는 실패하지 않습니다.
///
/// 주의: 규칙은 순서대로 적용됩니다. 앞선 규칙의 치환 결과가
/// 뒤따르는 규칙에 매칭되지 않도록 패턴을 설계해야 합니다.
pub fn redact_text(text: &str) -> (String, RedactResult) {
    let rules = patterns::builtin_rules();
    let mut result_text = text.to_string();
    let mut by_type: BTreeMap<String, usize> = BTreeMap::new();
    let mut total_count: usize = 0;

    for rule in rules {
        // find_iter()로 매칭 횟수를 먼저 세고, 매칭이 있을 때만 치환합니다.
        // replace_all()에 &str을 넘기면 캡처 기반 코드 경로를 타지 않아 더 효율적입니다.
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
    fn test_api_key_마스킹() {
        let input = "키는 sk-abcdefghijklmnopqrstuvwxyz1234 입니다";
        let (output, result) = redact_text(input);
        assert!(output.contains("[REDACTED:API_KEY]"));
        assert!(!output.contains("sk-abcdefghijklmnopqrstuvwxyz1234"));
        assert_eq!(result.total_count, 1);
        assert_eq!(result.by_type["API_KEY"], 1);
    }

    #[test]
    fn test_aws_key_마스킹() {
        let input = "AWS 키: AKIAIOSFODNN7EXAMPLE";
        let (output, result) = redact_text(input);
        assert!(output.contains("[REDACTED:AWS_KEY]"));
        assert_eq!(result.total_count, 1);
    }

    #[test]
    fn test_github_token_마스킹() {
        let input = "ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmn";
        let (output, result) = redact_text(input);
        assert!(output.contains("[REDACTED:GITHUB_TOKEN]"));
        assert_eq!(result.total_count, 1);
    }

    #[test]
    fn test_slack_token_마스킹() {
        let input = "토큰: xoxb-123456-abcdef";
        let (output, result) = redact_text(input);
        assert!(output.contains("[REDACTED:SLACK_TOKEN]"));
        assert_eq!(result.total_count, 1);
    }

    #[test]
    fn test_bearer_token_마스킹() {
        let input = "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.test";
        let (output, result) = redact_text(input);
        assert!(output.contains("[REDACTED:BEARER_TOKEN]"));
        assert_eq!(result.total_count, 1);
    }

    #[test]
    fn test_env_secret_따옴표감싼_값만_매칭() {
        let input = r#"password = "my_secret_pass""#;
        let (output, result) = redact_text(input);
        assert!(output.contains("[REDACTED:ENV_SECRET]"));
        assert_eq!(result.total_count, 1);
    }

    #[test]
    fn test_env_secret_따옴표없으면_미매칭() {
        let input = "password = some_value";
        let (_, result) = redact_text(input);
        assert_eq!(result.total_count, 0);
    }

    #[test]
    fn test_private_ip_마스킹() {
        let input = "서버 주소: 192.168.1.100";
        let (output, result) = redact_text(input);
        assert!(output.contains("[REDACTED:PRIVATE_IP]"));
        assert_eq!(result.total_count, 1);
    }

    #[test]
    fn test_private_key_헤더_마스킹() {
        let input = "-----BEGIN RSA PRIVATE KEY-----";
        let (output, result) = redact_text(input);
        assert!(output.contains("[REDACTED:PRIVATE_KEY]"));
        assert_eq!(result.total_count, 1);
    }

    #[test]
    fn test_민감정보_없으면_원본_유지() {
        let input = "일반 텍스트입니다. 아무 민감 정보 없음.";
        let (output, result) = redact_text(input);
        assert_eq!(output, input);
        assert_eq!(result.total_count, 0);
        assert!(result.by_type.is_empty());
    }

    #[test]
    fn test_여러_패턴_동시_매칭() {
        let input = "키: sk-abcdefghijklmnopqrstuvwxyz1234\n주소: 10.0.0.1";
        let (output, result) = redact_text(input);
        assert!(output.contains("[REDACTED:API_KEY]"));
        assert!(output.contains("[REDACTED:PRIVATE_IP]"));
        assert_eq!(result.total_count, 2);
        assert_eq!(result.by_type.len(), 2);
    }

    #[test]
    fn test_같은_패턴_여러번_매칭() {
        let input = "10.0.0.1 그리고 192.168.0.1";
        let (_, result) = redact_text(input);
        assert_eq!(result.total_count, 2);
        assert_eq!(result.by_type["PRIVATE_IP"], 2);
    }

    #[test]
    fn test_empty_결과_기본값() {
        let result = RedactResult::empty();
        assert_eq!(result.total_count, 0);
        assert!(result.by_type.is_empty());
    }

    #[test]
    fn test_merge_두_결과_합산() {
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
    fn test_format_summary_알파벳순() {
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
    fn test_api_key_짧으면_미매칭() {
        let input = "sk-short123";
        let (_, result) = redact_text(input);
        assert_eq!(result.total_count, 0);
    }

    #[test]
    fn test_public_ip_미매칭() {
        let input = "서버: 8.8.8.8";
        let (output, result) = redact_text(input);
        assert_eq!(output, input);
        assert_eq!(result.total_count, 0);
    }

    #[test]
    fn test_env_secret_작은따옴표_매칭() {
        let input = "secret = 'my_secret_value'";
        let (output, result) = redact_text(input);
        assert!(output.contains("[REDACTED:ENV_SECRET]"));
        assert_eq!(result.total_count, 1);
    }

    /// 실제 세션 로그에 가까운 프롬프트 텍스트로 통합 검증합니다.
    #[test]
    fn test_현실적_프롬프트_통합_마스킹() {
        let prompt = r#"[Session: abc123]
[USER] .env 파일에 api_key = "sk-proj-abcdefghijklmnopqrstuvwxyz1234" 넣었는데 작동 안 해
[ASSISTANT] 키 형식을 확인해보겠습니다. Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.payload 헤더로 테스트하세요
[USER] AWS 키도 설정했어 AKIAIOSFODNN7EXAMPLE 이거
[ASSISTANT] 사설 서버 10.0.1.50 에 배포하려면 ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmn 토큰이 필요합니다
[USER] -----BEGIN RSA PRIVATE KEY----- 이 키 파일도 필요한가요?"#;

        let (output, result) = redact_text(prompt);

        // 모든 민감 정보가 마스킹되었는지 확인
        assert!(output.contains("[REDACTED:ENV_SECRET]"));
        assert!(output.contains("[REDACTED:BEARER_TOKEN]"));
        assert!(output.contains("[REDACTED:AWS_KEY]"));
        assert!(output.contains("[REDACTED:PRIVATE_IP]"));
        assert!(output.contains("[REDACTED:GITHUB_TOKEN]"));
        assert!(output.contains("[REDACTED:PRIVATE_KEY]"));

        // 원본 민감 정보가 남아있지 않은지 확인
        assert!(!output.contains("sk-proj-abcdefghijklmnopqrstuvwxyz1234"));
        assert!(!output.contains("AKIAIOSFODNN7EXAMPLE"));
        assert!(!output.contains("10.0.1.50"));

        // 일반 텍스트는 그대로 유지
        assert!(output.contains("[Session: abc123]"));
        assert!(output.contains("[USER]"));
        assert!(output.contains("작동 안 해"));

        assert_eq!(result.total_count, 6);
        assert_eq!(result.by_type.len(), 6);
    }
}
