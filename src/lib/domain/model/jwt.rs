/// Length of a hexadecimal-encoded secret, in characters.
const SECRET_HEX_LENGTH: usize = 64;

/// Error returned when initialising or updating a `Jwt` value object.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum JwtError {
    /// The secret is not a 64-character hexadecimal string.
    #[error("jwt secret must be a {SECRET_HEX_LENGTH}-character hexadecimal string")]
    InvalidSecret,
    /// The token lifetime is zero.
    #[error("jwt ttl must be greater than zero")]
    InvalidTtl,
}

/// `JWT` token settings.
///
/// Deserialization is routed through `JwtConfig` and [`TryFrom`] so that the
/// layered configuration cannot bypass the validation performed by
/// [`Jwt::try_new`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(try_from = "JwtConfig")]
#[non_exhaustive]
pub struct Jwt {
    /// Secret key used to sign and verify tokens.
    secret: String,
    /// Lifetime of a token, in seconds.
    ttl: u64,
}

/// Unchecked deserialization mirror of [`Jwt`].
///
/// Serde builds this snapshot and hands it to the [`TryFrom`] implementation,
/// which validates it through [`Jwt::try_new`].
#[derive(serde::Deserialize)]
struct JwtConfig {
    /// Secret key used to sign and verify tokens.
    secret: String,
    /// Lifetime of a token, in seconds.
    ttl: u64,
}

impl TryFrom<JwtConfig> for Jwt {
    type Error = JwtError;

    fn try_from(config: JwtConfig) -> Result<Self, Self::Error> {
        Self::try_new(config.secret, config.ttl)
    }
}

impl Jwt {
    /// Return the secret key used to sign and verify tokens.
    #[must_use]
    pub fn secret(&self) -> &str {
        &self.secret
    }

    /// Update the secret key used to sign and verify tokens.
    ///
    /// # Errors
    ///
    /// Returns [`JwtError::InvalidSecret`] when `secret` is not a
    /// [`SECRET_HEX_LENGTH`]-character hexadecimal string.
    pub fn set_secret(&mut self, secret: String) -> Result<(), JwtError> {
        self.secret = Self::validate_secret(secret)?;
        Ok(())
    }

    /// Update the lifetime of a token, in seconds.
    ///
    /// # Errors
    ///
    /// Returns [`JwtError::InvalidTtl`] when `ttl` is zero.
    pub fn set_ttl(&mut self, ttl: u64) -> Result<(), JwtError> {
        self.ttl = Self::validate_ttl(ttl)?;
        Ok(())
    }

    /// Initialise a new `Jwt`, validating `secret` and `ttl`.
    ///
    /// # Errors
    ///
    /// Returns [`JwtError::InvalidSecret`] when `secret` is not a
    /// [`SECRET_HEX_LENGTH`]-character hexadecimal string, and
    /// [`JwtError::InvalidTtl`] when `ttl` is zero.
    pub fn try_new(secret: String, ttl: u64) -> Result<Self, JwtError> {
        let validated_secret = Self::validate_secret(secret)?;
        let validated_ttl = Self::validate_ttl(ttl)?;
        Ok(Self {
            secret: validated_secret,
            ttl: validated_ttl,
        })
    }

    /// Return the lifetime of a token, in seconds.
    #[must_use]
    pub fn ttl(&self) -> u64 {
        self.ttl
    }

    /// Validate `secret`.
    ///
    /// # Errors
    ///
    /// Returns [`JwtError::InvalidSecret`] when `secret` is not a
    /// [`SECRET_HEX_LENGTH`]-character hexadecimal string.
    fn validate_secret(secret: String) -> Result<String, JwtError> {
        let is_hex = secret.len() == SECRET_HEX_LENGTH
            && secret
                .chars()
                .all(|character| character.is_ascii_hexdigit());
        if is_hex {
            Ok(secret)
        } else {
            Err(JwtError::InvalidSecret)
        }
    }

    /// Validate `ttl`.
    ///
    /// # Errors
    ///
    /// Returns [`JwtError::InvalidTtl`] when `ttl` is zero.
    fn validate_ttl(ttl: u64) -> Result<u64, JwtError> {
        if ttl == 0 {
            return Err(JwtError::InvalidTtl);
        }
        Ok(ttl)
    }
}
