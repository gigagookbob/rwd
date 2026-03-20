// Caches today's analysis results to avoid redundant LLM calls.
// Reuses previous results when entry count hasn't changed.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::analyzer::insight::AnalysisResult;

pub type CacheError = Box<dyn std::error::Error>;

/// Update check cache. Stored at ~/.rwd/cache/update-check.json.
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCheckCache {
    pub checked_at: chrono::DateTime<chrono::Utc>,
    /// Latest version found at check time (e.g., "0.6.0")
    pub latest_version: String,
}

/// Cached analysis data for a single day.
#[derive(Debug, Serialize, Deserialize)]
pub struct TodayCache {
    pub date: String,
    pub claude_entry_count: usize,
    pub codex_session_count: usize,
    /// Per-source analysis results.
    pub sources: Vec<(String, AnalysisResult)>,
}

/// Returns cache directory path: ~/.rwd/cache/
fn cache_dir() -> Result<PathBuf, CacheError> {
    let home = dirs::home_dir().ok_or(crate::messages::error::HOME_DIR_NOT_FOUND)?;
    let dir = home.join(".rwd").join("cache");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Cache file path: ~/.rwd/cache/today-YYYY-MM-DD.json
fn cache_path(date: NaiveDate) -> Result<PathBuf, CacheError> {
    Ok(cache_dir()?.join(format!("today-{date}.json")))
}

/// Loads cache. Returns None on missing file or parse failure (cache miss is normal).
pub fn load_cache(date: NaiveDate) -> Option<TodayCache> {
    let path = cache_path(date).ok()?;
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Saves cache as pretty-printed JSON for easy debugging.
pub fn save_cache(cache: &TodayCache, date: NaiveDate) -> Result<(), CacheError> {
    let path = cache_path(date)?;
    let json = serde_json::to_string_pretty(cache)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// Update check cache path: ~/.rwd/cache/update-check.json
fn update_check_path() -> Result<PathBuf, CacheError> {
    Ok(cache_dir()?.join("update-check.json"))
}

/// Loads update check cache. Returns None on missing/corrupted file.
pub fn load_update_check() -> Option<UpdateCheckCache> {
    let path = update_check_path().ok()?;
    load_update_check_from(&path)
}

/// Loads update check cache from a specific path (for testability).
fn load_update_check_from(path: &std::path::Path) -> Option<UpdateCheckCache> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Saves update check cache.
pub fn save_update_check(cache: &UpdateCheckCache) -> Result<(), CacheError> {
    let path = update_check_path()?;
    save_update_check_to(cache, &path)
}

/// Saves update check cache to a specific path (for testability).
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
