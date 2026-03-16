// OpenAI Chat Completions API 클라이언트.
//
// Anthropic과 같은 reqwest + serde 패턴을 사용하지만, 요청/응답 JSON 스키마가 다릅니다.
// 주요 차이점:
// - 인증: Authorization: Bearer <key> (Anthropic은 x-api-key 헤더)
// - system prompt: messages 배열의 {"role": "system"} 메시지 (Anthropic은 top-level system 필드)
// - 응답: choices[0].message.content (Anthropic은 content[0].text)

use serde::{Deserialize, Serialize};

// OpenAI Chat Completions 엔드포인트
const API_URL: &str = "https://api.openai.com/v1/chat/completions";
// 분석에 사용할 모델 (M5에서 설정 파일로 변경 예정)
const MODEL: &str = "gpt-4o";

// === API 요청/응답 타입 (이 모듈 내부에서만 사용) ===

/// OpenAI Chat Completions 요청 본문.
/// Anthropic의 ApiRequest와 비교하면:
/// - system 필드가 없음 → system prompt를 messages 배열에 포함
/// - messages 배열에 role: "system", "user" 등 여러 역할의 메시지를 넣음
#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
}

/// Chat 메시지 (role + content).
/// Anthropic의 ApiMessage와 동일한 구조이지만, role에 "system"도 가능합니다.
#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

/// OpenAI Chat Completions 응답 본문.
/// Anthropic은 content 배열을 사용하지만, OpenAI는 choices 배열을 사용합니다.
#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

/// choices 배열의 각 항목.
/// index, message, finish_reason을 포함하지만, 우리는 message.content만 사용합니다.
#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

/// choice 내부의 메시지.
/// role은 항상 "assistant"이고, content에 LLM 응답 텍스트가 들어 있습니다.
#[derive(Deserialize)]
struct ChoiceMessage {
    content: String,
}

/// OpenAI Chat Completions API를 호출하여 원시 텍스트 응답을 반환합니다.
///
/// anthropic.rs의 call_anthropic_api()와 동일한 역할을 하지만:
/// - Authorization: Bearer 헤더로 인증합니다 (Anthropic은 x-api-key)
/// - system prompt를 messages 배열의 첫 번째 메시지로 전달합니다
/// - 응답에서 choices[0].message.content를 추출합니다
pub async fn call_openai_api(
    api_key: &str,
    system_prompt: &str,
    conversation_text: &str,
) -> Result<String, super::AnalyzerError> {
    let client = reqwest::Client::new();

    // OpenAI는 system prompt를 messages 배열에 {"role": "system"} 메시지로 전달합니다.
    // Anthropic은 top-level system 필드를 사용하는 것과 대조적입니다.
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
        max_tokens: 16384,
    };

    // Authorization: Bearer 헤더 — OpenAI의 인증 방식입니다.
    // format!()으로 문자열을 조합합니다 (Rust Book Ch.8.2 참조).
    let response = client
        .post(API_URL)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    // anthropic.rs와 동일한 에러 처리 패턴
    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_default();
        return Err(format!("OpenAI API 요청 실패 ({status}): {error_body}").into());
    }

    let chat_response: ChatResponse = response.json().await?;

    // choices 배열의 첫 번째 항목에서 content를 추출합니다.
    // .first()는 슬라이스의 첫 번째 요소를 Option으로 반환합니다 (Rust Book Ch.8.1).
    let text = chat_response
        .choices
        .first()
        .ok_or("OpenAI 응답에 choices가 비어 있습니다")?;

    Ok(text.message.content.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// OpenAI 응답 JSON을 올바르게 파싱하는지 테스트합니다.
    /// 실제 API 호출 없이 응답 파싱 로직만 검증합니다.
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

    /// choices가 빈 배열인 경우 .first()가 None을 반환하는지 확인합니다.
    #[test]
    fn test_empty_choices() {
        let json = r#"{"choices": []}"#;
        let response: ChatResponse = serde_json::from_str(json).unwrap();
        assert!(response.choices.first().is_none());
    }
}
