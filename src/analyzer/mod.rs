// analyzer 모듈은 파싱된 로그 데이터를 LLM API에 보내 인사이트를 추출하는 역할을 합니다.
// provider.rs의 LlmProvider enum으로 Anthropic, OpenAI 등 여러 프로바이더를 지원합니다.
// parser 모듈과 같은 디렉토리 구조를 사용합니다 (Rust Book Ch.7 참조).

pub mod anthropic;
pub mod insight;
pub mod openai;
pub mod prompt;
pub mod provider;

// parser 모듈과 동일한 에러 타입 패턴을 사용합니다.
// M5에서 thiserror로 전용 에러 타입을 만들 예정입니다.
pub type AnalyzerError = Box<dyn std::error::Error>;

// pub use로 외부에서 자주 사용할 타입들을 상위 모듈에서 바로 접근할 수 있게 합니다.
pub use insight::AnalysisResult;

use crate::parser::claude::LogEntry;
use crate::parser::codex::CodexEntry;

/// 로그 엔트리들을 분석하여 인사이트를 추출합니다.
/// 이 함수가 M3의 핵심 진입점입니다.
///
/// async fn은 비동기 함수를 선언합니다 (tokio 런타임 위에서 실행).
/// 네트워크 I/O(API 호출) 동안 다른 작업을 처리할 수 있게 해줍니다.
/// 호출 시 .await를 붙여야 실제로 실행됩니다 (Rust Async Book 참조).
///
/// provider::load_provider()로 프로바이더와 API 키를 읽고,
/// provider.call_api()로 선택된 프로바이더의 API를 호출합니다.
/// 이 함수 자체는 어떤 프로바이더가 사용되는지 알 필요가 없습니다.
pub async fn analyze_entries(entries: &[LogEntry]) -> Result<AnalysisResult, AnalyzerError> {
    let (provider, api_key) = provider::load_provider()?;
    let prompt_text = prompt::build_prompt(entries)?;
    let raw_response = provider.call_api(&api_key, &prompt_text).await?;
    let result = insight::parse_response(&raw_response)?;
    Ok(result)
}

/// 분석 결과를 기반으로 개발 진척사항 요약을 생성합니다.
///
/// session_summaries: 각 세션의 work_summary를 이어붙인 텍스트.
/// LLM에게 SUMMARY_PROMPT와 함께 전달하여 비개발자도 읽을 수 있는 요약을 생성합니다.
pub async fn analyze_summary(session_summaries: &str) -> Result<String, AnalyzerError> {
    let (provider, api_key) = provider::load_provider()?;
    let raw_response = provider.call_summary_api(&api_key, session_summaries).await?;
    Ok(raw_response)
}

/// Codex 세션의 엔트리들을 분석하여 인사이트를 추출합니다.
/// Claude용 analyze_entries()와 동일한 파이프라인이지만, Codex용 프롬프트를 사용합니다.
pub async fn analyze_codex_entries(
    entries: &[CodexEntry],
    session_id: &str,
) -> Result<AnalysisResult, AnalyzerError> {
    let (provider, api_key) = provider::load_provider()?;
    let prompt_text = prompt::build_codex_prompt(entries, session_id)?;
    let raw_response = provider.call_api(&api_key, &prompt_text).await?;
    let result = insight::parse_response(&raw_response)?;
    Ok(result)
}
