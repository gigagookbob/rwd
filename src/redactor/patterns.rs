// 내장 민감 정보 탐지 패턴을 정의합니다.
// LazyLock은 처음 접근 시 한 번만 초기화되는 지연 정적 변수입니다 (std::sync::LazyLock).
// 정규식 컴파일은 비용이 크므로, 한 번만 수행하고 재사용합니다.

use regex::Regex;
use std::sync::LazyLock;

/// 패턴 종류 — 향후 FixedPrefix를 Aho-Corasick으로 교체할 수 있습니다.
/// 현재는 양쪽 모두 Regex로 동작하며, kind는 메타데이터 역할만 합니다.
#[allow(dead_code)]
pub enum PatternKind {
    FixedPrefix,
    Regex,
}

/// 하나의 마스킹 규칙을 나타냅니다.
pub struct RedactorRule {
    pub name: &'static str,
    #[allow(dead_code)]
    pub kind: PatternKind,
    pub pattern: &'static Regex,
}

/// 내장 패턴 목록을 반환합니다.
/// 각 패턴은 LazyLock으로 한 번만 컴파일됩니다.
pub fn builtin_rules() -> Vec<RedactorRule> {
    // LazyLock<Regex>: 처음 접근 시 Regex::new()를 호출하여 컴파일합니다.
    // expect()는 정규식 문법 에러(프로그래밍 에러)일 때 panic합니다 — 런타임 입력이 아니므로 허용됩니다.
    static API_KEY: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\bsk-[a-zA-Z0-9]{20,}\b").expect("API_KEY regex"));
    static AWS_KEY: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\bAKIA[0-9A-Z]{16}\b").expect("AWS_KEY regex"));
    static GITHUB_TOKEN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\bgh[ps]_[a-zA-Z0-9]{36,}\b").expect("GITHUB_TOKEN regex"));
    static SLACK_TOKEN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\bxox[bpsa]-[a-zA-Z0-9\-]+\b").expect("SLACK_TOKEN regex"));
    static BEARER_TOKEN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"Bearer\s+[a-zA-Z0-9\-._~+/]+=*").expect("BEARER_TOKEN regex"));
    static ENV_SECRET: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?i)(password|secret|api_key)\s*=\s*["'][^"']+["']"#).expect("ENV_SECRET regex")
    });
    static PRIVATE_IP: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\b(10\.\d+\.\d+\.\d+|172\.(1[6-9]|2\d|3[01])\.\d+\.\d+|192\.168\.\d+\.\d+)\b")
            .expect("PRIVATE_IP regex")
    });
    static PRIVATE_KEY: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"-----BEGIN[A-Z ]*PRIVATE KEY-----").expect("PRIVATE_KEY regex")
    });

    vec![
        RedactorRule { name: "API_KEY", kind: PatternKind::FixedPrefix, pattern: &API_KEY },
        RedactorRule { name: "AWS_KEY", kind: PatternKind::FixedPrefix, pattern: &AWS_KEY },
        RedactorRule { name: "GITHUB_TOKEN", kind: PatternKind::FixedPrefix, pattern: &GITHUB_TOKEN },
        RedactorRule { name: "SLACK_TOKEN", kind: PatternKind::FixedPrefix, pattern: &SLACK_TOKEN },
        RedactorRule { name: "BEARER_TOKEN", kind: PatternKind::Regex, pattern: &BEARER_TOKEN },
        RedactorRule { name: "ENV_SECRET", kind: PatternKind::Regex, pattern: &ENV_SECRET },
        RedactorRule { name: "PRIVATE_IP", kind: PatternKind::Regex, pattern: &PRIVATE_IP },
        RedactorRule { name: "PRIVATE_KEY", kind: PatternKind::Regex, pattern: &PRIVATE_KEY },
    ]
}
