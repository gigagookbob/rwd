// LLM API 응답을 구조화된 인사이트 타입으로 파싱하고, 분할 분석 결과를 병합하는 모듈.
//
// serde::Deserialize로 JSON 응답을 자동 변환합니다.
// LLM에게 이 구조와 동일한 JSON 스키마로 응답하도록 프롬프트에서 지시합니다.

use serde::{Deserialize, Serialize};

/// LLM이 String 대신 Object를 반환할 때 방어적으로 처리하는 커스텀 디시리얼라이저.
///
/// serde_json::Value는 JSON의 모든 타입(String, Number, Object, Array, Null 등)을
/// 하나의 enum으로 표현합니다. "일단 어떤 타입이든 받아들이고,
/// String이 아니면 JSON 문자열로 변환"하는 방어 로직입니다.
///
/// 제네릭 라이프타임 `'de`는 serde가 역직렬화 시 빌려오는 데이터의 수명을 나타냅니다.
/// D: Deserializer<'de>는 "어떤 포맷(JSON, TOML 등)의 디시리얼라이저든 받겠다"는 뜻입니다.
fn string_or_stringify<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(deserializer)?;
    match v {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Null => Ok(String::new()),
        other => Ok(other.to_string()),
    }
}

/// Vec<String> 필드용 방어적 디시리얼라이저.
/// 배열의 각 원소가 String이 아닌 Object일 때도 JSON 문자열로 변환합니다.
fn vec_string_or_stringify<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let values: Vec<serde_json::Value> = Vec::deserialize(deserializer)?;
    Ok(values
        .into_iter()
        .map(|v| match v {
            serde_json::Value::String(s) => s,
            serde_json::Value::Null => String::new(),
            other => other.to_string(),
        })
        .collect())
}

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
    #[serde(deserialize_with = "string_or_stringify")]
    pub work_summary: String,
    /// 사용자의 선택 분기 — 어떤 결정을 왜 내렸는가
    pub decisions: Vec<Decision>,
    /// 사용자가 궁금했거나 헷갈렸던 것
    #[serde(deserialize_with = "vec_string_or_stringify")]
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
    #[serde(default, deserialize_with = "string_or_stringify")]
    pub title: String,
    /// 왜 이게 필요했고 어떻게 적용했는지 1-2줄
    #[serde(default, deserialize_with = "string_or_stringify")]
    pub detail: String,
}

/// 사용자의 선택 분기 (A vs B 중 왜 A를 선택했는가)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    #[serde(default, deserialize_with = "string_or_stringify")]
    pub what: String,
    #[serde(default, deserialize_with = "string_or_stringify")]
    pub why: String,
}

/// 모델이 틀려서 사용자가 수정한 것
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Correction {
    #[serde(default, deserialize_with = "string_or_stringify")]
    pub model_said: String,
    #[serde(default, deserialize_with = "string_or_stringify")]
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
        let preview_end = raw_text
            .char_indices()
            .nth(200)
            .map_or(raw_text.len(), |(idx, _)| idx);
        crate::messages::error::json_parse_failed(&e, &raw_text[..preview_end]).into()
    })
}

/// 마크다운 코드 펜스(```json ... ```)를 제거합니다.
///
/// .trim()은 문자열 양끝의 공백을 제거합니다 (Rust Book Ch.8 참조).
/// .strip_prefix(), .strip_suffix()는 특정 접두사/접미사를 제거하고 Option<&str>을 반환합니다.
/// 여러 AnalysisResult를 하나로 병합합니다.
/// 각 결과의 sessions Vec을 순서대로 합칩니다.
/// fallback 시 세션별 개별 분석 결과를 하나의 결과로 조합하기 위해 사용합니다.
pub fn merge_results(results: Vec<AnalysisResult>) -> AnalysisResult {
    let sessions = results
        .into_iter()
        .flat_map(|r| r.sessions)
        .collect();
    AnalysisResult { sessions }
}

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
    fn test_merge_results_여러_결과_병합() {
        let r1 = AnalysisResult {
            sessions: vec![SessionInsight {
                session_id: "s1".to_string(),
                work_summary: "작업1".to_string(),
                decisions: vec![],
                curiosities: vec![],
                corrections: vec![],
                til: vec![],
            }],
        };
        let r2 = AnalysisResult {
            sessions: vec![SessionInsight {
                session_id: "s2".to_string(),
                work_summary: "작업2".to_string(),
                decisions: vec![],
                curiosities: vec![],
                corrections: vec![],
                til: vec![],
            }],
        };
        let merged = merge_results(vec![r1, r2]);
        assert_eq!(merged.sessions.len(), 2);
        assert_eq!(merged.sessions[0].session_id, "s1");
        assert_eq!(merged.sessions[1].session_id, "s2");
    }

    #[test]
    fn test_merge_results_빈_벡터_빈_결과() {
        let merged = merge_results(vec![]);
        assert!(merged.sessions.is_empty());
    }

    #[test]
    fn test_parse_response_til_필드_없어도_기본값_빈배열() {
        let json = r#"{"sessions":[{"session_id":"s1","work_summary":"요약","decisions":[],"curiosities":[],"corrections":[]}]}"#;
        let result = parse_response(json).unwrap();
        assert!(result.sessions[0].til.is_empty());
    }

    // --- LLM 응답 필드 누락 내성 테스트 ---

    #[test]
    fn test_til_title_누락시_파싱_성공() {
        // LLM이 title을 빼고 detail만 반환하는 경우
        let json = r#"{"sessions":[{"session_id":"s1","work_summary":"요약","decisions":[],"curiosities":[],"corrections":[],"til":[{"detail":"이유 설명"}]}]}"#;
        let result = parse_response(json).unwrap();
        assert_eq!(result.sessions[0].til.len(), 1);
        assert_eq!(result.sessions[0].til[0].title, "");
        assert_eq!(result.sessions[0].til[0].detail, "이유 설명");
    }

    #[test]
    fn test_til_detail_누락시_파싱_성공() {
        let json = r#"{"sessions":[{"session_id":"s1","work_summary":"요약","decisions":[],"curiosities":[],"corrections":[],"til":[{"title":"배운 점"}]}]}"#;
        let result = parse_response(json).unwrap();
        assert_eq!(result.sessions[0].til[0].detail, "");
    }

    #[test]
    fn test_decision_필드_누락시_파싱_성공() {
        // LLM이 why를 빼고 what만 반환하는 경우
        let json = r#"{"sessions":[{"session_id":"s1","work_summary":"요약","decisions":[{"what":"serde 선택"}],"curiosities":[],"corrections":[],"til":[]}]}"#;
        let result = parse_response(json).unwrap();
        assert_eq!(result.sessions[0].decisions[0].what, "serde 선택");
        assert_eq!(result.sessions[0].decisions[0].why, "");
    }

    #[test]
    fn test_correction_필드_누락시_파싱_성공() {
        let json = r#"{"sessions":[{"session_id":"s1","work_summary":"요약","decisions":[],"curiosities":[],"corrections":[{"model_said":"잘못된 설명"}],"til":[]}]}"#;
        let result = parse_response(json).unwrap();
        assert_eq!(result.sessions[0].corrections[0].model_said, "잘못된 설명");
        assert_eq!(result.sessions[0].corrections[0].user_corrected, "");
    }

    // --- LLM 응답 타입 이탈 내성 테스트 (map → string 방어) ---

    #[test]
    fn test_curiosities_객체_배열이면_stringify() {
        // 에러 원인: LLM이 curiosities를 문자열 대신 객체 배열로 반환
        let json = r#"{"sessions":[{"session_id":"s1","work_summary":"요약","decisions":[],"curiosities":[{"question":"Xcode 호환성","context":"빌드 실패"}],"corrections":[]}]}"#;
        let result = parse_response(json).unwrap();
        assert_eq!(result.sessions[0].curiosities.len(), 1);
        assert!(result.sessions[0].curiosities[0].contains("Xcode 호환성"));
    }

    #[test]
    fn test_work_summary_객체이면_stringify() {
        // LLM이 work_summary를 객체로 반환하는 경우
        let json = r#"{"sessions":[{"session_id":"s1","work_summary":{"main":"요약","detail":"상세"},"decisions":[],"curiosities":[],"corrections":[]}]}"#;
        let result = parse_response(json).unwrap();
        assert!(result.sessions[0].work_summary.contains("요약"));
    }

    #[test]
    fn test_decision_why_객체이면_stringify() {
        let json = r#"{"sessions":[{"session_id":"s1","work_summary":"요약","decisions":[{"what":"선택","why":{"reason":"이유","context":"맥락"}}],"curiosities":[],"corrections":[]}]}"#;
        let result = parse_response(json).unwrap();
        assert!(result.sessions[0].decisions[0].why.contains("이유"));
    }
}
