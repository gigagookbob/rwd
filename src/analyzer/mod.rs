// analyzer 모듈은 파싱된 로그 데이터를 Claude API에 보내 인사이트를 추출하는 역할을 합니다.
// parser 모듈과 같은 디렉토리 구조를 사용합니다 (Rust Book Ch.7 참조).

pub mod client;
pub mod insight;
pub mod prompt;

// parser 모듈과 동일한 에러 타입 패턴을 사용합니다.
// M5에서 thiserror로 전용 에러 타입을 만들 예정입니다.
pub type AnalyzerError = Box<dyn std::error::Error>;

// pub use로 외부에서 자주 사용할 타입들을 상위 모듈에서 바로 접근할 수 있게 합니다.
pub use insight::AnalysisResult;

use crate::parser::claude::LogEntry;

/// 로그 엔트리들을 분석하여 인사이트를 추출합니다.
/// 이 함수가 M3의 핵심 진입점입니다.
///
/// async fn은 비동기 함수를 선언합니다 (tokio 런타임 위에서 실행).
/// 네트워크 I/O(API 호출) 동안 다른 작업을 처리할 수 있게 해줍니다.
/// 호출 시 .await를 붙여야 실제로 실행됩니다 (Rust Async Book 참조).
pub async fn analyze_entries(entries: &[LogEntry]) -> Result<AnalysisResult, AnalyzerError> {
    let api_key = load_api_key()?;
    let prompt_text = prompt::build_prompt(entries)?;
    let raw_response = client::call_claude_api(&api_key, &prompt_text).await?;
    let result = insight::parse_response(&raw_response)?;
    Ok(result)
}

/// ANTHROPIC_API_KEY를 .env 파일 또는 환경 변수에서 읽습니다.
///
/// dotenvy::dotenv()는 프로젝트 루트의 .env 파일을 읽어 환경 변수로 등록합니다.
/// .ok()로 파일이 없어도 에러 없이 넘어갑니다 — 환경 변수가 직접 설정된 경우를 지원합니다.
/// std::env::var()는 환경 변수를 읽어 Result<String, VarError>를 반환합니다.
/// .map_err()로 에러 메시지를 사용자 친화적으로 변환합니다 (Rust Book Ch.9 참조).
fn load_api_key() -> Result<String, AnalyzerError> {
    // .env 파일이 있으면 로드, 없으면 무시
    dotenvy::dotenv().ok();

    std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
        "ANTHROPIC_API_KEY가 설정되지 않았습니다. \
         .env 파일에 추가하거나 환경 변수를 설정해 주세요. \
         예: echo 'ANTHROPIC_API_KEY=sk-ant-...' > .env"
            .into()
    })
}
