// mod 키워드로 모듈을 선언합니다.
// Rust는 파일 하나가 모듈 하나에 대응됩니다 — cli.rs 파일이 cli 모듈이 됩니다 (Rust Book Ch.7 참조).
mod cli;

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
            println!("rwd today — 오늘의 개발 인사이트를 분석합니다. (M2에서 구현 예정)");
        }
    }
}
