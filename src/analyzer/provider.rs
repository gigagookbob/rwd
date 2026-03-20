// LLM provider abstraction. Supports Anthropic and OpenAI via a unified enum dispatch.

/// System prompt shared by all providers.
pub const SYSTEM_PROMPT: &str = r#"You are an AI coding session analyst. You receive transcripts of conversations between a developer and an AI coding assistant.

Analyze the conversation and extract insights in the following JSON format. Return ONLY valid JSON, no other text.
IMPORTANT: All values MUST be written in Korean (한국어).

{
  "sessions": [
    {
      "session_id": "the session identifier (keep original ID as-is)",
      "work_summary": "이 세션에서 수행한 작업을 1-2문장으로 요약 (한국어)",
      "decisions": [
        {
          "what": "결정 또는 선택 분기에 대한 설명 (한국어)",
          "why": "사용자가 이 옵션을 선택한 이유 (한국어)"
        }
      ],
      "curiosities": [
        "사용자가 궁금했거나 헷갈렸던 것 (한국어)"
      ],
      "corrections": [
        {
          "model_said": "AI가 틀리게 말한 내용 (한국어)",
          "user_corrected": "사용자가 수정한 내용 (한국어)"
        }
      ],
      "til": [
        {
          "title": "배운 것을 한 줄로 요약 (한국어)",
          "detail": "왜 이게 필요했고 어떻게 적용했는지 1-2줄 (한국어)"
        }
      ]
    }
  ]
}

Rules:
- Each session_id in the transcript should have its own entry in the sessions array.
- For decisions: look for moments where the user chose between alternatives, rejected a suggestion, or stated a preference.
- For curiosities: look for questions the user asked, concepts they wanted explained, or things they expressed uncertainty about.
- For corrections: look for cases where the user pointed out an error in the AI's response, provided factual corrections, or disagreed with the AI's approach.
- For til: extract what the user ACTUALLY LEARNED during the session. Do NOT simply rephrase curiosities or corrections. Look for moments where the user gained new understanding, discovered a technique, or resolved a confusion. Each item needs a concrete title and 1-2 lines of context explaining why it mattered in this session. If nothing was learned, use an empty array. Avoid generic knowledge — only include learnings specific to this session's context.
- If a category has no items for a session, use an empty array.
- work_summary should capture the main task or goal of the session.
- Return ONLY the JSON object. Do not wrap it in markdown code fences.
- ALL text values (except session_id) MUST be in Korean."#;

/// System prompt for development progress summaries.
pub const SUMMARY_PROMPT: &str = r#"You are a development progress summarizer. You receive session analysis results from a developer's day.

Generate a concise Markdown summary of what was accomplished today. This summary will be shared with both developers and non-developers.

Rules:
- Group by project name using Markdown h3 headers (### followed by project name)
- Under each header, list accomplishments as "- " bullet points
- Use clear, non-technical language that anyone can understand
- Focus on WHAT was done and the outcome, not HOW
- Keep each bullet to 1-2 sentences maximum
- Return ONLY the Markdown content (headers + bullet points), no additional text
- ALL text MUST be in Korean (한국어)
- If multiple tasks were done in the same project, use separate bullets under the same header"#;

/// System prompt for Slack-friendly message generation.
pub const SLACK_PROMPT: &str = r#"너는 개발자가 작성한 작업 내용을 비개발자도 이해할 수 있는 슬랙 공유 메시지로 변환하는 역할을 한다.

출력 형식:
- 항상 아래 형식으로 출력한다:
[금일 작업 공유]

- ...
- ...

말투:
- 모든 문장은 "~했습니다" 형태로 끝낸다
- 보고용 문체로 간결하게 작성한다
- 과장 없이 사실만 전달한다

난이도 조정:
- 개발 용어는 최대한 제거하거나 쉬운 표현으로 변환한다
- 예: API → 기능, 배포 → 반영/적용, staging → 테스트 환경, 디버깅 → 문제를 확인하고 수정, 토큰 → 알림 수신 정보/연결 정보
- 비개발자가 읽어도 이해 가능해야 한다

내용 정리:
- 같은 주제는 하나로 묶어서 작성한다
- 너무 세부적인 내용은 묶어서 단순화한다
- 핵심 결과 중심으로 작성한다
- 반드시 포함: 무엇을 했는지, 무엇이 개선됐는지, 어떤 문제를 해결했는지

길이:
- 전체는 5~7줄 이내
- 각 줄은 1~2문장으로 간결하게 작성

금지 사항:
- 영어 개발 용어 남용 금지
- 내부 코드/파일명/경로 언급 금지
- PR, 브랜치, 커밋 등 협업 도구 용어 금지
- 불필요한 수치/라인수/구현 디테일 제거

결과만 출력하고 설명은 하지 않는다."#;

/// LLM provider enum. Adding a new variant triggers compile errors at unhandled match arms.
pub enum LlmProvider {
    Anthropic,
    OpenAi,
}

impl LlmProvider {
    /// Maximum output tokens for the model (provider-specific constant).
    pub fn max_output_tokens(&self) -> u64 {
        match self {
            LlmProvider::Anthropic => 32_000,
            LlmProvider::OpenAi => 16_384,
        }
    }

    /// Calls the provider's API and returns the raw text response.
    pub async fn call_api(
        &self,
        api_key: &str,
        conversation_text: &str,
        max_tokens: u32,
    ) -> Result<String, super::AnalyzerError> {
        match self {
            LlmProvider::Anthropic => {
                super::anthropic::call_anthropic_api(api_key, SYSTEM_PROMPT, conversation_text, max_tokens)
                    .await
            }
            LlmProvider::OpenAi => {
                super::openai::call_openai_api(api_key, SYSTEM_PROMPT, conversation_text, max_tokens).await
            }
        }
    }

    /// Calls the summary API (uses SUMMARY_PROMPT).
    pub async fn call_summary_api(
        &self,
        api_key: &str,
        session_summaries: &str,
    ) -> Result<String, super::AnalyzerError> {
        match self {
            LlmProvider::Anthropic => {
                super::anthropic::call_anthropic_api(api_key, SUMMARY_PROMPT, session_summaries, 16384)
                    .await
            }
            LlmProvider::OpenAi => {
                super::openai::call_openai_api(api_key, SUMMARY_PROMPT, session_summaries, 16384).await
            }
        }
    }

    /// Calls the Slack message API (uses SLACK_PROMPT).
    pub async fn call_slack_api(
        &self,
        api_key: &str,
        session_summaries: &str,
    ) -> Result<String, super::AnalyzerError> {
        match self {
            LlmProvider::Anthropic => {
                super::anthropic::call_anthropic_api(api_key, SLACK_PROMPT, session_summaries, 4096)
                    .await
            }
            LlmProvider::OpenAi => {
                super::openai::call_openai_api(api_key, SLACK_PROMPT, session_summaries, 4096).await
            }
        }
    }

    /// API call with explicit max_tokens (used for chunk summarization).
    pub async fn call_api_with_max_tokens(
        &self,
        api_key: &str,
        system_prompt: &str,
        conversation_text: &str,
        max_tokens: u32,
    ) -> Result<String, super::AnalyzerError> {
        match self {
            LlmProvider::Anthropic => {
                super::anthropic::call_anthropic_api_with_max_tokens(
                    api_key, system_prompt, conversation_text, max_tokens,
                )
                .await
            }
            LlmProvider::OpenAi => {
                super::openai::call_openai_api_with_max_tokens(
                    api_key, system_prompt, conversation_text, max_tokens,
                )
                .await
            }
        }
    }

    /// Returns the display name for this provider.
    pub fn display_name(&self) -> &'static str {
        match self {
            LlmProvider::Anthropic => "Claude",
            LlmProvider::OpenAi => "OpenAI",
        }
    }

    /// Probes actual rate limits via a lightweight API call.
    /// Returns (RateLimits, probed) where probed=true means real values, false means defaults.
    pub async fn probe_rate_limits(
        &self,
        api_key: &str,
    ) -> (super::planner::RateLimits, bool) {
        let result = match self {
            LlmProvider::Anthropic => {
                super::anthropic::probe_anthropic_rate_limits(api_key).await
            }
            LlmProvider::OpenAi => {
                super::openai::probe_openai_rate_limits(api_key).await
            }
        };
        match result {
            Some(limits) => (limits, true),
            None => (super::planner::RateLimits::default_generous(), false),
        }
    }
}

/// Loads the LLM provider and API key from config (~/.config/rwd/config.toml).
pub fn load_provider() -> Result<(LlmProvider, String), super::AnalyzerError> {
    let config = crate::config::load_config_if_exists()
        .ok_or(crate::messages::error::NO_CONFIG)?;

    let provider = match config.llm.provider.as_str() {
        "openai" => LlmProvider::OpenAi,
        _ => LlmProvider::Anthropic,
    };
    Ok((provider, config.llm.api_key))
}
