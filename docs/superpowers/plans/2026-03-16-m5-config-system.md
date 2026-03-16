# M5 설정 시스템 구현 계획

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `.env` 의존을 제거하고, `rwd init` / `rwd config` 커맨드로 설정을 관리하는 시스템 구축. Obsidian vault 자동 감지 포함.

**Architecture:** `~/.config/rwd/config.toml`에 설정 파일을 저장한다. `config` 모듈이 TOML 읽기/쓰기를 담당하고, CLI 서브커맨드(`Init`, `Config`)가 사용자 인터랙션을 처리한다. 기존 `analyzer/provider.rs`와 `output/mod.rs`가 `.env` 대신 config 모듈에서 값을 읽도록 전환한다.

**Tech Stack:** `toml` (TOML 파싱), `dirs` (이미 사용 중, XDG 경로), `serde` (직렬화)

---

## 파일 구조

| 액션 | 파일 | 역할 |
|------|------|------|
| 생성 | `src/config.rs` | 설정 파일 읽기/쓰기, Obsidian vault 자동 감지 |
| 수정 | `src/cli.rs` | `Init`, `Config` 서브커맨드 추가 |
| 수정 | `src/main.rs` | `Init`, `Config` 커맨드 핸들러 추가 |
| 수정 | `src/analyzer/provider.rs` | `.env` → config 모듈로 전환 |
| 수정 | `src/output/mod.rs` | `.env` → config 모듈로 전환 |
| 수정 | `Cargo.toml` | `toml` 크레이트 추가 |

---

## Chunk 1: config 모듈 기반

### Task 1: `toml` 크레이트 추가 및 설정 구조체 정의

**Files:**
- Modify: `Cargo.toml`
- Create: `src/config.rs`
- Modify: `src/main.rs:3` (mod 선언 추가)

- [ ] **Step 1: Cargo.toml에 toml 크레이트 추가**

`Cargo.toml`의 `[dependencies]`에 추가:
```toml
toml = "0.8"
```

- [ ] **Step 2: config.rs 생성 — 설정 구조체 정의**

`src/config.rs` 생성:
```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 설정 파일의 최상위 구조체.
/// Serialize/Deserialize derive로 TOML ↔ Rust 구조체 자동 변환 (serde 패턴).
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

- [ ] **Step 3: main.rs에 mod config 선언 추가**

`src/main.rs` 상단에 `mod config;` 추가.

- [ ] **Step 4: cargo build로 컴파일 확인**

Run: `cargo build`
Expected: 성공 (warning 없음)

- [ ] **Step 5: 커밋**

```bash
git add Cargo.toml src/config.rs src/main.rs
git commit -m "feat: config 모듈 뼈대 — Config 구조체 및 toml 크레이트 추가"
```

---

### Task 2: 설정 파일 경로 및 읽기/쓰기 함수

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: 실패하는 테스트 작성 — config_path()**

`src/config.rs` 하단에 테스트 모듈 추가:
```rust
/// 설정 파일 경로를 반환합니다: ~/.config/rwd/config.toml
/// dirs::config_dir()는 OS별 설정 디렉토리를 반환합니다.
/// macOS: ~/Library/Application Support, Linux: ~/.config
/// 여기서는 Unix 관례에 맞게 ~/.config를 직접 사용합니다.
pub fn config_path() -> Result<PathBuf, ConfigError> {
    todo!()
}

pub type ConfigError = Box<dyn std::error::Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_path_rwd_디렉토리_포함() {
        let path = config_path().expect("경로 생성 성공");
        assert!(path.ends_with("rwd/config.toml"));
    }
}
```

- [ ] **Step 2: 테스트 실행 — 실패 확인**

Run: `cargo test test_config_path`
Expected: FAIL (`todo!()` 패닉)

- [ ] **Step 3: config_path() 구현**

`todo!()`를 교체:
```rust
pub fn config_path() -> Result<PathBuf, ConfigError> {
    let home = dirs::home_dir()
        .ok_or("홈 디렉토리를 찾을 수 없습니다")?;
    Ok(home.join(".config").join("rwd").join("config.toml"))
}
```

- [ ] **Step 4: 테스트 통과 확인**

Run: `cargo test test_config_path`
Expected: PASS

- [ ] **Step 5: save_config / load_config 테스트 작성**

```rust
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
```

- [ ] **Step 6: 테스트 실행 — 실패 확인**

Run: `cargo test test_save_and_load_config`
Expected: FAIL (함수 미존재)

- [ ] **Step 7: save_config / load_config 구현**

```rust
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
    // Unix 권한 0o600 = owner read+write only (Rust Book에는 없지만, 보안 관례).
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
```

- [ ] **Step 8: 테스트 통과 확인**

Run: `cargo test test_save_and_load_config`
Expected: PASS

- [ ] **Step 9: cargo clippy 확인 후 커밋**

Run: `cargo clippy && cargo test`
Expected: 경고 0, 테스트 전체 통과

```bash
git add src/config.rs
git commit -m "feat: config 읽기/쓰기 — config_path, save_config, load_config"
```

---

### Task 3: Obsidian vault 자동 감지

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: 실패하는 테스트 작성 — detect_obsidian_vault()**

```rust
#[test]
fn test_detect_obsidian_vault_obsidian폴더_있으면_경로반환() {
    let temp_dir = std::env::temp_dir().join("rwd_test_vault_detect");
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
    std::fs::create_dir_all(&temp_dir).expect("디렉토리 생성");

    let result = detect_vault_in_dir(&temp_dir);
    assert!(result.is_none());

    std::fs::remove_dir_all(&temp_dir).ok();
}
```

- [ ] **Step 2: 테스트 실행 — 실패 확인**

Run: `cargo test test_detect_obsidian_vault`
Expected: FAIL (함수 미존재)

- [ ] **Step 3: detect_vault_in_dir 구현**

```rust
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
```

- [ ] **Step 4: 테스트 통과 확인**

Run: `cargo test test_detect_obsidian_vault`
Expected: PASS

- [ ] **Step 5: detect_obsidian_vault() 통합 함수 작성**

```rust
/// Obsidian vault를 자동 감지합니다.
/// 1순위: ~/Documents/Obsidian/ 하위에서 .obsidian 마커 탐색
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
/// 여기서는 vault root만 반환합니다.
pub fn default_output_path() -> PathBuf {
    if let Some(vault) = detect_obsidian_vault() {
        return vault;
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".rwd").join("output")
}
```

- [ ] **Step 6: cargo clippy 확인 후 커밋**

Run: `cargo clippy && cargo test`
Expected: 경고 0, 테스트 전체 통과

```bash
git add src/config.rs
git commit -m "feat: Obsidian vault 자동 감지 — .obsidian 마커 기반 탐색"
```

---

## Chunk 2: CLI 커맨드 및 기존 모듈 전환

### Task 4: CLI에 Init, Config 서브커맨드 추가

**Files:**
- Modify: `src/cli.rs`

- [ ] **Step 1: cli.rs에 서브커맨드 추가**

```rust
use clap::{Parser, Subcommand};

/// AI 코딩 세션 로그를 분석하여 일일 개발 인사이트를 추출하는 CLI 도구
#[derive(Parser)]
#[command(name = "rwd", version, about)]
pub struct Cli {
    /// 실행할 서브커맨드
    #[command(subcommand)]
    pub command: Commands,
}

/// rwd가 지원하는 서브커맨드 목록
#[derive(Subcommand)]
pub enum Commands {
    /// 오늘의 세션 로그를 분석합니다
    Today,
    /// 초기 설정을 수행합니다 (API 키, 출력 경로)
    Init,
    /// 설정 값을 변경합니다
    Config {
        /// 설정 키 (output-path)
        key: String,
        /// 설정할 값
        value: String,
    },
}
```

- [ ] **Step 2: cargo build 확인**

Run: `cargo build`
Expected: 컴파일 에러 — `main.rs`의 match에서 `Init`, `Config` 미처리

- [ ] **Step 3: main.rs에 placeholder 핸들러 추가**

`main.rs`의 `match args.command`에 추가:
```rust
Commands::Init => {
    println!("TODO: init 구현 예정");
}
Commands::Config { key, value } => {
    println!("TODO: config {key} = {value}");
}
```

- [ ] **Step 4: cargo build 확인**

Run: `cargo build`
Expected: 성공

- [ ] **Step 5: rwd --help 출력 확인**

Run: `cargo run -- --help`
Expected: `today`, `init`, `config` 서브커맨드가 모두 표시

- [ ] **Step 6: 커밋**

```bash
git add src/cli.rs src/main.rs
git commit -m "feat: CLI에 init, config 서브커맨드 추가"
```

---

### Task 5: `rwd init` 구현

**Files:**
- Modify: `src/main.rs`
- Modify: `src/config.rs`

- [ ] **Step 1: config.rs에 run_init() 함수 작성**

```rust
/// rwd init 실행 — API 키를 입력받고, 출력 경로를 자동 감지하여 설정 파일에 저장합니다.
///
/// eprint!는 stderr로 프롬프트를 출력합니다 — stdout은 데이터 출력용으로 분리합니다.
/// stdin().read_line()은 사용자 입력을 한 줄 읽습니다 (Rust Book Ch.2 참조).
pub fn run_init() -> Result<(), ConfigError> {
    let config_file = config_path()?;

    // API 키 입력
    eprint!("LLM 프로바이더를 선택하세요 (anthropic/openai) [anthropic]: ");
    let mut provider_input = String::new();
    std::io::stdin().read_line(&mut provider_input)?;
    let provider = provider_input.trim();
    let provider = if provider.is_empty() { "anthropic" } else { provider };

    let key_prompt = match provider {
        "anthropic" => "Anthropic API 키를 입력하세요: ",
        "openai" => "OpenAI API 키를 입력하세요: ",
        _ => return Err(format!("지원하지 않는 프로바이더: {provider}").into()),
    };
    eprint!("{key_prompt}");
    let mut api_key = String::new();
    std::io::stdin().read_line(&mut api_key)?;
    let api_key = api_key.trim().to_string();

    if api_key.is_empty() {
        return Err("API 키가 비어있습니다.".into());
    }

    // 출력 경로 자동 감지
    let output_path = default_output_path();
    match detect_obsidian_vault() {
        Some(vault) => {
            println!("Obsidian vault 감지됨: {}", vault.display());
            println!("출력 경로: {}", output_path.display());
        }
        None => {
            println!("Obsidian vault를 찾지 못했습니다.");
            println!("기본 출력 경로: {}", output_path.display());
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
    println!("설정 저장 완료: {}", config_file.display());
    Ok(())
}
```

- [ ] **Step 2: main.rs에서 Init 핸들러 연결**

placeholder를 교체:
```rust
Commands::Init => {
    if let Err(e) = config::run_init() {
        eprintln!("초기 설정 실패: {e}");
        std::process::exit(1);
    }
}
```

- [ ] **Step 3: cargo build 확인**

Run: `cargo build`
Expected: 성공

- [ ] **Step 4: 실제 실행 테스트 (수동)**

Run: `cargo run -- init`
Expected: 프로바이더/API 키 입력 프롬프트 → 설정 저장 메시지 출력

- [ ] **Step 5: 설정 파일 생성 확인**

Run: `cat ~/.config/rwd/config.toml`
Expected: TOML 형식으로 저장된 설정 확인

- [ ] **Step 6: 커밋**

```bash
git add src/config.rs src/main.rs
git commit -m "feat: rwd init 구현 — API 키 입력 + Obsidian vault 자동 감지"
```

---

### Task 6: `rwd config` 구현

**Files:**
- Modify: `src/config.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: config.rs에 run_config() 함수 작성**

```rust
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
            println!("출력 경로 변경: {value}");
        }
        "provider" => {
            if !["anthropic", "openai"].contains(&value) {
                return Err(format!(
                    "지원하지 않는 프로바이더: '{value}'. 사용 가능: anthropic, openai"
                ).into());
            }
            config.llm.provider = value.to_string();
            println!("LLM 프로바이더 변경: {value}");
        }
        "api-key" => {
            config.llm.api_key = value.to_string();
            println!("API 키 변경 완료");
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
```

- [ ] **Step 2: main.rs에서 Config 핸들러 연결**

placeholder를 교체:
```rust
Commands::Config { key, value } => {
    if let Err(e) = config::run_config(&key, &value) {
        eprintln!("설정 변경 실패: {e}");
        std::process::exit(1);
    }
}
```

- [ ] **Step 3: cargo build 확인**

Run: `cargo build`
Expected: 성공

- [ ] **Step 4: 실제 실행 테스트 (수동)**

Run: `cargo run -- config output-path /tmp/test/Daily`
Expected: "출력 경로 변경: /tmp/test/Daily" 출력

Run: `cat ~/.config/rwd/config.toml`
Expected: output.path 값이 변경됨

- [ ] **Step 5: 커밋**

```bash
git add src/config.rs src/main.rs
git commit -m "feat: rwd config 구현 — output-path, provider, api-key 변경 지원"
```

---

### Task 7: 기존 모듈을 config 기반으로 전환

**Files:**
- Modify: `src/analyzer/provider.rs`
- Modify: `src/output/mod.rs`
- Modify: `src/main.rs`
- Modify: `src/config.rs`

이 태스크는 기존 `.env` 기반 로직을 `config.toml` 기반으로 전환합니다.
**전환 전략**: config 파일이 있으면 config에서 읽고, 없으면 기존 `.env` fallback (하위 호환성 유지).

- [ ] **Step 1: config.rs에 load_or_default() 함수 추가**

```rust
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
```

- [ ] **Step 2: analyzer/provider.rs의 load_provider() 수정**

`load_provider()` 함수 시작 부분에 config 우선 로직 추가:
```rust
pub fn load_provider() -> Result<(LlmProvider, String), super::AnalyzerError> {
    // config.toml이 있으면 우선 사용
    if let Some(config) = crate::config::load_config_if_exists() {
        let provider = match config.llm.provider.as_str() {
            "openai" => LlmProvider::OpenAi,
            _ => LlmProvider::Anthropic,
        };
        return Ok((provider, config.llm.api_key));
    }

    // fallback: 기존 .env 방식
    dotenvy::dotenv().ok();
    // ... 기존 코드 유지 ...
}
```

- [ ] **Step 3: output/mod.rs의 load_vault_path() 수정**

`load_vault_path()` 함수에 config 우선 로직 추가.
`config.output.path`는 vault root 경로를 저장합니다 (Daily/는 save_to_vault()가 붙임):
```rust
pub fn load_vault_path() -> Result<PathBuf, OutputError> {
    // config.toml이 있으면 우선 사용
    if let Some(config) = crate::config::load_config_if_exists() {
        let path = PathBuf::from(&config.output.path);
        if !path.exists() {
            std::fs::create_dir_all(&path)?;
        }
        return Ok(path);
    }

    // fallback: 기존 .env 방식
    dotenvy::dotenv().ok();
    // ... 기존 코드 유지 ...
}
```

- [ ] **Step 4: cargo build 확인**

Run: `cargo build`
Expected: 성공

- [ ] **Step 5: cargo clippy && cargo test 확인**

Run: `cargo clippy && cargo test`
Expected: 경고 0, 테스트 전체 통과

- [ ] **Step 6: 실제 동작 테스트 (수동)**

기존 .env로 동작 확인:
Run: `cargo run -- today`
Expected: 이전과 동일하게 동작

config.toml로 동작 확인:
1. `cargo run -- init` 으로 설정 생성
2. `.env` 파일을 임시 이름으로 변경 (`mv .env .env.bak`)
3. `cargo run -- today` 실행
4. config.toml 기반으로 정상 동작하는지 확인
5. `.env` 파일 복원 (`mv .env.bak .env`)

- [ ] **Step 7: 커밋**

```bash
git add src/config.rs src/analyzer/provider.rs src/output/mod.rs
git commit -m "feat: config.toml 기반으로 전환 — .env fallback 유지"
```

---

## Chunk 3: 정리 및 마무리

### Task 8: rwd init 시 기존 .env 마이그레이션 안내

> Task 9(dotenvy 제거)보다 먼저 진행합니다 — 사용자가 init을 통해 마이그레이션한 뒤 .env를 제거해야 합니다.

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: run_init()에 .env 감지 로직 추가**

`run_init()` 함수 시작 부분에 추가:
```rust
// 기존 .env 파일이 있으면 마이그레이션 안내
let cwd_env = std::path::Path::new(".env");
if cwd_env.exists() {
    eprintln!("기존 .env 파일이 감지되었습니다.");
    eprintln!("rwd init 완료 후 .env 파일은 더 이상 사용되지 않습니다.");
    eprintln!("설정은 ~/.config/rwd/config.toml에서 관리됩니다.\n");
}
```

- [ ] **Step 2: cargo build 확인**

Run: `cargo build`
Expected: 성공

- [ ] **Step 3: 커밋**

```bash
git add src/config.rs
git commit -m "feat: rwd init 시 기존 .env 파일 마이그레이션 안내 메시지 추가"
```

---

### Task 9: .env fallback 제거 및 dotenvy 의존성 정리

> Task 8(마이그레이션 안내)이 완료된 뒤 진행합니다.

**Files:**
- Modify: `src/analyzer/provider.rs`
- Modify: `src/output/mod.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: provider.rs에서 .env fallback 코드 제거**

`load_provider()`에서 `dotenvy::dotenv()` 및 `std::env::var()` 기반 코드를 제거하고, config 전용으로 변경.
`.env` 파일이 남아있는 사용자를 위해 에러 메시지에 마이그레이션 힌트 포함:
```rust
pub fn load_provider() -> Result<(LlmProvider, String), super::AnalyzerError> {
    let config = crate::config::load_config_if_exists().ok_or_else(|| {
        let hint = if std::path::Path::new(".env").exists() {
            " (기존 .env 사용자: `rwd init`으로 설정을 마이그레이션하세요)"
        } else {
            ""
        };
        format!("설정 파일이 없습니다. `rwd init`을 먼저 실행해 주세요.{hint}")
    })?;

    let provider = match config.llm.provider.as_str() {
        "openai" => LlmProvider::OpenAi,
        _ => LlmProvider::Anthropic,
    };
    Ok((provider, config.llm.api_key))
}
```

- [ ] **Step 2: output/mod.rs에서 .env fallback 코드 제거**

`load_vault_path()`를 config 전용으로 변경:
```rust
pub fn load_vault_path() -> Result<PathBuf, OutputError> {
    let config = crate::config::load_config_if_exists().ok_or_else(|| {
        let hint = if std::path::Path::new(".env").exists() {
            " (기존 .env 사용자: `rwd init`으로 설정을 마이그레이션하세요)"
        } else {
            ""
        };
        format!("설정 파일이 없습니다. `rwd init`을 먼저 실행해 주세요.{hint}")
    })?;

    let path = PathBuf::from(&config.output.path);
    if !path.exists() {
        std::fs::create_dir_all(&path)?;
    }
    Ok(path)
}
```

- [ ] **Step 3: Cargo.toml에서 dotenvy 제거**

`dotenvy = "0.15"` 줄 삭제.

- [ ] **Step 4: cargo build — dotenvy 참조 남아있으면 제거**

Run: `cargo build`
Expected: 성공 (dotenvy import 남아있으면 컴파일 에러로 알려줌)

- [ ] **Step 5: cargo clippy && cargo test 확인**

Run: `cargo clippy && cargo test`
Expected: 경고 0, 테스트 전체 통과

- [ ] **Step 6: 커밋**

```bash
git add Cargo.toml src/analyzer/provider.rs src/output/mod.rs
git commit -m "refactor: .env/dotenvy 제거 — config.toml 단일 소스로 전환"
```

---

## 완료 조건 체크리스트

- [ ] `rwd init` — API 키 입력, Obsidian vault 자동 감지, `~/.config/rwd/config.toml` 생성
- [ ] `rwd config output-path <경로>` — 출력 경로 변경
- [ ] `rwd config provider <이름>` — LLM 프로바이더 변경
- [ ] `rwd config api-key <키>` — API 키 변경
- [ ] `rwd today` — config.toml 기반으로 정상 동작
- [ ] `.env` 및 `dotenvy` 의존성 완전 제거
- [ ] `cargo build`, `cargo clippy`, `cargo test` 전체 통과
