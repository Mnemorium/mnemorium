use std::time::Duration;

/// Outbound port limiting setup attempts process-wide.
///
/// The per-client limiter is HTTP middleware; this port is checked inside the
/// setup use case, which is the one place that sees every attempt whatever the
/// client address.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait SetupAttemptLimiter: Send + Sync {
    /// Count one attempt and return how long to wait when the budget is spent.
    fn check(&self) -> Option<Duration>;
}
