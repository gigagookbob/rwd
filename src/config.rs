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
    /// 민감 정보 마스킹 설정. 섹션이 없으면 None → 기본 활성.
    pub redactor: Option<RedactorConfig>,
}

impl Config {
    /// redactor 활성 여부를 반환합니다.
    /// redactor 섹션이 없으면(None) 기본값 true (활성)입니다.
    /// is_none_or()는 Option이 None이면 true를, Some이면 클로저 결과를 반환합니다 (Rust 1.82+).
    pub fn is_redactor_enabled(&self) -> bool {
        self.redactor.as_ref().is_none_or(|r| r.enabled)
    }
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

/// Redactor(민감 정보 마스킹) 설정.
#[derive(Debug, Serialize, Deserialize)]
pub struct RedactorConfig {
    pub enabled: bool,
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

/// 설정 파일을 읽습니다. 파일이 없으면 None을 반환합니다.
/// 호출부에서 None일 때 기존 .env fallback을 사용합니다.
pub fn load_config_if_exists() -> Option<Config> {
    let path = config_path().ok()?;
    if path.exists() {
        load_config(&path).ok()
    } else {
        None
    }
}

/// rwd init 실행 — API 키를 입력받고, 출력 경로를 자동 감지하여 설정 파일에 저장합니다.
///
/// eprint!는 stderr로 프롬프트를 출력합니다 — stdout은 데이터 출력용으로 분리합니다.
/// stdin().read_line()은 사용자 입력을 한 줄 읽습니다 (Rust Book Ch.2 참조).
pub fn run_init() -> Result<(), ConfigError> {
    let config_file = config_path()?;

    // 프로바이더 선택
    eprint!("LLM 프로바이더를 선택하세요 (anthropic/openai) [anthropic]: ");
    let mut provider_input = String::new();
    std::io::stdin().read_line(&mut provider_input)?;
    let provider = provider_input.trim();
    let provider = if provider.is_empty() { "anthropic" } else { provider };

    // 프로바이더 검증
    if !["anthropic", "openai"].contains(&provider) {
        return Err(format!("지원하지 않는 프로바이더: {provider}").into());
    }

    // API 키 입력 — rpassword로 마스킹 (터미널에 입력이 보이지 않음)
    let key_prompt = match provider {
        "anthropic" => "Anthropic API 키를 입력하세요: ",
        "openai" => "OpenAI API 키를 입력하세요: ",
        _ => unreachable!(),
    };
    let api_key = rpassword::prompt_password(key_prompt)
        .map_err(|e| format!("API 키 입력 실패: {e}"))?;
    let api_key = api_key.trim().to_string();

    if api_key.is_empty() {
        return Err("API 키가 비어있습니다.".into());
    }

    // 마스킹된 키 표시 (앞 8자만 보여주고 나머지는 ***)
    let masked = if api_key.len() > 8 {
        format!("{}***", &api_key[..8])
    } else {
        "***".to_string()
    };
    eprintln!("API 키 설정됨: {masked}");

    // 출력 경로 — vault 감지 결과를 기본값으로 제안하고, 사용자에게 확인받습니다.
    let default_path = detect_obsidian_vault()
        .unwrap_or_else(default_output_path);
    eprint!("마크다운 저장 경로 [{}]: ", default_path.display());
    let mut path_input = String::new();
    std::io::stdin().read_line(&mut path_input)?;
    let path_input = path_input.trim();
    let output_path = if path_input.is_empty() {
        default_path
    } else {
        PathBuf::from(path_input)
    };
    eprintln!("출력 경로: {}", output_path.display());

    let config = Config {
        llm: LlmConfig {
            provider: provider.to_string(),
            api_key,
        },
        output: OutputConfig {
            path: output_path.to_string_lossy().to_string(),
        },
        redactor: None,
    };

    save_config(&config, &config_file)?;
    eprintln!("설정 저장 완료: {}", config_file.display());
    Ok(())
}

/// rwd config <key> <value> — 개별 설정 값을 변경합니다.
/// 기존 설정 파일을 읽고, 해당 키만 수정한 뒤 다시 저장합니다.
pub fn run_config(key: &str, value: &str) -> Result<(), ConfigError> {
    let config_file = config_path()?;

    if !config_file.exists() {
        return Err("설정 파일이 없습니다. 먼저 `rwd init`을 실행해 주세요.".into());
    }

    let mut config = load_config(&config_file)?;

    match key {
        "output-path" => {
            config.output.path = value.to_string();
            eprintln!("출력 경로 변경: {value}");
        }
        "provider" => {
            if !["anthropic", "openai"].contains(&value) {
                return Err(format!(
                    "지원하지 않는 프로바이더: '{value}'. 사용 가능: anthropic, openai"
                ).into());
            }
            config.llm.provider = value.to_string();
            eprintln!("LLM 프로바이더 변경: {value}");
        }
        "api-key" => {
            config.llm.api_key = value.to_string();
            let masked = if value.len() > 8 {
                format!("{}***", &value[..8])
            } else {
                "***".to_string()
            };
            eprintln!("API 키 변경됨: {masked}");
        }
        _ => {
            return Err(format!(
                "알 수 없는 설정 키: '{key}'. 사용 가능: output-path, provider, api-key"
            ).into());
        }
    }

    save_config(&config, &config_file)?;
    Ok(())
}

/// Obsidian 앱의 설정 파일(obsidian.json)에서 vault 경로를 읽습니다.
/// macOS: ~/Library/Application Support/obsidian/obsidian.json
///
/// obsidian.json 형식: {"vaults":{"id":{"path":"/path/to/vault","ts":...,"open":true}}}
/// serde_json::Value로 동적 파싱합니다 — 구조가 바뀌어도 유연하게 대응합니다.
fn detect_vault_from_obsidian_json() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let json_path = home
        .join("Library")
        .join("Application Support")
        .join("obsidian")
        .join("obsidian.json");

    let content = std::fs::read_to_string(&json_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    // vaults 객체에서 첫 번째 vault의 path를 추출합니다.
    // as_object()는 JSON 값을 Map으로 변환 — None이면 해당 키가 없거나 객체가 아닌 것입니다.
    let vaults = json.get("vaults")?.as_object()?;
    for (_id, vault_info) in vaults {
        if let Some(path_str) = vault_info.get("path").and_then(|v| v.as_str()) {
            let path = PathBuf::from(path_str);
            if path.exists() {
                return Some(path);
            }
        }
    }
    None
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
/// 1순위: obsidian.json에서 vault 경로 읽기 (가장 정확)
/// 2순위: ~/Documents/Obsidian/ 하위에서 .obsidian 마커 탐색 (fallback)
/// 찾지 못하면 None 반환 — 호출부에서 기본 경로를 사용합니다.
pub fn detect_obsidian_vault() -> Option<PathBuf> {

    // 1순위: Obsidian 앱 설정에서 직접 읽기
    if let Some(vault) = detect_vault_from_obsidian_json() {
        return Some(vault);
    }

    // 2순위: .obsidian 마커 기반 탐색
    let home = dirs::home_dir()?;
    let obsidian_dir = home.join("Documents").join("Obsidian");
    detect_vault_in_dir(&obsidian_dir)
}

/// Obsidian vault 미감지 시 사용하는 기본 출력 경로.
/// ~/.rwd/output을 반환합니다.
pub fn default_output_path() -> PathBuf {
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
            redactor: None,
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

    #[test]
    fn test_config_redactor_없으면_none() {
        let toml_str = r#"
[llm]
provider = "anthropic"
api_key = "sk-test"

[output]
path = "/tmp/vault"
"#;
        let config: Config = toml::from_str(toml_str).expect("파싱 성공");
        assert!(config.redactor.is_none());
    }

    #[test]
    fn test_config_redactor_있으면_파싱() {
        let toml_str = r#"
[llm]
provider = "anthropic"
api_key = "sk-test"

[output]
path = "/tmp/vault"

[redactor]
enabled = false
"#;
        let config: Config = toml::from_str(toml_str).expect("파싱 성공");
        assert_eq!(config.redactor.unwrap().enabled, false);
    }
}
