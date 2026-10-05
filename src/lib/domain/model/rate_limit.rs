use std::net::IpAddr;
use std::str::FromStr as _;

/// Default value for [`RateLimit::burst_size`].
const DEFAULT_BURST_SIZE: u32 = 5;
/// Default value for [`RateLimit::period_seconds`].
const DEFAULT_PERIOD_SECONDS: u64 = 12;
/// Separator between addresses in the [`RateLimit::trusted_proxies`] list.
const TRUSTED_PROXIES_SEPARATOR: char = ',';

/// Error returned when initialising or updating a `RateLimit` value object.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RateLimitError {
    /// The burst size is zero.
    #[error("rate limit burst size must be greater than zero")]
    InvalidBurstSize,
    /// The replenishment period is zero.
    #[error("rate limit period must be greater than zero")]
    InvalidPeriodSeconds,
    /// An entry of the trusted-proxy list is not a valid IP address.
    #[error("rate limit trusted proxy must be a valid IP address")]
    InvalidTrustedProxy,
}

/// Login rate-limiting settings.
///
/// Deserialization is routed through `RateLimitConfig` and [`TryFrom`] so that
/// the layered configuration cannot bypass the validation performed by
/// [`RateLimit::try_new`]. Serialization uses the same mirror so the layered
/// configuration round-trips: `trusted_proxies` is written as a comma-separated
/// string, not a JSON array, because the `config` crate merges the persisted
/// snapshot back through the same string-typed field.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(try_from = "RateLimitConfig", into = "RateLimitConfig")]
#[non_exhaustive]
pub struct RateLimit {
    /// Number of requests a client address may spend before throttling.
    burst_size: u32,
    /// Seconds after which one element of the quota is replenished.
    period_seconds: u64,
    /// Addresses of the reverse proxies whose client-address headers are
    /// trusted. Empty means the forwarded headers are never trusted and the
    /// client key is always the peer address.
    trusted_proxies: Vec<IpAddr>,
}

/// Unchecked deserialization mirror of [`RateLimit`].
///
/// Serde builds this snapshot and hands it to the [`TryFrom`] implementation,
/// which validates it through [`RateLimit::try_new`]. Serialization goes the
/// other way — through [`From`] — so the two directions agree on the wire form.
#[derive(serde::Deserialize, serde::Serialize)]
struct RateLimitConfig {
    /// Number of requests a client address may spend before throttling.
    #[serde(default = "default_burst_size")]
    burst_size: u32,
    /// Seconds after which one element of the quota is replenished.
    #[serde(default = "default_period_seconds")]
    period_seconds: u64,
    /// Comma-separated addresses of the trusted reverse proxies.
    #[serde(default)]
    trusted_proxies: String,
}

impl TryFrom<RateLimitConfig> for RateLimit {
    type Error = RateLimitError;

    fn try_from(config: RateLimitConfig) -> Result<Self, Self::Error> {
        Self::try_new(
            config.burst_size,
            config.period_seconds,
            parse_trusted_proxies(&config.trusted_proxies)?,
        )
    }
}

impl From<&RateLimit> for RateLimitConfig {
    fn from(rate_limit: &RateLimit) -> Self {
        Self {
            burst_size: rate_limit.burst_size,
            period_seconds: rate_limit.period_seconds,
            trusted_proxies: rate_limit.trusted_proxies_value(),
        }
    }
}

impl From<RateLimit> for RateLimitConfig {
    fn from(rate_limit: RateLimit) -> Self {
        Self::from(&rate_limit)
    }
}

impl Default for RateLimit {
    fn default() -> Self {
        Self {
            burst_size: DEFAULT_BURST_SIZE,
            period_seconds: DEFAULT_PERIOD_SECONDS,
            trusted_proxies: Vec::new(),
        }
    }
}

impl RateLimit {
    /// Return the number of requests a client address may spend before
    /// throttling.
    #[must_use]
    pub fn burst_size(&self) -> u32 {
        self.burst_size
    }

    /// Return the seconds after which one element of the quota is replenished.
    #[must_use]
    pub fn period_seconds(&self) -> u64 {
        self.period_seconds
    }

    /// Update the number of requests a client address may spend before
    /// throttling.
    ///
    /// # Errors
    ///
    /// Returns [`RateLimitError::InvalidBurstSize`] when `burst_size` is zero.
    pub fn set_burst_size(&mut self, burst_size: u32) -> Result<(), RateLimitError> {
        self.burst_size = Self::validate_burst_size(burst_size)?;
        Ok(())
    }

    /// Update the seconds after which one element of the quota is replenished.
    ///
    /// # Errors
    ///
    /// Returns [`RateLimitError::InvalidPeriodSeconds`] when `period_seconds`
    /// is zero.
    pub fn set_period_seconds(&mut self, period_seconds: u64) -> Result<(), RateLimitError> {
        self.period_seconds = Self::validate_period_seconds(period_seconds)?;
        Ok(())
    }

    /// Update the addresses of the trusted reverse proxies.
    pub fn set_trusted_proxies(&mut self, trusted_proxies: Vec<IpAddr>) {
        self.trusted_proxies = trusted_proxies;
    }

    /// Return the addresses of the trusted reverse proxies.
    #[must_use]
    pub fn trusted_proxies(&self) -> &[IpAddr] {
        &self.trusted_proxies
    }

    /// Return the trusted proxies rendered as the persisted comma-separated
    /// list.
    #[must_use]
    pub fn trusted_proxies_value(&self) -> String {
        self.trusted_proxies
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<String>>()
            .join(&TRUSTED_PROXIES_SEPARATOR.to_string())
    }

    /// Initialise a new `RateLimit`, validating `burst_size` and
    /// `period_seconds`.
    ///
    /// # Errors
    ///
    /// Returns [`RateLimitError::InvalidBurstSize`] when `burst_size` is zero,
    /// and [`RateLimitError::InvalidPeriodSeconds`] when `period_seconds` is
    /// zero.
    pub fn try_new(
        burst_size: u32,
        period_seconds: u64,
        trusted_proxies: Vec<IpAddr>,
    ) -> Result<Self, RateLimitError> {
        let validated_burst_size = Self::validate_burst_size(burst_size)?;
        let validated_period_seconds = Self::validate_period_seconds(period_seconds)?;
        Ok(Self {
            burst_size: validated_burst_size,
            period_seconds: validated_period_seconds,
            trusted_proxies,
        })
    }

    /// Validate `burst_size`.
    ///
    /// # Errors
    ///
    /// Returns [`RateLimitError::InvalidBurstSize`] when `burst_size` is zero.
    fn validate_burst_size(burst_size: u32) -> Result<u32, RateLimitError> {
        if burst_size == 0 {
            return Err(RateLimitError::InvalidBurstSize);
        }
        Ok(burst_size)
    }

    /// Validate `period_seconds`.
    ///
    /// # Errors
    ///
    /// Returns [`RateLimitError::InvalidPeriodSeconds`] when `period_seconds`
    /// is zero.
    fn validate_period_seconds(period_seconds: u64) -> Result<u64, RateLimitError> {
        if period_seconds == 0 {
            return Err(RateLimitError::InvalidPeriodSeconds);
        }
        Ok(period_seconds)
    }
}

/// Parse the comma-separated trusted-proxy list.
///
/// # Errors
///
/// Returns [`RateLimitError::InvalidTrustedProxy`] when a non-empty entry is
/// not a valid IP address.
pub fn parse_trusted_proxies(value: &str) -> Result<Vec<IpAddr>, RateLimitError> {
    value
        .split(TRUSTED_PROXIES_SEPARATOR)
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| IpAddr::from_str(entry).map_err(|_| RateLimitError::InvalidTrustedProxy))
        .collect()
}

/// Default value for [`RateLimit::burst_size`].
fn default_burst_size() -> u32 {
    DEFAULT_BURST_SIZE
}

/// Default value for [`RateLimit::period_seconds`].
fn default_period_seconds() -> u64 {
    DEFAULT_PERIOD_SECONDS
}
