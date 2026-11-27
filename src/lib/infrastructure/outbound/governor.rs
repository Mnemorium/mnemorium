//! The governor-backed rate limiters.
//!
//! `governor` keeps one entry per key and never reclaims it on its own; an
//! attacker rotating client addresses or identifiers would otherwise grow the
//! map without bound. [`RateLimitCleanup`] holds the prune closures the
//! composition root runs periodically.
//!
//! The process-lifetime limiters are built here, beside the per-use
//! [`KeyedLoginAttemptLimiter`] adapter that delegates to them.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use governor::clock::DefaultClock;
use governor::state::keyed::DefaultKeyedStateStore;
use governor::{Quota, RateLimiter};

use crate::domain::NumericID;
use crate::domain::model::RateLimit;
use crate::domain::port::{LoginAttemptKey, LoginAttemptLimiter, SetupAttemptLimiter};

/// Periodic upkeep for the governor rate limiters.
///
/// Each prune closure drops the entries whose quota has fully replenished. The
/// closures are opaque so no `governor` type leaks into a signature.
#[derive(Default)]
pub(crate) struct RateLimitCleanup {
    /// Per-limiter prune closures.
    prunes: Vec<Box<dyn Fn() + Send + Sync>>,
}

impl RateLimitCleanup {
    /// Build a cleanup from a single prune closure.
    pub(crate) fn from_prune(prune: Box<dyn Fn() + Send + Sync>) -> Self {
        Self {
            prunes: vec![prune],
        }
    }

    /// Fold `other`'s prune closures into `self`.
    pub(crate) fn merge(&mut self, other: Self) {
        self.prunes.extend(other.prunes);
    }

    /// Create an empty cleanup.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Run every registered prune.
    pub(crate) fn prune(&self) {
        for prune in &self.prunes {
            prune();
        }
    }
}

/// Keyed governor limiter backing the account dimension.
type KeyedLimiter = RateLimiter<AccountKey, DefaultKeyedStateStore<AccountKey>, DefaultClock>;

/// Hashable key one login attempt is counted under.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum AccountKey {
    /// The submitted identifier that resolved to no account.
    Identifier(String),
    /// An account the submitted identifier resolved to.
    User(NumericID),
}

impl From<LoginAttemptKey> for AccountKey {
    fn from(key: LoginAttemptKey) -> Self {
        match key {
            LoginAttemptKey::User(id) => Self::User(id),
            LoginAttemptKey::Identifier(identifier) => Self::Identifier(identifier),
        }
    }
}

/// Process-lifetime account-keyed login limiter state, built from the login
/// burst and period.
///
/// The composition root builds one per process and registers its cleanup; the
/// Identity factory wraps it in the per-use [`KeyedLoginAttemptLimiter`]. The
/// state outlives any single call, so an account is throttled across every
/// attempt the process sees.
pub(crate) struct LoginAttemptLimit {
    /// The keyed limiter; absent when the configuration cannot build one.
    limiter: Option<Arc<KeyedLimiter>>,
    /// Configured refill period, reported as the wait when a key is spent.
    period: Duration,
}

impl LoginAttemptLimit {
    /// Count one attempt under `key`, returning the configured period when spent.
    pub(crate) fn check(&self, key: LoginAttemptKey) -> Option<Duration> {
        let limiter = self.limiter.as_ref()?;
        limiter
            .check_key(&AccountKey::from(key))
            .err()
            .map(|_| self.period)
    }

    /// Build the cleanup that drops fully replenished keys.
    pub(crate) fn cleanup(&self) -> RateLimitCleanup {
        let Some(limiter) = self.limiter.as_ref().map(Arc::clone) else {
            return RateLimitCleanup::new();
        };
        RateLimitCleanup::from_prune(Box::new(move || limiter.retain_recent()))
    }

    /// The number of keys currently tracked; exercises pruning in the tests.
    #[cfg(test)]
    pub(crate) fn key_count(&self) -> usize {
        self.limiter.as_ref().map_or(0, |limiter| limiter.len())
    }

    /// Build the account limiter from `rate_limit`.
    #[must_use]
    pub(crate) fn new(rate_limit: &RateLimit) -> Self {
        let period = Duration::from_secs(rate_limit.period_seconds());
        let quota = Quota::with_period(period).and_then(|period_quota| {
            NonZeroU32::new(rate_limit.burst_size()).map(|burst| period_quota.allow_burst(burst))
        });
        Self {
            limiter: quota.map(|configured_quota| Arc::new(RateLimiter::keyed(configured_quota))),
            period,
        }
    }
}

/// Keyed governor limiter backing the process-wide setup dimension.
type SetupLimiter = RateLimiter<SetupKey, DefaultKeyedStateStore<SetupKey>, DefaultClock>;

/// The one key the process-wide setup budget is counted under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SetupKey {
    /// The whole process shares one budget.
    Process,
}

/// Process-lifetime setup limiter state, built from the login burst and period.
///
/// The composition root builds one per process and registers its cleanup; the
/// Identity factory wraps it in the per-use setup use case. The key is constant,
/// so the budget is process-wide rather than per account.
pub(crate) struct SetupAttemptLimit {
    /// The keyed limiter; absent when the configuration cannot build one.
    limiter: Option<Arc<SetupLimiter>>,
    /// Configured refill period, reported as the wait when the budget is spent.
    period: Duration,
}

impl SetupAttemptLimit {
    /// Build the cleanup that drops fully replenished keys.
    pub(crate) fn cleanup(&self) -> RateLimitCleanup {
        let Some(limiter) = self.limiter.as_ref().map(Arc::clone) else {
            return RateLimitCleanup::new();
        };
        RateLimitCleanup::from_prune(Box::new(move || limiter.retain_recent()))
    }

    /// Build the process-wide limiter from `rate_limit`.
    #[must_use]
    pub(crate) fn new(rate_limit: &RateLimit) -> Self {
        let period = Duration::from_secs(rate_limit.period_seconds());
        let quota = Quota::with_period(period).and_then(|period_quota| {
            NonZeroU32::new(rate_limit.burst_size()).map(|burst| period_quota.allow_burst(burst))
        });
        Self {
            limiter: quota.map(|configured_quota| Arc::new(RateLimiter::keyed(configured_quota))),
            period,
        }
    }
}

impl SetupAttemptLimiter for SetupAttemptLimit {
    fn check(&self) -> Option<Duration> {
        let limiter = self.limiter.as_ref()?;
        limiter
            .check_key(&SetupKey::Process)
            .err()
            .map(|_| self.period)
    }
}

/// Account-keyed login limiter, built per use over the process-lifetime limit.
///
/// The per-client limiter is HTTP middleware; this adapter is checked inside
/// the login use case, so one account is throttled across client addresses. An
/// identifier that resolves to no account is keyed by the submitted value,
/// which is never logged.
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
    fn an_unknown_identifier_is_throttled_under_its_submitted_key() -> Result<(), Box<dyn Error>> {
        // Arrange
        let limiter = limiter(&rate_limit(1, 60)?);

        // Act & Assert: the submitted identifier is spent like any account key.
        assert!(
            limiter
                .check(LoginAttemptKey::Identifier("ghost".to_owned()))
                .is_none()
        );
        assert!(
            limiter
                .check(LoginAttemptKey::Identifier("ghost".to_owned()))
                .is_some()
        );

        // Another identifier and a user identifier each keep their own bucket.
        assert!(
            limiter
                .check(LoginAttemptKey::Identifier("other".to_owned()))
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

    #[test]
    fn setup_attempts_share_one_process_budget() -> Result<(), Box<dyn Error>> {
        // Arrange
        let limit = SetupAttemptLimit::new(&rate_limit(2, 60)?);

        // Act & Assert
        assert!(limit.check().is_none());
        assert!(limit.check().is_none());
        assert!(limit.check().is_some());
        Ok(())
    }
}
