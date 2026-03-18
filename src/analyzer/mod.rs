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
/// provider.call_api()로 선택된 프로바이더의 API를 호출합니다.
/// 이 함수 자체는 어떤 프로바이더가 사용되는지 알 필요가 없습니다.
pub async fn analyze_entries(
    entries: &[LogEntry],
    redactor_enabled: bool,
) -> Result<(AnalysisResult, RedactResult), AnalyzerError> {
    let (provider, api_key) = provider::load_provider()?;

    // 1. Probe: 사용자의 실제 rate limit 확인
    // eprint!(개행 없음)로 출력 후, 결과를 \r로 같은 줄에 덮어씁니다.
    eprint!("⠋ API 한도 확인 중...");
    let (limits, probed) = provider.probe_rate_limits(&api_key).await;
    if probed {
        eprintln!(
            "\r✓ ITPM: {} | OTPM: {} | RPM: {}",
            limits.input_tokens_per_minute,
            limits.output_tokens_per_minute,
            limits.requests_per_minute,
        );
    } else {
        eprintln!(
            "\r⚠ rate limit 확인 실패, 기본값으로 진행합니다. (ITPM: {} | OTPM: {} | RPM: {})",
            limits.input_tokens_per_minute,
            limits.output_tokens_per_minute,
            limits.requests_per_minute,
        );
    }

    // 2. Estimate: 세션별 토큰 추정
    let estimates = prompt::estimate_sessions(entries);

    // 3. Plan: 실행 계획 수립
    let plan = planner::build_execution_plan(&limits, &estimates);

    // 4. Display: 계획 출력
    if plan.is_single_shot {
        eprintln!(
            "✓ 전체 로그를 한 번에 분석합니다 (추정 {}토큰)",
            plan.total_estimated_tokens
        );
    } else {
        eprintln!(
            "✓ 세션 {}개 분석 예정 (총 {} 토큰 추정)",
            plan.steps.len(),
            plan.total_estimated_tokens
        );
        for step in &plan.steps {
            let strategy_desc = match &step.strategy {
                StepStrategy::Direct => "직접 분석".to_string(),
                StepStrategy::Summarize { chunks } => {
                    format!("요약 후 분석 ({chunks} 청크)")
                }
            };
            eprintln!(
                "  • {}: {} 토큰 → {}",
                step.session_id, step.estimated_tokens, strategy_desc
            );
        }
    }

    // 5. Execute
    if plan.is_single_shot {
        let prompt_text = prompt::build_prompt(entries)?;
        let (final_prompt, redact_result) = if redactor_enabled {
            crate::redactor::redact_text(&prompt_text)
        } else {
            (prompt_text, RedactResult::empty())
        };
        let raw_response = provider.call_api(&api_key, &final_prompt).await?;
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
    let raw_response = provider.call_api(&api_key, &final_prompt).await?;
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
        eprintln!(
            "⠋ [{}/{}] {} 분석 중...",
            i + 1,
            total_steps,
            step.session_id
        );

        let session_entries: Vec<LogEntry> = entries
            .iter()
            .filter(|e| entry_session_id(e) == Some(step.session_id.as_str()))
            .cloned()
            .collect();

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
                eprintln!("✓ [{}/{}] 완료", i + 1, total_steps);
                results.push(analysis);
                total_redact.merge(redact);
            }
            Err(e) => {
                let err_msg = e.to_string();
                if err_msg.contains("429") {
                    eprintln!(
                        "⚠ [{}/{}] rate limit 초과, 60초 대기 후 재시도...",
                        i + 1,
                        total_steps
                    );
                    tokio::time::sleep(
                        std::time::Duration::from_secs(60),
                    )
                    .await;

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
                            eprintln!(
                                "✓ [{}/{}] 재시도 성공",
                                i + 1,
                                total_steps
                            );
                            results.push(analysis);
                            total_redact.merge(redact);
                        }
                        Err(retry_err) => {
                            eprintln!(
                                "⚠ [{}/{}] {} 스킵 (재시도 실패): {}",
                                i + 1,
                                total_steps,
                                step.session_id,
                                retry_err
                            );
                        }
                    }
                } else {
                    eprintln!(
                        "⚠ [{}/{}] {} 스킵: {}",
                        i + 1,
                        total_steps,
                        step.session_id,
                        err_msg
                    );
                }
            }
        }

        // rate pacing: 마지막 스텝이 아니면 대기
        if i + 1 < total_steps {
            let wait =
                summarizer::calculate_wait(step.estimated_tokens, &plan.rate_limits);
            if wait > 0.0 {
                eprintln!("⠋ 다음 요청까지 대기 중... ({:.0}초)", wait);
                tokio::time::sleep(std::time::Duration::from_secs_f64(wait)).await;
            }
        }
    }

    if results.is_empty() {
        return Err("모든 세션의 분석에 실패했습니다.".into());
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
    let raw_response = provider.call_api(api_key, &final_prompt).await?;
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
    let raw_response = provider.call_api(api_key, &final_prompt).await?;
    let result = insight::parse_response(&raw_response)?;
    Ok((result, redact_result))
}
