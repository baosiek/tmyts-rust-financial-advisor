use std::future::Future;
use std::time::{Duration, Instant};

use thiserror::Error;

/// Why a timed operation did not produce a value.
#[derive(Debug, Error)]
pub enum TimedError<E> {
    /// The operation did not finish within the limit and was cancelled.
    #[error("timed out after {} ms", .0.as_millis())]
    TimedOut(Duration),

    /// The operation finished, but with its own error.
    #[error(transparent)]
    Failed(E),
}

/// The outcome of one timed operation: its name, how long it ran, and its result.
#[derive(Debug)]
pub struct Pending<T, E> {
    pub name: &'static str,
    pub elapsed: Duration,
    pub result: Result<T, TimedError<E>>,
}

/// Awaits `fut`, but gives up after `limit`. Always reports the elapsed time.
pub async fn timed<T, E, Fut>(name: &'static str, limit: Duration, fut: Fut) -> Pending<T, E>
where
    Fut: Future<Output = Result<T, E>>,
{
    let start = Instant::now();
    let result = match tokio::time::timeout(limit, fut).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(e)) => Err(TimedError::Failed(e)),
        Err(_elapse) => Err(TimedError::TimedOut(limit)),
    };
    Pending {
        name,
        elapsed: start.elapsed(),
        result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fast_success_is_returned() {
        let pending = timed("ok", Duration::from_secs(1), async { Ok::<_, String>(42) }).await;
        assert_eq!(pending.name, "ok");
        assert!(matches!(pending.result, Ok(42)));
    }

    #[tokio::test]
    async fn operation_error_is_kept() {
        let pending = timed("fails", Duration::from_secs(1), async {
            Err::<u32, _>("boom".to_string())
        })
        .await;
        assert!(matches!(pending.result, Err(TimedError::Failed(ref e)) if e == "boom"));
    }

    #[tokio::test]
    async fn slow_operation_times_out() {
        let limit = Duration::from_millis(20);
        let pending = timed("slow", limit, async {
            tokio::time::sleep(Duration::from_secs(5)).await;
            Ok::<_, String>(())
        })
        .await;
        assert!(matches!(pending.result, Err(TimedError::TimedOut(d)) if d == limit));
        assert!(pending.elapsed < Duration::from_secs(1));
    }
}
