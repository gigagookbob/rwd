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

/// 업데이트 체크 캐시. ~/.rwd/cache/update-check.json에 저장.
/// chrono의 serde feature를 활용하여 DateTime을 JSON으로 자동 변환합니다 (Cargo.toml에 이미 활성화됨).
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCheckCache {
    /// 마지막 체크 시각. UTC 기준으로 저장하여 타임존 변경에 영향받지 않도록 합니다.
    /// DateTime<Utc>를 사용하는 이유: TTL 비교 시 동일한 타입끼리 뺄셈해야 하기 때문입니다.
    /// chrono의 Sub 구현은 양쪽 타입이 같아야 동작합니다 (DateTime<Utc> - DateTime<Utc>).
    pub checked_at: chrono::DateTime<chrono::Utc>,
    /// 그때 확인한 최신 버전 (예: "0.6.0")
    pub latest_version: String,
}

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
    let home = dirs::home_dir().ok_or(crate::messages::error::HOME_DIR_NOT_FOUND)?;
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

/// 업데이트 체크 캐시 파일 경로: ~/.rwd/cache/update-check.json
/// 기존 cache_path()와 동일한 패턴이지만, 날짜가 아닌 고정 파일명을 사용합니다.
fn update_check_path() -> Result<PathBuf, CacheError> {
    Ok(cache_dir()?.join("update-check.json"))
}

/// 업데이트 체크 캐시를 로드합니다. 파일 없음/손상 시 None을 반환합니다.
/// 기존 load_cache()와 동일한 패턴: 캐시 미스는 정상 동작이므로 Option으로 처리.
pub fn load_update_check() -> Option<UpdateCheckCache> {
    let path = update_check_path().ok()?;
    load_update_check_from(&path)
}

/// 지정 경로에서 업데이트 체크 캐시를 로드합니다. 테스트에서 경로를 주입할 수 있도록 분리.
fn load_update_check_from(path: &std::path::Path) -> Option<UpdateCheckCache> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// 업데이트 체크 캐시를 저장합니다. 기존 save_cache()와 동일한 패턴.
pub fn save_update_check(cache: &UpdateCheckCache) -> Result<(), CacheError> {
    let path = update_check_path()?;
    save_update_check_to(cache, &path)
}

/// 지정 경로에 업데이트 체크 캐시를 저장합니다. 테스트에서 경로를 주입할 수 있도록 분리.
fn save_update_check_to(cache: &UpdateCheckCache, path: &std::path::Path) -> Result<(), CacheError> {
    let json = serde_json::to_string_pretty(cache)?;
    std::fs::write(path, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_check_cache_직렬화_역직렬화_동일_데이터_반환() {
        let cache = UpdateCheckCache {
            checked_at: chrono::Utc::now(),
            latest_version: "0.6.0".to_string(),
        };
        let json = serde_json::to_string_pretty(&cache).expect("직렬화 성공");
        let loaded: UpdateCheckCache = serde_json::from_str(&json).expect("역직렬화 성공");
        assert_eq!(loaded.latest_version, "0.6.0");
    }

    #[test]
    fn test_update_check_save_후_load_동일_데이터_반환() {
        let temp_dir = std::env::temp_dir().join("rwd_test_update_check");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).expect("디렉토리 생성");
        let path = temp_dir.join("update-check.json");

        let cache = UpdateCheckCache {
            checked_at: chrono::Utc::now(),
            latest_version: "0.7.0".to_string(),
        };

        save_update_check_to(&cache, &path).expect("저장 성공");
        let loaded = load_update_check_from(&path);
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().latest_version, "0.7.0");

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_update_check_load_파일없으면_none_반환() {
        let path = std::env::temp_dir().join("rwd_test_nonexistent_update_check.json");
        let _ = std::fs::remove_file(&path);
        let loaded = load_update_check_from(&path);
        assert!(loaded.is_none());
    }
}
