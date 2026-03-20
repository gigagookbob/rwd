# M5 Config System Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the `.env` dependency and build a configuration management system via `rwd init` / `rwd config` commands. Includes Obsidian vault auto-detection.

**Architecture:** Store the config file at `~/.config/rwd/config.toml`. The `config` module handles TOML read/write, and CLI subcommands (`Init`, `Config`) handle user interaction. Migrate existing `analyzer/provider.rs` and `output/mod.rs` to read from the config module instead of `.env`.

**Tech Stack:** `toml` (TOML parsing), `dirs` (already in use, XDG paths), `serde` (serialization)

---

## File Structure

| Action | File | Role |
|--------|------|------|
| Create | `src/config.rs` | Config file read/write, Obsidian vault auto-detection |
| Modify | `src/cli.rs` | Add `Init`, `Config` subcommands |
| Modify | `src/main.rs` | Add `Init`, `Config` command handlers |
| Modify | `src/analyzer/provider.rs` | Switch from `.env` to config module |
| Modify | `src/output/mod.rs` | Switch from `.env` to config module |
| Modify | `Cargo.toml` | Add `toml` crate |

---

## Chunk 1: Config Module Foundation

### Task 1: Add `toml` crate and define config structs

**Files:**
- Modify: `Cargo.toml`
- Create: `src/config.rs`
- Modify: `src/main.rs:3` (add mod declaration)

- [ ] **Step 1: Add toml crate to Cargo.toml**

Add to `[dependencies]` in `Cargo.toml`:
```toml
toml = "0.8"
```

- [ ] **Step 2: Create config.rs — define config structs**

Create `src/config.rs`:
```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Top-level config file struct.
/// Serialize/Deserialize derive enables automatic TOML ↔ Rust struct conversion (serde pattern).
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub llm: LlmConfig,
    pub output: OutputConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: String,
    pub api_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OutputConfig {
    pub path: String,
}
```

- [ ] **Step 3: Add mod config declaration to main.rs**

Add `mod config;` at the top of `src/main.rs`.

- [ ] **Step 4: Verify compilation with cargo build**

Run: `cargo build`
Expected: success (no warnings)

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src/config.rs src/main.rs
git commit -m "feat: config module skeleton — Config struct and toml crate"
```

---

### Task 2: Config file path and read/write functions

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Write a failing test — config_path()**

Add a test module at the bottom of `src/config.rs`:
```rust
/// Returns the config file path: ~/.config/rwd/config.toml
/// dirs::config_dir() returns the OS-specific config directory.
/// macOS: ~/Library/Application Support, Linux: ~/.config
/// Here we use ~/.config directly to follow Unix conventions.
pub fn config_path() -> Result<PathBuf, ConfigError> {
    todo!()
}

pub type ConfigError = Box<dyn std::error::Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_path_contains_rwd_directory() {
        let path = config_path().expect("path creation should succeed");
        assert!(path.ends_with("rwd/config.toml"));
    }
}
```

- [ ] **Step 2: Run test — verify failure**

Run: `cargo test test_config_path`
Expected: FAIL (`todo!()` panic)

- [ ] **Step 3: Implement config_path()**

Replace `todo!()`:
```rust
pub fn config_path() -> Result<PathBuf, ConfigError> {
    let home = dirs::home_dir()
        .ok_or("Could not find home directory")?;
    Ok(home.join(".config").join("rwd").join("config.toml"))
}
```

- [ ] **Step 4: Verify test passes**

Run: `cargo test test_config_path`
Expected: PASS

- [ ] **Step 5: Write save_config / load_config tests**

```rust
#[test]
fn test_save_and_load_config_roundtrip() {
    let temp_dir = std::env::temp_dir().join("rwd_test_config");
    let _ = std::fs::remove_dir_all(&temp_dir); // Clean up previous test remnants
    std::fs::create_dir_all(&temp_dir).expect("create directory");
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

    save_config(&config, &path).expect("save should succeed");
    let loaded = load_config(&path).expect("load should succeed");

    assert_eq!(loaded.llm.provider, "anthropic");
    assert_eq!(loaded.llm.api_key, "sk-test-key");
    assert_eq!(loaded.output.path, "/tmp/vault");

    std::fs::remove_dir_all(&temp_dir).ok();
}
```

- [ ] **Step 6: Run test — verify failure**

Run: `cargo test test_save_and_load_config`
Expected: FAIL (functions don't exist)

- [ ] **Step 7: Implement save_config / load_config**

```rust
/// Saves config to a TOML file.
/// toml::to_string_pretty() converts the Config struct to a human-readable TOML string.
/// create_dir_all() auto-creates parent directories if they don't exist.
pub fn save_config(config: &Config, path: &std::path::Path) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let toml_str = toml::to_string_pretty(config)?;
    std::fs::write(path, toml_str)?;

    // Set permissions to owner read/write only since the file contains API keys.
    // Unix permission 0o600 = owner read+write only (security convention).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
}

/// Reads config from a TOML file.
/// toml::from_str() deserializes a TOML string into a Config struct.
pub fn load_config(path: &std::path::Path) -> Result<Config, ConfigError> {
    let content = std::fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}
```

- [ ] **Step 8: Verify test passes**

Run: `cargo test test_save_and_load_config`
Expected: PASS

- [ ] **Step 9: Run cargo clippy, then commit**

Run: `cargo clippy && cargo test`
Expected: 0 warnings, all tests pass

```bash
git add src/config.rs
git commit -m "feat: config read/write — config_path, save_config, load_config"
```

---

### Task 3: Obsidian vault auto-detection

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Write a failing test — detect_obsidian_vault()**

```rust
#[test]
fn test_detect_obsidian_vault_returns_path_when_obsidian_folder_exists() {
    let temp_dir = std::env::temp_dir().join("rwd_test_vault_detect");
    let vault_dir = temp_dir.join("TestVault");
    let obsidian_marker = vault_dir.join(".obsidian");
    std::fs::create_dir_all(&obsidian_marker).expect("create directory");

    let result = detect_vault_in_dir(&temp_dir);
    assert!(result.is_some());
    assert_eq!(result.unwrap(), vault_dir);

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_detect_obsidian_vault_returns_none_when_not_found() {
    let temp_dir = std::env::temp_dir().join("rwd_test_no_vault");
    std::fs::create_dir_all(&temp_dir).expect("create directory");

    let result = detect_vault_in_dir(&temp_dir);
    assert!(result.is_none());

    std::fs::remove_dir_all(&temp_dir).ok();
}
```

- [ ] **Step 2: Run test — verify failure**

Run: `cargo test test_detect_obsidian_vault`
Expected: FAIL (function doesn't exist)

- [ ] **Step 3: Implement detect_vault_in_dir**

```rust
/// Finds a directory containing a .obsidian folder within the given directory.
/// The .obsidian folder is the marker that Obsidian uses to recognize a vault.
/// Uses read_dir() for 1-level depth search only — deep nesting is unnecessary.
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
```

- [ ] **Step 4: Verify test passes**

Run: `cargo test test_detect_obsidian_vault`
Expected: PASS

- [ ] **Step 5: Write detect_obsidian_vault() integration function**

```rust
/// Auto-detects the Obsidian vault.
/// Priority: search for .obsidian marker under ~/Documents/Obsidian/
/// Returns None if not found — the caller falls back to a default path.
pub fn detect_obsidian_vault() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let obsidian_dir = home.join("Documents").join("Obsidian");
    detect_vault_in_dir(&obsidian_dir)
}

/// Determines the default output path (vault root).
/// 1. Auto-detect Obsidian vault → {vault} (return vault root)
/// 2. Detection failed → ~/.rwd/output (default path)
///
/// Note: The Daily/ subdirectory is appended automatically by save_to_vault().
/// This function only returns the vault root.
pub fn default_output_path() -> PathBuf {
    if let Some(vault) = detect_obsidian_vault() {
        return vault;
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".rwd").join("output")
}
```

- [ ] **Step 6: Run cargo clippy, then commit**

Run: `cargo clippy && cargo test`
Expected: 0 warnings, all tests pass

```bash
git add src/config.rs
git commit -m "feat: Obsidian vault auto-detection — .obsidian marker based search"
```

---

## Chunk 2: CLI Commands and Module Migration

### Task 4: Add Init, Config subcommands to CLI

**Files:**
- Modify: `src/cli.rs`

- [ ] **Step 1: Add subcommands to cli.rs**

```rust
use clap::{Parser, Subcommand};

/// CLI tool for analyzing AI coding session logs and extracting daily development insights
#[derive(Parser)]
#[command(name = "rwd", version, about)]
pub struct Cli {
    /// Subcommand to execute
    #[command(subcommand)]
    pub command: Commands,
}

/// List of subcommands supported by rwd
#[derive(Subcommand)]
pub enum Commands {
    /// Analyze today's session logs
    Today,
    /// Run initial setup (API key, output path)
    Init,
    /// Change a configuration value
    Config {
        /// Config key (output-path)
        key: String,
        /// Value to set
        value: String,
    },
}
```

- [ ] **Step 2: Run cargo build**

Run: `cargo build`
Expected: compile error — `Init`, `Config` not handled in `main.rs` match

- [ ] **Step 3: Add placeholder handlers to main.rs**

Add to `match args.command` in `main.rs`:
```rust
Commands::Init => {
    println!("TODO: init implementation pending");
}
Commands::Config { key, value } => {
    println!("TODO: config {key} = {value}");
}
```

- [ ] **Step 4: Run cargo build**

Run: `cargo build`
Expected: success

- [ ] **Step 5: Verify rwd --help output**

Run: `cargo run -- --help`
Expected: `today`, `init`, `config` subcommands all displayed

- [ ] **Step 6: Commit**

```bash
git add src/cli.rs src/main.rs
git commit -m "feat: add init, config subcommands to CLI"
```

---

### Task 5: Implement `rwd init`

**Files:**
- Modify: `src/main.rs`
- Modify: `src/config.rs`

- [ ] **Step 1: Write run_init() function in config.rs**

```rust
/// Runs rwd init — prompts for API key, auto-detects output path, and saves to config file.
///
/// eprint! outputs prompts to stderr — stdout is reserved for data output.
/// stdin().read_line() reads one line of user input (Rust Book Ch.2).
pub fn run_init() -> Result<(), ConfigError> {
    let config_file = config_path()?;

    // Provider selection
    eprint!("Select LLM provider (anthropic/openai) [anthropic]: ");
    let mut provider_input = String::new();
    std::io::stdin().read_line(&mut provider_input)?;
    let provider = provider_input.trim();
    let provider = if provider.is_empty() { "anthropic" } else { provider };

    let key_prompt = match provider {
        "anthropic" => "Enter your Anthropic API key: ",
        "openai" => "Enter your OpenAI API key: ",
        _ => return Err(format!("Unsupported provider: {provider}").into()),
    };
    eprint!("{key_prompt}");
    let mut api_key = String::new();
    std::io::stdin().read_line(&mut api_key)?;
    let api_key = api_key.trim().to_string();

    if api_key.is_empty() {
        return Err("API key is empty.".into());
    }

    // Auto-detect output path
    let output_path = default_output_path();
    match detect_obsidian_vault() {
        Some(vault) => {
            println!("Obsidian vault detected: {}", vault.display());
            println!("Output path: {}", output_path.display());
        }
        None => {
            println!("Could not find an Obsidian vault.");
            println!("Default output path: {}", output_path.display());
        }
    }

    let config = Config {
        llm: LlmConfig {
            provider: provider.to_string(),
            api_key,
        },
        output: OutputConfig {
            path: output_path.to_string_lossy().to_string(),
        },
    };

    save_config(&config, &config_file)?;
    println!("Config saved: {}", config_file.display());
    Ok(())
}
```

- [ ] **Step 2: Connect Init handler in main.rs**

Replace placeholder:
```rust
Commands::Init => {
    if let Err(e) = config::run_init() {
        eprintln!("Initial setup failed: {e}");
        std::process::exit(1);
    }
}
```

- [ ] **Step 3: Run cargo build**

Run: `cargo build`
Expected: success

- [ ] **Step 4: Manual run test**

Run: `cargo run -- init`
Expected: provider/API key prompts → config saved message

- [ ] **Step 5: Verify config file creation**

Run: `cat ~/.config/rwd/config.toml`
Expected: config saved in TOML format

- [ ] **Step 6: Commit**

```bash
git add src/config.rs src/main.rs
git commit -m "feat: implement rwd init — API key input + Obsidian vault auto-detection"
```

---

### Task 6: Implement `rwd config`

**Files:**
- Modify: `src/config.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write run_config() function in config.rs**

```rust
/// rwd config <key> <value> — changes an individual config value.
/// Reads the existing config file, modifies the specified key, and saves it back.
pub fn run_config(key: &str, value: &str) -> Result<(), ConfigError> {
    let config_file = config_path()?;

    if !config_file.exists() {
        return Err("Config file not found. Please run `rwd init` first.".into());
    }

    let mut config = load_config(&config_file)?;

    match key {
        "output-path" => {
            config.output.path = value.to_string();
            println!("Output path changed: {value}");
        }
        "provider" => {
            if !["anthropic", "openai"].contains(&value) {
                return Err(format!(
                    "Unsupported provider: '{value}'. Available: anthropic, openai"
                ).into());
            }
            config.llm.provider = value.to_string();
            println!("LLM provider changed: {value}");
        }
        "api-key" => {
            config.llm.api_key = value.to_string();
            println!("API key updated");
        }
        _ => {
            return Err(format!(
                "Unknown config key: '{key}'. Available: output-path, provider, api-key"
            ).into());
        }
    }

    save_config(&config, &config_file)?;
    Ok(())
}
```

- [ ] **Step 2: Connect Config handler in main.rs**

Replace placeholder:
```rust
Commands::Config { key, value } => {
    if let Err(e) = config::run_config(&key, &value) {
        eprintln!("Config change failed: {e}");
        std::process::exit(1);
    }
}
```

- [ ] **Step 3: Run cargo build**

Run: `cargo build`
Expected: success

- [ ] **Step 4: Manual run test**

Run: `cargo run -- config output-path /tmp/test/Daily`
Expected: "Output path changed: /tmp/test/Daily" output

Run: `cat ~/.config/rwd/config.toml`
Expected: output.path value changed

- [ ] **Step 5: Commit**

```bash
git add src/config.rs src/main.rs
git commit -m "feat: implement rwd config — output-path, provider, api-key changes"
```

---

### Task 7: Migrate existing modules to config-based

**Files:**
- Modify: `src/analyzer/provider.rs`
- Modify: `src/output/mod.rs`
- Modify: `src/main.rs`
- Modify: `src/config.rs`

This task migrates the existing `.env`-based logic to `config.toml`-based.
**Migration strategy**: read from config file if present, otherwise fall back to existing `.env` (backward compatibility).

- [ ] **Step 1: Add load_or_default() function to config.rs**

```rust
/// Reads the config file. Returns None if the file doesn't exist.
/// The caller uses existing .env fallback when None.
pub fn load_config_if_exists() -> Option<Config> {
    let path = config_path().ok()?;
    if path.exists() {
        load_config(&path).ok()
    } else {
        None
    }
}
```

- [ ] **Step 2: Modify load_provider() in analyzer/provider.rs**

Add config-first logic at the beginning of `load_provider()`:
```rust
pub fn load_provider() -> Result<(LlmProvider, String), super::AnalyzerError> {
    // Use config.toml if available
    if let Some(config) = crate::config::load_config_if_exists() {
        let provider = match config.llm.provider.as_str() {
            "openai" => LlmProvider::OpenAi,
            _ => LlmProvider::Anthropic,
        };
        return Ok((provider, config.llm.api_key));
    }

    // Fallback: existing .env approach
    dotenvy::dotenv().ok();
    // ... existing code remains ...
}
```

- [ ] **Step 3: Modify load_vault_path() in output/mod.rs**

Add config-first logic to `load_vault_path()`.
`config.output.path` stores the vault root path (Daily/ is appended by save_to_vault()):
```rust
pub fn load_vault_path() -> Result<PathBuf, OutputError> {
    // Use config.toml if available
    if let Some(config) = crate::config::load_config_if_exists() {
        let path = PathBuf::from(&config.output.path);
        if !path.exists() {
            std::fs::create_dir_all(&path)?;
        }
        return Ok(path);
    }

    // Fallback: existing .env approach
    dotenvy::dotenv().ok();
    // ... existing code remains ...
}
```

- [ ] **Step 4: Run cargo build**

Run: `cargo build`
Expected: success

- [ ] **Step 5: Run cargo clippy && cargo test**

Run: `cargo clippy && cargo test`
Expected: 0 warnings, all tests pass

- [ ] **Step 6: Manual test**

Verify with existing .env:
Run: `cargo run -- today`
Expected: works the same as before

Verify with config.toml:
1. Create config with `cargo run -- init`
2. Temporarily rename `.env` (`mv .env .env.bak`)
3. Run `cargo run -- today`
4. Verify it works correctly using config.toml
5. Restore `.env` (`mv .env.bak .env`)

- [ ] **Step 7: Commit**

```bash
git add src/config.rs src/analyzer/provider.rs src/output/mod.rs
git commit -m "feat: switch to config.toml-based — .env fallback preserved"
```

---

## Chunk 3: Cleanup and Finalization

### Task 8: Migration notice when running rwd init with existing .env

> Proceed before Task 9 (dotenvy removal) — users need to migrate via init before .env is removed.

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Add .env detection logic to run_init()**

Add at the beginning of `run_init()`:
```rust
// Notify if existing .env file is detected
let cwd_env = std::path::Path::new(".env");
if cwd_env.exists() {
    eprintln!("Existing .env file detected.");
    eprintln!("After completing rwd init, the .env file will no longer be used.");
    eprintln!("Configuration is now managed at ~/.config/rwd/config.toml.\n");
}
```

- [ ] **Step 2: Run cargo build**

Run: `cargo build`
Expected: success

- [ ] **Step 3: Commit**

```bash
git add src/config.rs
git commit -m "feat: add .env migration notice message during rwd init"
```

---

### Task 9: Remove .env fallback and dotenvy dependency

> Proceed after Task 8 (migration notice) is complete.

**Files:**
- Modify: `src/analyzer/provider.rs`
- Modify: `src/output/mod.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: Remove .env fallback code from provider.rs**

Change `load_provider()` to config-only. Include migration hint in error message for users who still have .env:
```rust
pub fn load_provider() -> Result<(LlmProvider, String), super::AnalyzerError> {
    let config = crate::config::load_config_if_exists().ok_or_else(|| {
        let hint = if std::path::Path::new(".env").exists() {
            " (existing .env users: run `rwd init` to migrate your config)"
        } else {
            ""
        };
        format!("Config file not found. Please run `rwd init` first.{hint}")
    })?;

    let provider = match config.llm.provider.as_str() {
        "openai" => LlmProvider::OpenAi,
        _ => LlmProvider::Anthropic,
    };
    Ok((provider, config.llm.api_key))
}
```

- [ ] **Step 2: Remove .env fallback code from output/mod.rs**

Change `load_vault_path()` to config-only:
```rust
pub fn load_vault_path() -> Result<PathBuf, OutputError> {
    let config = crate::config::load_config_if_exists().ok_or_else(|| {
        let hint = if std::path::Path::new(".env").exists() {
            " (existing .env users: run `rwd init` to migrate your config)"
        } else {
            ""
        };
        format!("Config file not found. Please run `rwd init` first.{hint}")
    })?;

    let path = PathBuf::from(&config.output.path);
    if !path.exists() {
        std::fs::create_dir_all(&path)?;
    }
    Ok(path)
}
```

- [ ] **Step 3: Remove dotenvy from Cargo.toml**

Delete the `dotenvy = "0.15"` line.

- [ ] **Step 4: Build — remove any remaining dotenvy references**

Run: `cargo build`
Expected: success (any remaining dotenvy imports will cause compile errors)

- [ ] **Step 5: Run cargo clippy && cargo test**

Run: `cargo clippy && cargo test`
Expected: 0 warnings, all tests pass

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml src/analyzer/provider.rs src/output/mod.rs
git commit -m "refactor: remove .env/dotenvy — switch to config.toml as single source"
```

---

## Completion Checklist

- [ ] `rwd init` — API key input, Obsidian vault auto-detection, `~/.config/rwd/config.toml` creation
- [ ] `rwd config output-path <path>` — change output path
- [ ] `rwd config provider <name>` — change LLM provider
- [ ] `rwd config api-key <key>` — change API key
- [ ] `rwd today` — works correctly based on config.toml
- [ ] `.env` and `dotenvy` dependency fully removed
- [ ] `cargo build`, `cargo clippy`, `cargo test` all pass
