// Codex CLI non-interactive provider backend.
//
// Runs `codex exec` with JSONL output and parses the final agent message.

use serde::Deserialize;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use super::{AnalyzerError, ApiUsage};

/// Calls Codex for JSON analysis output.
pub async fn call_codex_json_api(
    system_prompt: &str,
    conversation_text: &str,
    max_tokens: u32,
    model: &str,
    reasoning_effort: &str,
) -> Result<(String, ApiUsage), AnalyzerError> {
    let prompt = compose_prompt(system_prompt, conversation_text);
    run_codex_exec(&prompt, None, max_tokens, model, reasoning_effort)
}

/// Calls Codex for plain-text output (summary/slack/chunk summarize).
pub async fn call_codex_text_api(
    system_prompt: &str,
    conversation_text: &str,
    max_tokens: u32,
    model: &str,
    reasoning_effort: &str,
) -> Result<(String, ApiUsage), AnalyzerError> {
    let prompt = compose_prompt(system_prompt, conversation_text);
    run_codex_exec(&prompt, None, max_tokens, model, reasoning_effort)
}

fn compose_prompt(system_prompt: &str, conversation_text: &str) -> String {
    format!("[System Instructions]\n{system_prompt}\n\n[Conversation]\n{conversation_text}")
}

fn run_codex_exec(
    prompt: &str,
    schema_path: Option<&Path>,
    max_tokens: u32,
    model: &str,
    reasoning_effort: &str,
) -> Result<(String, ApiUsage), AnalyzerError> {
    let temp_dir = std::env::temp_dir();

    let mut cmd = Command::new("codex");
    cmd.arg("exec")
        .arg("--json")
        .arg("--color")
        .arg("never")
        .arg("--ephemeral")
        .arg("--skip-git-repo-check")
        .arg("-s")
        .arg("read-only")
        .arg("-C")
        .arg(&temp_dir)
        .arg("--model")
        .arg(model)
        .arg("-c")
        .arg(format!("model_reasoning_effort={reasoning_effort}"))
        .arg("-c")
        .arg(format!("model_max_output_tokens={max_tokens}"));

    if let Some(path) = schema_path {
        cmd.arg("--output-schema").arg(path);
    }

    cmd.arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn()?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(prompt.as_bytes())?;
    }
    let output = child.wait_with_output()?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = summarize_error(&stdout, &stderr);
        return Err(format!("Codex exec failed ({}): {detail}", output.status).into());
    }

    let stdout = String::from_utf8(output.stdout)?;
    parse_jsonl_events(&stdout)
}

fn summarize_error(stdout: &str, stderr: &str) -> String {
    if let Some(message) = summarize_stdout_error(stdout) {
        return message;
    }

    stderr
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .unwrap_or_else(|| "unknown error".to_string())
}

fn summarize_stdout_error(stdout: &str) -> Option<String> {
    for line in stdout.lines().rev() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(event_type) = value.get("type").and_then(|t| t.as_str()) else {
            continue;
        };
        if event_type == "turn.failed"
            && let Some(message) = value
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
        {
            return Some(message.trim().to_string());
        }
        if event_type == "error"
            && let Some(message) = value.get("message").and_then(|m| m.as_str())
        {
            return Some(message.trim().to_string());
        }
    }
    None
}

fn parse_jsonl_events(stdout: &str) -> Result<(String, ApiUsage), AnalyzerError> {
    let mut last_message: Option<String> = None;
    let mut usage = ApiUsage::default();

    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(event) = serde_json::from_str::<ExecEvent>(line) else {
            continue;
        };

        if event.event_type == "item.completed"
            && let Some(item) = event.item
            && item.item_type == "agent_message"
            && let Some(text) = item.text
        {
            last_message = Some(text);
        }

        if event.event_type == "turn.completed"
            && let Some(u) = event.usage
        {
            usage.input_tokens = u.input_tokens;
            usage.output_tokens = u.output_tokens;
        }
    }

    let message = last_message.ok_or("Codex exec finished without agent_message output")?;
    Ok((message, usage))
}

#[derive(Deserialize)]
struct ExecEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    item: Option<ExecItem>,
    #[serde(default)]
    usage: Option<ExecUsage>,
}

#[derive(Deserialize)]
struct ExecItem {
    #[serde(rename = "type")]
    item_type: String,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Deserialize)]
struct ExecUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_jsonl_events_extracts_agent_message_and_usage() {
        let output = r#"{"type":"thread.started","thread_id":"t1"}
{"type":"turn.started"}
{"type":"item.completed","item":{"id":"i1","type":"agent_message","text":"{\"sessions\":[]}"}}
{"type":"turn.completed","usage":{"input_tokens":10,"output_tokens":20}}"#;

        let (message, usage) = parse_jsonl_events(output).expect("parse");
        assert_eq!(message, "{\"sessions\":[]}");
        assert_eq!(usage.input_tokens, 10);
        assert_eq!(usage.output_tokens, 20);
    }

    #[test]
    fn test_parse_jsonl_events_ignores_non_json_lines() {
        let output = r#"not-json
{"type":"item.completed","item":{"id":"i1","type":"agent_message","text":"ok"}}
{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":2}}"#;

        let (message, usage) = parse_jsonl_events(output).expect("parse");
        assert_eq!(message, "ok");
        assert_eq!(usage.input_tokens, 1);
        assert_eq!(usage.output_tokens, 2);
    }

    #[test]
    fn test_parse_jsonl_events_missing_agent_message_returns_error() {
        let output = r#"{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":2}}"#;
        let result = parse_jsonl_events(output);
        assert!(result.is_err());
    }
}
