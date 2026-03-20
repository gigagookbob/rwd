// config 모듈은 설정 파일(~/.config/rwd/config.toml)의 읽기/쓰기를 담당합니다.
// 기존 .env 방식을 대체하여, rwd init / rwd config 커맨드로 설정을 관리합니다.

use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input, Select};
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
        .ok_or(crate::messages::error::HOME_DIR_NOT_FOUND)?;
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
    eprint!("{}", crate::messages::init::SELECT_PROVIDER);
    let mut provider_input = String::new();
    std::io::stdin().read_line(&mut provider_input)?;
    let provider = provider_input.trim();
    let provider = if provider.is_empty() { "anthropic" } else { provider };

    // 프로바이더 검증
    if !["anthropic", "openai"].contains(&provider) {
        return Err(crate::messages::init::unsupported_provider(provider).into());
    }

    // API 키 입력 — rpassword로 마스킹 (터미널에 입력이 보이지 않음)
    let key_prompt = match provider {
        "anthropic" => crate::messages::init::ENTER_API_KEY_ANTHROPIC,
        "openai" => crate::messages::init::ENTER_API_KEY_OPENAI,
        _ => unreachable!(),
    };
    let api_key = rpassword::prompt_password(key_prompt)
        .map_err(|e| crate::messages::init::api_key_input_failed(&e))?;
    let api_key = api_key.trim().to_string();

    if api_key.is_empty() {
        return Err(crate::messages::init::API_KEY_EMPTY.into());
    }

    // 마스킹된 키 표시 (앞 8자만 보여주고 나머지는 ***)
    let masked = if api_key.len() > 8 {
        format!("{}***", &api_key[..8])
    } else {
        "***".to_string()
    };
    eprintln!("{}", crate::messages::init::api_key_set(&masked));

    // 출력 경로 — vault 감지 결과를 기본값으로 제안하고, 사용자에게 확인받습니다.
    let default_path = detect_obsidian_vault()
        .unwrap_or_else(default_output_path);
    eprint!("{}", crate::messages::init::output_path_prompt(&default_path.display()));
    let mut path_input = String::new();
    std::io::stdin().read_line(&mut path_input)?;
    let path_input = path_input.trim();
    let output_path = if path_input.is_empty() {
        default_path
    } else {
        PathBuf::from(path_input)
    };
    eprintln!("{}", crate::messages::init::output_path_set(&output_path.display()));

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
    eprintln!("{}", crate::messages::init::config_saved(&config_file.display()));
    Ok(())
}

/// rwd config <key> <value> — 개별 설정 값을 변경합니다.
/// 기존 설정 파일을 읽고, 해당 키만 수정한 뒤 다시 저장합니다.
pub fn run_config(key: &str, value: &str) -> Result<(), ConfigError> {
    let config_file = config_path()?;

    if !config_file.exists() {
        return Err(crate::messages::config::NO_CONFIG.into());
    }

    let mut config = load_config(&config_file)?;

    match key {
        "output-path" => {
            config.output.path = value.to_string();
            eprintln!("{}", crate::messages::config::output_path_changed(value));
        }
        "provider" => {
            if !["anthropic", "openai"].contains(&value) {
                return Err(crate::messages::config::unsupported_provider(value).into());
            }
            config.llm.provider = value.to_string();
            eprintln!("{}", crate::messages::config::provider_changed(value));
        }
        "api-key" => {
            config.llm.api_key = value.to_string();
            eprintln!("{}", crate::messages::config::api_key_changed(&mask_api_key(value)));
        }
        _ => {
            return Err(crate::messages::config::unknown_key(key).into());
        }
    }

    save_config(&config, &config_file)?;
    Ok(())
}

/// Esc를 지원하는 패스워드 입력.
/// console::Term으로 키를 한 글자씩 읽어, 입력은 *로 표시합니다.
/// Esc → None (취소), Enter → Some(입력값).
/// console::Key는 터미널 키 이벤트를 Rust enum으로 표현합니다.
fn read_password_with_esc(prompt: &str) -> Result<Option<String>, ConfigError> {
    use console::{Key, Term};
    let term = Term::stderr();
    eprint!("{prompt}");
    let mut input = String::new();
    loop {
        match term.read_key()? {
            Key::Escape => {
                term.write_line("")?;
                return Ok(None);
            }
            Key::Enter => {
                term.write_line("")?;
                return Ok(Some(input));
            }
            Key::Backspace => {
                if !input.is_empty() {
                    input.pop();
                    term.clear_chars(1)?;
                }
            }
            Key::Char(c) => {
                input.push(c);
                eprint!("*");
            }
            _ => {}
        }
    }
}

/// 실제 API를 호출하여 provider/키 조합이 유효한지 검증합니다.
/// 각 provider의 models 엔드포인트에 GET 요청을 보내 인증을 확인합니다.
/// 네트워크 오류 시에도 프로그램이 중단되지 않도록 에러를 무시합니다.
///
/// reqwest::Client::builder().timeout()으로 최대 대기 시간을 5초로 제한합니다.
async fn verify_api_key(provider: &str, api_key: &str) {
    let dim = "\x1b[2m";
    let green = "\x1b[32m";
    let yellow = "\x1b[33m";
    let reset = "\x1b[0m";

    eprint!("{dim}{}{reset}", crate::messages::verify::VERIFYING_KEY);

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => {
            eprintln!("\r{dim}{}{reset}", crate::messages::verify::VERIFY_SKIPPED_CLIENT);
            return;
        }
    };

    let result = match provider {
        "anthropic" => {
            client
                .get("https://api.anthropic.com/v1/models")
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01")
                .send()
                .await
        }
        "openai" => {
            client
                .get("https://api.openai.com/v1/models")
                .header("Authorization", format!("Bearer {api_key}"))
                .send()
                .await
        }
        _ => return,
    };

    // \r로 "검증 중..." 줄을 덮어씁니다.
    match result {
        Ok(resp) if resp.status().is_success() => {
            eprintln!("\r{green}{}{reset}                    ", crate::messages::verify::KEY_VERIFIED);
        }
        Ok(resp) => {
            let status = resp.status().as_u16();
            eprintln!("\r{yellow}{}{reset}", crate::messages::verify::key_invalid(status));
        }
        Err(_) => {
            eprintln!("\r{dim}{}{reset}       ", crate::messages::verify::VERIFY_SKIPPED_NETWORK);
        }
    }
}

/// API 키를 마스킹합니다.
/// 8자 초과: 앞 8자 + ***, 4자 초과: 앞 4자 + ***, 그 외: ***
fn mask_api_key(key: &str) -> String {
    if key.len() > 8 {
        format!("{}***", &key[..8])
    } else if key.len() > 4 {
        format!("{}***", &key[..4])
    } else {
        "***".to_string()
    }
}

/// rwd config (인자 없이) — 대화형 메뉴로 설정을 변경합니다.
///
/// dialoguer 크레이트의 Select, Input 위젯을 사용합니다.
/// Select: 화살표 키로 항목을 선택하는 위젯 — 키보드만으로 조작합니다.
/// Input: 텍스트를 직접 입력받는 위젯 — .default()로 현재값을 기본값으로 제안합니다.
/// API 키 입력은 console::Term으로 직접 구현하여 Esc 취소를 지원합니다.
pub async fn run_config_interactive() -> Result<(), ConfigError> {
    let config_file = config_path()?;

    if !config_file.exists() {
        return Err(crate::messages::config::NO_CONFIG.into());
    }

    let mut config = load_config(&config_file)?;
    let theme = ColorfulTheme::default();

    // ANSI 색상 코드 — 메뉴 항목에 키 이름과 현재값을 구분합니다.
    let cyan = "\x1b[36m";
    let dim = "\x1b[2m";
    let reset = "\x1b[0m";

    eprintln!("{dim}{}{reset}", crate::messages::config::NAV_HINT);

    // loop로 메뉴를 반복합니다 — Esc나 "나가기"를 선택하면 break로 탈출합니다.
    loop {
        // 매 반복마다 현재 설정값으로 메뉴를 다시 구성합니다.
        let redactor_status = if config.is_redactor_enabled() { "on" } else { "off" };
        let items = vec![
            format!("{cyan}provider{reset}      {dim}[{}]{reset}", config.llm.provider),
            format!("{cyan}api-key{reset}       {dim}[{}]{reset}", mask_api_key(&config.llm.api_key)),
            format!("{cyan}output-path{reset}   {dim}[{}]{reset}", config.output.path),
            format!("{cyan}redactor{reset}      {dim}[{}]{reset}", redactor_status),
            format!("{dim}{}{reset}", crate::messages::config::EXIT),
        ];

        // interact_opt()는 Esc를 누르면 Ok(None)을 반환합니다.
        // interact()와 달리 취소 동작을 지원하는 메서드입니다.
        let selection = Select::with_theme(&theme)
            .with_prompt(crate::messages::config::SELECT_SETTING)
            .items(&items)
            .default(0)
            .interact_opt()?;

        // None이면 Esc — 루프 탈출
        let Some(selection) = selection else { break };

        let green = "\x1b[32m";

        match selection {
            // provider — Select로 선택지 제공
            0 => {
                let old = config.llm.provider.clone();
                let providers = ["anthropic", "openai"];
                let current_idx = providers
                    .iter()
                    .position(|&p| p == old)
                    .unwrap_or(0);

                // 하위 Select도 interact_opt()로 Esc 뒤로가기를 지원합니다.
                let Some(chosen) = Select::with_theme(&theme)
                    .with_prompt(crate::messages::config::LLM_PROVIDER)
                    .items(&providers)
                    .default(current_idx)
                    .interact_opt()?
                else {
                    continue;
                };

                let new_provider = providers[chosen];
                if new_provider == old {
                    eprintln!("{dim}{}{reset}\n", crate::messages::config::NO_CHANGE);
                } else {
                    config.llm.provider = new_provider.to_string();
                    save_config(&config, &config_file)?;
                    eprintln!("{green}{}{reset}", crate::messages::config::changed(&old, new_provider));
                    verify_api_key(&config.llm.provider, &config.llm.api_key).await;
                    eprintln!();
                }
            }
            // api-key — 커스텀 패스워드 입력 (Esc = 취소)
            1 => {
                let Some(new_key) = read_password_with_esc(crate::messages::config::NEW_API_KEY)? else {
                    continue;
                };
                let new_key = new_key.trim().to_string();

                if new_key.is_empty() {
                    continue;
                }
                // Confirm 위젯: y/n 또는 yes/no로 확인을 받습니다.
                // .default(false)로 기본값을 "no"로 설정하여 실수 방지합니다.
                let confirmed = Confirm::with_theme(&theme)
                    .with_prompt(crate::messages::config::CONFIRM_API_KEY)
                    .default(false)
                    .interact()?;
                if !confirmed {
                    continue;
                }
                let old_masked = mask_api_key(&config.llm.api_key);
                config.llm.api_key = new_key;
                save_config(&config, &config_file)?;
                eprintln!(
                    "{green}{}{reset}",
                    crate::messages::config::changed(&old_masked, &mask_api_key(&config.llm.api_key))
                );
                verify_api_key(&config.llm.provider, &config.llm.api_key).await;
                eprintln!();
            }
            // output-path — Input으로 텍스트 입력 (현재값을 기본값으로)
            2 => {
                let old = config.output.path.clone();
                let new_path: String = Input::with_theme(&theme)
                    .with_prompt(crate::messages::config::OUTPUT_PATH)
                    .default(old.clone())
                    .interact_text()?;

                if new_path == old {
                    eprintln!("{dim}{}{reset}\n", crate::messages::config::NO_CHANGE);
                } else {
                    config.output.path = new_path.clone();
                    save_config(&config, &config_file)?;
                    eprintln!("{green}{}{reset}\n", crate::messages::config::changed(&old, &new_path));
                }
            }
            // redactor — Select로 on/off 선택
            3 => {
                let old_enabled = config.is_redactor_enabled();
                let options = ["on", "off"];
                let current_idx = if old_enabled { 0 } else { 1 };

                let Some(chosen) = Select::with_theme(&theme)
                    .with_prompt(crate::messages::config::REDACTOR)
                    .items(&options)
                    .default(current_idx)
                    .interact_opt()?
                else {
                    continue;
                };

                let enabled = chosen == 0;
                if enabled == old_enabled {
                    eprintln!("{dim}{}{reset}\n", crate::messages::config::NO_CHANGE);
                } else {
                    config.redactor = Some(RedactorConfig { enabled });
                    save_config(&config, &config_file)?;
                    let old_label = if old_enabled { "on" } else { "off" };
                    let new_label = options[chosen];
                    eprintln!("{green}{}{reset}\n", crate::messages::config::changed(old_label, new_label));
                }
            }
            // 나가기 — Select가 출력한 "✔ ... 나가기" 줄을 지웁니다.
            // console::Term::clear_last_lines()는 지정한 줄 수만큼 터미널 출력을 지웁니다.
            4 => {
                console::Term::stderr().clear_last_lines(1)?;
                break;
            }
            _ => unreachable!(),
        }
    }

    eprintln!("\x1b[32m{}\x1b[0m", crate::messages::config::config_saved(&config_file.display()));
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
