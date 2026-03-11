// Claude Messages API HTTP 클라이언트.
//
// reqwest 크레이트로 비동기 HTTP 요청을 보냅니다.
// async/await 패턴으로 네트워크 I/O 동안 스레드를 차단하지 않습니다.

use serde::{Deserialize, Serialize};

// Claude Messages API 엔드포인트
const API_URL: &str = "https://api.anthropic.com/v1/messages";
// API 버전 헤더 값
const API_VERSION: &str = "2023-06-01";
// 분석에 사용할 모델 (M5에서 설정 파일로 변경 예정)
const MODEL: &str = "claude-opus-4-6";

/// 시스템 프롬프트 — LLM에게 인사이트 추출 방법과 JSON 응답 형식을 지시합니다.
const SYSTEM_PROMPT: &str = r#"You are an AI coding session analyst. You receive transcripts of conversations between a developer and an AI coding assistant.

Analyze the conversation and extract insights in the following JSON format. Return ONLY valid JSON, no other text.

{
  "sessions": [
    {
      "session_id": "the session identifier",
      "work_summary": "1-2 sentence summary of what was accomplished in this session",
      "decisions": [
        {
          "what": "description of the decision or choice point",
          "why": "the reason the user chose this option"
        }
      ],
      "curiosities": [
        "thing the user was curious about or confused by"
      ],
      "corrections": [
        {
          "model_said": "what the AI said that was wrong",
          "user_corrected": "what the user said to correct it"
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
- If a category has no items for a session, use an empty array.
- work_summary should capture the main task or goal of the session.
- Return ONLY the JSON object. Do not wrap it in markdown code fences."#;

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

/// Claude Messages API를 호출하여 원시 텍스트 응답을 반환합니다.
///
/// reqwest::Client는 HTTP 클라이언트입니다 — 빌더 패턴으로 요청을 구성합니다.
/// .post(url): POST 요청 생성
/// .header(key, value): HTTP 헤더 추가
/// .json(&body): 구조체를 JSON으로 직렬화하여 요청 본문에 설정 (serde::Serialize 필요)
/// .send().await: 비동기로 요청을 보내고 응답을 기다림
/// .error_for_status(): HTTP 상태 코드가 4xx/5xx이면 Err로 변환
pub async fn call_claude_api(
    api_key: &str,
    conversation_text: &str,
) -> Result<String, super::AnalyzerError> {
    let client = reqwest::Client::new();

    let request_body = ApiRequest {
        model: MODEL.to_string(),
        max_tokens: 4096,
        system: SYSTEM_PROMPT.to_string(),
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
