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
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    pub rate_limits: RateLimits,
    pub steps: Vec<ExecutionStep>,
    pub total_estimated_tokens: u64,
}

/// Builds an execution plan from rate limits and per-session estimates.
///
/// Strategy branches:
/// - individual session <= ITPM -> Direct
/// - individual session > ITPM -> Summarize (chunk and summarize)
pub fn build_execution_plan(
    limits: &RateLimits,
    estimates: &[SessionEstimate],
    _model_max_output: u64,
) -> ExecutionPlan {
    let safe_itpm = limits.input_tokens_per_minute.max(1);
    let total: u64 = estimates.iter().map(|e| e.estimated_tokens).sum();

    let steps: Vec<ExecutionStep> = estimates
        .iter()
        .map(|est| {
            let strategy = if est.estimated_tokens <= safe_itpm {
                StepStrategy::Direct
            } else {
                let chunks = ((est.estimated_tokens as f64) / (safe_itpm as f64)).ceil() as usize;
                StepStrategy::Summarize {
                    chunks: chunks.max(2),
                }
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
    fn test_build_plan_direct_when_sessions_fit_individually() {
        let limits = RateLimits {
            input_tokens_per_minute: 30_000,
            output_tokens_per_minute: 8_000,
            requests_per_minute: 50,
        };
        let estimates = vec![
            SessionEstimate {
                session_id: "s1".into(),
                estimated_tokens: 10_000,
            },
            SessionEstimate {
                session_id: "s2".into(),
                estimated_tokens: 20_000,
            },
        ];
        let plan = build_execution_plan(&limits, &estimates, 32_000);
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
        let estimates = vec![SessionEstimate {
            session_id: "s1".into(),
            estimated_tokens: 50_000,
        }];
        let plan = build_execution_plan(&limits, &estimates, 32_000);
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(
            plan.steps[0].strategy,
            StepStrategy::Summarize { chunks: 2 }
        );
    }

    #[test]
    fn test_build_plan_total_estimated_tokens_is_sum() {
        let limits = RateLimits::default_generous();
        let estimates = vec![
            SessionEstimate {
                session_id: "s1".into(),
                estimated_tokens: 5_000,
            },
            SessionEstimate {
                session_id: "s2".into(),
                estimated_tokens: 7_000,
            },
        ];
        let plan = build_execution_plan(&limits, &estimates, 32_000);
        assert_eq!(plan.total_estimated_tokens, 12_000);
    }
}
