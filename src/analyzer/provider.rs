// LLM 프로바이더 추상화 모듈.
//
// LlmProvider enum으로 여러 LLM API를 통합합니다.
// enum + match 패턴은 LogEntry, Commands와 동일한 방식입니다 (Rust Book Ch.6 참조).
// Anthropic과 OpenAI를 지원하며, 환경 변수로 프로바이더를 선택합니다.

/// 시스템 프롬프트 — 모든 프로바이더가 공유합니다.
/// 프로바이더에 관계없이 동일한 분석 지시를 전달합니다.
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

/// 개발 진척사항 요약용 시스템 프롬프트.
/// 비개발자도 이해할 수 있는 한국어 Markdown을 생성하도록 지시합니다.
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

/// LLM 프로바이더를 나타내는 enum.
///
/// enum은 "이것 또는 저것" 중 하나의 값을 표현합니다 (Rust Book Ch.6.1).
/// 각 변형(variant)은 서로 다른 프로바이더에 대응합니다.
/// match 표현식으로 모든 변형을 처리해야 컴파일됩니다 — 새 프로바이더 추가 시
/// 컴파일러가 처리하지 않은 곳을 자동으로 알려줍니다.
pub enum LlmProvider {
    Anthropic,
    OpenAi,
}

impl LlmProvider {
    /// 모델별 최대 출력 토큰.
    /// 모델 스펙이며 tier/사용자와 무관한 상수.
    pub fn max_output_tokens(&self) -> u64 {
        match self {
            LlmProvider::Anthropic => 32_000,
            LlmProvider::OpenAi => 16_384,
        }
    }

    /// 선택된 프로바이더의 API를 호출하여 원시 텍스트 응답을 반환합니다.
    ///
    /// &self는 이 메서드가 LlmProvider 값의 참조를 받는다는 의미입니다.
    /// match self로 어떤 프로바이더인지 확인하고, 해당 모듈의 함수를 호출합니다.
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

    /// 개발 진척사항 요약 API를 호출합니다.
    /// call_api()와 동일한 구조이지만, SUMMARY_PROMPT를 사용합니다.
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

    /// max_tokens를 지정할 수 있는 API 호출.
    /// 요약 호출 시 max_tokens를 2000으로 제한한다.
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

    /// 프로바이더의 표시 이름을 반환합니다.
    pub fn display_name(&self) -> &'static str {
        match self {
            LlmProvider::Anthropic => "Claude",
            LlmProvider::OpenAi => "OpenAI",
        }
    }

    /// API probe 호출로 사용자의 실제 rate limit을 확인한다.
    /// 반환: (RateLimits, probed) — probed가 true면 실제 확인, false면 기본값 fallback.
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

/// 설정 파일(~/.config/rwd/config.toml)에서 LLM 프로바이더와 API 키를 읽습니다.
///
/// 반환: (프로바이더, API 키) 튜플.
/// 튜플은 서로 다른 타입의 값을 묶는 간단한 방법입니다 (Rust Book Ch.3.2).
pub fn load_provider() -> Result<(LlmProvider, String), super::AnalyzerError> {
    let config = crate::config::load_config_if_exists()
        .ok_or("설정 파일이 없습니다. `rwd init`을 먼저 실행해 주세요.")?;

    let provider = match config.llm.provider.as_str() {
        "openai" => LlmProvider::OpenAi,
        _ => LlmProvider::Anthropic,
    };
    Ok((provider, config.llm.api_key))
}
