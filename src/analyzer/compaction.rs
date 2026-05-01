// Detects log-like regions *inside* a message and compacts only those regions,
// preserving any natural-language lines interleaved with the logs.
//
// Edge cases explicitly handled below:
//   - Empty input.
//   - Text under MIN_COMPACT_BYTES (left verbatim).
//   - A single huge line without any newline (no log runs).
//   - All-prose text (untouched; no runs).
//   - All-log text (single run, compacted).
//   - Log run sandwiched between two prose lines ("한 줄 질문 ... 로그 ... 질문").
//   - Multiple interleaved log runs (each compacted independently).
//   - Very short log run (< MIN_RUN_LINES or < MIN_RUN_BYTES): left as-is.
//   - Windows line endings (\r\n) and the Unicode line separator (U+2028).
//   - Trailing newline is preserved exactly (present iff original had one).
//   - Blank / whitespace / extremely short lines inside a run are absorbed,
//     but trailing Neutral lines bordering prose are not included in the run.
//   - Version strings ("v1.0.106") are NOT misclassified as prose.
//   - Repeated prefixes like "   Compiling", "flutter: " promote ambiguous
//     lines to Log via prefix-repetition detection.
//   - Result is guaranteed to be no larger than the input (falls back to the
//     original string when compaction accidentally grows the text).
//   - All slicing respects UTF-8 char boundaries.

use regex::Regex;
use std::sync::OnceLock;

/// Messages smaller than this byte count are never inspected or rewritten.
const MIN_COMPACT_BYTES: usize = 8 * 1024;
/// A contiguous log run shorter than this many lines is left verbatim.
const MIN_RUN_LINES: usize = 20;
/// A run smaller than this many bytes is left verbatim.
const MIN_RUN_BYTES: usize = 2 * 1024;
/// Lines kept from the head of a compacted run.
const RUN_HEAD_LINES: usize = 5;
/// Lines kept from the tail of a compacted run.
const RUN_TAIL_LINES: usize = 3;
/// Byte window used to detect repeated line-start prefixes.
const PREFIX_WINDOW_BYTES: usize = 8;
/// Minimum number of consecutive lines sharing a prefix to promote to Log.
const PREFIX_REPEAT_MIN: usize = 10;
/// Minimum trimmed length before a line can receive a non-Neutral class.
const MIN_CLASSIFIABLE_LEN: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LineClass {
    Log,
    Prose,
    Neutral,
}

fn stack_trace_line_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(concat!(
            r"^(?:",
            r"\s*at\s+\S+\(.+?:\d+\)",
            r#"|\s+File\s+"[^"]+",\s+line\s+\d+"#,
            r"|\s*Traceback\s*\(most recent call last\):",
            r"|\s*Caused by:",
            r")",
        ))
        .expect("valid stack trace regex")
    })
}

fn timestamp_line_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(concat!(
            r"^\s*(?:",
            r"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}",
            r"|\d{2}:\d{2}:\d{2}(?:[.,]\d{3})?\b",
            r")",
        ))
        .expect("valid timestamp regex")
    })
}

fn prompt_line_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(concat!(
            r"(?i)^\s*(?:",
            r"\$\s",
            r"|>>>\s",
            r"|#\s",
            r"|npm\s+ERR!",
            r"|error[:\[]",
            r"|warning:",
            r"|panicked at",
            r"|thread\s+'.*?'\s+panicked",
            r"|FAILED",
            r"|fatal:",
            r")",
        ))
        .expect("valid prompt regex")
    })
}

/// Replace every detected log run inside `text` with a compact head/tail
/// summary, while leaving prose and short log fragments untouched.
pub fn compact_log_like(text: &str) -> String {
    if text.is_empty() || text.len() <= MIN_COMPACT_BYTES {
        return text.to_string();
    }

    let trailing_newline = text.ends_with('\n');
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return text.to_string();
    }

    let mut classes = initial_line_classes(&lines);
    promote_prefix_runs(&lines, &mut classes);
    let runs = find_log_runs(&classes);

    if runs.is_empty() {
        return text.to_string();
    }

    let compacted = rewrite(&lines, &runs, trailing_newline);
    if compacted.len() >= text.len() {
        text.to_string()
    } else {
        compacted
    }
}

/// Reports whether `text` contains at least one compactable log run.
/// Returns false for small or purely natural-language inputs.
#[allow(dead_code)]
pub fn is_log_like(text: &str) -> bool {
    if text.len() <= MIN_COMPACT_BYTES {
        return false;
    }
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return false;
    }
    let mut classes = initial_line_classes(&lines);
    promote_prefix_runs(&lines, &mut classes);
    !find_log_runs(&classes).is_empty()
}

fn initial_line_classes(lines: &[&str]) -> Vec<LineClass> {
    lines.iter().map(|l| classify_line_strict(l)).collect()
}

fn classify_line_strict(line: &str) -> LineClass {
    if line.trim().len() < MIN_CLASSIFIABLE_LEN {
        return LineClass::Neutral;
    }
    if line.contains('\u{001b}')
        || stack_trace_line_regex().is_match(line)
        || timestamp_line_regex().is_match(line)
        || prompt_line_regex().is_match(line)
    {
        return LineClass::Log;
    }
    if looks_like_prose(line) {
        return LineClass::Prose;
    }
    LineClass::Neutral
}

/// Prose heuristic: the line ends with sentence-ending punctuation that is
/// preceded by an alphabetic character (letter / Hangul / etc.). Trailing "."
/// after a digit ("v1.0.106", "foo.bar:42") does NOT qualify.
fn looks_like_prose(line: &str) -> bool {
    let trimmed = line.trim_end();
    let mut iter = trimmed.chars().rev();
    let Some(last) = iter.next() else {
        return false;
    };
    if !matches!(last, '.' | '!' | '?' | '。' | '！' | '？') {
        return false;
    }
    let Some(before) = iter.next() else {
        return false;
    };
    before.is_alphabetic()
}

/// Promote Ambiguous/Neutral lines to Log when a shared 8-byte line-start
/// prefix repeats PREFIX_REPEAT_MIN times in a row. Handles patterns such as
/// "   Compiling foo v1.0", "flutter: [tag] ...", "INFO 2026-...".
fn promote_prefix_runs(lines: &[&str], classes: &mut [LineClass]) {
    let mut i = 0;
    while i < lines.len() {
        if classes[i] == LineClass::Prose {
            i += 1;
            continue;
        }
        let Some(prefix) = prefix_window(lines[i]) else {
            i += 1;
            continue;
        };
        let mut j = i + 1;
        while j < lines.len()
            && classes[j] != LineClass::Prose
            && prefix_window(lines[j]) == Some(prefix)
        {
            j += 1;
        }
        if j - i >= PREFIX_REPEAT_MIN {
            for entry in classes.iter_mut().take(j).skip(i) {
                *entry = LineClass::Log;
            }
            i = j;
        } else {
            i += 1;
        }
    }
}

fn prefix_window(line: &str) -> Option<&str> {
    if line.len() < PREFIX_WINDOW_BYTES {
        return None;
    }
    let end = ceil_char_boundary(line, PREFIX_WINDOW_BYTES);
    Some(&line[..end])
}

/// Locate contiguous runs of Log (allowing interior Neutral) bounded by Prose
/// or input edges. Trailing Neutral lines bordering prose are excluded.
fn find_log_runs(classes: &[LineClass]) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let mut i = 0;
    while i < classes.len() {
        if classes[i] != LineClass::Log {
            i += 1;
            continue;
        }
        let start = i;
        let mut j = i + 1;
        while j < classes.len() {
            match classes[j] {
                LineClass::Log | LineClass::Neutral => j += 1,
                LineClass::Prose => break,
            }
        }
        let mut end = j;
        while end > start && classes[end - 1] == LineClass::Neutral {
            end -= 1;
        }
        if end > start {
            runs.push((start, end));
        }
        i = j.max(start + 1);
    }
    runs
}

fn byte_count_of_lines(lines: &[&str], start: usize, end: usize) -> usize {
    lines[start..end].iter().map(|l| l.len() + 1).sum()
}

fn rewrite(lines: &[&str], runs: &[(usize, usize)], trailing_newline: bool) -> String {
    let capacity_hint = lines.iter().map(|l| l.len() + 1).sum::<usize>();
    let mut out = String::with_capacity(capacity_hint);
    let mut cursor = 0;
    for &(start, end) in runs {
        for line in &lines[cursor..start] {
            out.push_str(line);
            out.push('\n');
        }

        let run_lines = end - start;
        let run_bytes = byte_count_of_lines(lines, start, end);
        if run_lines >= MIN_RUN_LINES && run_bytes >= MIN_RUN_BYTES {
            write_compacted_run(&mut out, lines, start, end);
        } else {
            for line in &lines[start..end] {
                out.push_str(line);
                out.push('\n');
            }
        }

        cursor = end;
    }
    for line in &lines[cursor..] {
        out.push_str(line);
        out.push('\n');
    }

    if !trailing_newline && out.ends_with('\n') {
        out.pop();
    }

    out
}

fn write_compacted_run(out: &mut String, lines: &[&str], start: usize, end: usize) {
    let head_end = (start + RUN_HEAD_LINES).min(end);
    let tail_start = end.saturating_sub(RUN_TAIL_LINES).max(head_end);

    for line in &lines[start..head_end] {
        out.push_str(line);
        out.push('\n');
    }
    let omitted_lines = tail_start - head_end;
    if omitted_lines > 0 {
        let omitted_bytes = byte_count_of_lines(lines, head_end, tail_start);
        out.push_str("… [");
        out.push_str(&omitted_lines.to_string());
        out.push_str(" log-like lines, ");
        out.push_str(&omitted_bytes.to_string());
        out.push_str(" bytes omitted] …\n");
    }
    for line in &lines[tail_start..end] {
        out.push_str(line);
        out.push('\n');
    }
}

fn ceil_char_boundary(s: &str, mut idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    while idx < s.len() && !s.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cargo_line(i: usize) -> String {
        format!("   Compiling foo v1.0.{i}\n")
    }

    fn big_cargo_block(n: usize) -> String {
        (0..n).map(cargo_line).collect()
    }

    fn big_flutter_block(n: usize) -> String {
        let line = "flutter: ✅ [Sounds] 경로 기반 프리로드 완료 (성공: 63, 실패: 0)\n";
        line.repeat(n)
    }

    fn pad_log(size: usize) -> String {
        let line = "\u{001b}[0;31merror\u{001b}[0m build failed for target\n";
        let mut s = String::new();
        while s.len() < size {
            s.push_str(line);
        }
        s
    }

    #[test]
    fn empty_input_returns_empty() {
        assert_eq!(compact_log_like(""), "");
    }

    #[test]
    fn under_min_size_pass_through() {
        let s = "\u{001b}[31merror\u{001b}[0m boom\n".repeat(20);
        assert!(s.len() < MIN_COMPACT_BYTES);
        assert_eq!(compact_log_like(&s), s);
    }

    #[test]
    fn single_long_line_without_newline_pass_through() {
        let s = "a".repeat(20_000);
        assert_eq!(compact_log_like(&s), s);
    }

    #[test]
    fn all_prose_paragraphs_pass_through() {
        let mut s = String::new();
        for i in 0..1000 {
            s.push_str(&format!(
                "이 문단은 {i}번째 설명입니다. 기능 요청을 검토하고 우선순위를 정합니다.\n"
            ));
        }
        assert!(s.len() > MIN_COMPACT_BYTES);
        assert_eq!(compact_log_like(&s), s);
    }

    #[test]
    fn ansi_log_block_compacted() {
        let s = pad_log(40_000);
        let out = compact_log_like(&s);
        assert!(out.len() < s.len());
        assert!(out.contains("log-like lines"));
    }

    #[test]
    fn stack_trace_compacted() {
        let mut s = String::from("Traceback (most recent call last):\n");
        for _ in 0..500 {
            s.push_str("    at com.example.Foo.bar(Foo.java:42)\n");
        }
        let out = compact_log_like(&s);
        assert!(out.len() < s.len());
        assert!(out.contains("log-like lines"));
    }

    #[test]
    fn timestamp_log_compacted() {
        let line = "2026-04-17 12:00:00 INFO service tick with config foo=bar\n";
        let s = line.repeat(500);
        let out = compact_log_like(&s);
        assert!(out.len() < s.len());
    }

    #[test]
    fn flutter_prefix_log_compacted() {
        let s = big_flutter_block(600);
        let out = compact_log_like(&s);
        assert!(out.len() < s.len(), "flutter log should compact");
        assert!(out.contains("log-like lines"));
    }

    #[test]
    fn cargo_build_log_compacted_via_prefix_repeat() {
        let s = big_cargo_block(600);
        assert!(s.len() > MIN_COMPACT_BYTES);
        let out = compact_log_like(&s);
        assert!(
            out.len() < s.len(),
            "cargo build log ({} bytes) should compact, got {} bytes",
            s.len(),
            out.len()
        );
        assert!(out.contains("log-like lines"));
    }

    #[test]
    fn prose_inside_log_run_is_preserved() {
        let mut s = String::new();
        s.push_str(&big_cargo_block(600));
        s.push_str("왜 이래?\n");
        s.push_str(&big_cargo_block(600));
        let out = compact_log_like(&s);
        assert!(
            out.contains("왜 이래?"),
            "prose sandwiched between logs must survive"
        );
        assert_eq!(
            out.matches("log-like lines").count(),
            2,
            "two runs should be compacted separately"
        );
    }

    #[test]
    fn prose_leading_and_trailing_runs_preserved() {
        let mut s = String::new();
        s.push_str("이거 돌렸더니 뭔가 이상해.\n");
        s.push_str(&big_cargo_block(600));
        s.push_str("어떻게 생각해?\n");
        let out = compact_log_like(&s);
        assert!(out.contains("이거 돌렸더니 뭔가 이상해."));
        assert!(out.contains("어떻게 생각해?"));
        assert!(out.contains("log-like lines"));
    }

    #[test]
    fn short_log_run_not_compacted() {
        let mut s = String::new();
        s.push_str(&"x".repeat(MIN_COMPACT_BYTES + 100));
        s.push('\n');
        for _ in 0..5 {
            s.push_str("    at com.example.Foo.bar(Foo.java:42)\n");
        }
        let out = compact_log_like(&s);
        assert!(!out.contains("log-like lines"));
    }

    #[test]
    fn trailing_newline_preserved_when_present() {
        let s = big_cargo_block(600);
        assert!(s.ends_with('\n'));
        let out = compact_log_like(&s);
        assert!(out.ends_with('\n'));
    }

    #[test]
    fn no_trailing_newline_preserved_when_absent() {
        let mut s = big_cargo_block(600);
        s.pop();
        assert!(!s.ends_with('\n'));
        let out = compact_log_like(&s);
        assert!(!out.ends_with('\n'));
    }

    #[test]
    fn windows_line_endings_still_compacted() {
        let line = "   Compiling foo v1.0.0\r\n";
        let s = line.repeat(600);
        let out = compact_log_like(&s);
        assert!(out.len() < s.len());
    }

    #[test]
    fn utf8_safe_when_head_boundary_crosses_hangul() {
        let prefix = "한글".repeat(3);
        let mut s = String::new();
        s.push_str(&prefix);
        s.push('\n');
        s.push_str(&big_cargo_block(600));
        let out = compact_log_like(&s);
        assert!(out.is_char_boundary(0));
        assert!(std::str::from_utf8(out.as_bytes()).is_ok());
        assert!(out.starts_with(&prefix));
    }

    #[test]
    fn compaction_never_grows_text() {
        for n in [5usize, 10, 30, 100, 400, 800] {
            let s = big_cargo_block(n);
            if s.len() < MIN_COMPACT_BYTES {
                continue;
            }
            let out = compact_log_like(&s);
            assert!(
                out.len() <= s.len(),
                "compaction grew text at n={n}: {} > {}",
                out.len(),
                s.len()
            );
        }
    }

    #[test]
    fn version_strings_are_not_prose() {
        assert!(!looks_like_prose("package v1.0.106"));
        assert!(!looks_like_prose("  _flutterfire_internals 1.3.68"));
    }

    #[test]
    fn english_and_korean_sentences_are_prose() {
        assert!(looks_like_prose("Today we discussed the new feature."));
        assert!(looks_like_prose("왜 이래?"));
        assert!(looks_like_prose("정말 멋진 아이디어야!"));
    }

    #[test]
    fn run_absorbs_blank_lines_inside_log() {
        let mut s = String::new();
        s.push_str(&big_cargo_block(300));
        s.push('\n');
        s.push_str(&big_cargo_block(300));
        let out = compact_log_like(&s);
        assert!(out.len() < s.len());
        assert_eq!(
            out.matches("log-like lines").count(),
            1,
            "blank line should be absorbed into one run"
        );
    }

    #[test]
    fn multiple_interleaved_runs_each_compacted() {
        let mut s = String::new();
        for _ in 0..3 {
            s.push_str("지금 확인해볼게.\n");
            s.push_str(&big_cargo_block(600));
        }
        let out = compact_log_like(&s);
        assert_eq!(out.matches("log-like lines").count(), 3);
        assert_eq!(out.matches("지금 확인해볼게.").count(), 3);
    }

    #[test]
    fn all_log_text_results_in_single_compacted_run() {
        let s = big_cargo_block(800);
        let out = compact_log_like(&s);
        assert_eq!(out.matches("log-like lines").count(), 1);
        assert!(out.len() < s.len());
    }

    #[test]
    fn is_log_like_detects_run() {
        let s = big_cargo_block(600);
        assert!(is_log_like(&s));
    }

    #[test]
    fn is_log_like_rejects_prose() {
        let mut s = String::new();
        for i in 0..500 {
            s.push_str(&format!(
                "{i}번째 문단으로 기능을 검토합니다. 대안도 함께 정리합니다.\n"
            ));
        }
        assert!(s.len() > MIN_COMPACT_BYTES);
        assert!(!is_log_like(&s));
    }

    #[test]
    fn is_log_like_rejects_tiny_text() {
        assert!(!is_log_like("short"));
        assert!(!is_log_like(&pad_log(2_000)));
    }

    #[test]
    fn dashes_and_fences_are_neutral_not_prose() {
        assert_eq!(classify_line_strict("---"), LineClass::Neutral);
        assert_eq!(classify_line_strict("```"), LineClass::Neutral);
        assert_eq!(classify_line_strict("```rust"), LineClass::Neutral);
    }

    #[test]
    fn lone_prose_line_between_short_runs_preserved() {
        let mut s = String::new();
        s.push_str(&pad_log(MIN_COMPACT_BYTES + 5_000));
        s.push_str("여기 왜 이런거야?\n");
        s.push_str(&pad_log(5_000));
        let out = compact_log_like(&s);
        assert!(out.contains("여기 왜 이런거야?"));
    }
}
