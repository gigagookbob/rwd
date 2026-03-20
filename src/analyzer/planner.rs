// Rate-limit-aware execution planning module.
//
// Builds an ExecutionPlan from probed RateLimits and per-session token estimates.

/// API rate limit information.
/// Extracted from probe response headers, or default_generous() on failure.
#[derive(Debug, Clone)]
pub struct RateLimits {
    pub input_tokens_per_minute: u64,
    pub output_tokens_per_minute: u64,
    pub requests_per_minute: u64,
}

impl RateLimits {
    /// Generous defaults used when probing fails.
    /// Most users will proceed with single_shot; the runtime safety net handles actual limits.
    pub fn default_generous() -> Self {
        Self {
            input_tokens_per_minute: 1_000_000,
            output_tokens_per_minute: 200_000,
            requests_per_minute: 1_000,
        }
    }
}

/// Per-session token estimate.
#[derive(Debug, Clone)]
pub struct SessionEstimate {
    pub session_id: String,
    pub estimated_tokens: u64,
}

/// Strategy for an individual execution step.
#[derive(Debug, Clone, PartialEq)]
pub enum StepStrategy {
    /// Within ITPM -- send as-is.
    Direct,
    /// Exceeds ITPM -- split into chunks and summarize.
    Summarize { chunks: usize },
}

/// A single step in the execution plan.
#[derive(Debug, Clone)]
pub struct ExecutionStep {
    pub session_id: String,
    pub strategy: StepStrategy,
    pub estimated_tokens: u64,
}

/// Overall execution plan.
/// When is_single_shot is true, all sessions are sent in one API call (no overhead for high-tier users).
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    pub rate_limits: RateLimits,
    pub steps: Vec<ExecutionStep>,
    pub total_estimated_tokens: u64,
    pub is_single_shot: bool,
    /// Dynamic max_tokens value for API calls.
    pub recommended_max_tokens: u64,
}

/// Token budget reserved for the analyze_summary() call.
const SUMMARY_BUDGET_TOKENS: u64 = 5_000;

/// Estimated output tokens per session (conservative: ~1500 based on real-world measurements).
const OUTPUT_TOKENS_PER_SESSION: u64 = 1_500;

/// Output margin ratio (30%).
const OUTPUT_MARGIN: f64 = 1.3;

/// Builds an execution plan from rate limits and per-session estimates.
///
/// Strategy branches:
/// - total + budget <= ITPM AND estimated output <= model_max_output -> single_shot
/// - individual session <= ITPM -> Direct (sequential per-session)
/// - individual session > ITPM -> Summarize (chunk and summarize)
pub fn build_execution_plan(
    limits: &RateLimits,
    estimates: &[SessionEstimate],
    model_max_output: u64,
) -> ExecutionPlan {
    let itpm = limits.input_tokens_per_minute;
    let total: u64 = estimates.iter().map(|e| e.estimated_tokens).sum();
    let num_sessions = estimates.len() as u64;

    let estimated_output = (num_sessions * OUTPUT_TOKENS_PER_SESSION) as f64 * OUTPUT_MARGIN;
    let recommended_max_tokens = (estimated_output as u64).min(model_max_output);

    // Single-shot condition: both input AND output fit within limits.
    if total + SUMMARY_BUDGET_TOKENS <= itpm
        && (estimated_output as u64) <= model_max_output
    {
        return ExecutionPlan {
            rate_limits: limits.clone(),
            steps: Vec::new(),
            total_estimated_tokens: total,
            is_single_shot: true,
            recommended_max_tokens,
        };
    }

    // Per-session strategy selection.
    let steps: Vec<ExecutionStep> = estimates
        .iter()
        .map(|est| {
            let strategy = if est.estimated_tokens <= itpm {
                StepStrategy::Direct
            } else {
                let chunks = ((est.estimated_tokens as f64) / (itpm as f64)).ceil() as usize;
                StepStrategy::Summarize { chunks: chunks.max(2) }
            };
            ExecutionStep {
                session_id: est.session_id.clone(),
                strategy,
                estimated_tokens: est.estimated_tokens,
            }
        })
        .collect();

    ExecutionPlan {
        rate_limits: limits.clone(),
        steps,
        total_estimated_tokens: total,
        is_single_shot: false,
        recommended_max_tokens,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_generous_returns_high_limits() {
        let limits = RateLimits::default_generous();
        assert_eq!(limits.input_tokens_per_minute, 1_000_000);
        assert_eq!(limits.output_tokens_per_minute, 200_000);
        assert_eq!(limits.requests_per_minute, 1_000);
    }

    #[test]
    fn test_build_plan_single_shot_when_total_fits() {
        let limits = RateLimits {
            input_tokens_per_minute: 100_000,
            output_tokens_per_minute: 50_000,
            requests_per_minute: 100,
        };
        let estimates = vec![
            SessionEstimate { session_id: "s1".into(), estimated_tokens: 10_000 },
            SessionEstimate { session_id: "s2".into(), estimated_tokens: 20_000 },
        ];
        let plan = build_execution_plan(&limits, &estimates, 32_000);
        assert!(plan.is_single_shot);
        assert!(plan.steps.is_empty());
        assert_eq!(plan.total_estimated_tokens, 30_000);
    }

    #[test]
    fn test_build_plan_direct_when_sessions_fit_individually() {
        let limits = RateLimits {
            input_tokens_per_minute: 30_000,
            output_tokens_per_minute: 8_000,
            requests_per_minute: 50,
        };
        let estimates = vec![
            SessionEstimate { session_id: "s1".into(), estimated_tokens: 10_000 },
            SessionEstimate { session_id: "s2".into(), estimated_tokens: 20_000 },
        ];
        let plan = build_execution_plan(&limits, &estimates, 32_000);
        assert!(!plan.is_single_shot);
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].strategy, StepStrategy::Direct);
        assert_eq!(plan.steps[1].strategy, StepStrategy::Direct);
    }

    #[test]
    fn test_build_plan_summarize_when_session_exceeds_itpm() {
        let limits = RateLimits {
            input_tokens_per_minute: 30_000,
            output_tokens_per_minute: 8_000,
            requests_per_minute: 50,
        };
        let estimates = vec![
            SessionEstimate { session_id: "s1".into(), estimated_tokens: 50_000 },
        ];
        let plan = build_execution_plan(&limits, &estimates, 32_000);
        assert!(!plan.is_single_shot);
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].strategy, StepStrategy::Summarize { chunks: 2 });
    }

    #[test]
    fn test_build_plan_default_generous_is_single_shot() {
        let limits = RateLimits::default_generous();
        let estimates = vec![
            SessionEstimate { session_id: "s1".into(), estimated_tokens: 50_000 },
        ];
        let plan = build_execution_plan(&limits, &estimates, 32_000);
        assert!(plan.is_single_shot);
    }

    #[test]
    fn test_build_plan_reserves_summary_budget() {
        let limits = RateLimits {
            input_tokens_per_minute: 35_000,
            output_tokens_per_minute: 8_000,
            requests_per_minute: 50,
        };
        let estimates = vec![
            SessionEstimate { session_id: "s1".into(), estimated_tokens: 31_000 },
        ];
        let plan = build_execution_plan(&limits, &estimates, 32_000);
        assert!(!plan.is_single_shot);
    }

    #[test]
    fn test_build_plan_exact_boundary_is_single_shot() {
        let limits = RateLimits {
            input_tokens_per_minute: 35_000,
            output_tokens_per_minute: 8_000,
            requests_per_minute: 50,
        };
        let estimates = vec![
            SessionEstimate { session_id: "s1".into(), estimated_tokens: 30_000 },
        ];
        let plan = build_execution_plan(&limits, &estimates, 32_000);
        assert!(plan.is_single_shot);
    }

    #[test]
    fn test_single_shot_blocked_by_output_limit() {
        let limits = RateLimits::default_generous();
        let estimates: Vec<SessionEstimate> = (0..22)
            .map(|i| SessionEstimate {
                session_id: format!("s{i}"),
                estimated_tokens: 10_000,
            })
            .collect();
        let plan = build_execution_plan(&limits, &estimates, 32_000);
        assert!(!plan.is_single_shot);
        assert!(!plan.steps.is_empty());
    }

    #[test]
    fn test_single_shot_allowed_when_output_fits() {
        let limits = RateLimits::default_generous();
        let estimates = vec![
            SessionEstimate { session_id: "s1".into(), estimated_tokens: 50_000 },
            SessionEstimate { session_id: "s2".into(), estimated_tokens: 30_000 },
        ];
        let plan = build_execution_plan(&limits, &estimates, 32_000);
        assert!(plan.is_single_shot);
    }

    #[test]
    fn test_recommended_max_tokens_scales_with_sessions() {
        let limits = RateLimits::default_generous();
        let estimates: Vec<SessionEstimate> = (0..10)
            .map(|i| SessionEstimate {
                session_id: format!("s{i}"),
                estimated_tokens: 5_000,
            })
            .collect();
        let plan = build_execution_plan(&limits, &estimates, 32_000);
        assert_eq!(plan.recommended_max_tokens, 19_500);
    }
}
