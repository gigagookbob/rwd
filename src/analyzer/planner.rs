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
}
