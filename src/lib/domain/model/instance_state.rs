use std::fmt::{Debug, Formatter, Result as FmtResult};

/// Length of a hexadecimal-encoded secret, in characters.
const SECRET_HEX_LENGTH: usize = 64;

/// Error returned when initialising an [`InstanceState`] value object.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum InstanceStateError {
    /// The token signing secret is not a 64-character hexadecimal string.
    #[error("jwt secret must be a {SECRET_HEX_LENGTH}-character hexadecimal string")]
    InvalidJwtSecret,
    /// The pepper is not a 64-character hexadecimal string.
    #[error("pepper must be a {SECRET_HEX_LENGTH}-character hexadecimal string")]
    InvalidPepper,
}

/// The instance's non-derivable identity.
///
/// Holds the site pepper, the token signing secret and the instant the identity
/// was established. Configuration cannot supply any of these: configuration is
/// total and derivable, identity is existential.
///
/// The value is deliberately not `Serialize`: the persisted artifact is owned
/// by the outbound adapter, so the file format (and its version field) never
/// leaks into the domain.
#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct InstanceState {
    /// Instant the identity was established, as Unix seconds.
    created_at: i64,
    /// Token signing secret.
    jwt_secret: String,
    /// Site-wide secret mixed into password hashes.
    pepper: String,
}

impl InstanceState {
    /// Return the instant the identity was established, as Unix seconds.
    #[must_use]
    pub fn created_at(&self) -> i64 {
        self.created_at
    }

    /// Return the token signing secret.
    #[must_use]
    pub fn jwt_secret(&self) -> &str {
        &self.jwt_secret
    }

    /// Return the site-wide secret mixed into password hashes.
    #[must_use]
    pub fn pepper(&self) -> &str {
        &self.pepper
    }

    /// Initialise a new `InstanceState`, validating both secrets.
    ///
    /// # Errors
    ///
    /// Returns [`InstanceStateError::InvalidPepper`] when `pepper` is not a
    /// [`SECRET_HEX_LENGTH`]-character hexadecimal string, and
    /// [`InstanceStateError::InvalidJwtSecret`] when `jwt_secret` is not.
    pub fn try_new(
        created_at: i64,
        pepper: String,
        jwt_secret: String,
    ) -> Result<Self, InstanceStateError> {
        let pepper_is_hex = pepper.len() == SECRET_HEX_LENGTH
            && pepper
                .chars()
                .all(|character| character.is_ascii_hexdigit());
        if !pepper_is_hex {
            return Err(InstanceStateError::InvalidPepper);
        }
        let jwt_secret_is_hex = jwt_secret.len() == SECRET_HEX_LENGTH
            && jwt_secret
                .chars()
                .all(|character| character.is_ascii_hexdigit());
        if !jwt_secret_is_hex {
            return Err(InstanceStateError::InvalidJwtSecret);
        }
        Ok(Self {
            created_at,
            jwt_secret,
            pepper,
        })
    }
}

impl Debug for InstanceState {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("InstanceState")
            .field("created_at", &self.created_at)
            .field("jwt_secret", &"**")
            .field("pepper", &"**")
            .finish_non_exhaustive()
    }
}
