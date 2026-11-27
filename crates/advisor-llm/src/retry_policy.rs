use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum RetryPolicyError {
    #[error("max_attempts must be at least 1 (1 means no retries)")]
    NoAttempts,
    #[error("multiplier must be at least 1.0, got {0}")]
    Multiplier(f64),
    #[error("jitter must be between 0.0 and 1.0, got {0}")]
    Jitter(f64),
    #[error("initial_backoff ({initial_ms} ms) is larger than max_backoff ({max_ms} ms)")]
    BackoffOrder { initial_ms: u128, max_ms: u128 },
}

/// When and how long to wait before trying a failed LLM call again.
///
/// Exponential backoff: the wait before retry `n` is
/// `initial_backoff * multiplier^(n-1)`, capped at `max_backoff`. Jitter then
/// takes off a random part of it (up to `jitter` × the wait), so many clients
/// that failed together do not all retry at the same moment.
#[derive(Debug, Clone, PartialEq)]
pub struct RetryPolicy {
    max_attempts: u32,
    initial_backoff: Duration,
    max_backoff: Duration,
    multiplier: f64,
    jitter: f64,
}

impl RetryPolicy {
    /// `max_attempts` counts the first try: 3 means one call and up to two retries.
    pub fn new(
        max_attempts: u32,
        initial_backoff: Duration,
        max_backoff: Duration,
        multiplier: f64,
        jitter: f64,
    ) -> Result<Self, RetryPolicyError> {
        if max_attempts == 0 {
            return Err(RetryPolicyError::NoAttempts);
        }
        // NaN fails every comparison, so it is named explicitly.
        if multiplier.is_nan() || multiplier < 1.0 {
            return Err(RetryPolicyError::Multiplier(multiplier));
        }
        if !(0.0..=1.0).contains(&jitter) {
            return Err(RetryPolicyError::Jitter(jitter));
        }
        if initial_backoff > max_backoff {
            return Err(RetryPolicyError::BackoffOrder {
                initial_ms: initial_backoff.as_millis(),
                max_ms: max_backoff.as_millis(),
            });
        }
        Ok(Self {
            max_attempts,
            initial_backoff,
            max_backoff,
            multiplier,
            jitter,
        })
    }

    pub fn max_attempts(&self) -> u32 {
        self.max_attempts
    }

    pub fn max_backoff(&self) -> Duration {
        self.max_backoff
    }

    /// The wait before retry number `retry` (1 = the first retry), without jitter.
    pub fn backoff(&self, retry: u32) -> Duration {
        let exponent = i32::try_from(retry.saturating_sub(1)).unwrap_or(i32::MAX);
        let secs = self.initial_backoff.as_secs_f64() * self.multiplier.powi(exponent);
        // Also guards against overflow: a huge exponent gives infinity, capped here.
        Duration::from_secs_f64(secs.min(self.max_backoff.as_secs_f64()))
    }

    /// The wait with jitter applied: a random value in `[backoff × (1 - jitter), backoff]`.
    pub fn backoff_with_jitter(&self, retry: u32) -> Duration {
        self.backoff(retry)
            .mul_f64(1.0 - self.jitter * random_unit())
    }
}

/// A random number in `[0, 1)`. The standard library's hasher seeds are random
/// per process and per `RandomState`, which is plenty for spreading retries;
/// it saves a dependency on `rand` for one number.
fn random_unit() -> f64 {
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(0);
    (hasher.finish() >> 11) as f64 / (1u64 << 53) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(jitter: f64) -> RetryPolicy {
        RetryPolicy::new(
            5,
            Duration::from_millis(500),
            Duration::from_secs(3),
            2.0,
            jitter,
        )
        .unwrap()
    }

    #[test]
    fn backoff_doubles_and_is_capped() {
        let p = policy(0.0);
        assert_eq!(p.backoff(1), Duration::from_millis(500));
        assert_eq!(p.backoff(2), Duration::from_millis(1000));
        assert_eq!(p.backoff(3), Duration::from_millis(2000));
        assert_eq!(
            p.backoff(4),
            Duration::from_secs(3),
            "capped at max_backoff"
        );
        assert_eq!(p.backoff(1000), Duration::from_secs(3), "no overflow");
    }

    #[test]
    fn jitter_stays_within_its_range() {
        let p = policy(0.5);
        for _ in 0..1000 {
            let wait = p.backoff_with_jitter(2);
            assert!(wait >= Duration::from_millis(500) && wait <= Duration::from_millis(1000));
        }
        assert_eq!(
            policy(0.0).backoff_with_jitter(2),
            Duration::from_millis(1000)
        );
    }

    #[test]
    fn invalid_policies_are_rejected() {
        let ms = Duration::from_millis;
        assert_eq!(
            RetryPolicy::new(0, ms(1), ms(2), 2.0, 0.0),
            Err(RetryPolicyError::NoAttempts)
        );
        assert!(RetryPolicy::new(3, ms(1), ms(2), 0.5, 0.0).is_err());
        assert!(RetryPolicy::new(3, ms(1), ms(2), f64::NAN, 0.0).is_err());
        assert!(RetryPolicy::new(3, ms(1), ms(2), 2.0, 1.5).is_err());
        assert!(RetryPolicy::new(3, ms(5), ms(2), 2.0, 0.0).is_err());
    }
}
