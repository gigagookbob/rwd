// Large session chunking and summarization module.
//
// Splits sessions exceeding ITPM into message-level chunks,
// summarizes each chunk via the LLM, then combines results.

use super::planner::RateLimits;
use super::prompt::estimate_tokens;

use crate::config::Lang;

/// Safety cap on the number of chunk-summarize API calls we make for a single
/// session. A pathological session that still explodes into dozens of chunks
/// after compaction would otherwise burn through the provider's daily quota.
pub const MAX_CHUNKS_PER_SESSION: usize = 3;

const CHUNK_SUMMARIZE_PROMPT_EN: &str = include_str!("../../prompts/chunk_summarize_en.md");
const CHUNK_SUMMARIZE_PROMPT_KO: &str = include_str!("../../prompts/chunk_summarize_ko.md");

pub fn get_chunk_summarize_prompt(lang: &Lang) -> &'static str {
    match lang {
        Lang::En => CHUNK_SUMMARIZE_PROMPT_EN,
        Lang::Ko => CHUNK_SUMMARIZE_PROMPT_KO,
    }
}

/// Splits messages into chunks that fit within the ITPM limit.
/// Splits at message boundaries when possible, and mid-message only when needed to satisfy
/// hard character caps from providers like Codex CLI.
pub fn split_into_chunks(
    messages: &[(String, String)],
    itpm: u64,
    max_chunk_chars: usize,
) -> Vec<Vec<(String, String)>> {
    if messages.is_empty() {
        return Vec::new();
    }

    let mut chunks: Vec<Vec<(String, String)>> = Vec::new();
    let mut current_chunk: Vec<(String, String)> = Vec::new();
    let mut current_tokens: u64 = 0;
    let mut current_chars: usize = 0;
    let max_chunk_chars = max_chunk_chars.max(1);

    for (role, text) in messages {
        let parts = split_message_text_by_chars(role, text, max_chunk_chars);
        for part in parts {
            let msg_tokens = estimate_tokens(&part);
            let msg_chars = rendered_message_chars(role, &part);

            // Adding this message would exceed a limit -- start a new chunk.
            if !current_chunk.is_empty()
                && (current_tokens + msg_tokens > itpm
                    || current_chars.saturating_add(msg_chars) > max_chunk_chars)
            {
                chunks.push(current_chunk);
                current_chunk = Vec::new();
                current_tokens = 0;
                current_chars = 0;
            }

            current_chunk.push((role.clone(), part));
            current_tokens += msg_tokens;
            current_chars = current_chars.saturating_add(msg_chars);
        }
    }

    if !current_chunk.is_empty() {
        chunks.push(current_chunk);
    }

    chunks
}

fn rendered_message_chars(role: &str, text: &str) -> usize {
    // Render format is "[ROLE] {text}\n".
    role.chars().count() + 4 + text.chars().count()
}

fn split_message_text_by_chars(role: &str, text: &str, max_chunk_chars: usize) -> Vec<String> {
    let role_overhead = role.chars().count() + 4;
    let max_text_chars = max_chunk_chars.saturating_sub(role_overhead).max(1);
    let total_chars = text.chars().count();
    if total_chars <= max_text_chars {
        return vec![text.to_string()];
    }

    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut count = 0usize;
    for (idx, _) in text.char_indices() {
        if count == max_text_chars {
            parts.push(text[start..idx].to_string());
            start = idx;
            count = 0;
        }
        count += 1;
    }
    if start < text.len() {
        parts.push(text[start..].to_string());
    }
    if parts.is_empty() {
        parts.push(String::new());
    }
    parts
}

/// Calculates wait time based on ITPM/RPM limits.
/// Returns max(itpm_wait, rpm_wait).
pub fn calculate_wait(used_tokens: u64, limits: &RateLimits) -> f64 {
    let itpm_wait = (used_tokens as f64 / limits.input_tokens_per_minute as f64) * 60.0;
    let rpm_wait = 60.0 / limits.requests_per_minute as f64;
    itpm_wait.max(rpm_wait)
}

/// Summarizes a large session's chunks and returns the combined summary text.
/// Applies rate pacing between chunk API calls.
pub async fn summarize_chunks(
    chunks: &[Vec<(String, String)>],
    provider: &super::provider::LlmProvider,
    api_key: &str,
    limits: &RateLimits,
    lang: &Lang,
) -> Result<String, super::AnalyzerError> {
    let effective_chunks: &[Vec<(String, String)>] = if chunks.len() > MAX_CHUNKS_PER_SESSION {
        eprintln!(
            "{}",
            crate::messages::status::chunk_cap_applied(chunks.len(), MAX_CHUNKS_PER_SESSION)
        );
        &chunks[..MAX_CHUNKS_PER_SESSION]
    } else {
        chunks
    };

    let mut summaries: Vec<String> = Vec::new();
    let total = effective_chunks.len();

    for (i, chunk) in effective_chunks.iter().enumerate() {
        let chunk_text: String = chunk
            .iter()
            .map(|(role, text)| format!("[{role}] {text}"))
            .collect::<Vec<_>>()
            .join("\n");

        let sp = super::start_spinner(crate::messages::status::chunk_summarizing(i + 1, total));

        let (summary, _usage) = provider
            .call_api_with_max_tokens(api_key, get_chunk_summarize_prompt(lang), &chunk_text, 2000)
            .await?;
        super::stop_spinner(sp);
        eprintln!("{}", crate::messages::status::chunk_done(i + 1, total));
        summaries.push(summary);

        // Rate pacing: countdown wait unless this is the last chunk.
        if i + 1 < total {
            let chunk_tokens = estimate_tokens(&chunk_text);
            let wait = calculate_wait(chunk_tokens, limits);
            if wait > 0.0 {
                super::countdown_sleep(wait.ceil() as u64).await;
            }
        }
    }

    Ok(summaries.join("\n\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    const TEST_MAX_CHARS: usize = 1_048_576;

    #[test]
    fn test_split_into_chunks_respects_token_limit() {
        let messages = vec![
            ("USER".to_string(), "a".repeat(20)),
            ("USER".to_string(), "b".repeat(20)),
            ("USER".to_string(), "c".repeat(20)),
        ];
        let chunks = split_into_chunks(&messages, 25, TEST_MAX_CHARS);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), 2);
        assert_eq!(chunks[1].len(), 1);
    }

    #[test]
    fn test_split_into_chunks_single_message_exceeds_limit() {
        let messages = vec![("USER".to_string(), "a".repeat(100))];
        let chunks = split_into_chunks(&messages, 25, 32);
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn test_split_into_chunks_empty_messages() {
        let messages: Vec<(String, String)> = vec![];
        let chunks = split_into_chunks(&messages, 30_000, TEST_MAX_CHARS);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_split_into_chunks_respects_char_limit() {
        let messages = vec![("ASSISTANT".to_string(), "x".repeat(200))];
        let chunks = split_into_chunks(&messages, 1_000_000, 64);
        assert!(chunks.len() >= 3);
        for chunk in chunks {
            let rendered: usize = chunk
                .iter()
                .map(|(role, text)| rendered_message_chars(role, text))
                .sum();
            assert!(rendered <= 64);
        }
    }

    #[test]
    fn test_calculate_wait_itpm_based() {
        let limits = RateLimits {
            input_tokens_per_minute: 30_000,
            output_tokens_per_minute: 8_000,
            requests_per_minute: 1_000,
        };
        let wait = calculate_wait(15_000, &limits);
        assert!((wait - 30.0).abs() < 0.1);
    }

    #[test]
    fn test_calculate_wait_rpm_based() {
        let limits = RateLimits {
            input_tokens_per_minute: 1_000_000,
            output_tokens_per_minute: 200_000,
            requests_per_minute: 50,
        };
        let wait = calculate_wait(100, &limits);
        assert!((wait - 1.2).abs() < 0.1);
    }
}
