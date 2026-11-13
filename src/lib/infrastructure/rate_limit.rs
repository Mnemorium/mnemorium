use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use governor::clock::DefaultClock;
use governor::state::keyed::DefaultKeyedStateStore;
use governor::{Quota, RateLimiter};
use sha2::{Digest as _, Sha256};

use crate::domain::NumericID;
use crate::domain::model::RateLimit;
use crate::domain::port::LoginAttemptKey;

/// Periodic upkeep for the login rate limiters.
///
/// `governor` keeps one entry per client address, or per account, and never
/// reclaims it on its own; an attacker rotating addresses would otherwise grow
/// the map without bound. Each prune closure drops the entries whose quota has
/// fully replenished. The closures are opaque so no `governor` type leaks into a
/// signature.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum AccountKey {
    /// An account the submitted username resolved to.
    User(NumericID),
    /// The digest of a username that resolved to no account.
    UsernameDigest([u8; 32]),
}

impl From<LoginAttemptKey> for AccountKey {
    fn from(key: LoginAttemptKey) -> Self {
        match key {
            LoginAttemptKey::User(id) => Self::User(id),
            LoginAttemptKey::Username(username) => {
                let mut digest = [0u8; 32];
                digest.copy_from_slice(&Sha256::digest(username.as_bytes()));
                Self::UsernameDigest(digest)
            }
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
///
/// [`KeyedLoginAttemptLimiter`]: crate::infrastructure::outbound::KeyedLoginAttemptLimiter
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
