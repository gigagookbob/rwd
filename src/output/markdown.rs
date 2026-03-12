// AnalysisResult를 Markdown 문자열로 변환하는 모듈.
//
// I/O가 없는 순수 함수로 구성되어 단위 테스트가 쉽습니다.
// format! 매크로와 String::push_str()로 문자열을 조립합니다 (Rust Book Ch.8.2 참조).

use chrono::NaiveDate;

use crate::analyzer::insight::{AnalysisResult, SessionInsight};

/// AnalysisResult와 날짜를 받아 Markdown 문자열을 생성합니다.
///
/// String::new()는 빈 문자열을 힙에 할당합니다.
/// push_str()은 문자열 슬라이스(&str)를 String 끝에 추가합니다 (Rust Book Ch.8.2 참조).
/// format!은 포매팅된 새 String을 반환하는 매크로입니다.
pub fn render_markdown(analysis: &AnalysisResult, date: NaiveDate) -> String {
    let mut md = String::new();

    // 제목 — NaiveDate는 Display 트레이트를 구현하여 "YYYY-MM-DD" 형식으로 출력됩니다.
    md.push_str(&format!("# {date} Dev Session Review\n\n"));

    // TIL 수집용 Vec — 모든 세션에서 curiosities와 corrections를 모읍니다.
    let mut til_items: Vec<String> = Vec::new();

    for session in &analysis.sessions {
        render_session(&mut md, session, &mut til_items);
    }

    // TIL 섹션 — 전체 세션에서 수집한 학습 항목을 하나로 합칩니다.
    render_til_section(&mut md, &til_items);

    md
}

/// 세션 하나의 Markdown을 생성하고, TIL 항목도 수집합니다.
///
/// &mut String은 가변 참조로, 함수 안에서 문자열에 내용을 추가할 수 있습니다 (Rust Book Ch.4 참조).
/// &mut Vec<String>도 마찬가지로 벡터에 항목을 추가할 수 있게 합니다.
fn render_session(md: &mut String, session: &SessionInsight, til_items: &mut Vec<String>) {
    md.push_str(&format!("## Session: {}\n\n", session.session_id));
    md.push_str(&format!("### 작업 요약\n{}\n\n", session.work_summary));

    // 비어있는 섹션은 출력하지 않습니다.
    // .is_empty()는 Vec이나 String의 길이가 0인지 확인합니다.
    if !session.decisions.is_empty() {
        md.push_str("### 주요 의사결정\n");
        for d in &session.decisions {
            md.push_str(&format!("- **{}**: {}\n", d.what, d.why));
        }
        md.push('\n');
    }

    if !session.curiosities.is_empty() {
        md.push_str("### 궁금했던 것 / 헷갈렸던 것\n");
        for c in &session.curiosities {
            md.push_str(&format!("- {c}\n"));
            // TIL에도 추가 — 궁금했던 것은 배움의 시작점입니다.
            til_items.push(c.clone());
        }
        md.push('\n');
    }

    if !session.corrections.is_empty() {
        md.push_str("### 모델 오류 및 수정\n");
        for c in &session.corrections {
            md.push_str(&format!("- **모델**: {}\n  **수정**: {}\n", c.model_said, c.user_corrected));
            // 모델 수정 사항도 학습 경험이므로 TIL에 추가합니다.
            til_items.push(c.user_corrected.clone());
        }
        md.push('\n');
    }

    md.push_str("---\n\n");
}

/// TIL(Today I Learned) 섹션을 Markdown에 추가합니다.
///
/// Vec::dedup()는 연속된 중복 요소만 제거하므로, sort() 후 호출합니다 (Rust Book Ch.8.1 참조).
fn render_til_section(md: &mut String, til_items: &[String]) {
    if til_items.is_empty() {
        return;
    }

    // 중복 제거를 위해 정렬 후 dedup합니다.
    // .to_vec()로 소유권을 가진 복사본을 만듭니다 — 원본 슬라이스는 변경할 수 없기 때문입니다.
    let mut unique_items = til_items.to_vec();
    unique_items.sort();
    unique_items.dedup();

    md.push_str("## TIL (Today I Learned)\n");
    for item in &unique_items {
        md.push_str(&format!("- {item}\n"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::insight::{Correction, Decision, SessionInsight};

    /// 테스트용 AnalysisResult를 생성하는 헬퍼 함수.
    /// 필드가 모두 pub이므로 구조체 리터럴로 직접 생성할 수 있습니다.
    fn make_test_analysis() -> AnalysisResult {
        AnalysisResult {
            sessions: vec![SessionInsight {
                session_id: "test-session-1".to_string(),
                work_summary: "파서 모듈 구현".to_string(),
                decisions: vec![Decision {
                    what: "serde 사용".to_string(),
                    why: "자동 역직렬화가 편리".to_string(),
                }],
                curiosities: vec!["serde의 tag 속성이란?".to_string()],
                corrections: vec![],
            }],
        }
    }

    fn test_date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 3, 11).expect("유효한 날짜")
    }

    #[test]
    fn test_render_markdown_단일세션_제목과_요약_포함() {
        let analysis = make_test_analysis();
        let md = render_markdown(&analysis, test_date());

        assert!(md.contains("# 2026-03-11 Dev Session Review"));
        assert!(md.contains("## Session: test-session-1"));
        assert!(md.contains("### 작업 요약\n파서 모듈 구현"));
    }

    #[test]
    fn test_render_markdown_decisions_포함시_의사결정섹션_생성() {
        let analysis = make_test_analysis();
        let md = render_markdown(&analysis, test_date());

        assert!(md.contains("### 주요 의사결정"));
        assert!(md.contains("- **serde 사용**: 자동 역직렬화가 편리"));
    }

    #[test]
    fn test_render_markdown_빈_decisions일때_의사결정섹션_미생성() {
        let analysis = AnalysisResult {
            sessions: vec![SessionInsight {
                session_id: "s1".to_string(),
                work_summary: "요약".to_string(),
                decisions: vec![],
                curiosities: vec![],
                corrections: vec![],
            }],
        };
        let md = render_markdown(&analysis, test_date());

        assert!(!md.contains("### 주요 의사결정"));
    }

    #[test]
    fn test_render_markdown_til_curiosities와_corrections에서_추출() {
        let analysis = AnalysisResult {
            sessions: vec![SessionInsight {
                session_id: "s1".to_string(),
                work_summary: "요약".to_string(),
                decisions: vec![],
                curiosities: vec!["궁금한 점".to_string()],
                corrections: vec![Correction {
                    model_said: "틀린 내용".to_string(),
                    user_corrected: "올바른 내용".to_string(),
                }],
            }],
        };
        let md = render_markdown(&analysis, test_date());

        assert!(md.contains("## TIL (Today I Learned)"));
        assert!(md.contains("- 궁금한 점"));
        assert!(md.contains("- 올바른 내용"));
    }
}
