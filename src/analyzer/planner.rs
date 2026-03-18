// Rate Limit 인식 실행 계획 모듈.
//
// API probe 결과(RateLimits)와 세션별 토큰 추정(SessionEstimate)을 기반으로
// 실행 전략(ExecutionPlan)을 수립합니다.

/// API rate limit 정보.
/// probe 호출의 응답 헤더에서 추출하거나, 실패 시 default_generous()를 사용한다.
#[derive(Debug, Clone)]
pub struct RateLimits {
    pub input_tokens_per_minute: u64,
    pub output_tokens_per_minute: u64,
    pub requests_per_minute: u64,
}

impl RateLimits {
    /// probe 실패 시 사용하는 관대한 기본값.
    /// 대부분의 사용자가 single_shot으로 진행하게 되며,
    /// 실제 제한에 걸리면 런타임 안전망이 처리한다.
    pub fn default_generous() -> Self {
        Self {
            input_tokens_per_minute: 1_000_000,
            output_tokens_per_minute: 200_000,
            requests_per_minute: 1_000,
        }
    }
}

/// 세션별 토큰 추정 결과.
#[derive(Debug, Clone)]
pub struct SessionEstimate {
    pub session_id: String,
    pub estimated_tokens: u64,
    pub entry_count: usize,
}

/// 개별 실행 스텝의 전략.
#[derive(Debug, Clone, PartialEq)]
pub enum StepStrategy {
    /// ITPM 이내 — 그대로 전송
    Direct,
    /// ITPM 초과 — 청크 분할 후 요약
    Summarize { chunks: usize },
}

/// 실행 계획의 개별 스텝.
#[derive(Debug, Clone)]
pub struct ExecutionStep {
    pub session_id: String,
    pub strategy: StepStrategy,
    pub estimated_tokens: u64,
}

/// 전체 실행 계획.
/// is_single_shot이면 기존처럼 한 번에 전송 (높은 tier에서 오버헤드 없음).
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    pub rate_limits: RateLimits,
    pub steps: Vec<ExecutionStep>,
    pub total_estimated_tokens: u64,
    pub is_single_shot: bool,
}

/// analyze_summary() 호출을 위해 예약하는 토큰 여유분.
const SUMMARY_BUDGET_TOKENS: u64 = 5_000;

/// rate limit과 세션별 추정치를 기반으로 실행 계획을 수립한다.
///
/// 전략 분기:
/// - 전체 합계 + 여유분 ≤ ITPM → single_shot (한 번에 전송)
/// - 개별 세션 ≤ ITPM → Direct (세션별 순차)
/// - 개별 세션 > ITPM → Summarize (청크 분할 후 요약)
pub fn build_execution_plan(
    limits: &RateLimits,
    estimates: &[SessionEstimate],
) -> ExecutionPlan {
    let itpm = limits.input_tokens_per_minute;
    let total: u64 = estimates.iter().map(|e| e.estimated_tokens).sum();

    // 전체가 ITPM 안에 들어가면 single_shot
    if total + SUMMARY_BUDGET_TOKENS <= itpm {
        return ExecutionPlan {
            rate_limits: limits.clone(),
            steps: Vec::new(),
            total_estimated_tokens: total,
            is_single_shot: true,
        };
    }

    // 세션별로 전략 결정
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
            SessionEstimate { session_id: "s1".into(), estimated_tokens: 10_000, entry_count: 5 },
            SessionEstimate { session_id: "s2".into(), estimated_tokens: 20_000, entry_count: 10 },
        ];
        let plan = build_execution_plan(&limits, &estimates);
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
            SessionEstimate { session_id: "s1".into(), estimated_tokens: 10_000, entry_count: 5 },
            SessionEstimate { session_id: "s2".into(), estimated_tokens: 20_000, entry_count: 10 },
        ];
        let plan = build_execution_plan(&limits, &estimates);
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
            SessionEstimate { session_id: "s1".into(), estimated_tokens: 50_000, entry_count: 20 },
        ];
        let plan = build_execution_plan(&limits, &estimates);
        assert!(!plan.is_single_shot);
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].strategy, StepStrategy::Summarize { chunks: 2 });
    }

    #[test]
    fn test_build_plan_default_generous_is_single_shot() {
        let limits = RateLimits::default_generous();
        let estimates = vec![
            SessionEstimate { session_id: "s1".into(), estimated_tokens: 50_000, entry_count: 20 },
        ];
        let plan = build_execution_plan(&limits, &estimates);
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
            SessionEstimate { session_id: "s1".into(), estimated_tokens: 31_000, entry_count: 15 },
        ];
        let plan = build_execution_plan(&limits, &estimates);
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
            SessionEstimate { session_id: "s1".into(), estimated_tokens: 30_000, entry_count: 15 },
        ];
        let plan = build_execution_plan(&limits, &estimates);
        assert!(plan.is_single_shot);
    }
}
