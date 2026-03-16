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
