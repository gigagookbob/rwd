// mod 키워드로 모듈을 선언합니다.
// Rust는 파일 하나가 모듈 하나에 대응됩니다 — cli.rs 파일이 cli 모듈이 됩니다 (Rust Book Ch.7 참조).
mod cli;
mod parser;

// use 키워드로 다른 모듈의 항목을 현재 스코프로 가져옵니다.
use clap::Parser;
use cli::Commands;

fn main() {
    // Parser::parse()는 커맨드라인 인자를 읽어서 Cli 구조체로 변환합니다.
    // --help나 --version이 입력되면 자동으로 처리하고 프로그램을 종료합니다.
    let args = cli::Cli::parse();

    // match는 enum의 모든 가능한 값을 처리하는 표현식입니다 (Rust Book Ch.6 참조).
    // Rust 컴파일러는 모든 변형(variant)을 처리했는지 검사합니다 — 빠뜨리면 컴파일 에러가 납니다.
    match args.command {
        Commands::Today => {
            // run_today()가 Err를 반환하면 에러 메시지를 출력하고 종료합니다.
            // if let은 Result가 특정 variant인 경우만 처리합니다 (Rust Book Ch.6.3 참조).
            if let Err(e) = run_today() {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    }
}

/// 오늘의 세션 로그를 파싱하고 요약 정보를 출력합니다.
/// 별도 함수로 분리하여 Result를 반환하면, ?로 에러를 깔끔하게 전파할 수 있습니다 (Rust Book Ch.9 참조).
fn run_today() -> Result<(), parser::ParseError> {
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

    Ok(())
}
