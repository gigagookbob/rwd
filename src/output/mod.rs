// output 모듈은 분석 결과를 Markdown 파일로 변환하여 Obsidian vault에 저장하는 역할을 합니다.
// parser, analyzer 모듈과 같은 디렉토리 구조를 사용합니다 (Rust Book Ch.7 참조).

pub mod markdown;

// M5에서 thiserror로 전용 에러 타입을 만들 예정입니다.
pub type OutputError = Box<dyn std::error::Error>;

pub use markdown::render_markdown;

use std::path::{Path, PathBuf};

use chrono::NaiveDate;

/// Obsidian vault 경로를 환경 변수에서 읽습니다.
///
/// analyzer/mod.rs의 load_api_key()와 같은 패턴입니다:
/// dotenvy로 .env 파일을 로드한 뒤, std::env::var()로 환경 변수를 읽습니다.
///
/// PathBuf는 소유권을 가진 경로 타입입니다 — String이 &str의 소유 버전인 것처럼,
/// PathBuf는 &Path의 소유 버전입니다 (Rust Book Ch.12 참조).
/// PathBuf::from()은 문자열을 OS 경로로 변환합니다.
pub fn load_vault_path() -> Result<PathBuf, OutputError> {
    // .env 파일이 있으면 로드, 없으면 무시 — 환경 변수가 직접 설정된 경우를 지원합니다.
    dotenvy::dotenv().ok();

    let path_str = std::env::var("RWD_VAULT_PATH").map_err(|_| {
        "RWD_VAULT_PATH가 설정되지 않았습니다. \
         .env 파일에 추가하거나 환경 변수를 설정해 주세요. \
         예: echo 'RWD_VAULT_PATH=/path/to/obsidian/vault' >> .env"
    })?;

    let path = PathBuf::from(&path_str);

    // .exists()와 .is_dir()는 파일 시스템을 조회하여 경로 상태를 확인합니다.
    if !path.exists() {
        return Err(format!("Vault 경로가 존재하지 않습니다: {}", path.display()).into());
    }
    if !path.is_dir() {
        return Err(format!("Vault 경로가 디렉토리가 아닙니다: {}", path.display()).into());
    }

    Ok(path)
}

/// 날짜 기반 파일명으로 Markdown 내용을 vault에 저장합니다.
///
/// Path::join()은 경로와 파일명을 결합하여 새 PathBuf를 반환합니다 (Rust Book Ch.12 참조).
/// std::fs::write()는 파일 내용 전체를 한 번에 기록합니다 — 파일이 없으면 생성하고, 있으면 덮어씁니다.
pub fn save_to_vault(
    vault_path: &Path,
    date: NaiveDate,
    content: &str,
) -> Result<PathBuf, OutputError> {
    // 파일명: "2026-03-11.md" — NaiveDate의 Display 트레이트가 "YYYY-MM-DD" 형식을 제공합니다.
    let filename = format!("{date}.md");
    // .join()은 OS에 맞는 경로 구분자로 결합합니다 (Windows: \, Unix: /).
    let file_path = vault_path.join(&filename);

    std::fs::write(&file_path, content)?;

    Ok(file_path)
}

/// 파일이 이미 존재할 때 사용자에게 덮어쓸지 확인합니다.
///
/// std::io::stdin().read_line()은 사용자의 키보드 입력을 한 줄 읽습니다 (Rust Book Ch.2 참조).
/// &mut input은 가변 참조로, read_line이 input 변수에 입력 내용을 기록할 수 있게 합니다.
/// 반환값 Result<usize, io::Error>에서 usize는 읽은 바이트 수입니다.
pub fn confirm_overwrite(path: &Path) -> Result<bool, OutputError> {
    // 파일이 없으면 확인 불필요 — 바로 저장 진행
    if !path.exists() {
        return Ok(true);
    }

    // eprintln!은 stderr에 출력합니다 — 프롬프트 메시지는 데이터 출력(stdout)과 분리하는 것이 CLI 관례입니다.
    eprint!(
        "파일이 이미 존재합니다: {}\n덮어쓰시겠습니까? (y/N): ",
        path.display()
    );

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    // .trim()은 양끝 공백과 개행 문자를 제거합니다.
    // .to_lowercase()는 대소문자를 통일하여 "Y", "y", "YES" 등을 모두 처리합니다.
    let answer = input.trim().to_lowercase();
    Ok(answer == "y" || answer == "yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_to_vault_파일생성_확인() {
        // std::env::temp_dir()는 OS의 임시 디렉토리 경로를 반환합니다.
        // 테스트 격리를 위해 고유한 하위 디렉토리를 만듭니다.
        let temp_dir = std::env::temp_dir().join("rwd_test_output");
        std::fs::create_dir_all(&temp_dir).expect("임시 디렉토리 생성 실패");

        let date = NaiveDate::from_ymd_opt(2026, 3, 11).expect("유효한 날짜");
        let content = "# 테스트 Markdown";

        let result = save_to_vault(&temp_dir, date, content);
        assert!(result.is_ok());

        let saved_path = result.expect("저장 성공");
        assert!(saved_path.exists());
        assert_eq!(
            std::fs::read_to_string(&saved_path).expect("파일 읽기"),
            content
        );

        // 테스트 후 정리 — 생성한 파일과 디렉토리를 삭제합니다.
        std::fs::remove_file(&saved_path).ok();
        std::fs::remove_dir(&temp_dir).ok();
    }
}
