// Handles config file (~/.config/rwd/config.toml) read/write.

use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Select};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub type ConfigError = Box<dyn std::error::Error>;

/// Supported languages for LLM output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    En,
    Ko,
}

impl std::fmt::Display for Lang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Lang::En => write!(f, "en"),
            Lang::Ko => write!(f, "ko"),
        }
    }
}

/// Top-level config file structure.
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub llm: LlmConfig,
    pub output: OutputConfig,
    /// Sensitive data masking config. None means default-enabled.
    pub redactor: Option<RedactorConfig>,
    /// LLM output language. None triggers migration prompt on first use.
    pub lang: Option<Lang>,
}

impl Config {
    /// Returns whether the redactor is enabled (defaults to true when absent).
    pub fn is_redactor_enabled(&self) -> bool {
        self.redactor.as_ref().is_none_or(|r| r.enabled)
    }
}

/// LLM provider settings.
#[derive(Debug, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: String,
    pub api_key: String,
}

/// Markdown output settings.
/// `path` stores the vault root; `save_to_vault()` appends sub-directories.
#[derive(Debug, Serialize, Deserialize)]
pub struct OutputConfig {
    pub path: String,
}

/// Redactor (sensitive data masking) settings.
#[derive(Debug, Serialize, Deserialize)]
pub struct RedactorConfig {
    pub enabled: bool,
}

/// Returns the config file path: ~/.config/rwd/config.toml
pub fn config_path() -> Result<PathBuf, ConfigError> {
    let home = dirs::home_dir()
        .ok_or(crate::messages::error::HOME_DIR_NOT_FOUND)?;
    Ok(home.join(".config").join("rwd").join("config.toml"))
}

/// Saves config to a TOML file. Creates parent directories if needed.
pub fn save_config(config: &Config, path: &std::path::Path) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let toml_str = toml::to_string_pretty(config)?;
    std::fs::write(path, toml_str)?;

    // Restrict to owner-only read/write (0o600) since the file contains an API key.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
}

/// Loads config from a TOML file.
pub fn load_config(path: &std::path::Path) -> Result<Config, ConfigError> {
    let content = std::fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}

/// Loads config if the file exists; returns None otherwise.
pub fn load_config_if_exists() -> Option<Config> {
    let path = config_path().ok()?;
    if path.exists() {
        load_config(&path).ok()
    } else {
        None
    }
}

/// `rwd init` — prompts for API key, detects output path, and saves config.
pub fn run_init() -> Result<(), ConfigError> {
    let config_file = config_path()?;

    // Provider selection
    eprint!("{}", crate::messages::init::SELECT_PROVIDER);
    let mut provider_input = String::new();
    std::io::stdin().read_line(&mut provider_input)?;
    let provider = provider_input.trim();
    let provider = if provider.is_empty() { "anthropic" } else { provider };

    if !["anthropic", "openai"].contains(&provider) {
        return Err(crate::messages::init::unsupported_provider(provider).into());
    }

    // API key input (masked)
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

    // Show masked key (first 8 chars + ***)
    let masked = if api_key.len() > 8 {
        format!("{}***", &api_key[..8])
    } else {
        "***".to_string()
    };
    eprintln!("{}", crate::messages::init::api_key_set(&masked));

    // Output path — suggest detected vault path as default
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

    // Language selection
    eprint!("{}", crate::messages::lang::SELECT);
    let mut lang_input = String::new();
    std::io::stdin().read_line(&mut lang_input)?;
    let lang = match lang_input.trim() {
        "ko" => Lang::Ko,
        _ => Lang::En,
    };

    let config = Config {
        llm: LlmConfig {
            provider: provider.to_string(),
            api_key,
        },
        output: OutputConfig {
            path: output_path.to_string_lossy().to_string(),
        },
        redactor: None,
        lang: Some(lang),
    };

    save_config(&config, &config_file)?;
    eprintln!("{}", crate::messages::init::config_saved(&config_file.display()));
    Ok(())
}

/// `rwd config <key> <value>` — updates a single config value.
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
        "lang" => {
            let lang = match value {
                "ko" => Lang::Ko,
                "en" => Lang::En,
                _ => return Err(crate::messages::lang::unsupported(value).into()),
            };
            config.lang = Some(lang);
            eprintln!("Language changed: {value}");
        }
        _ => {
            return Err(crate::messages::config::unknown_key(key).into());
        }
    }

    save_config(&config, &config_file)?;
    Ok(())
}

/// Reads a password with Esc support. Esc returns None (cancel), Enter returns the input.
/// Reads a visible text input with Esc-to-cancel support.
/// Returns `None` if user presses Escape, `Some(default)` if Enter on empty input.
fn read_input_with_esc(prompt: &str, default: &str) -> Result<Option<String>, ConfigError> {
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
                let value = if input.is_empty() {
                    default.to_string()
                } else {
                    input
                };
                return Ok(Some(value));
            }
            Key::Backspace => {
                if !input.is_empty() {
                    input.pop();
                    term.clear_chars(1)?;
                }
            }
            Key::Char(c) if !c.is_ascii_control() => {
                input.push(c);
                eprint!("{c}");
            }
            _ => {}
        }
    }
}

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

/// Verifies an API key by sending a lightweight request to the provider's models endpoint.
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

/// Masks an API key for display.
fn mask_api_key(key: &str) -> String {
    if key.len() > 8 {
        format!("{}***", &key[..8])
    } else if key.len() > 4 {
        format!("{}***", &key[..4])
    } else {
        "***".to_string()
    }
}

/// `rwd config` (no args) — interactive menu for changing settings.
pub async fn run_config_interactive() -> Result<(), ConfigError> {
    let config_file = config_path()?;

    if !config_file.exists() {
        return Err(crate::messages::config::NO_CONFIG.into());
    }

    let mut config = load_config(&config_file)?;
    let theme = ColorfulTheme::default();

    let cyan = "\x1b[36m";
    let dim = "\x1b[2m";
    let reset = "\x1b[0m";

    eprintln!("{dim}{}{reset}", crate::messages::config::NAV_HINT);

    loop {
        let redactor_status = if config.is_redactor_enabled() { "on" } else { "off" };
        let items = vec![
            format!("{cyan}provider{reset}      {dim}[{}]{reset}", config.llm.provider),
            format!("{cyan}api-key{reset}       {dim}[{}]{reset}", mask_api_key(&config.llm.api_key)),
            format!("{cyan}output-path{reset}   {dim}[{}]{reset}", config.output.path),
            format!("{cyan}redactor{reset}      {dim}[{}]{reset}", redactor_status),
            format!("{cyan}lang{reset}          {dim}[{}]{reset}", config.lang.as_ref().map_or("not set", |l| match l { Lang::En => "en", Lang::Ko => "ko" })),
            format!("{dim}{}{reset}", crate::messages::config::EXIT),
        ];

        let selection = Select::with_theme(&theme)
            .with_prompt(crate::messages::config::SELECT_SETTING)
            .items(&items)
            .default(0)
            .interact_opt()?;

        let Some(selection) = selection else { break };

        let green = "\x1b[32m";

        match selection {
            // provider
            0 => {
                let old = config.llm.provider.clone();
                let providers = ["anthropic", "openai"];
                let current_idx = providers
                    .iter()
                    .position(|&p| p == old)
                    .unwrap_or(0);

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
            // api-key
            1 => {
                let Some(new_key) = read_password_with_esc(crate::messages::config::NEW_API_KEY)? else {
                    continue;
                };
                let new_key = new_key.trim().to_string();

                if new_key.is_empty() {
                    continue;
                }
                let confirmed = Confirm::with_theme(&theme)
                    .with_prompt(crate::messages::config::CONFIRM_API_KEY)
                    .default(false)
                    .interact_opt()?;
                if confirmed != Some(true) {
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
            // output-path
            2 => {
                let old = config.output.path.clone();
                let prompt = format!(
                    "  {} ({old}): ",
                    crate::messages::config::OUTPUT_PATH
                );
                let Some(new_path) = read_input_with_esc(&prompt, &old)? else {
                    continue;
                };

                if new_path == old {
                    eprintln!("{dim}{}{reset}\n", crate::messages::config::NO_CHANGE);
                } else {
                    config.output.path = new_path.clone();
                    save_config(&config, &config_file)?;
                    eprintln!("{green}{}{reset}\n", crate::messages::config::changed(&old, &new_path));
                }
            }
            // redactor
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
            // lang
            4 => {
                let langs = ["en", "ko"];
                let current_idx = config.lang.as_ref().map_or(0, |l| match l {
                    Lang::En => 0,
                    Lang::Ko => 1,
                });

                let Some(chosen) = Select::with_theme(&theme)
                    .with_prompt(crate::messages::config::LANGUAGE)
                    .items(&langs)
                    .default(current_idx)
                    .interact_opt()?
                else {
                    continue;
                };

                let new_lang = if chosen == 0 { Lang::En } else { Lang::Ko };
                let old_label = config.lang.as_ref().map_or("not set".to_string(), |l| l.to_string());
                if config.lang.as_ref() == Some(&new_lang) {
                    eprintln!("{dim}{}{reset}\n", crate::messages::config::NO_CHANGE);
                } else {
                    config.lang = Some(new_lang.clone());
                    save_config(&config, &config_file)?;
                    eprintln!("{green}{}{reset}\n", crate::messages::config::changed(&old_label, &new_lang.to_string()));
                }
            }
            // exit
            5 => {
                console::Term::stderr().clear_last_lines(1)?;
                break;
            }
            _ => unreachable!(),
        }
    }

    eprintln!("\x1b[32m{}\x1b[0m", crate::messages::config::config_saved(&config_file.display()));
    Ok(())
}

/// Reads vault path from Obsidian's app config (obsidian.json).
/// macOS:   ~/Library/Application Support/obsidian/obsidian.json
/// Windows: %APPDATA%/obsidian/obsidian.json
/// Linux:   ~/.config/obsidian/obsidian.json
fn detect_vault_from_obsidian_json() -> Option<PathBuf> {
    let base = if cfg!(target_os = "macos") {
        dirs::home_dir()?.join("Library").join("Application Support")
    } else if cfg!(target_os = "windows") {
        dirs::config_dir()? // %APPDATA%
    } else {
        dirs::home_dir()?.join(".config")
    };
    let json_path = base.join("obsidian").join("obsidian.json");

    let content = std::fs::read_to_string(&json_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

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

/// Finds a directory containing `.obsidian` (Obsidian vault marker) under the given path.
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

/// Auto-detects the Obsidian vault path.
/// Priority: 1) obsidian.json, 2) .obsidian marker under ~/Documents/Obsidian/.
pub fn detect_obsidian_vault() -> Option<PathBuf> {

    if let Some(vault) = detect_vault_from_obsidian_json() {
        return Some(vault);
    }

    let home = dirs::home_dir()?;
    let obsidian_dir = home.join("Documents").join("Obsidian");
    detect_vault_in_dir(&obsidian_dir)
}

/// Default output path when no Obsidian vault is detected: ~/.rwd/output
pub fn default_output_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".rwd").join("output")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_path_includes_rwd_dir() {
        let path = config_path().expect("path creation");
        assert!(path.ends_with("rwd/config.toml"));
    }

    #[test]
    fn test_save_and_load_config_roundtrip() {
        let temp_dir = std::env::temp_dir().join("rwd_test_config");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).expect("create dir");
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
            lang: Some(Lang::En),
        };

        save_config(&config, &path).expect("save");
        let loaded = load_config(&path).expect("load");

        assert_eq!(loaded.llm.provider, "anthropic");
        assert_eq!(loaded.llm.api_key, "sk-test-key");
        assert_eq!(loaded.output.path, "/tmp/vault");
        assert_eq!(loaded.lang, Some(Lang::En));

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_detect_vault_returns_path_when_obsidian_dir_exists() {
        let temp_dir = std::env::temp_dir().join("rwd_test_vault_detect");
        let _ = std::fs::remove_dir_all(&temp_dir);
        let vault_dir = temp_dir.join("TestVault");
        let obsidian_marker = vault_dir.join(".obsidian");
        std::fs::create_dir_all(&obsidian_marker).expect("create dir");

        let result = detect_vault_in_dir(&temp_dir);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), vault_dir);

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_detect_vault_returns_none_when_missing() {
        let temp_dir = std::env::temp_dir().join("rwd_test_no_vault");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).expect("create dir");

        let result = detect_vault_in_dir(&temp_dir);
        assert!(result.is_none());

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_config_redactor_none_when_missing() {
        let toml_str = r#"
[llm]
provider = "anthropic"
api_key = "sk-test"

[output]
path = "/tmp/vault"
"#;
        let config: Config = toml::from_str(toml_str).expect("parse");
        assert!(config.redactor.is_none());
    }

    #[test]
    fn test_config_redactor_parses_when_present() {
        let toml_str = r#"
[llm]
provider = "anthropic"
api_key = "sk-test"

[output]
path = "/tmp/vault"

[redactor]
enabled = false
"#;
        let config: Config = toml::from_str(toml_str).expect("parse");
        assert_eq!(config.redactor.unwrap().enabled, false);
    }

    #[test]
    fn test_config_lang_none_when_missing() {
        let toml_str = r#"
[llm]
provider = "anthropic"
api_key = "sk-test"

[output]
path = "/tmp/vault"
"#;
        let config: Config = toml::from_str(toml_str).expect("parse");
        assert!(config.lang.is_none());
    }

    #[test]
    fn test_config_lang_parses_ko() {
        // lang must appear before table sections in TOML
        let toml_str = r#"
lang = "ko"

[llm]
provider = "anthropic"
api_key = "sk-test"

[output]
path = "/tmp/vault"
"#;
        let config: Config = toml::from_str(toml_str).expect("parse");
        assert_eq!(config.lang, Some(Lang::Ko));
    }

    #[test]
    fn test_config_lang_parses_en() {
        let toml_str = r#"
lang = "en"

[llm]
provider = "anthropic"
api_key = "sk-test"

[output]
path = "/tmp/vault"
"#;
        let config: Config = toml::from_str(toml_str).expect("parse");
        assert_eq!(config.lang, Some(Lang::En));
    }

    #[test]
    fn test_lang_display() {
        assert_eq!(Lang::En.to_string(), "en");
        assert_eq!(Lang::Ko.to_string(), "ko");
    }

    #[test]
    fn test_lang_roundtrip_serialization() {
        let config = Config {
            llm: LlmConfig {
                provider: "anthropic".to_string(),
                api_key: "sk-test".to_string(),
            },
            output: OutputConfig {
                path: "/tmp/vault".to_string(),
            },
            redactor: None,
            lang: Some(Lang::Ko),
        };
        let serialized = toml::to_string_pretty(&config).expect("serialize");
        assert!(serialized.contains("lang = \"ko\""));
        let loaded: Config = toml::from_str(&serialized).expect("deserialize");
        assert_eq!(loaded.lang, Some(Lang::Ko));
    }
}
