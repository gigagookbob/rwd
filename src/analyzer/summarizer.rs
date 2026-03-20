// Large session chunking and summarization module.
//
// Splits sessions exceeding ITPM into message-level chunks,
// summarizes each chunk via the LLM, then combines results.

use super::planner::RateLimits;
use super::prompt::estimate_tokens;

/// Prompt for chunk summarization.
/// Instructs the LLM to preserve key content aligned with rwd's insight categories.
pub const CHUNK_SUMMARIZE_PROMPT: &str = r#"다음 개발 세션 대화에서 아래 항목을 중심으로 요약하라:
- 내린 기술적 결정과 그 이유
- 실수나 수정 사항
- 새로 배운 점 (TIL)
- 흥미로운 발견이나 의문점
원문의 구체적 기술 용어와 맥락을 보존하라."#;

/// Splits messages into chunks that fit within the ITPM limit.
/// Splits only at message boundaries (never mid-message).
/// A single message exceeding the limit becomes its own chunk.
pub fn split_into_chunks(
    messages: &[(String, String)],
    itpm: u64,
) -> Vec<Vec<(String, String)>> {
    if messages.is_empty() {
        return Vec::new();
    }

    let mut chunks: Vec<Vec<(String, String)>> = Vec::new();
    let mut current_chunk: Vec<(String, String)> = Vec::new();
    let mut current_tokens: u64 = 0;

    for (role, text) in messages {
        let msg_tokens = estimate_tokens(text);

        // Adding this message would exceed the limit -- start a new chunk.
        if !current_chunk.is_empty() && current_tokens + msg_tokens > itpm {
            chunks.push(current_chunk);
            current_chunk = Vec::new();
            current_tokens = 0;
        }

        current_chunk.push((role.clone(), text.clone()));
        current_tokens += msg_tokens;
    }

    if !current_chunk.is_empty() {
        chunks.push(current_chunk);
    }

    chunks
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
) -> Result<String, super::AnalyzerError> {
    let mut summaries: Vec<String> = Vec::new();
    let total = chunks.len();

    for (i, chunk) in chunks.iter().enumerate() {
        let chunk_text: String = chunk
            .iter()
            .map(|(role, text)| format!("[{role}] {text}"))
            .collect::<Vec<_>>()
            .join("\n");

        let sp = super::start_spinner(crate::messages::status::chunk_summarizing(i + 1, total));

        let summary = provider
            .call_api_with_max_tokens(
                api_key,
                CHUNK_SUMMARIZE_PROMPT,
                &chunk_text,
                2000,
            )
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

    #[test]
    fn test_split_into_chunks_respects_token_limit() {
        let messages = vec![
            ("USER".to_string(), "a".repeat(20)),
            ("USER".to_string(), "b".repeat(20)),
            ("USER".to_string(), "c".repeat(20)),
        ];
        let chunks = split_into_chunks(&messages, 25);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), 2);
        assert_eq!(chunks[1].len(), 1);
    }

    #[test]
    fn test_split_into_chunks_single_message_exceeds_limit() {
        let messages = vec![
            ("USER".to_string(), "a".repeat(100)),
        ];
        let chunks = split_into_chunks(&messages, 25);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 1);
    }

    #[test]
    fn test_split_into_chunks_empty_messages() {
        let messages: Vec<(String, String)> = vec![];
        let chunks = split_into_chunks(&messages, 30_000);
        assert!(chunks.is_empty());
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
