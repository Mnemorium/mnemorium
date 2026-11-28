use std::fmt::{Debug, Formatter, Result as FmtResult};

use chrono::NaiveDateTime;

use crate::domain::NumericID;

/// Error returned when initialising or updating a `Credential`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum CredentialError {
    /// The password hash is empty.
    #[error("password hash must not be empty")]
    PasswordHashEmpty,
}

/// A credential, holding the password hash used for authentication.
#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct Credential {
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
    pub(crate) fn id(&self) -> NumericID {
        self.id
    }

    /// Return the password hash.
    #[must_use]
    pub(crate) fn password_hash(&self) -> &str {
        &self.password_hash
    }

    /// Initialise a new `Credential`, validating `password_hash` is non-empty.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialError::PasswordHashEmpty`] when `password_hash` is
    /// empty.
    pub(crate) fn try_new(
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
    pub(crate) fn updated_at(&self) -> NaiveDateTime {
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

impl Debug for Credential {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("Credential")
            .field("id", &self.id)
            .field("password_hash", &"**")
            .field("updated_at", &self.updated_at)
            .finish_non_exhaustive()
    }
}
