// LLM provider abstraction. Supports Anthropic and OpenAI via a unified enum dispatch.
// Prompts are loaded from prompts/*.md at compile time via include_str!().

use crate::config::Lang;

const SYSTEM_PROMPT_EN: &str = include_str!("../../prompts/system_en.md");
const SYSTEM_PROMPT_KO: &str = include_str!("../../prompts/system_ko.md");
const SUMMARY_PROMPT_EN: &str = include_str!("../../prompts/summary_en.md");
const SUMMARY_PROMPT_KO: &str = include_str!("../../prompts/summary_ko.md");
const SLACK_PROMPT_EN: &str = include_str!("../../prompts/slack_en.md");
const SLACK_PROMPT_KO: &str = include_str!("../../prompts/slack_ko.md");

fn get_system_prompt(lang: &Lang) -> &'static str {
    match lang {
        Lang::En => SYSTEM_PROMPT_EN,
        Lang::Ko => SYSTEM_PROMPT_KO,
    }
}

fn get_summary_prompt(lang: &Lang) -> &'static str {
    match lang {
        Lang::En => SUMMARY_PROMPT_EN,
        Lang::Ko => SUMMARY_PROMPT_KO,
    }
}

fn get_slack_prompt(lang: &Lang) -> &'static str {
    match lang {
        Lang::En => SLACK_PROMPT_EN,
        Lang::Ko => SLACK_PROMPT_KO,
    }
}

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

    /// Calls the provider's API with the system prompt selected by language.
    pub async fn call_api(
        &self,
        api_key: &str,
        conversation_text: &str,
        max_tokens: u32,
        lang: &Lang,
    ) -> Result<String, super::AnalyzerError> {
        let prompt = get_system_prompt(lang);
        match self {
            LlmProvider::Anthropic => {
                super::anthropic::call_anthropic_api(api_key, prompt, conversation_text, max_tokens)
                    .await
            }
            LlmProvider::OpenAi => {
                super::openai::call_openai_api(api_key, prompt, conversation_text, max_tokens).await
            }
        }
    }

    /// Calls the summary API with language-selected prompt.
    pub async fn call_summary_api(
        &self,
        api_key: &str,
        session_summaries: &str,
        lang: &Lang,
    ) -> Result<String, super::AnalyzerError> {
        let prompt = get_summary_prompt(lang);
        match self {
            LlmProvider::Anthropic => {
                super::anthropic::call_anthropic_api(api_key, prompt, session_summaries, 16384)
                    .await
            }
            LlmProvider::OpenAi => {
                super::openai::call_openai_api(api_key, prompt, session_summaries, 16384).await
            }
        }
    }

    /// Calls the Slack message API with language-selected prompt.
    pub async fn call_slack_api(
        &self,
        api_key: &str,
        session_summaries: &str,
        lang: &Lang,
    ) -> Result<String, super::AnalyzerError> {
        let prompt = get_slack_prompt(lang);
        match self {
            LlmProvider::Anthropic => {
                super::anthropic::call_anthropic_api(api_key, prompt, session_summaries, 4096)
                    .await
            }
            LlmProvider::OpenAi => {
                super::openai::call_openai_api(api_key, prompt, session_summaries, 4096).await
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
