// Converts AnalysisResult into a Markdown string.
//
// Pure functions with no I/O, making unit testing straightforward.

use chrono::NaiveDate;

use crate::analyzer::insight::{AnalysisResult, SessionInsight, TilItem};

/// Combines analysis results from multiple sources into a single Markdown document.
/// Each source is separated by a ## heading.
///
/// sources: slice of (source name, analysis result) tuples.
pub fn render_combined_markdown(sources: &[(&str, &AnalysisResult)], date: NaiveDate) -> String {
    let mut md = String::new();
    md.push_str(&format!("# {date} Dev Session Review\n\n"));

    let mut all_til_items: Vec<(&str, &TilItem)> = Vec::new();

    for (source_name, analysis) in sources {
        md.push_str(&format!("## {source_name}\n\n"));
        for session in &analysis.sessions {
            render_session(&mut md, session);
            for til in &session.til {
                all_til_items.push((&session.session_id, til));
            }
        }
    }

    render_til_section(&mut md, &all_til_items);
    md
}

/// Single-source version of render_combined_markdown. Used directly in tests.
#[cfg(test)]
pub fn render_markdown(analysis: &AnalysisResult, date: NaiveDate) -> String {
    let mut md = String::new();

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

/// Renders a single session's Markdown.
/// TIL items are collected separately and not rendered here.
fn render_session(md: &mut String, session: &SessionInsight) {
    md.push_str(&format!("## Session: {}\n\n", session.session_id));
    md.push_str(&format!(
        "{}\n{}\n\n",
        crate::messages::markdown::WORK_SUMMARY_HEADER,
        session.work_summary
    ));

    if !session.decisions.is_empty() {
        md.push_str(&format!(
            "{}\n",
            crate::messages::markdown::DECISIONS_HEADER
        ));
        for d in &session.decisions {
            md.push_str(&format!("- **{}**: {}\n", d.what, d.why));
        }
        md.push('\n');
    }

    if !session.curiosities.is_empty() {
        md.push_str(&format!(
            "{}\n",
            crate::messages::markdown::CURIOSITIES_HEADER
        ));
        for c in &session.curiosities {
            md.push_str(&format!("- {c}\n"));
        }
        md.push('\n');
    }

    if !session.corrections.is_empty() {
        md.push_str(&format!(
            "{}\n",
            crate::messages::markdown::CORRECTIONS_HEADER
        ));
        for c in &session.corrections {
            md.push_str(&format!(
                "- {}: {}\n  {}: {}\n",
                crate::messages::markdown::CORRECTION_MODEL,
                c.model_said,
                crate::messages::markdown::CORRECTION_FIX,
                c.user_corrected
            ));
        }
        md.push('\n');
    }

    md.push_str("---\n\n");
}

/// Renders the TIL (Today I Learned) section.
/// Each item has a ### title, description, and session ID as an HTML comment for traceability.
fn render_til_section(md: &mut String, til_items: &[(&str, &TilItem)]) {
    if til_items.is_empty() {
        return;
    }

    md.push_str("## TIL (Today I Learned)\n\n");
    for (session_id, til) in til_items {
        md.push_str(&format!("### {}\n", til.title));
        md.push_str(&format!("{}\n", til.detail));
        // Session ID as HTML comment -- invisible in rendered Markdown but traceable.
        md.push_str(&format!("<!-- session: {} -->\n\n", session_id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::insight::{Decision, SessionInsight, TilItem};

    /// Helper to create a test AnalysisResult.
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
        NaiveDate::from_ymd_opt(2026, 3, 11).expect("valid date")
    }

    #[test]
    fn test_render_markdown_single_session_title_and_summary() {
        let analysis = make_test_analysis();
        let md = render_markdown(&analysis, test_date());

        assert!(md.contains("# 2026-03-11 Dev Session Review"));
        assert!(md.contains("## Session: test-session-1"));
        assert!(md.contains("### Work Summary\n파서 모듈 구현"));
    }

    #[test]
    fn test_render_markdown_decisions_section_when_present() {
        let analysis = make_test_analysis();
        let md = render_markdown(&analysis, test_date());

        assert!(md.contains("### Key Decisions"));
        assert!(md.contains("- **serde 사용**: 자동 역직렬화가 편리"));
    }

    #[test]
    fn test_render_markdown_no_decisions_section_when_empty() {
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

        assert!(!md.contains("### Key Decisions"));
    }

    #[test]
    fn test_render_combined_markdown_two_sources_separate_sections() {
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
    fn test_render_combined_markdown_single_source() {
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
    fn test_render_markdown_til_includes_title_desc_session_id() {
        let analysis = make_test_analysis();
        let md = render_markdown(&analysis, test_date());

        assert!(md.contains("## TIL (Today I Learned)"));
        assert!(md.contains("### serde tag는 중첩 JSON에서 안 먹힌다"));
        assert!(md.contains("2단계 파싱 필요"));
        assert!(md.contains("<!-- session: test-session-1 -->"));
    }

    #[test]
    fn test_render_markdown_no_til_section_when_empty() {
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
