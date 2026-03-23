// Anthropic Claude Messages API client.

use serde::{Deserialize, Serialize};
use super::planner::RateLimits;

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";
const MODEL: &str = "claude-opus-4-6";

/// Claude Messages API request body.
#[derive(Serialize)]
struct ApiRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<ApiMessage>,
}

/// API message (role + content).
#[derive(Serialize)]
struct ApiMessage {
    role: String,
    content: String,
}

/// Claude Messages API response body.
#[derive(Deserialize)]
struct ApiResponse {
    content: Vec<ApiContentBlock>,
    #[serde(default)]
    usage: Option<UsageInfo>,
}

/// Token usage from the API response.
#[derive(Deserialize)]
struct UsageInfo {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
}

/// A content block in the response.
#[derive(Deserialize)]
struct ApiContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    text: Option<String>,
}

/// Calls the Anthropic Claude Messages API and returns the raw text response.
pub async fn call_anthropic_api(
    api_key: &str,
    system_prompt: &str,
    conversation_text: &str,
    max_tokens: u32,
) -> Result<(String, super::ApiUsage), super::AnalyzerError> {
    let client = reqwest::Client::new();

    let request_body = ApiRequest {
        model: MODEL.to_string(),
        max_tokens,
        system: system_prompt.to_string(),
        messages: vec![ApiMessage {
            role: "user".to_string(),
            content: conversation_text.to_string(),
        }],
    };

    let response = client
        .post(API_URL)
        .header("x-api-key", api_key)
        .header("anthropic-version", API_VERSION)
        .header("content-type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_default();
        return Err(crate::messages::error::api_request_failed(&status, &error_body).into());
    }

    let api_response: ApiResponse = response.json().await?;

    let usage = api_response.usage.map(|u| super::ApiUsage {
        input_tokens: u.input_tokens,
        output_tokens: u.output_tokens,
    }).unwrap_or_default();

    let text = api_response
        .content
        .iter()
        .find(|block| block.block_type == "text")
        .and_then(|block| block.text.as_deref())
        .ok_or(crate::messages::error::API_NO_TEXT_BLOCK)?;

    Ok((text.to_string(), usage))
}

/// API call variant with explicit max_tokens. Used to cap output size for summaries.
pub async fn call_anthropic_api_with_max_tokens(
    api_key: &str,
    system_prompt: &str,
    conversation_text: &str,
    max_tokens: u32,
) -> Result<(String, super::ApiUsage), super::AnalyzerError> {
    let client = reqwest::Client::new();
    let request_body = ApiRequest {
        model: MODEL.to_string(),
        max_tokens,
        system: system_prompt.to_string(),
        messages: vec![ApiMessage {
            role: "user".to_string(),
            content: conversation_text.to_string(),
        }],
    };
    let response = client
        .post(API_URL)
        .header("x-api-key", api_key)
        .header("anthropic-version", API_VERSION)
        .header("content-type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_default();
        return Err(crate::messages::error::api_request_failed(&status, &error_body).into());
    }
    let api_response: ApiResponse = response.json().await?;

    let usage = api_response.usage.map(|u| super::ApiUsage {
        input_tokens: u.input_tokens,
        output_tokens: u.output_tokens,
    }).unwrap_or_default();

    let text = api_response
        .content
        .iter()
        .find(|block| block.block_type == "text")
        .and_then(|block| block.text.as_deref())
        .ok_or(crate::messages::error::API_NO_TEXT_BLOCK)?;
    Ok((text.to_string(), usage))
}

/// Sends a minimal request to probe rate limits from response headers.
/// Returns None on failure; the caller falls back to default_generous().
pub async fn probe_anthropic_rate_limits(api_key: &str) -> Option<RateLimits> {
    let client = reqwest::Client::new();

    let request_body = ApiRequest {
        model: MODEL.to_string(),
        max_tokens: 1,
        system: String::new(),
        messages: vec![ApiMessage {
            role: "user".to_string(),
            content: "ping".to_string(),
        }],
    };

    let response = client
        .post(API_URL)
        .header("x-api-key", api_key)
        .header("anthropic-version", API_VERSION)
        .header("content-type", "application/json")
        .json(&request_body)
        .send()
        .await
        .ok()?;

    parse_anthropic_rate_headers(&response)
}

/// Extracts rate limit values from Anthropic response headers.
fn parse_anthropic_rate_headers(response: &reqwest::Response) -> Option<RateLimits> {
    let headers = response.headers();

    let itpm = headers
        .get("anthropic-ratelimit-input-tokens-limit")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())?;

    let otpm = headers
        .get("anthropic-ratelimit-output-tokens-limit")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(itpm / 4);

    let rpm = headers
        .get("anthropic-ratelimit-requests-limit")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(50);

    Some(RateLimits {
        input_tokens_per_minute: itpm,
        output_tokens_per_minute: otpm,
        requests_per_minute: rpm,
    })
}
