// Built-in sensitive data detection patterns.
// Each regex is compiled once via LazyLock and reused.

use regex::Regex;
use std::sync::LazyLock;

/// Pattern kind — metadata only, both currently use Regex.
#[allow(dead_code)]
pub enum PatternKind {
    FixedPrefix,
    Regex,
}

/// A single masking rule.
pub struct RedactorRule {
    pub name: &'static str,
    #[allow(dead_code)]
    pub kind: PatternKind,
    pub pattern: &'static Regex,
}

/// Returns the list of built-in redaction rules.
pub fn builtin_rules() -> Vec<RedactorRule> {
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
