//! Exponential backoff policy for upload retries.
//!
//! Pure, deterministic-enough helpers used by the retry path to decide how
//! long to wait before the next attempt. Jitter is the only nondeterministic
//! element and is bounded to ±10% so tests can assert tight ranges.

use std::time::Duration;

/// Retry policy parameters, built from user configuration.
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    /// Base delay applied before the first retry.
    pub base: Duration,
    /// Maximum delay the exponential growth is clamped to.
    pub cap: Duration,
    /// Number of attempts allowed before a study is marked terminally failed.
    pub max_attempts: u32,
}

impl RetryPolicy {
    /// Build a policy from raw config seconds/counts, guarding against
    /// nonsensical zero values that would otherwise busy-loop the scheduler.
    pub fn from_config(base_seconds: u64, cap_seconds: u64, max_attempts: u32) -> Self {
        let base = Duration::from_secs(base_seconds.max(1));
        let cap = Duration::from_secs(cap_seconds.max(base_seconds.max(1)));
        Self {
            base,
            cap,
            max_attempts: max_attempts.max(1),
        }
    }

    /// Whether a study that has already made `attempt_no` attempts should be
    /// retried again (`true`) or marked terminally `FAILED` (`false`).
    pub fn should_retry(&self, attempt_no: u32) -> bool {
        attempt_no < self.max_attempts
    }
}

/// Delay before the retry that follows `attempt_no` (1-based: `attempt_no = 1`
/// is the delay after the first failed attempt). Grows as
/// `base * 2^(attempt_no - 1)`, clamped to `cap`, with ±10% jitter.
pub fn next_retry_delay(attempt_no: u32, policy: &RetryPolicy) -> Duration {
    let exponent = attempt_no.saturating_sub(1).min(32);
    let factor = 1u64.checked_shl(exponent).unwrap_or(u64::MAX);

    let base_ms = policy.base.as_millis() as u64;
    let cap_ms = policy.cap.as_millis() as u64;
    let raw_ms = base_ms.saturating_mul(factor).min(cap_ms);

    Duration::from_millis(apply_jitter(raw_ms))
}

/// Apply ±10% jitter to a millisecond delay to avoid a thundering herd of
/// retries all firing on the same tick after an outage.
fn apply_jitter(delay_ms: u64) -> u64 {
    if delay_ms == 0 {
        return 0;
    }
    let span = (delay_ms / 10).max(1); // 10% of the delay
    // Cheap, dependency-free pseudo-randomness seeded from the wall clock.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    let offset = nanos % (2 * span + 1); // 0..=2*span
    (delay_ms + offset).saturating_sub(span) // delay ± span
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> RetryPolicy {
        RetryPolicy::from_config(30, 3600, 10)
    }

    #[test]
    fn grows_exponentially_within_jitter_bounds() {
        let p = policy();
        // attempt 1 -> ~30s, attempt 2 -> ~60s, attempt 3 -> ~120s
        for (attempt, expected_secs) in [(1u32, 30u64), (2, 60), (3, 120)] {
            let d = next_retry_delay(attempt, &p).as_millis() as u64;
            let expected_ms = expected_secs * 1000;
            let span = expected_ms / 10 + 1;
            assert!(
                d >= expected_ms - span && d <= expected_ms + span,
                "attempt {attempt}: {d}ms not within ±10% of {expected_ms}ms"
            );
        }
    }

    #[test]
    fn clamps_to_cap() {
        let p = policy();
        // A large attempt number must not exceed cap + jitter.
        let d = next_retry_delay(20, &p).as_millis() as u64;
        let cap_ms = 3600 * 1000;
        assert!(d <= cap_ms + cap_ms / 10 + 1, "{d}ms exceeded cap+jitter");
    }

    #[test]
    fn should_retry_respects_max_attempts() {
        let p = policy();
        assert!(p.should_retry(9));
        assert!(!p.should_retry(10));
        assert!(!p.should_retry(11));
    }

    #[test]
    fn from_config_guards_zero_values() {
        let p = RetryPolicy::from_config(0, 0, 0);
        assert_eq!(p.max_attempts, 1);
        assert!(p.base >= Duration::from_secs(1));
        assert!(p.cap >= p.base);
    }
}
