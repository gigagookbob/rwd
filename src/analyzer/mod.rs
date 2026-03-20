// analyzer 모듈은 파싱된 로그 데이터를 LLM API에 보내 인사이트를 추출하는 역할을 합니다.
// provider.rs의 LlmProvider enum으로 Anthropic, OpenAI 등 여러 프로바이더를 지원합니다.
// parser 모듈과 같은 디렉토리 구조를 사용합니다 (Rust Book Ch.7 참조).

pub mod anthropic;
pub mod insight;
pub mod openai;
pub mod planner;
pub mod prompt;
pub mod provider;
pub mod summarizer;

// parser 모듈과 동일한 에러 타입 패턴을 사용합니다.
// M5에서 thiserror로 전용 에러 타입을 만들 예정입니다.
pub type AnalyzerError = Box<dyn std::error::Error>;

// pub use로 외부에서 자주 사용할 타입들을 상위 모듈에서 바로 접근할 수 있게 합니다.
pub use insight::AnalysisResult;

use crate::parser::claude::LogEntry;
use crate::parser::codex::CodexEntry;
use crate::redactor::RedactResult;
use planner::{ExecutionPlan, StepStrategy};

/// 로그 엔트리들을 분석하여 인사이트를 추출합니다.
/// 이 함수가 M3의 핵심 진입점입니다.
///
/// async fn은 비동기 함수를 선언합니다 (tokio 런타임 위에서 실행).
/// 네트워크 I/O(API 호출) 동안 다른 작업을 처리할 수 있게 해줍니다.
/// 호출 시 .await를 붙여야 실제로 실행됩니다 (Rust Async Book 참조).
///
/// provider::load_provider()로 프로바이더와 API 키를 읽고,
/// 애니메이션 스피너를 시작합니다. 반환된 JoinHandle을 abort()하면 스피너가 멈춥니다.
/// eprint!("\r...")로 같은 줄을 덮어쓰는 방식이라 줄이 밀리지 않습니다.
///
/// Arc<AtomicBool>: 스레드 간 공유 가능한 bool. Ordering::Relaxed는 가장 가벼운 동기화입니다.
/// tokio::spawn: 비동기 태스크를 백그라운드에서 실행합니다.
fn start_spinner(msg: String) -> tokio::task::JoinHandle<()> {
    use std::io::Write;
    tokio::spawn(async move {
        let frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        let mut i = 0;
        loop {
            eprint!("\r{} {}", frames[i % frames.len()], msg);
            let _ = std::io::stderr().flush();
            i += 1;
            tokio::time::sleep(std::time::Duration::from_millis(80)).await;
        }
    })
}

/// 스피너를 멈추고 해당 줄을 지웁니다.
fn stop_spinner(handle: tokio::task::JoinHandle<()>) {
    handle.abort();
    eprint!("\r\x1b[2K");
}

/// 카운트다운 대기: 1초마다 남은 시간을 갱신하여 표시합니다.
async fn countdown_sleep(total_secs: u64) {
    use std::io::Write;
    let frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    let mut i = 0;
    for remaining in (1..=total_secs).rev() {
        // 1초를 80ms 단위로 나눠서 스피너 애니메이션을 계속 돌립니다.
        for _ in 0..12 {
            eprint!("\r{} {}", frames[i % frames.len()], crate::messages::status::countdown_waiting(remaining));
            let _ = std::io::stderr().flush();
            i += 1;
            tokio::time::sleep(std::time::Duration::from_millis(83)).await;
        }
    }
    eprint!("\r\x1b[2K");
}

/// provider.call_api()로 선택된 프로바이더의 API를 호출합니다.
/// 이 함수 자체는 어떤 프로바이더가 사용되는지 알 필요가 없습니다.
pub async fn analyze_entries(
    entries: &[LogEntry],
    redactor_enabled: bool,
    verbose: bool,
) -> Result<(AnalysisResult, RedactResult), AnalyzerError> {
    let (provider, api_key) = provider::load_provider()?;

    // 1. Probe: 사용자의 실제 rate limit 확인
    let sp = start_spinner(crate::messages::status::PROBING_RATE_LIMITS.into());
    let (limits, probed) = provider.probe_rate_limits(&api_key).await;
    stop_spinner(sp);
    if probed {
        eprintln!(
            "{}",
            crate::messages::status::rate_limit_ok(
                limits.input_tokens_per_minute,
                limits.output_tokens_per_minute,
                limits.requests_per_minute,
            )
        );
    } else {
        eprintln!(
            "{}",
            crate::messages::status::rate_limit_fallback(
                limits.input_tokens_per_minute,
                limits.output_tokens_per_minute,
                limits.requests_per_minute,
            )
        );
    }

    // 2. Estimate: 세션별 토큰 추정
    let estimates = prompt::estimate_sessions(entries);

    // 3. Plan: 실행 계획 수립
    let plan = planner::build_execution_plan(&limits, &estimates, provider.max_output_tokens());

    // 4. Display: 계획 출력
    if plan.is_single_shot {
        eprintln!(
            "{}",
            crate::messages::status::plan_single_shot(plan.total_estimated_tokens)
        );
    } else {
        eprintln!(
            "{}",
            crate::messages::status::plan_multi_step(plan.steps.len(), plan.total_estimated_tokens)
        );
        if verbose {
            for step in &plan.steps {
                match &step.strategy {
                    StepStrategy::Direct => {
                        eprintln!(
                            "{}",
                            crate::messages::status::plan_step_direct(
                                &step.session_id,
                                step.estimated_tokens,
                            )
                        );
                    }
                    StepStrategy::Summarize { chunks } => {
                        eprintln!(
                            "{}",
                            crate::messages::status::plan_step_summarize(
                                &step.session_id,
                                step.estimated_tokens,
                                *chunks,
                            )
                        );
                    }
                };
            }
        }
    }

    // 5. Execute — 모든 eprintln 출력이 끝난 후 스피너를 시작합니다.
    if plan.is_single_shot {
        let sp = start_spinner(crate::messages::status::ANALYZING_INSIGHT.into());
        let prompt_text = prompt::build_prompt(entries)?;
        let (final_prompt, redact_result) = if redactor_enabled {
            crate::redactor::redact_text(&prompt_text)
        } else {
            (prompt_text, RedactResult::empty())
        };
        let raw_response = provider.call_api(&api_key, &final_prompt, plan.recommended_max_tokens as u32).await?;
        stop_spinner(sp);
        let result = insight::parse_response(&raw_response)?;
        Ok((result, redact_result))
    } else {
        execute_plan(&plan, entries, &provider, &api_key, redactor_enabled).await
    }
}

/// LogEntry에서 session_id를 추출합니다.
/// SystemEntry는 Option<String>, FileHistorySnapshotEntry는 session_id 없음.
fn entry_session_id(entry: &LogEntry) -> Option<&str> {
    match entry {
        LogEntry::User(e) => Some(&e.session_id),
        LogEntry::Assistant(e) => Some(&e.session_id),
        LogEntry::Progress(e) => Some(&e.session_id),
        LogEntry::System(e) => e.session_id.as_deref(),
        LogEntry::FileHistorySnapshot(_) | LogEntry::Other(_) => None,
    }
}

/// 분석 결과를 기반으로 개발 진척사항 요약을 생성합니다.
///
/// session_summaries: 각 세션의 work_summary를 이어붙인 텍스트.
/// LLM에게 SUMMARY_PROMPT와 함께 전달하여 비개발자도 읽을 수 있는 요약을 생성합니다.
pub async fn analyze_summary(session_summaries: &str) -> Result<String, AnalyzerError> {
    let (provider, api_key) = provider::load_provider()?;
    let raw_response = provider.call_summary_api(&api_key, session_summaries).await?;
    Ok(raw_response)
}

/// 분석 결과를 기반으로 슬랙 공유용 메시지를 생성합니다.
///
/// session_summaries: 각 세션의 work_summary를 이어붙인 텍스트.
/// LLM에게 SLACK_PROMPT와 함께 전달하여 비개발자도 읽을 수 있는 슬랙 메시지를 생성합니다.
pub async fn analyze_slack(session_summaries: &str) -> Result<String, AnalyzerError> {
    let (provider, api_key) = provider::load_provider()?;
    let raw_response = provider.call_slack_api(&api_key, session_summaries).await?;
    Ok(raw_response)
}

/// Codex 세션의 엔트리들을 분석하여 인사이트를 추출합니다.
/// Claude용 analyze_entries()와 동일한 파이프라인이지만, Codex용 프롬프트를 사용합니다.
pub async fn analyze_codex_entries(
    entries: &[CodexEntry],
    session_id: &str,
    redactor_enabled: bool,
) -> Result<(AnalysisResult, RedactResult), AnalyzerError> {
    let (provider, api_key) = provider::load_provider()?;
    let prompt_text = prompt::build_codex_prompt(entries, session_id)?;
    let (final_prompt, redact_result) = if redactor_enabled {
        crate::redactor::redact_text(&prompt_text)
    } else {
        (prompt_text, RedactResult::empty())
    };
    let raw_response = provider.call_api(&api_key, &final_prompt, 1_950).await?;
    let result = insight::parse_response(&raw_response)?;
    Ok((result, redact_result))
}

/// 실행 계획을 받아 순차 실행하고 결과를 병합한다.
async fn execute_plan(
    plan: &ExecutionPlan,
    entries: &[LogEntry],
    provider: &provider::LlmProvider,
    api_key: &str,
    redactor_enabled: bool,
) -> Result<(AnalysisResult, RedactResult), AnalyzerError> {
    let mut results = Vec::new();
    let mut total_redact = RedactResult::empty();
    let total_steps = plan.steps.len();

    for (i, step) in plan.steps.iter().enumerate() {
        let session_entries: Vec<LogEntry> = entries
            .iter()
            .filter(|e| entry_session_id(e) == Some(step.session_id.as_str()))
            .cloned()
            .collect();

        // Direct 스텝은 외부 스피너, Summarize 스텝은 내부에 자체 스피너가 있으므로 생략
        let use_spinner = step.strategy == StepStrategy::Direct;
        let sp = if use_spinner {
            Some(start_spinner(
                crate::messages::status::step_analyzing(i + 1, total_steps, &step.session_id)
            ))
        } else {
            None
        };

        let result = match &step.strategy {
            StepStrategy::Direct => {
                execute_direct_step(
                    &session_entries, provider, api_key, redactor_enabled,
                )
                .await
            }
            StepStrategy::Summarize { .. } => {
                execute_summarize_step(
                    &session_entries,
                    &step.session_id,
                    provider,
                    api_key,
                    &plan.rate_limits,
                    redactor_enabled,
                )
                .await
            }
        };

        match result {
            Ok((analysis, redact)) => {
                if let Some(h) = sp { stop_spinner(h); }
                eprintln!("{}", crate::messages::status::step_done(i + 1, total_steps));
                results.push(analysis);
                total_redact.merge(redact);
            }
            Err(e) => {
                if let Some(h) = sp { stop_spinner(h); }
                let err_msg = e.to_string();
                if err_msg.contains("429") {
                    // 429 rate limit: 60초 카운트다운 후 재시도
                    countdown_sleep(60).await;

                    let retry_sp = if use_spinner {
                        Some(start_spinner(
                            crate::messages::status::step_retrying(i + 1, total_steps, &step.session_id)
                        ))
                    } else {
                        None
                    };

                    let retry = match &step.strategy {
                        StepStrategy::Direct => {
                            execute_direct_step(
                                &session_entries,
                                provider,
                                api_key,
                                redactor_enabled,
                            )
                            .await
                        }
                        StepStrategy::Summarize { .. } => {
                            execute_summarize_step(
                                &session_entries,
                                &step.session_id,
                                provider,
                                api_key,
                                &plan.rate_limits,
                                redactor_enabled,
                            )
                            .await
                        }
                    };

                    match retry {
                        Ok((analysis, redact)) => {
                            if let Some(h) = retry_sp { stop_spinner(h); }
                            eprintln!("{}", crate::messages::status::step_retry_success(i + 1, total_steps));
                            results.push(analysis);
                            total_redact.merge(redact);
                        }
                        Err(retry_err) => {
                            if let Some(h) = retry_sp { stop_spinner(h); }
                            eprintln!(
                                "{}",
                                crate::messages::status::step_skip_retry(i + 1, total_steps, &step.session_id, &retry_err)
                            );
                        }
                    }
                } else if err_msg.contains(crate::messages::error::JSON_PARSE_FAILED_MARKER) {
                    // JSON parse failure: LLM responses are non-deterministic, retry once without waiting
                    let retry_sp = if use_spinner {
                        Some(start_spinner(
                            crate::messages::status::step_reanalyzing(i + 1, total_steps, &step.session_id)
                        ))
                    } else {
                        None
                    };

                    let retry = match &step.strategy {
                        StepStrategy::Direct => {
                            execute_direct_step(
                                &session_entries, provider, api_key, redactor_enabled,
                            )
                            .await
                        }
                        StepStrategy::Summarize { .. } => {
                            execute_summarize_step(
                                &session_entries, &step.session_id,
                                provider, api_key, &plan.rate_limits, redactor_enabled,
                            )
                            .await
                        }
                    };

                    match retry {
                        Ok((analysis, redact)) => {
                            if let Some(h) = retry_sp { stop_spinner(h); }
                            eprintln!("{}", crate::messages::status::step_reanalysis_success(i + 1, total_steps));
                            results.push(analysis);
                            total_redact.merge(redact);
                        }
                        Err(retry_err) => {
                            if let Some(h) = retry_sp { stop_spinner(h); }
                            eprintln!(
                                "{}",
                                crate::messages::status::step_skip_reanalysis(i + 1, total_steps, &step.session_id, &retry_err)
                            );
                        }
                    }
                } else {
                    eprintln!(
                        "{}",
                        crate::messages::status::step_skip(i + 1, total_steps, &step.session_id, &err_msg)
                    );
                }
            }
        }

        // rate pacing: 마지막 스텝이 아니면 카운트다운 대기
        if i + 1 < total_steps {
            let wait =
                summarizer::calculate_wait(step.estimated_tokens, &plan.rate_limits);
            if wait > 0.0 {
                countdown_sleep(wait.ceil() as u64).await;
            }
        }
    }

    if results.is_empty() {
        return Err(crate::messages::error::ALL_SESSIONS_FAILED.into());
    }

    Ok((insight::merge_results(results), total_redact))
}

/// Direct 스텝: 세션 프롬프트를 그대로 전송.
async fn execute_direct_step(
    entries: &[LogEntry],
    provider: &provider::LlmProvider,
    api_key: &str,
    redactor_enabled: bool,
) -> Result<(AnalysisResult, RedactResult), AnalyzerError> {
    let prompt_text = prompt::build_prompt(entries)?;
    let (final_prompt, redact_result) = if redactor_enabled {
        crate::redactor::redact_text(&prompt_text)
    } else {
        (prompt_text, RedactResult::empty())
    };
    let raw_response = provider.call_api(api_key, &final_prompt, 4_096).await?;
    let result = insight::parse_response(&raw_response)?;
    Ok((result, redact_result))
}

/// Summarize 스텝: 대형 세션을 청크별 요약 후 분석.
async fn execute_summarize_step(
    entries: &[LogEntry],
    session_id: &str,
    provider: &provider::LlmProvider,
    api_key: &str,
    limits: &planner::RateLimits,
    redactor_enabled: bool,
) -> Result<(AnalysisResult, RedactResult), AnalyzerError> {
    let messages = prompt::extract_messages(entries);
    let chunks =
        summarizer::split_into_chunks(&messages, limits.input_tokens_per_minute);
    let summary_text =
        summarizer::summarize_chunks(&chunks, provider, api_key, limits).await?;

    let prompt_with_session = format!("[Session: {session_id}]\n{summary_text}");
    let (final_prompt, redact_result) = if redactor_enabled {
        crate::redactor::redact_text(&prompt_with_session)
    } else {
        (prompt_with_session, RedactResult::empty())
    };
    let raw_response = provider.call_api(api_key, &final_prompt, 4_096).await?;
    let result = insight::parse_response(&raw_response)?;
    Ok((result, redact_result))
}
