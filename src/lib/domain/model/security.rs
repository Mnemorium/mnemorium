use crate::domain::model::jwt::Jwt;
use crate::domain::model::jwt::JwtError;

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
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[non_exhaustive]
pub struct Security {
    /// `JWT` settings.
    jwt: Jwt,
    /// Site-wide secret mixed into password hashes.
    pepper: String,
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

    /// Return the site-wide secret mixed into password hashes.
    #[must_use]
    pub fn pepper(&self) -> &str {
        &self.pepper
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

    /// Initialise a new `Security`, validating `pepper`.
    ///
    /// # Errors
    ///
    /// Returns [`SecurityError::InvalidPepper`] when `pepper` is not a
    /// [`PEPPER_HEX_LENGTH`]-character hexadecimal string.
    pub fn try_new(jwt: Jwt, pepper: String) -> Result<Self, SecurityError> {
        let validated_pepper = Self::validate_pepper(pepper)?;
        Ok(Self {
            jwt,
            pepper: validated_pepper,
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
