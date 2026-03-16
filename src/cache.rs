// cache 모듈은 today 분석 결과를 캐싱하여 LLM 호출을 줄이는 역할을 합니다.
// 엔트리 수가 변하지 않으면 이전 분석 결과를 재사용합니다.
//
// 캐시 파일 위치: ~/.rwd/cache/today-YYYY-MM-DD.json
// serde_json으로 직렬화/역직렬화합니다 (Rust Book Ch.8 참조).

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::analyzer::insight::AnalysisResult;

pub type CacheError = Box<dyn std::error::Error>;

/// 캐시 파일에 저장되는 데이터.
/// date: "YYYY-MM-DD" 문자열로 저장합니다.
/// sources: (소스 이름, AnalysisResult) 튜플의 Vec입니다.
#[derive(Debug, Serialize, Deserialize)]
pub struct TodayCache {
    pub date: String,
    pub claude_entry_count: usize,
    pub codex_session_count: usize,
    /// 소스별 분석 결과. (소스 이름, AnalysisResult) 튜플.
    pub sources: Vec<(String, AnalysisResult)>,
}

/// 캐시 디렉토리 경로: ~/.rwd/cache/
///
/// dirs::home_dir()은 OS별 홈 디렉토리를 반환합니다.
/// Option::ok_or()는 None일 때 에러로 변환합니다 (Rust Book Ch.9 참조).
fn cache_dir() -> Result<PathBuf, CacheError> {
    let home = dirs::home_dir().ok_or("홈 디렉토리를 찾을 수 없습니다")?;
    let dir = home.join(".rwd").join("cache");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// 캐시 파일 경로: ~/.rwd/cache/today-YYYY-MM-DD.json
fn cache_path(date: NaiveDate) -> Result<PathBuf, CacheError> {
    Ok(cache_dir()?.join(format!("today-{date}.json")))
}

/// 캐시를 로드합니다. 파일이 없거나 파싱 실패 시 None을 반환합니다.
///
/// Option<T>를 반환하는 이유: 캐시 미스는 정상 동작이므로 에러가 아닙니다.
/// .ok()?는 Result를 Option으로 변환하고, None이면 함수에서 None을 반환합니다.
pub fn load_cache(date: NaiveDate) -> Option<TodayCache> {
    let path = cache_path(date).ok()?;
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// 캐시를 저장합니다.
///
/// serde_json::to_string_pretty()는 들여쓰기된 JSON을 생성합니다.
/// 디버깅 시 파일을 직접 읽기 쉽게 하기 위해 pretty print를 사용합니다.
pub fn save_cache(cache: &TodayCache, date: NaiveDate) -> Result<(), CacheError> {
    let path = cache_path(date)?;
    let json = serde_json::to_string_pretty(cache)?;
    std::fs::write(&path, json)?;
    Ok(())
}
