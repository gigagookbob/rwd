// LLM API 응답을 구조화된 인사이트 타입으로 파싱하는 모듈.
//
// serde::Deserialize로 JSON 응답을 자동 변환합니다.
// LLM에게 이 구조와 동일한 JSON 스키마로 응답하도록 프롬프트에서 지시합니다.

use serde::{Deserialize, Serialize};

/// LLM이 추출한 인사이트의 전체 응답을 담는 구조체.
/// Debug는 디버그 출력용, Deserialize는 JSON → 구조체 변환용, Serialize는 구조체 → JSON 변환용.
/// Clone은 캐시 저장 시 소유권 이동 없이 복제하기 위해 필요합니다 (Rust Book Ch.4 참조).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub sessions: Vec<SessionInsight>,
}

/// 세션별 인사이트.
/// ARCHITECTURE.md에서 정의한 인사이트 카테고리를 반영합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInsight {
    pub session_id: String,
    pub work_summary: String,
    /// 사용자의 선택 분기 — 어떤 결정을 왜 내렸는가
    pub decisions: Vec<Decision>,
    /// 사용자가 궁금했거나 헷갈렸던 것
    pub curiosities: Vec<String>,
    /// 모델이 틀리거나 몰라서 사용자가 수정한 것
    pub corrections: Vec<Correction>,
    /// 사용자가 이 세션에서 실제로 배운 것 (제목 + 맥락 설명)
    #[serde(default)]
    pub til: Vec<TilItem>,
}

/// 세션에서 배운 것 (Today I Learned).
/// curiosities/corrections에서 파생하지 않고, LLM이 대화에서 직접 추출합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TilItem {
    /// 배운 것을 한 줄로
    pub title: String,
    /// 왜 이게 필요했고 어떻게 적용했는지 1-2줄
    pub detail: String,
}

/// 사용자의 선택 분기 (A vs B 중 왜 A를 선택했는가)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub what: String,
    pub why: String,
}

/// 모델이 틀려서 사용자가 수정한 것
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Correction {
    pub model_said: String,
    pub user_corrected: String,
}

/// Claude API의 원시 텍스트 응답을 AnalysisResult로 파싱합니다.
///
/// LLM이 JSON만 반환하도록 프롬프트에서 지시하지만,
/// 간혹 마크다운 코드 펜스(```json...```)로 감싸는 경우가 있어 방어적으로 처리합니다.
pub fn parse_response(raw_text: &str) -> Result<AnalysisResult, super::AnalyzerError> {
    // 코드 펜스가 있으면 제거합니다.
    let cleaned = strip_code_fences(raw_text);

    serde_json::from_str::<AnalysisResult>(&cleaned).map_err(|e| {
        format!(
            "LLM 응답 JSON 파싱 실패: {e}\n응답 내용 (처음 200자): {}",
            &raw_text[..raw_text.len().min(200)]
        )
        .into()
    })
}

/// 마크다운 코드 펜스(```json ... ```)를 제거합니다.
///
/// .trim()은 문자열 양끝의 공백을 제거합니다 (Rust Book Ch.8 참조).
/// .strip_prefix(), .strip_suffix()는 특정 접두사/접미사를 제거하고 Option<&str>을 반환합니다.
fn strip_code_fences(text: &str) -> String {
    let trimmed = text.trim();

    // ```json 또는 ``` 으로 시작하는 경우 처리
    // let chains: if 조건 안에서 패턴 매칭과 불리언 조건을 연결합니다 (Rust 2024 Edition).
    if let Some(after_prefix) = trimmed.strip_prefix("```json")
        && let Some(content) = after_prefix.strip_suffix("```")
    {
        return content.trim().to_string();
    }
    if let Some(after_prefix) = trimmed.strip_prefix("```")
        && let Some(content) = after_prefix.strip_suffix("```")
    {
        return content.trim().to_string();
    }

    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_response_valid_json_returns_analysis_result() {
        let json = r#"{"sessions":[{"session_id":"s1","work_summary":"파서 모듈 구현","decisions":[{"what":"serde 사용","why":"자동 역직렬화가 편리"}],"curiosities":["serde의 tag 속성은 무엇인가?"],"corrections":[]}]}"#;
        let result = parse_response(json).unwrap();
        assert_eq!(result.sessions.len(), 1);
        assert_eq!(result.sessions[0].decisions.len(), 1);
        assert_eq!(result.sessions[0].curiosities.len(), 1);
        assert_eq!(result.sessions[0].corrections.len(), 0);
    }

    #[test]
    fn test_parse_response_strips_code_fences_and_parses() {
        let json = "```json\n{\"sessions\":[]}\n```";
        let result = parse_response(json).unwrap();
        assert_eq!(result.sessions.len(), 0);
    }

    #[test]
    fn test_parse_response_invalid_json_returns_error() {
        let result = parse_response("이것은 JSON이 아닙니다");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_response_til_필드_포함시_파싱() {
        let json = r#"{"sessions":[{"session_id":"s1","work_summary":"요약","decisions":[],"curiosities":[],"corrections":[],"til":[{"title":"serde tag 한계","detail":"중첩 JSON에서 안 먹힌다"}]}]}"#;
        let result = parse_response(json).unwrap();
        assert_eq!(result.sessions[0].til.len(), 1);
        assert_eq!(result.sessions[0].til[0].title, "serde tag 한계");
    }

    #[test]
    fn test_parse_response_til_필드_없어도_기본값_빈배열() {
        let json = r#"{"sessions":[{"session_id":"s1","work_summary":"요약","decisions":[],"curiosities":[],"corrections":[]}]}"#;
        let result = parse_response(json).unwrap();
        assert!(result.sessions[0].til.is_empty());
    }
}
