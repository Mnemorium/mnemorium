use crate::domain::model::jwt::Jwt;
use crate::domain::model::jwt::JwtError;
use crate::domain::model::rate_limit::RateLimit;

/// Length of a hexadecimal-encoded pepper, in characters.
const PEPPER_HEX_LENGTH: usize = 64;

/// Error returned when initialising or updating a `Security` value object.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SecurityError {
    /// The `JWT` settings are invalid.
    #[error("invalid jwt configuration")]
    InvalidJwt(#[from] JwtError),
    /// The pepper is not a 64-character hexadecimal string.
    #[error("pepper must be a {PEPPER_HEX_LENGTH}-character hexadecimal string")]
    InvalidPepper,
}

/// Security-related settings.
///
/// Deserialization is routed through `SecurityConfig` and [`TryFrom`] so that
/// the layered configuration cannot bypass the validation performed by
/// [`Security::try_new`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(try_from = "SecurityConfig")]
#[non_exhaustive]
pub struct Security {
    /// `JWT` settings.
    jwt: Jwt,
    /// Whether the Root Admin default password is still logged to standard
    /// output on runtime start.
    #[serde(default = "default_log_root_admin_password")]
    log_root_admin_password: bool,
    /// Site-wide secret mixed into password hashes.
    pepper: String,
    /// Login rate-limiting settings.
    #[serde(default)]
    rate_limit: RateLimit,
}

/// Unchecked deserialization mirror of [`Security`].
///
/// Serde builds this snapshot and hands it to the [`TryFrom`] implementation,
/// which validates it through [`Security::try_new`].
#[derive(serde::Deserialize)]
struct SecurityConfig {
    /// `JWT` settings.
    jwt: Jwt,
    /// Whether the Root Admin default password is still logged to standard
    /// output on runtime start.
    #[serde(default = "default_log_root_admin_password")]
    log_root_admin_password: bool,
    /// Site-wide secret mixed into password hashes.
    pepper: String,
    /// Login rate-limiting settings.
    #[serde(default)]
    rate_limit: RateLimit,
}

impl TryFrom<SecurityConfig> for Security {
    type Error = SecurityError;

    fn try_from(config: SecurityConfig) -> Result<Self, Self::Error> {
        let mut security =
            Self::try_new(config.jwt, config.pepper, config.log_root_admin_password)?;
        security.set_rate_limit(config.rate_limit);
        Ok(security)
    }
}

impl Security {
    /// Return the `JWT` settings.
    #[must_use]
    pub fn jwt(&self) -> &Jwt {
        &self.jwt
    }

    /// Return a mutable reference to the `JWT` settings.
    #[must_use]
    pub fn jwt_mut(&mut self) -> &mut Jwt {
        &mut self.jwt
    }

    /// Return whether the Root Admin default password is still logged to
    /// standard output on runtime start.
    #[must_use]
    pub fn log_root_admin_password(&self) -> bool {
        self.log_root_admin_password
    }

    /// Return the site-wide secret mixed into password hashes.
    #[must_use]
    pub fn pepper(&self) -> &str {
        &self.pepper
    }

    /// Return the login rate-limiting settings.
    #[must_use]
    pub fn rate_limit(&self) -> &RateLimit {
        &self.rate_limit
    }

    /// Update whether the Root Admin default password is logged to standard
    /// output on runtime start.
    pub fn set_log_root_admin_password(&mut self, log_root_admin_password: bool) {
        self.log_root_admin_password = log_root_admin_password;
    }

    /// Update the site-wide secret mixed into password hashes.
    ///
    /// # Errors
    ///
    /// Returns [`SecurityError::InvalidPepper`] when `pepper` is not a
    /// [`PEPPER_HEX_LENGTH`]-character hexadecimal string.
    pub fn set_pepper(&mut self, pepper: String) -> Result<(), SecurityError> {
        self.pepper = Self::validate_pepper(pepper)?;
        Ok(())
    }

    /// Update the login rate-limiting settings.
    pub fn set_rate_limit(&mut self, rate_limit: RateLimit) {
        self.rate_limit = rate_limit;
    }

    /// Initialise a new `Security`, validating `pepper`.
    ///
    /// # Errors
    ///
    /// Returns [`SecurityError::InvalidPepper`] when `pepper` is not a
    /// [`PEPPER_HEX_LENGTH`]-character hexadecimal string.
    pub fn try_new(
        jwt: Jwt,
        pepper: String,
        log_root_admin_password: bool,
    ) -> Result<Self, SecurityError> {
        let validated_pepper = Self::validate_pepper(pepper)?;
        Ok(Self {
            jwt,
            log_root_admin_password,
            pepper: validated_pepper,
            rate_limit: RateLimit::default(),
        })
    }

    /// Validate `pepper`.
    ///
    /// # Errors
    ///
    /// Returns [`SecurityError::InvalidPepper`] when `pepper` is not a
    /// [`PEPPER_HEX_LENGTH`]-character hexadecimal string.
    fn validate_pepper(pepper: String) -> Result<String, SecurityError> {
        let is_hex = pepper.len() == PEPPER_HEX_LENGTH
            && pepper
                .chars()
                .all(|character| character.is_ascii_hexdigit());
        if is_hex {
            Ok(pepper)
        } else {
            Err(SecurityError::InvalidPepper)
        }
    }
}

/// Default value for [`Security::log_root_admin_password`].
fn default_log_root_admin_password() -> bool {
    true
}
