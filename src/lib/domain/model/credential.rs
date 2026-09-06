use chrono::NaiveDateTime;

use crate::domain::alias::NumericID;

/// Error returned when initialising or updating a `Credential`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CredentialError {
    /// The password hash is empty.
    #[error("password hash must not be empty")]
    PasswordHashEmpty,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// A credential, holding the password hash used for authentication.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Credential {
    /// Unique identifier of the credential.
    id: NumericID,
    /// Password hash, in an algorithm-agnostic format.
    password_hash: String,
    /// Date and time of the last update of the password hash.
    updated_at: NaiveDateTime,
}

impl Credential {
    /// Return the unique identifier.
    #[must_use]
    pub fn id(&self) -> NumericID {
        self.id
    }

    /// Return the password hash.
    #[must_use]
    pub fn password_hash(&self) -> &str {
        &self.password_hash
    }

    /// Set the password hash, without touching `updated_at` (the caller
    /// updates the timestamp).
    ///
    /// # Errors
    ///
    /// Returns [`CredentialError::PasswordHashEmpty`] when `password_hash` is
    /// empty.
    pub fn set_password_hash(&mut self, password_hash: String) -> Result<(), CredentialError> {
        self.password_hash = Self::validate_password_hash(password_hash)?;
        Ok(())
    }

    /// Initialise a new `Credential`, validating `password_hash` is non-empty.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialError::PasswordHashEmpty`] when `password_hash` is
    /// empty.
    pub fn try_new(
        id: NumericID,
        password_hash: String,
        updated_at: NaiveDateTime,
    ) -> Result<Self, CredentialError> {
        let validated_hash = Self::validate_password_hash(password_hash)?;
        Ok(Self {
            id,
            password_hash: validated_hash,
            updated_at,
        })
    }

    /// Return the date and time of the last update of the password hash.
    #[must_use]
    pub fn updated_at(&self) -> NaiveDateTime {
        self.updated_at
    }

    /// Validate `password_hash`.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialError::PasswordHashEmpty`] when `password_hash` is
    /// empty.
    fn validate_password_hash(password_hash: String) -> Result<String, CredentialError> {
        if password_hash.is_empty() {
            return Err(CredentialError::PasswordHashEmpty);
        }
        Ok(password_hash)
    }
}
