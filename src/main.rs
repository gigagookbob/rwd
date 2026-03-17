// mod 키워드로 모듈을 선언합니다.
// Rust는 파일 하나가 모듈 하나에 대응됩니다 — cli.rs 파일이 cli 모듈이 됩니다 (Rust Book Ch.7 참조).
mod analyzer;
mod cache;
mod cli;
mod config;
mod output;
mod parser;
mod redactor;
mod update;

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

    // 모든 커맨드 실행 전에 업데이트 알림을 표시합니다.
    // Commands::Update는 자체적으로 버전 체크를 하므로 스킵합니다 (중복 알림 방지).
    if !matches!(args.command, Commands::Update) {
        update::notify_if_update_available().await;
    }

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
        Commands::Update => {
            if let Err(e) = update::run_update().await {
                eprintln!("업데이트 실패: {e}");
                std::process::exit(1);
            }
        }
        Commands::Summary => {
            if let Err(e) = run_summary().await {
                eprintln!("Error: {e}");
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
    let loaded_config = config::load_config_if_exists();
    if loaded_config.is_none() {
        eprintln!("설정 파일이 없습니다. 먼저 `rwd init`을 실행해 주세요.");
        std::process::exit(1);
    }
    let redactor_enabled = loaded_config.as_ref().unwrap().is_redactor_enabled();

    // 시스템 타임존(KST) 기준으로 "오늘"을 결정합니다.
    // UTC 대신 Local을 사용하여 KST 00:00~23:59 범위의 세션을 올바르게 포함합니다.
    let today = chrono::Local::now().date_naive();

    // === Claude Code 로그 수집 ===
    let claude_entries = collect_claude_entries(today);

    // === Codex 로그 수집 ===
    let codex_sessions = collect_codex_sessions(today);

    if claude_entries.is_empty() && codex_sessions.is_empty() {
        println!("No log entries found for today ({today}).");
        return Ok(());
    }

    let claude_count = claude_entries.len();
    let codex_count = codex_sessions.len();

    let now = chrono::Local::now();
    println!("\n=== rwd today ({}) ===", now.format("%Y-%m-%d %H:%M"));

    // === Claude Code 요약 출력 ===
    if !claude_entries.is_empty() {
        let summaries = parser::summarize_entries(&claude_entries);
        let earliest = claude_earliest_time(&claude_entries);
        let time_range = format_time_range(earliest, now);

        println!("\nClaude Code");
        println!("{time_range}");
        println!("총 세션: {}", summaries.len());

        let (total_in, total_out) = claude_total_tokens(&summaries);
        println!("총 in token: {} | 총 out token: {}", format_number(total_in), format_number(total_out));
    }

    // === Codex 요약 출력 ===
    println!("\nCodex");
    if codex_sessions.is_empty() {
        println!("오늘 진행된 Codex 세션은 없습니다.");
    } else {
        let earliest = codex_earliest_time(&codex_sessions);
        let time_range = format_time_range(earliest, now);
        println!("{time_range}");
        println!("총 세션: {}", codex_sessions.len());
        println!("총 in token: - | 총 out token: -");
    }

    // === 캐시 확인 ===
    // 엔트리 수가 동일하면 이전 분석 결과를 재사용하여 LLM 호출을 생략합니다.
    // Rust 2024 Edition의 let chains를 사용하여 두 조건을 하나의 if로 합칩니다.
    if let Some(cached) = cache::load_cache(today)
        && cached.claude_entry_count == claude_count
        && cached.codex_session_count == codex_count
    {
        println!("\n캐시된 분석 결과를 사용합니다. (엔트리 수 변경 없음)");
        let source_refs: Vec<(&str, &analyzer::AnalysisResult)> = cached
            .sources
            .iter()
            .map(|(name, result)| (name.as_str(), result))
            .collect();
        for (name, analysis) in &source_refs {
            println!("\n=== {name} 인사이트 ===");
            print_insights(analysis);
        }
        save_combined_analysis(&source_refs, today);
        return Ok(());
    }

    // === LLM 분석 ===
    let provider_label = analyzer::provider::load_provider()
        .map(|(p, _)| p.display_name().to_string())
        .unwrap_or_else(|_| "LLM".to_string());
    println!("\n{provider_label} API로 인사이트 분석 중...");

    let mut sources: Vec<(String, analyzer::AnalysisResult)> = Vec::new();
    let mut total_redact = redactor::RedactResult::empty();

    // Claude 분석
    if !claude_entries.is_empty() {
        match analyzer::analyze_entries(&claude_entries, redactor_enabled).await {
            Ok((result, redact_result)) => {
                total_redact.merge(redact_result);
                sources.push(("Claude Code".to_string(), result));
            }
            Err(e) => eprintln!("Claude Code 분석 실패: {e}"),
        }
    }

    // Codex 분석 — 세션별로 개별 분석
    for (summary, entries) in &codex_sessions {
        let id_display = if summary.session_id.len() >= 8 {
            &summary.session_id[..8]
        } else {
            &summary.session_id
        };
        match analyzer::analyze_codex_entries(entries, &summary.session_id, redactor_enabled).await {
            Ok((result, redact_result)) => {
                total_redact.merge(redact_result);
                sources.push(("Codex".to_string(), result));
            }
            Err(e) => eprintln!("Codex 분석 실패 ({id_display}): {e}"),
        }
    }

    // 결과 출력 및 저장
    if total_redact.total_count > 0 {
        println!("민감 정보 {}건 마스킹됨 ({})", total_redact.total_count, total_redact.format_summary());
    }

    if !sources.is_empty() {
        let source_refs: Vec<(&str, &analyzer::AnalysisResult)> = sources
            .iter()
            .map(|(name, result)| (name.as_str(), result))
            .collect();
        for (name, analysis) in &source_refs {
            println!("\n=== {name} 인사이트 ===");
            print_insights(analysis);
        }
        save_combined_analysis(&source_refs, today);

        // 분석 결과를 캐시에 저장합니다.
        let cache_data = cache::TodayCache {
            date: today.to_string(),
            claude_entry_count: claude_count,
            codex_session_count: codex_count,
            sources,
        };
        if let Err(e) = cache::save_cache(&cache_data, today) {
            eprintln!("캐시 저장 실패: {e}");
        }
    }

    Ok(())
}

/// 오늘의 개발 진척사항 요약을 생성합니다.
///
/// 1. 오늘 캐시를 로드합니다. 없으면 run_today()를 실행한 후 다시 로드합니다.
/// 2. 캐시의 모든 세션 work_summary를 모아서 LLM에 전달합니다.
/// 3. 생성된 요약을 터미널에 출력하고, Daily Markdown 파일에 추가하며, 클립보드에 복사합니다.
async fn run_summary() -> Result<(), Box<dyn std::error::Error>> {
    let today = chrono::Local::now().date_naive();

    // 캐시가 없으면 today 분석을 먼저 실행합니다.
    let cached = match cache::load_cache(today) {
        Some(c) => c,
        None => {
            println!("캐시가 없습니다. today 분석을 먼저 실행합니다...");
            run_today().await?;
            match cache::load_cache(today) {
                Some(c) => c,
                None => {
                    eprintln!("분석 후에도 캐시를 찾을 수 없습니다.");
                    std::process::exit(1);
                }
            }
        }
    };

    // 모든 세션 work_summary를 하나의 텍스트로 합칩니다.
    // 소스 이름과 세션 요약을 함께 제공하여 LLM이 프로젝트별로 그룹화할 수 있도록 합니다.
    let mut summaries_text = String::new();
    for (source_name, analysis) in &cached.sources {
        for session in &analysis.sessions {
            summaries_text.push_str(&format!(
                "[{source_name} / {}] {}\n",
                session.session_id, session.work_summary
            ));
        }
    }

    if summaries_text.is_empty() {
        println!("요약할 세션이 없습니다.");
        return Ok(());
    }

    println!("개발 진척사항 요약 생성 중...");
    let summary = analyzer::analyze_summary(&summaries_text).await?;

    println!("\n=== 개발 진척사항 ===");
    println!("{summary}");

    // Daily Markdown 파일에 요약 섹션을 추가합니다.
    append_summary_to_markdown(today, &summary);

    // 클립보드에 복사합니다.
    copy_to_clipboard(&summary);
    println!("\n클립보드에 복사되었습니다.");

    Ok(())
}

/// 텍스트를 시스템 클립보드에 복사합니다.
/// macOS: pbcopy, Linux: xclip 사용.
///
/// std::process::Command로 외부 프로세스를 실행합니다 (Rust Book Ch.12 참조).
/// Stdio::piped()는 stdin을 파이프로 연결하여 데이터를 전달할 수 있게 합니다.
fn copy_to_clipboard(text: &str) {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut cmd = if cfg!(target_os = "macos") {
        Command::new("pbcopy")
    } else {
        let mut c = Command::new("xclip");
        c.arg("-selection").arg("clipboard");
        c
    };

    if let Ok(mut child) = cmd.stdin(Stdio::piped()).spawn() {
        if let Some(stdin) = child.stdin.as_mut() {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();
    }
}

/// 기존 Daily Markdown 파일에 `## 개발 진척사항` 섹션을 덮어쓰거나 추가합니다.
///
/// 기존 섹션이 있으면 해당 섹션만 교체하고, 없으면 파일 끝에 추가합니다.
/// 섹션의 범위: `## 개발 진척사항`부터 다음 `## ` 헤더(또는 파일 끝)까지.
fn append_summary_to_markdown(date: chrono::NaiveDate, summary: &str) {
    let vault_path = match output::load_vault_path() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Vault 경로 로드 실패: {e}");
            return;
        }
    };

    let file_path = vault_path.join("Daily").join(format!("{date}.md"));
    if !file_path.exists() {
        eprintln!("Daily Markdown 파일이 없습니다: {}", file_path.display());
        return;
    }

    let existing = match std::fs::read_to_string(&file_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("파일 읽기 실패: {e}");
            return;
        }
    };

    let section_header = "## 개발 진척사항";
    let new_section = format!("{section_header}\n\n{summary}\n");

    // 기존 섹션이 있으면 교체, 없으면 끝에 추가합니다.
    // .find()는 패턴의 시작 바이트 위치를 반환합니다 (Rust Book Ch.8 참조).
    let updated = if let Some(start) = existing.find(section_header) {
        // 섹션 끝: 다음 "## " 헤더의 시작 위치 또는 파일 끝.
        // start 이후부터 탐색하되, 헤더 자체는 건너뜁니다.
        let after_header = start + section_header.len();
        let end = existing[after_header..]
            .find("\n## ")
            .map(|pos| after_header + pos + 1) // +1: 개행 문자 다음에 ## 이 오도록
            .unwrap_or(existing.len());

        format!("{}{}\n{}", &existing[..start], new_section, &existing[end..])
    } else {
        format!("{}\n{}\n", existing.trim_end(), new_section)
    };

    match std::fs::write(&file_path, updated) {
        Ok(()) => println!("Markdown 저장 완료: {}", file_path.display()),
        Err(e) => eprintln!("파일 저장 실패: {e}"),
    }
}

/// Claude Code 로그를 수집합니다. 디렉토리가 없으면 빈 Vec을 반환합니다.
/// 기존 run_today()는 디렉토리 부재 시 에러로 중단했지만,
/// Codex 전용 사용자도 지원하기 위해 빈 결과로 진행합니다.
fn collect_claude_entries(today: chrono::NaiveDate) -> Vec<parser::claude::LogEntry> {
    match parser::discover_log_dir() {
        Ok(dir) => println!("Scanning Claude Code: {}", dir.display()),
        Err(_) => return Vec::new(),
    }

    let mut all_entries = Vec::new();
    if let Ok(project_dirs) = parser::list_project_dirs() {
        for project_dir in project_dirs {
            if let Ok(session_files) = parser::list_session_files(&project_dir) {
                for session_file in session_files {
                    if let Ok(entries) = parser::parse_jsonl_file(&session_file) {
                        let today_entries = parser::filter_entries_by_date(entries, today);
                        all_entries.extend(today_entries);
                    }
                }
            }
        }
    }
    all_entries
}

/// Codex 세션 로그를 수집합니다. 디렉토리가 없으면 빈 Vec을 반환합니다.
fn collect_codex_sessions(
    today: chrono::NaiveDate,
) -> Vec<(parser::codex::CodexSessionSummary, Vec<parser::codex::CodexEntry>)> {
    let sessions_dir = match parser::codex::discover_codex_sessions_dir() {
        Ok(dir) => {
            println!("Scanning Codex: {}", dir.display());
            dir
        }
        Err(_) => return Vec::new(),
    };

    // 로컬 타임존과 UTC의 날짜 차이를 고려하여 전날 디렉토리도 함께 스캔합니다.
    let session_files =
        match parser::codex::list_session_files_for_local_date(&sessions_dir, today) {
            Ok(files) => files,
            Err(_) => return Vec::new(),
        };

    let mut sessions = Vec::new();
    for file in session_files {
        if let Ok(entries) = parser::codex::parse_codex_jsonl_file(&file) {
            // 세션의 첫 엔트리 날짜가 로컬 기준 "오늘"인지 확인합니다.
            let session_date = entries.iter().find_map(parser::codex::entry_local_date);
            if session_date != Some(today) {
                continue;
            }
            let summary = parser::codex::summarize_codex_entries(&entries);
            // 대화 내용이 있는 세션만 포함
            if summary.user_count > 0 || summary.assistant_count > 0 {
                sessions.push((summary, entries));
            }
        }
    }
    sessions
}

/// Claude 엔트리들에서 가장 이른 로컬 타임스탬프를 찾습니다.
fn claude_earliest_time(entries: &[parser::claude::LogEntry]) -> Option<chrono::DateTime<chrono::Local>> {
    entries
        .iter()
        .filter_map(parser::claude::entry_timestamp)
        .min()
        .map(|ts| ts.with_timezone(&chrono::Local))
}

/// Codex 세션들에서 가장 이른 로컬 타임스탬프를 찾습니다.
fn codex_earliest_time(
    sessions: &[(parser::codex::CodexSessionSummary, Vec<parser::codex::CodexEntry>)],
) -> Option<chrono::DateTime<chrono::Local>> {
    sessions
        .iter()
        .flat_map(|(_, entries)| entries.iter())
        .filter_map(|e| match e {
            parser::codex::CodexEntry::SessionMeta { timestamp, .. }
            | parser::codex::CodexEntry::UserMessage { timestamp, .. }
            | parser::codex::CodexEntry::AssistantMessage { timestamp, .. }
            | parser::codex::CodexEntry::FunctionCall { timestamp, .. } => Some(*timestamp),
            parser::codex::CodexEntry::Other => None,
        })
        .min()
        .map(|ts| ts.with_timezone(&chrono::Local))
}

/// Claude 세션 요약들의 총 토큰 수를 계산합니다. (total_in, total_out)
fn claude_total_tokens(summaries: &[parser::claude::SessionSummary]) -> (u64, u64) {
    summaries.iter().fold((0, 0), |(acc_in, acc_out), s| {
        let total_in = s.total_input_tokens
            + s.total_cache_creation_tokens
            + s.total_cache_read_tokens;
        (acc_in + total_in, acc_out + s.total_output_tokens)
    })
}

/// 시간 범위를 "HH:MM ~ HH:MM" 형식으로 포매팅합니다.
fn format_time_range(
    earliest: Option<chrono::DateTime<chrono::Local>>,
    now: chrono::DateTime<chrono::Local>,
) -> String {
    match earliest {
        Some(start) => format!("{} ~ {}", start.format("%H:%M"), now.format("%H:%M")),
        None => format!("? ~ {}", now.format("%H:%M")),
    }
}

/// 숫자를 천 단위 콤마로 포매팅합니다.
fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

/// 여러 소스의 분석 결과를 결합하여 Markdown으로 저장합니다.
fn save_combined_analysis(
    sources: &[(&str, &analyzer::AnalysisResult)],
    date: chrono::NaiveDate,
) {
    let vault_path = match output::load_vault_path() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Vault 경로 로드 실패: {e}");
            return;
        }
    };

    let markdown = output::render_combined_markdown(sources, date);

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
