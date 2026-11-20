use std::time::Duration;

use crate::domain::NumericID;

/// The key one login attempt is counted under.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum LoginAttemptKey {
    /// A submitted username or email address that resolved to no account.
    Identifier(String),
    /// An account the submitted identifier resolved to.
    User(NumericID),
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
