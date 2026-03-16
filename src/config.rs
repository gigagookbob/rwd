// config 모듈은 설정 파일(~/.config/rwd/config.toml)의 읽기/쓰기를 담당합니다.
// 기존 .env 방식을 대체하여, rwd init / rwd config 커맨드로 설정을 관리합니다.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// M5에서 thiserror로 전환 예정이지만, 기존 모듈과 동일한 에러 타입 패턴을 사용합니다.
pub type ConfigError = Box<dyn std::error::Error>;

/// 설정 파일의 최상위 구조체.
/// Serialize/Deserialize derive로 TOML ↔ Rust 구조체 자동 변환 (serde 패턴).
/// Serialize는 Rust → TOML 변환, Deserialize는 TOML → Rust 변환을 담당합니다.
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub llm: LlmConfig,
    pub output: OutputConfig,
}

/// LLM 프로바이더 관련 설정.
#[derive(Debug, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: String,
    pub api_key: String,
}

/// Markdown 출력 관련 설정.
/// path는 vault root 경로를 저장합니다 — Daily/ 하위 디렉토리는 save_to_vault()가 붙입니다.
#[derive(Debug, Serialize, Deserialize)]
pub struct OutputConfig {
    pub path: String,
}

/// 설정 파일 경로를 반환합니다: ~/.config/rwd/config.toml
/// dirs::home_dir()로 홈 디렉토리를 찾고, Unix 관례에 맞게 ~/.config를 사용합니다.
pub fn config_path() -> Result<PathBuf, ConfigError> {
    let home = dirs::home_dir()
        .ok_or("홈 디렉토리를 찾을 수 없습니다")?;
    Ok(home.join(".config").join("rwd").join("config.toml"))
}

/// 설정을 TOML 파일로 저장합니다.
/// toml::to_string_pretty()는 Config 구조체를 읽기 좋은 TOML 문자열로 변환합니다.
/// create_dir_all()로 부모 디렉토리가 없으면 자동 생성합니다.
pub fn save_config(config: &Config, path: &std::path::Path) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let toml_str = toml::to_string_pretty(config)?;
    std::fs::write(path, toml_str)?;

    // API 키가 포함된 파일이므로 소유자만 읽기/쓰기 가능하도록 권한 설정합니다.
    // Unix 권한 0o600 = owner read+write only (보안 관례).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
}

/// TOML 파일에서 설정을 읽습니다.
/// toml::from_str()는 TOML 문자열을 Config 구조체로 역직렬화합니다.
pub fn load_config(path: &std::path::Path) -> Result<Config, ConfigError> {
    let content = std::fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}

/// 주어진 디렉토리 하위에서 .obsidian 폴더가 있는 디렉토리를 찾습니다.
/// .obsidian 폴더는 Obsidian이 vault로 인식하는 마커입니다.
/// read_dir()로 1단계 깊이만 탐색합니다 — 깊은 중첩은 불필요합니다.
pub fn detect_vault_in_dir(search_dir: &std::path::Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(search_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path.join(".obsidian").is_dir() {
            return Some(path);
        }
    }
    None
}

/// Obsidian vault를 자동 감지합니다.
/// ~/Documents/Obsidian/ 하위에서 .obsidian 마커를 탐색합니다.
/// 찾지 못하면 None 반환 — 호출부에서 기본 경로를 사용합니다.
pub fn detect_obsidian_vault() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let obsidian_dir = home.join("Documents").join("Obsidian");
    detect_vault_in_dir(&obsidian_dir)
}

/// 기본 출력 경로(vault root)를 결정합니다.
/// 1. Obsidian vault 자동 감지 → {vault} (vault root 반환)
/// 2. 감지 실패 → ~/.rwd/output (기본 경로)
///
/// 주의: Daily/ 하위 디렉토리는 save_to_vault()가 자동으로 붙입니다.
pub fn default_output_path() -> PathBuf {
    if let Some(vault) = detect_obsidian_vault() {
        return vault;
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".rwd").join("output")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_path_rwd_디렉토리_포함() {
        let path = config_path().expect("경로 생성 성공");
        assert!(path.ends_with("rwd/config.toml"));
    }

    #[test]
    fn test_save_and_load_config_왕복_확인() {
        let temp_dir = std::env::temp_dir().join("rwd_test_config");
        let _ = std::fs::remove_dir_all(&temp_dir); // 이전 테스트 잔여물 정리
        std::fs::create_dir_all(&temp_dir).expect("디렉토리 생성");
        let path = temp_dir.join("config.toml");

        let config = Config {
            llm: LlmConfig {
                provider: "anthropic".to_string(),
                api_key: "sk-test-key".to_string(),
            },
            output: OutputConfig {
                path: "/tmp/vault".to_string(),
            },
        };

        save_config(&config, &path).expect("저장 성공");
        let loaded = load_config(&path).expect("로드 성공");

        assert_eq!(loaded.llm.provider, "anthropic");
        assert_eq!(loaded.llm.api_key, "sk-test-key");
        assert_eq!(loaded.output.path, "/tmp/vault");

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_detect_obsidian_vault_obsidian폴더_있으면_경로반환() {
        let temp_dir = std::env::temp_dir().join("rwd_test_vault_detect");
        let _ = std::fs::remove_dir_all(&temp_dir);
        let vault_dir = temp_dir.join("TestVault");
        let obsidian_marker = vault_dir.join(".obsidian");
        std::fs::create_dir_all(&obsidian_marker).expect("디렉토리 생성");

        let result = detect_vault_in_dir(&temp_dir);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), vault_dir);

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_detect_obsidian_vault_없으면_None() {
        let temp_dir = std::env::temp_dir().join("rwd_test_no_vault");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).expect("디렉토리 생성");

        let result = detect_vault_in_dir(&temp_dir);
        assert!(result.is_none());

        std::fs::remove_dir_all(&temp_dir).ok();
    }
}
