// Parses LLM JSON responses into structured insight types and merges split results.

use serde::{Deserialize, Serialize};

/// Defensive deserializer: accepts any JSON type and stringifies non-String values.
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

/// Defensive deserializer for Vec<String> fields.
/// Converts non-String array elements (e.g., objects) to JSON strings.
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

/// Top-level response containing all LLM-extracted insights.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub sessions: Vec<SessionInsight>,
}

/// Per-session insight matching the insight categories defined in ARCHITECTURE.md.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInsight {
    pub session_id: String,
    #[serde(deserialize_with = "string_or_stringify")]
    pub work_summary: String,
    /// User's decision points -- what was chosen and why.
    pub decisions: Vec<Decision>,
    /// Questions or confusions the user expressed.
    #[serde(deserialize_with = "vec_string_or_stringify")]
    pub curiosities: Vec<String>,
    /// Cases where the user corrected the model's mistakes.
    pub corrections: Vec<Correction>,
    /// Concrete learnings from this session (title + context).
    #[serde(default)]
    pub til: Vec<TilItem>,
}

/// Today I Learned item.
/// Extracted directly from the conversation by the LLM, not derived from curiosities/corrections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TilItem {
    /// One-line summary of what was learned.
    #[serde(default, deserialize_with = "string_or_stringify")]
    pub title: String,
    /// 1-2 lines on why it mattered and how it was applied.
    #[serde(default, deserialize_with = "string_or_stringify")]
    pub detail: String,
}

/// A decision point (why option A was chosen over B).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    #[serde(default, deserialize_with = "string_or_stringify")]
    pub what: String,
    #[serde(default, deserialize_with = "string_or_stringify")]
    pub why: String,
}

/// A correction where the user fixed a model mistake.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Correction {
    #[serde(default, deserialize_with = "string_or_stringify")]
    pub model_said: String,
    #[serde(default, deserialize_with = "string_or_stringify")]
    pub user_corrected: String,
}

/// Parses Claude API raw text response into an AnalysisResult.
///
/// The LLM is instructed to return JSON only, but sometimes:
/// - wraps it in markdown code fences (```json...```)
/// - prepends free-form text before the JSON object
///
/// We strip those defensively.
pub fn parse_response(raw_text: &str) -> Result<AnalysisResult, super::AnalyzerError> {
    let cleaned = strip_code_fences(raw_text);

    serde_json::from_str::<AnalysisResult>(&cleaned)
        .or_else(|_| extract_json_object(&cleaned))
        .map_err(|e| {
            let preview_end = raw_text
                .char_indices()
                .nth(200)
                .map_or(raw_text.len(), |(idx, _)| idx);
            crate::messages::error::json_parse_failed(&e, &raw_text[..preview_end]).into()
        })
}

/// Extracts a JSON object from text that may contain non-JSON content.
///
/// Tries `{"sessions"` first (most specific), then falls back to any `{`.
fn extract_json_object(text: &str) -> Result<AnalysisResult, serde_json::Error> {
    // Try the most specific marker first.
    if let Some(start) = text.find("{\"sessions\"")
        && let Ok(result) = serde_json::from_str::<AnalysisResult>(&text[start..])
    {
        return Ok(result);
    }
    // Fall back to first '{'.
    if let Some(start) = text.find('{') {
        return serde_json::from_str::<AnalysisResult>(&text[start..]);
    }
    // Nothing found — re-parse full text to produce the original error.
    serde_json::from_str::<AnalysisResult>(text)
}

/// Merges multiple AnalysisResults into one by concatenating their session vecs.
/// Used to combine per-session fallback results into a single output.
pub fn merge_results(results: Vec<AnalysisResult>) -> AnalysisResult {
    let sessions = results.into_iter().flat_map(|r| r.sessions).collect();
    AnalysisResult { sessions }
}

fn strip_code_fences(text: &str) -> String {
    let trimmed = text.trim();

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
    fn test_parse_response_til_field_present() {
        let json = r#"{"sessions":[{"session_id":"s1","work_summary":"요약","decisions":[],"curiosities":[],"corrections":[],"til":[{"title":"serde tag 한계","detail":"중첩 JSON에서 안 먹힌다"}]}]}"#;
        let result = parse_response(json).unwrap();
        assert_eq!(result.sessions[0].til.len(), 1);
        assert_eq!(result.sessions[0].til[0].title, "serde tag 한계");
    }

    #[test]
    fn test_merge_results_multiple() {
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
    fn test_merge_results_empty_vec_empty_result() {
        let merged = merge_results(vec![]);
        assert!(merged.sessions.is_empty());
    }

    #[test]
    fn test_parse_response_til_defaults_to_empty() {
        let json = r#"{"sessions":[{"session_id":"s1","work_summary":"요약","decisions":[],"curiosities":[],"corrections":[]}]}"#;
        let result = parse_response(json).unwrap();
        assert!(result.sessions[0].til.is_empty());
    }

    // --- LLM response field-missing tolerance tests ---

    #[test]
    fn test_til_missing_title_parses_ok() {
        let json = r#"{"sessions":[{"session_id":"s1","work_summary":"요약","decisions":[],"curiosities":[],"corrections":[],"til":[{"detail":"이유 설명"}]}]}"#;
        let result = parse_response(json).unwrap();
        assert_eq!(result.sessions[0].til.len(), 1);
        assert_eq!(result.sessions[0].til[0].title, "");
        assert_eq!(result.sessions[0].til[0].detail, "이유 설명");
    }

    #[test]
    fn test_til_missing_detail_parses_ok() {
        let json = r#"{"sessions":[{"session_id":"s1","work_summary":"요약","decisions":[],"curiosities":[],"corrections":[],"til":[{"title":"배운 점"}]}]}"#;
        let result = parse_response(json).unwrap();
        assert_eq!(result.sessions[0].til[0].detail, "");
    }

    #[test]
    fn test_decision_missing_field_parses_ok() {
        let json = r#"{"sessions":[{"session_id":"s1","work_summary":"요약","decisions":[{"what":"serde 선택"}],"curiosities":[],"corrections":[],"til":[]}]}"#;
        let result = parse_response(json).unwrap();
        assert_eq!(result.sessions[0].decisions[0].what, "serde 선택");
        assert_eq!(result.sessions[0].decisions[0].why, "");
    }

    #[test]
    fn test_correction_missing_field_parses_ok() {
        let json = r#"{"sessions":[{"session_id":"s1","work_summary":"요약","decisions":[],"curiosities":[],"corrections":[{"model_said":"잘못된 설명"}],"til":[]}]}"#;
        let result = parse_response(json).unwrap();
        assert_eq!(result.sessions[0].corrections[0].model_said, "잘못된 설명");
        assert_eq!(result.sessions[0].corrections[0].user_corrected, "");
    }

    // --- LLM response type-mismatch tolerance tests (map -> string defense) ---

    #[test]
    fn test_curiosities_object_array_stringify() {
        let json = r#"{"sessions":[{"session_id":"s1","work_summary":"요약","decisions":[],"curiosities":[{"question":"Xcode 호환성","context":"빌드 실패"}],"corrections":[]}]}"#;
        let result = parse_response(json).unwrap();
        assert_eq!(result.sessions[0].curiosities.len(), 1);
        assert!(result.sessions[0].curiosities[0].contains("Xcode 호환성"));
    }

    #[test]
    fn test_work_summary_object_stringify() {
        let json = r#"{"sessions":[{"session_id":"s1","work_summary":{"main":"요약","detail":"상세"},"decisions":[],"curiosities":[],"corrections":[]}]}"#;
        let result = parse_response(json).unwrap();
        assert!(result.sessions[0].work_summary.contains("요약"));
    }

    #[test]
    fn test_decision_why_object_stringify() {
        let json = r#"{"sessions":[{"session_id":"s1","work_summary":"요약","decisions":[{"what":"선택","why":{"reason":"이유","context":"맥락"}}],"curiosities":[],"corrections":[]}]}"#;
        let result = parse_response(json).unwrap();
        assert!(result.sessions[0].decisions[0].why.contains("이유"));
    }

    #[test]
    fn test_parse_response_with_preamble_text() {
        let raw = r#"Looking at this conversation, here is the analysis:

{"sessions":[{"session_id":"s1","work_summary":"작업 요약","decisions":[],"curiosities":[],"corrections":[]}]}"#;
        let result = parse_response(raw).unwrap();
        assert_eq!(result.sessions.len(), 1);
        assert_eq!(result.sessions[0].session_id, "s1");
    }
}
