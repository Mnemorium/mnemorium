use std::time::Duration;

use crate::domain::NumericID;

/// The key one login attempt is counted under.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum LoginAttemptKey {
    /// An account the submitted username resolved to.
    User(NumericID),
    /// A submitted username that resolved to no account.
    Username(String),
}

/// Outbound port limiting login attempts per account.
///
/// The per-client limiter is HTTP middleware; this port is checked inside the
/// login use case, which is the only place that knows which account a username
/// resolved to.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait LoginAttemptLimiter: Send + Sync {
    /// Count one attempt and return how long to wait when the key's quota is spent.
    fn check(&self, key: LoginAttemptKey) -> Option<Duration>;
}
