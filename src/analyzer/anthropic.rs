// Anthropic Claude Messages API 클라이언트.
//
// reqwest 크레이트로 비동기 HTTP 요청을 보냅니다.
// async/await 패턴으로 네트워크 I/O 동안 스레드를 차단하지 않습니다.

use serde::{Deserialize, Serialize};
use super::planner::RateLimits;

// Claude Messages API 엔드포인트
const API_URL: &str = "https://api.anthropic.com/v1/messages";
// API 버전 헤더 값
const API_VERSION: &str = "2023-06-01";
// 분석에 사용할 모델 (M5에서 설정 파일로 변경 예정)
const MODEL: &str = "claude-opus-4-6";

// SYSTEM_PROMPT는 provider.rs로 이동했습니다 — 모든 프로바이더가 공유하는 상수입니다.

// === API 요청/응답 타입 (이 모듈 내부에서만 사용) ===

/// Claude Messages API 요청 본문.
/// Serialize 트레이트는 구조체를 JSON으로 변환(직렬화)합니다 — Deserialize의 반대 방향입니다.
/// Deserialize: JSON → 구조체, Serialize: 구조체 → JSON (Rust Book Ch.10 참조).
#[derive(Serialize)]
struct ApiRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<ApiMessage>,
}

/// API 메시지 (role + content)
#[derive(Serialize)]
struct ApiMessage {
    role: String,
    content: String,
}

/// Claude Messages API 응답 본문
#[derive(Deserialize)]
struct ApiResponse {
    content: Vec<ApiContentBlock>,
}

/// 응답의 content 블록
#[derive(Deserialize)]
struct ApiContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    text: Option<String>,
}

/// Anthropic Claude Messages API를 호출하여 원시 텍스트 응답을 반환합니다.
///
/// reqwest::Client는 HTTP 클라이언트입니다 — 빌더 패턴으로 요청을 구성합니다.
/// .post(url): POST 요청 생성
/// .header(key, value): HTTP 헤더 추가
/// .json(&body): 구조체를 JSON으로 직렬화하여 요청 본문에 설정 (serde::Serialize 필요)
/// .send().await: 비동기로 요청을 보내고 응답을 기다림
/// .error_for_status(): HTTP 상태 코드가 4xx/5xx이면 Err로 변환
pub async fn call_anthropic_api(
    api_key: &str,
    system_prompt: &str,
    conversation_text: &str,
) -> Result<String, super::AnalyzerError> {
    let client = reqwest::Client::new();

    let request_body = ApiRequest {
        model: MODEL.to_string(),
        max_tokens: 16384,
        system: system_prompt.to_string(),
        messages: vec![ApiMessage {
            role: "user".to_string(),
            content: conversation_text.to_string(),
        }],
    };

    // .await는 비동기 작업이 완료될 때까지 현재 태스크를 일시 중단합니다.
    // 중단 동안 tokio 런타임은 다른 태스크를 처리할 수 있습니다.
    let response = client
        .post(API_URL)
        .header("x-api-key", api_key)
        .header("anthropic-version", API_VERSION)
        .header("content-type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    // 4xx/5xx 에러 시 응답 본문을 읽어 상세한 에러 메시지를 제공합니다.
    // .is_success()는 HTTP 상태 코드가 2xx인지 확인합니다.
    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_default();
        return Err(format!("API 요청 실패 ({status}): {error_body}").into());
    }

    let api_response: ApiResponse = response.json().await?;

    // 첫 번째 text 블록의 내용을 추출합니다.
    // .iter()로 이터레이터를 만들고, .find()로 조건에 맞는 첫 요소를 찾습니다.
    let text = api_response
        .content
        .iter()
        .find(|block| block.block_type == "text")
        .and_then(|block| block.text.as_deref())
        .ok_or("API 응답에 텍스트 블록이 없습니다")?;

    Ok(text.to_string())
}

/// max_tokens를 지정할 수 있는 API 호출 변형.
/// 요약 호출 시 2000으로 제한하여 출력 크기를 통제한다.
pub async fn call_anthropic_api_with_max_tokens(
    api_key: &str,
    system_prompt: &str,
    conversation_text: &str,
    max_tokens: u32,
) -> Result<String, super::AnalyzerError> {
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
        return Err(format!("API 요청 실패 ({status}): {error_body}").into());
    }
    let api_response: ApiResponse = response.json().await?;
    let text = api_response
        .content
        .iter()
        .find(|block| block.block_type == "text")
        .and_then(|block| block.text.as_deref())
        .ok_or("API 응답에 텍스트 블록이 없습니다")?;
    Ok(text.to_string())
}

/// Anthropic API에 최소 요청을 보내 응답 헤더에서 rate limit을 읽는다.
/// 실패 시 None을 반환하며, 호출자가 default_generous로 대체한다.
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

/// Anthropic 응답 헤더에서 rate limit 값을 추출한다.
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
