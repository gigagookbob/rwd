// Converts analysis results to Markdown and saves them to the Obsidian vault.

pub mod markdown;

// TODO: Replace with a dedicated error type via thiserror.
pub type OutputError = Box<dyn std::error::Error>;

pub use markdown::render_combined_markdown;

use std::path::{Path, PathBuf};

use chrono::NaiveDate;

/// Loads the Obsidian vault path from config (~/.config/rwd/config.toml).
pub fn load_vault_path() -> Result<PathBuf, OutputError> {
    let config = crate::config::load_config_if_exists().ok_or(crate::messages::error::NO_CONFIG)?;

    let path = PathBuf::from(&config.output.path);
    if !path.exists() {
        std::fs::create_dir_all(&path)?;
    }
    Ok(path)
}

/// Saves Markdown content to the output path with a date-based filename.
/// The output.path is expected to be the final directory (e.g., vault/Daily),
/// so no additional subdirectories are appended.
pub fn save_to_vault(
    vault_path: &Path,
    date: NaiveDate,
    content: &str,
) -> Result<PathBuf, OutputError> {
    std::fs::create_dir_all(vault_path)?;

    let filename = format!("{date}.md");
    let file_path = vault_path.join(&filename);

    std::fs::write(&file_path, content)?;

    Ok(file_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_to_vault_creates_file() {
        let temp_dir = std::env::temp_dir().join("rwd_test_output");
        std::fs::create_dir_all(&temp_dir).expect("failed to create temp dir");

        let date = NaiveDate::from_ymd_opt(2026, 3, 11).expect("valid date");
        let content = "# 테스트 Markdown";

        let result = save_to_vault(&temp_dir, date, content);
        assert!(result.is_ok());

        let saved_path = result.expect("save succeeded");
        assert!(saved_path.exists());
        assert!(saved_path.starts_with(&temp_dir));
        assert_eq!(
            std::fs::read_to_string(&saved_path).expect("read file"),
            content
        );

        std::fs::remove_file(&saved_path).ok();
        std::fs::remove_dir(&temp_dir).ok();
    }
}
