//! Startup health checks, adapted from `report()` in tmyts-fin-advisor's main.rs:
//! every service is checked in parallel with `timed`, every result is logged,
//! and startup stops if any check failed.

use std::sync::Arc;
use std::time::Duration;

use advisor_core::{HealthCheck, Pending, TimedError, timed};
use futures::future::join_all;
use thiserror::Error;

#[derive(Debug, Error)]
#[error("startup aborted: {failed} of {total} health checks failed (see the log above)")]
pub struct StartupError {
    pub failed: usize,
    pub total: usize,
}

/// Runs all checks concurrently: the total time is that of the slowest one.
pub async fn check_services(
    checks: &[Arc<dyn HealthCheck>],
    limit: Duration,
) -> Result<(), StartupError> {
    let results = join_all(
        checks
            .iter()
            .map(|service| timed(service.service_name(), limit, service.check())),
    )
    .await;

    report(&results)
}

pub async fn check_provider(
    check: &Arc<dyn HealthCheck>,
    limit: Duration,
) -> Result<(), StartupError> {
    let result = timed(check.service_name(), limit, check.check()).await;
    report(&[result])
}

fn report(results: &[Pending<String, advisor_core::HealthError>]) -> Result<(), StartupError> {
    // Log every result first, so all failures are visible before aborting.
    for pending in results {
        let elapsed_ms = pending.elapsed.as_millis() as u64;
        match &pending.result {
            Ok(detail) => {
                tracing::info!(service = pending.name, elapsed_ms, %detail, "health check passed")
            }
            Err(TimedError::TimedOut(limit)) => tracing::error!(
                service = pending.name,
                limit_ms = limit.as_millis() as u64,
                "health check timed out"
            ),
            Err(TimedError::Failed(e)) => {
                tracing::error!(service = pending.name, elapsed_ms, error = %e, "health check failed")
            }
        }
    }

    let failed = results
        .iter()
        .filter(|pending| pending.result.is_err())
        .count();
    if failed > 0 {
        return Err(StartupError {
            failed,
            total: results.len(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use advisor_core::HealthError;
    use async_trait::async_trait;

    struct FakeService {
        name: &'static str,
        delay: Duration,
        healthy: bool,
    }

    #[async_trait]
    impl HealthCheck for FakeService {
        fn service_name(&self) -> &'static str {
            self.name
        }

        async fn check(&self) -> Result<String, HealthError> {
            tokio::time::sleep(self.delay).await;
            if self.healthy {
                Ok("ok".to_string())
            } else {
                Err("connection refused".into())
            }
        }
    }

    fn service(name: &'static str, delay_ms: u64, healthy: bool) -> Arc<dyn HealthCheck> {
        Arc::new(FakeService {
            name,
            delay: Duration::from_millis(delay_ms),
            healthy,
        })
    }

    #[tokio::test]
    async fn all_healthy_passes() {
        let checks = [service("a", 1, true), service("b", 1, true)];
        assert!(
            check_services(&checks, Duration::from_secs(1))
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn failures_and_timeouts_are_counted() {
        let checks = [
            service("ok", 1, true),
            service("down", 1, false),
            service("slow", 5_000, true),
        ];
        let err = check_services(&checks, Duration::from_millis(50))
            .await
            .unwrap_err();
        assert_eq!((err.failed, err.total), (2, 3));
    }

    #[tokio::test]
    async fn no_services_is_fine() {
        assert!(check_services(&[], Duration::from_secs(1)).await.is_ok());
    }
}
