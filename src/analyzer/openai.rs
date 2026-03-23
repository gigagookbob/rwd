// OpenAI Chat Completions API client.

use serde::{Deserialize, Serialize};
use super::planner::RateLimits;

const API_URL: &str = "https://api.openai.com/v1/chat/completions";
const MODEL: &str = "gpt-4o";

/// OpenAI Chat Completions request body.
#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
}

/// Chat message (role + content).
#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

/// OpenAI Chat Completions response body.
#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<UsageInfo>,
}

/// Token usage from the API response.
#[derive(Deserialize)]
struct UsageInfo {
    #[serde(default)]
    prompt_tokens: u64,
    #[serde(default)]
    completion_tokens: u64,
}

/// A single choice from the choices array.
#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

/// The message inside a choice (role is always "assistant").
#[derive(Deserialize)]
struct ChoiceMessage {
    content: String,
}

/// Calls the OpenAI Chat Completions API and returns the raw text response.
pub async fn call_openai_api(
    api_key: &str,
    system_prompt: &str,
    conversation_text: &str,
    max_tokens: u32,
) -> Result<(String, super::ApiUsage), super::AnalyzerError> {
    let client = reqwest::Client::new();

    let request_body = ChatRequest {
        model: MODEL.to_string(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: conversation_text.to_string(),
            },
        ],
        max_tokens,
    };

    let response = client
        .post(API_URL)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_default();
        return Err(crate::messages::error::openai_api_request_failed(&status, &error_body).into());
    }

    let chat_response: ChatResponse = response.json().await?;

    let usage = chat_response.usage.map(|u| super::ApiUsage {
        input_tokens: u.prompt_tokens,
        output_tokens: u.completion_tokens,
    }).unwrap_or_default();

    let text = chat_response
        .choices
        .first()
        .ok_or(crate::messages::error::OPENAI_EMPTY_CHOICES)?;

    Ok((text.message.content.clone(), usage))
}

/// API call variant with explicit max_tokens.
pub async fn call_openai_api_with_max_tokens(
    api_key: &str,
    system_prompt: &str,
    conversation_text: &str,
    max_tokens: u32,
) -> Result<(String, super::ApiUsage), super::AnalyzerError> {
    let client = reqwest::Client::new();
    let request_body = ChatRequest {
        model: MODEL.to_string(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: conversation_text.to_string(),
            },
        ],
        max_tokens,
    };
    let response = client
        .post(API_URL)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_default();
        return Err(crate::messages::error::openai_api_request_failed(&status, &error_body).into());
    }
    let chat_response: ChatResponse = response.json().await?;

    let usage = chat_response.usage.map(|u| super::ApiUsage {
        input_tokens: u.prompt_tokens,
        output_tokens: u.completion_tokens,
    }).unwrap_or_default();

    let text = chat_response
        .choices
        .first()
        .ok_or(crate::messages::error::OPENAI_EMPTY_CHOICES)?;
    Ok((text.message.content.clone(), usage))
}

/// Sends a minimal request to probe rate limits from response headers.
pub async fn probe_openai_rate_limits(api_key: &str) -> Option<RateLimits> {
    let client = reqwest::Client::new();

    let request_body = ChatRequest {
        model: MODEL.to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: "ping".to_string(),
        }],
        max_tokens: 1,
    };

    let response = client
        .post(API_URL)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .ok()?;

    parse_openai_rate_headers(&response)
}

/// Extracts rate limit values from OpenAI response headers.
fn parse_openai_rate_headers(response: &reqwest::Response) -> Option<RateLimits> {
    let headers = response.headers();

    let tpm = headers
        .get("x-ratelimit-limit-tokens")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())?;

    let rpm = headers
        .get("x-ratelimit-limit-requests")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(50);

    Some(RateLimits {
        input_tokens_per_minute: tpm,
        output_tokens_per_minute: tpm / 4,
        requests_per_minute: rpm,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies response JSON parsing without making actual API calls.
    #[test]
    fn test_parse_openai_response() {
        let json = r#"{
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1677652288,
            "model": "gpt-4o",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "{\"sessions\": []}"
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 11,
                "total_tokens": 21
            }
        }"#;

        let response: ChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.choices.len(), 1);
        assert_eq!(response.choices[0].message.content, "{\"sessions\": []}");
    }

    /// Verifies .first() returns None for an empty choices array.
    #[test]
    fn test_empty_choices() {
        let json = r#"{"choices": []}"#;
        let response: ChatResponse = serde_json::from_str(json).unwrap();
        assert!(response.choices.first().is_none());
    }
}
