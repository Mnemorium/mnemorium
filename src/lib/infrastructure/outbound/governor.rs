//! The account-keyed login limiter adapter.

use std::sync::Arc;
use std::time::Duration;

use crate::domain::port::{LoginAttemptKey, LoginAttemptLimiter};
use crate::infrastructure::rate_limit::LoginAttemptLimit;

/// Account-keyed login limiter, built per use over the process-lifetime limit.
///
/// The per-client limiter is HTTP middleware; this adapter is checked inside
/// the login use case, so one account is throttled across client addresses. A
/// username that resolves to no account is keyed by a `SHA-256` digest of the
/// submitted value, which is never logged.
pub(crate) struct KeyedLoginAttemptLimiter {
    /// The process-lifetime account limit the adapter reads.
    limit: Arc<LoginAttemptLimit>,
}

impl KeyedLoginAttemptLimiter {
    /// Wrap `limit` in a fresh adapter.
    #[must_use]
    pub(crate) fn new(limit: Arc<LoginAttemptLimit>) -> Self {
        Self { limit }
    }
}

impl LoginAttemptLimiter for KeyedLoginAttemptLimiter {
    fn check(&self, key: LoginAttemptKey) -> Option<Duration> {
        self.limit.check(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::{ClientIpHeader, RateLimit};

    use std::error::Error;
    use std::thread::sleep;

    /// A validated rate limit with `burst` and a `period_seconds` refill.
    fn rate_limit(burst: u32, period_seconds: u64) -> Result<RateLimit, Box<dyn Error>> {
        Ok(RateLimit::try_new(
            burst,
            ClientIpHeader::XForwardedFor,
            period_seconds,
            Vec::new(),
        )?)
    }

    /// A limiter over a fresh account limit.
    fn limiter(rate_limit: &RateLimit) -> KeyedLoginAttemptLimiter {
        KeyedLoginAttemptLimiter::new(Arc::new(LoginAttemptLimit::new(rate_limit)))
    }

    #[test]
    fn one_account_is_throttled_while_another_keeps_its_quota() -> Result<(), Box<dyn Error>> {
        // Arrange
        let limiter = limiter(&rate_limit(2, 60)?);

        // Act & Assert: the first account spends its burst and is then refused.
        assert!(limiter.check(LoginAttemptKey::User(7)).is_none());
        assert!(limiter.check(LoginAttemptKey::User(7)).is_none());
        assert!(limiter.check(LoginAttemptKey::User(7)).is_some());

        // A different account still has its own quota.
        assert!(limiter.check(LoginAttemptKey::User(8)).is_none());
        Ok(())
    }

    #[test]
    fn an_unknown_username_is_throttled_under_a_derived_key() -> Result<(), Box<dyn Error>> {
        // Arrange
        let limiter = limiter(&rate_limit(1, 60)?);

        // Act & Assert: the derived key is spent like any account key.
        assert!(
            limiter
                .check(LoginAttemptKey::Username("ghost".to_owned()))
                .is_none()
        );
        assert!(
            limiter
                .check(LoginAttemptKey::Username("ghost".to_owned()))
                .is_some()
        );

        // Another username and a user identifier each keep their own bucket.
        assert!(
            limiter
                .check(LoginAttemptKey::Username("other".to_owned()))
                .is_none()
        );
        assert!(limiter.check(LoginAttemptKey::User(0)).is_none());
        Ok(())
    }

    #[test]
    fn the_quota_refills_after_the_period() -> Result<(), Box<dyn Error>> {
        // Arrange
        let limiter = limiter(&rate_limit(1, 1)?);
        assert!(limiter.check(LoginAttemptKey::User(7)).is_none());
        assert!(limiter.check(LoginAttemptKey::User(7)).is_some());

        // Act
        sleep(Duration::from_millis(1_100));

        // Assert
        assert!(limiter.check(LoginAttemptKey::User(7)).is_none());
        Ok(())
    }

    #[test]
    fn pruning_drops_a_fully_replenished_key() -> Result<(), Box<dyn Error>> {
        // Arrange: `retain_recent` keeps a key until its arrival time is two
        // periods in the past, so the observation waits two periods.
        let limit = Arc::new(LoginAttemptLimit::new(&rate_limit(1, 1)?));
        let limiter = KeyedLoginAttemptLimiter::new(Arc::clone(&limit));
        assert!(limiter.check(LoginAttemptKey::User(7)).is_none());
        assert_eq!(limit.key_count(), 1);
        sleep(Duration::from_millis(2_100));

        // Act
        limit.cleanup().prune();

        // Assert
        assert_eq!(limit.key_count(), 0);
        Ok(())
    }
}
