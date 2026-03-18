// clap의 derive 매크로를 사용하여 CLI를 선언적으로 정의합니다.
// derive 매크로는 구조체/열거형에 붙여서 코드를 자동 생성하는 기능입니다 (Rust Book Ch.5 참조).
// Parser 트레이트를 derive하면 clap이 커맨드라인 파싱 로직을 자동으로 만들어줍니다.
use clap::{Parser, Subcommand};

/// AI 코딩 세션 로그를 분석하여 일일 개발 인사이트를 추출하는 CLI 도구
#[derive(Parser)]
#[command(name = "rwd", version, about)]
pub struct Cli {
    /// 실행할 서브커맨드
    #[command(subcommand)]
    pub command: Commands,
}

/// rwd가 지원하는 서브커맨드 목록
// enum은 여러 가지 가능한 값 중 하나를 표현합니다 (Rust Book Ch.6 참조).
// 지금은 Today 하나뿐이지만, M2 이후에 다른 명령어를 추가할 수 있습니다.
#[derive(Subcommand)]
pub enum Commands {
    /// 오늘의 세션 로그를 분석합니다
    Today {
        /// 세션별 실행 계획 상세 출력
        #[arg(long, short)]
        verbose: bool,
    },
    /// 초기 설정을 수행합니다 (API 키, 출력 경로)
    Init,
    /// 설정 값을 변경합니다 (인자 없이 실행하면 대화형 메뉴)
    Config {
        /// 설정 키 (output-path, provider, api-key)
        key: Option<String>,
        /// 설정할 값
        value: Option<String>,
    },
    /// 최신 버전으로 업데이트합니다
    Update,
    /// 오늘의 개발 진척사항 요약을 생성합니다
    Summary,
}
