/// Default value for [`RateLimit::behind_proxy`].
const DEFAULT_BEHIND_PROXY: bool = false;
/// Default value for [`RateLimit::burst_size`].
const DEFAULT_BURST_SIZE: u32 = 5;
/// Default value for [`RateLimit::period_seconds`].
const DEFAULT_PERIOD_SECONDS: u64 = 12;

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
}

/// Login rate-limiting settings.
///
/// Deserialization is routed through `RateLimitConfig` and [`TryFrom`] so that
/// the layered configuration cannot bypass the validation performed by
/// [`RateLimit::try_new`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(try_from = "RateLimitConfig")]
#[non_exhaustive]
pub struct RateLimit {
    /// Whether the server sits behind a reverse proxy that sets the client
    /// address headers.
    behind_proxy: bool,
    /// Number of requests a client address may spend before throttling.
    burst_size: u32,
    /// Seconds after which one element of the quota is replenished.
    period_seconds: u64,
}

/// Unchecked deserialization mirror of [`RateLimit`].
///
/// Serde builds this snapshot and hands it to the [`TryFrom`] implementation,
/// which validates it through [`RateLimit::try_new`].
#[derive(serde::Deserialize)]
struct RateLimitConfig {
    /// Whether the server sits behind a reverse proxy that sets the client
    /// address headers.
    #[serde(default = "default_behind_proxy")]
    behind_proxy: bool,
    /// Number of requests a client address may spend before throttling.
    #[serde(default = "default_burst_size")]
    burst_size: u32,
    /// Seconds after which one element of the quota is replenished.
    #[serde(default = "default_period_seconds")]
    period_seconds: u64,
}

impl TryFrom<RateLimitConfig> for RateLimit {
    type Error = RateLimitError;

    fn try_from(config: RateLimitConfig) -> Result<Self, Self::Error> {
        Self::try_new(
            config.behind_proxy,
            config.burst_size,
            config.period_seconds,
        )
    }
}

impl Default for RateLimit {
    fn default() -> Self {
        Self {
            behind_proxy: DEFAULT_BEHIND_PROXY,
            burst_size: DEFAULT_BURST_SIZE,
            period_seconds: DEFAULT_PERIOD_SECONDS,
        }
    }
}

impl RateLimit {
    /// Return whether the server sits behind a reverse proxy that sets the
    /// client address headers.
    #[must_use]
    pub fn behind_proxy(&self) -> bool {
        self.behind_proxy
    }

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

    /// Update whether the server sits behind a reverse proxy that sets the
    /// client address headers.
    pub fn set_behind_proxy(&mut self, behind_proxy: bool) {
        self.behind_proxy = behind_proxy;
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

    /// Initialise a new `RateLimit`, validating `burst_size` and
    /// `period_seconds`.
    ///
    /// # Errors
    ///
    /// Returns [`RateLimitError::InvalidBurstSize`] when `burst_size` is zero,
    /// and [`RateLimitError::InvalidPeriodSeconds`] when `period_seconds` is
    /// zero.
    pub fn try_new(
        behind_proxy: bool,
        burst_size: u32,
        period_seconds: u64,
    ) -> Result<Self, RateLimitError> {
        let validated_burst_size = Self::validate_burst_size(burst_size)?;
        let validated_period_seconds = Self::validate_period_seconds(period_seconds)?;
        Ok(Self {
            behind_proxy,
            burst_size: validated_burst_size,
            period_seconds: validated_period_seconds,
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

/// Default value for [`RateLimit::behind_proxy`].
fn default_behind_proxy() -> bool {
    DEFAULT_BEHIND_PROXY
}

/// Default value for [`RateLimit::burst_size`].
fn default_burst_size() -> u32 {
    DEFAULT_BURST_SIZE
}

/// Default value for [`RateLimit::period_seconds`].
fn default_period_seconds() -> u64 {
    DEFAULT_PERIOD_SECONDS
}
