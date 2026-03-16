// mod 키워드로 모듈을 선언합니다.
// Rust는 파일 하나가 모듈 하나에 대응됩니다 — cli.rs 파일이 cli 모듈이 됩니다 (Rust Book Ch.7 참조).
mod analyzer;
mod cli;
mod config;
mod output;
mod parser;

// use 키워드로 다른 모듈의 항목을 현재 스코프로 가져옵니다.
use clap::Parser;
use cli::Commands;

/// #[tokio::main]은 async fn main()을 동기 main()으로 변환하는 속성 매크로입니다.
/// 내부적으로 tokio 런타임을 생성하고, async 블록을 실행합니다.
/// tokio는 Rust의 비동기 런타임으로, async/await를 실행하는 "엔진" 역할을 합니다.
/// (tokio 공식 튜토리얼: https://tokio.rs/tokio/tutorial 참조)
#[tokio::main]
async fn main() {
    // Parser::parse()는 커맨드라인 인자를 읽어서 Cli 구조체로 변환합니다.
    // --help나 --version이 입력되면 자동으로 처리하고 프로그램을 종료합니다.
    let args = cli::Cli::parse();

    // match는 enum의 모든 가능한 값을 처리하는 표현식입니다 (Rust Book Ch.6 참조).
    // Rust 컴파일러는 모든 변형(variant)을 처리했는지 검사합니다 — 빠뜨리면 컴파일 에러가 납니다.
    match args.command {
        Commands::Today => {
            // run_today()가 async이므로 .await로 완료를 기다립니다.
            // .await는 "이 비동기 작업이 끝날 때까지 기다려라"는 의미입니다.
            if let Err(e) = run_today().await {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Init => {
            if let Err(e) = config::run_init() {
                eprintln!("초기 설정 실패: {e}");
                std::process::exit(1);
            }
        }
        Commands::Config { key, value } => {
            if let Err(e) = config::run_config(&key, &value) {
                eprintln!("설정 변경 실패: {e}");
                std::process::exit(1);
            }
        }
    }
}

/// 오늘의 세션 로그를 파싱하고, LLM 분석을 수행하여 인사이트를 출력합니다.
///
/// async fn은 비동기 함수를 선언합니다 — 내부에서 .await를 사용할 수 있습니다.
/// 비동기 함수는 호출 시 즉시 실행되지 않고, .await를 만나야 실행됩니다.
/// 여기서는 analyzer::analyze_entries() 호출이 네트워크 I/O를 수행하므로 async가 필요합니다.
async fn run_today() -> Result<(), parser::ParseError> {
    // 설정 파일이 없으면 init을 먼저 실행하도록 안내하고 중단합니다.
    if config::load_config_if_exists().is_none() {
        eprintln!("설정 파일이 없습니다. 먼저 `rwd init`을 실행해 주세요.");
        std::process::exit(1);
    }

    let today = chrono::Utc::now().date_naive();
    let base_dir = parser::discover_log_dir()?;

    println!("Scanning: {}", base_dir.display());

    let mut all_entries = Vec::new();

    // 모든 프로젝트 디렉토리를 순회하며 오늘의 로그를 수집합니다.
    for project_dir in parser::list_project_dirs()? {
        for session_file in parser::list_session_files(&project_dir)? {
            let entries = parser::parse_jsonl_file(&session_file)?;
            // filter_entries_by_date는 Vec의 소유권을 가져가고 필터된 Vec을 반환합니다.
            let today_entries = parser::filter_entries_by_date(entries, today);
            // .extend()는 다른 Vec의 모든 요소를 현재 Vec에 추가합니다 (Rust Book Ch.8.1 참조).
            all_entries.extend(today_entries);
        }
    }

    if all_entries.is_empty() {
        println!("No log entries found for today ({today}).");
        return Ok(());
    }

    let summaries = parser::summarize_entries(&all_entries);

    println!("\n=== rwd today ({today}) ===");
    println!("Sessions: {}", summaries.len());

    for s in &summaries {
        // &s.session_id[..8]은 문자열의 처음 8글자만 슬라이스합니다 (Rust Book Ch.4.3 참조).
        let total_in = s.total_input_tokens
            + s.total_cache_creation_tokens
            + s.total_cache_read_tokens;
        println!("\nSession: {}...", &s.session_id[..8]);
        println!("  User messages:      {}", s.user_count);
        println!("  Assistant messages:  {}", s.assistant_count);
        println!("  Tool uses:          {}", s.tool_use_count);
        println!(
            "  Tokens (in/out):    {}/{}",
            total_in, s.total_output_tokens
        );
    }

    // LLM 분석 수행 — provider::load_provider()가 프로바이더 선택과 API 키 로딩을 담당합니다.
    // 여기서는 표시 이름만 읽어 사용자에게 어떤 프로바이더가 사용되는지 알려줍니다.
    let provider_label = analyzer::provider::load_provider()
        .map(|(p, _)| p.display_name().to_string())
        .unwrap_or_else(|_| "LLM".to_string());
    println!("\n{provider_label} API로 인사이트 분석 중...");
    // analyzer 실패 시에도 기존 요약은 이미 출력되었으므로 프로그램을 종료하지 않습니다.
    // match로 성공/실패를 명시적으로 처리합니다 (?로 전파하지 않음).
    match analyzer::analyze_entries(&all_entries).await {
        Ok(analysis) => {
            print_insights(&analysis);
            // [M4] 분석 결과를 Markdown으로 변환하여 vault에 저장합니다.
            save_analysis(&analysis, today);
        }
        Err(e) => eprintln!("분석 실패: {e}"),
    }

    Ok(())
}

/// [M4] 분석 결과를 Markdown으로 변환하여 Obsidian vault에 저장합니다.
///
/// 각 단계에서 실패하면 에러를 출력하고 return합니다 — 프로그램을 중단하지 않습니다.
/// vault 경로가 설정되지 않아도 터미널 출력(print_insights)은 이미 완료된 상태입니다.
fn save_analysis(analysis: &analyzer::AnalysisResult, date: chrono::NaiveDate) {
    let vault_path = match output::load_vault_path() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Vault 경로 로드 실패: {e}");
            return;
        }
    };

    let markdown = output::render_markdown(analysis, date);

    match output::save_to_vault(&vault_path, date, &markdown) {
        Ok(saved) => println!("\nMarkdown 저장 완료: {}", saved.display()),
        Err(e) => eprintln!("파일 저장 실패: {e}"),
    }
}

/// 분석 결과를 터미널에 출력합니다.
fn print_insights(analysis: &analyzer::AnalysisResult) {
    println!("\n=== 인사이트 분석 결과 ===");

    for session in &analysis.sessions {
        println!("\n--- Session: {} ---", session.session_id);
        println!("요약: {}", session.work_summary);

        if !session.decisions.is_empty() {
            println!("\n선택 분기:");
            for d in &session.decisions {
                println!("  - {}", d.what);
                println!("    이유: {}", d.why);
            }
        }

        if !session.curiosities.is_empty() {
            println!("\n궁금/헷갈렸던 것:");
            for c in &session.curiosities {
                println!("  - {c}");
            }
        }

        if !session.corrections.is_empty() {
            println!("\n모델 수정:");
            for c in &session.corrections {
                println!("  - 모델: {}", c.model_said);
                println!("    수정: {}", c.user_corrected);
            }
        }
    }
}
