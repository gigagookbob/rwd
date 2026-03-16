// AnalysisResult를 Markdown 문자열로 변환하는 모듈.
//
// I/O가 없는 순수 함수로 구성되어 단위 테스트가 쉽습니다.
// format! 매크로와 String::push_str()로 문자열을 조립합니다 (Rust Book Ch.8.2 참조).

use chrono::NaiveDate;

use crate::analyzer::insight::{AnalysisResult, SessionInsight, TilItem};

/// 여러 소스의 분석 결과를 하나의 Markdown으로 결합합니다.
/// 각 소스는 ## 헤딩으로 구분됩니다.
///
/// sources: (소스 이름, 분석 결과) 튜플의 슬라이스.
/// 향후 새로운 에이전트 추가 시 sources에 추가하면 됩니다.
pub fn render_combined_markdown(
    sources: &[(&str, &AnalysisResult)],
    date: NaiveDate,
) -> String {
    let mut md = String::new();
    md.push_str(&format!("# {date} Dev Session Review\n\n"));

    // TIL 항목을 (세션ID, TilItem) 튜플로 수집합니다.
    let mut all_til_items: Vec<(&str, &TilItem)> = Vec::new();

    for (source_name, analysis) in sources {
        md.push_str(&format!("## {source_name}\n\n"));
        for session in &analysis.sessions {
            render_session(&mut md, session);
            // 각 TIL 항목에 세션 ID를 함께 수집합니다.
            for til in &session.til {
                all_til_items.push((&session.session_id, til));
            }
        }
    }

    render_til_section(&mut md, &all_til_items);
    md
}

/// AnalysisResult와 날짜를 받아 Markdown 문자열을 생성합니다.
///
/// render_combined_markdown의 단일 소스 버전으로, 테스트에서 직접 사용됩니다.
/// 프로덕션에서는 render_combined_markdown을 사용하세요.
///
/// String::new()는 빈 문자열을 힙에 할당합니다.
/// push_str()은 문자열 슬라이스(&str)를 String 끝에 추가합니다 (Rust Book Ch.8.2 참조).
/// format!은 포매팅된 새 String을 반환하는 매크로입니다.
#[cfg(test)]
pub fn render_markdown(analysis: &AnalysisResult, date: NaiveDate) -> String {
    let mut md = String::new();

    // 제목 — NaiveDate는 Display 트레이트를 구현하여 "YYYY-MM-DD" 형식으로 출력됩니다.
    md.push_str(&format!("# {date} Dev Session Review\n\n"));

    let mut til_items: Vec<(&str, &TilItem)> = Vec::new();

    for session in &analysis.sessions {
        render_session(&mut md, session);
        for til in &session.til {
            til_items.push((&session.session_id, til));
        }
    }

    render_til_section(&mut md, &til_items);

    md
}

/// 세션 하나의 Markdown을 생성합니다.
/// TIL은 별도로 수집되므로 이 함수에서는 처리하지 않습니다.
///
/// &mut String은 가변 참조로, 함수 안에서 문자열에 내용을 추가할 수 있습니다 (Rust Book Ch.4 참조).
fn render_session(md: &mut String, session: &SessionInsight) {
    md.push_str(&format!("## Session: {}\n\n", session.session_id));
    md.push_str(&format!("### 작업 요약\n{}\n\n", session.work_summary));

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
        }
        md.push('\n');
    }

    if !session.corrections.is_empty() {
        md.push_str("### 모델 오류 및 수정\n");
        for c in &session.corrections {
            md.push_str(&format!("- **모델**: {}\n  **수정**: {}\n", c.model_said, c.user_corrected));
        }
        md.push('\n');
    }

    md.push_str("---\n\n");
}

/// TIL(Today I Learned) 섹션을 Markdown에 추가합니다.
/// 각 항목은 ### 제목 + 설명 + 세션 ID HTML 주석으로 구성됩니다.
///
/// til_items: (세션ID, TilItem) 튜플의 슬라이스.
/// 세션 ID는 HTML 주석으로 포함되어 나중에 원본 세션을 추적할 수 있습니다.
fn render_til_section(md: &mut String, til_items: &[(&str, &TilItem)]) {
    if til_items.is_empty() {
        return;
    }

    md.push_str("## TIL (Today I Learned)\n\n");
    for (session_id, til) in til_items {
        md.push_str(&format!("### {}\n", til.title));
        md.push_str(&format!("{}\n", til.detail));
        // 세션 ID를 HTML 주석으로 포함 — Markdown 렌더링에 영향 없이 추적 가능
        md.push_str(&format!("<!-- session: {} -->\n\n", session_id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::insight::{Correction, Decision, SessionInsight, TilItem};

    /// 테스트용 AnalysisResult를 생성하는 헬퍼 함수.
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
                til: vec![TilItem {
                    title: "serde tag는 중첩 JSON에서 안 먹힌다".to_string(),
                    detail: "Codex JSONL처럼 type이 두 곳에 있으면 2단계 파싱 필요".to_string(),
                }],
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
                til: vec![],
            }],
        };
        let md = render_markdown(&analysis, test_date());

        assert!(!md.contains("### 주요 의사결정"));
    }

    #[test]
    fn test_render_combined_markdown_두소스_섹션_분리() {
        let claude = AnalysisResult {
            sessions: vec![SessionInsight {
                session_id: "c1".to_string(),
                work_summary: "Claude 작업".to_string(),
                decisions: vec![],
                curiosities: vec![],
                corrections: vec![],
                til: vec![],
            }],
        };
        let codex = AnalysisResult {
            sessions: vec![SessionInsight {
                session_id: "x1".to_string(),
                work_summary: "Codex 작업".to_string(),
                decisions: vec![],
                curiosities: vec![],
                corrections: vec![],
                til: vec![],
            }],
        };
        let sources = vec![("Claude Code", &claude), ("Codex", &codex)];
        let date = NaiveDate::from_ymd_opt(2026, 3, 16).unwrap();
        let md = render_combined_markdown(&sources, date);

        assert!(md.contains("## Claude Code"));
        assert!(md.contains("## Codex"));
        assert!(md.contains("Claude 작업"));
        assert!(md.contains("Codex 작업"));
    }

    #[test]
    fn test_render_combined_markdown_단일소스_정상동작() {
        let claude = AnalysisResult {
            sessions: vec![SessionInsight {
                session_id: "c1".to_string(),
                work_summary: "Claude 작업".to_string(),
                decisions: vec![],
                curiosities: vec![],
                corrections: vec![],
                til: vec![],
            }],
        };
        let sources = vec![("Claude Code", &claude)];
        let date = NaiveDate::from_ymd_opt(2026, 3, 16).unwrap();
        let md = render_combined_markdown(&sources, date);

        assert!(md.contains("## Claude Code"));
        assert!(md.contains("Claude 작업"));
    }

    #[test]
    fn test_render_markdown_til_제목_설명_세션id_포함() {
        let analysis = make_test_analysis();
        let md = render_markdown(&analysis, test_date());

        assert!(md.contains("## TIL (Today I Learned)"));
        assert!(md.contains("### serde tag는 중첩 JSON에서 안 먹힌다"));
        assert!(md.contains("2단계 파싱 필요"));
        assert!(md.contains("<!-- session: test-session-1 -->"));
    }

    #[test]
    fn test_render_markdown_til_비어있으면_섹션_미생성() {
        let analysis = AnalysisResult {
            sessions: vec![SessionInsight {
                session_id: "s1".to_string(),
                work_summary: "요약".to_string(),
                decisions: vec![],
                curiosities: vec![],
                corrections: vec![],
                til: vec![],
            }],
        };
        let md = render_markdown(&analysis, test_date());

        assert!(!md.contains("## TIL"));
    }
}
