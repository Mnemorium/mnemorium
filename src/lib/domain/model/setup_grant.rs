use std::fmt::{Debug, Formatter, Result as FmtResult};

use chrono::NaiveDateTime;

use crate::domain::NumericID;

/// Identifier of the single Root Admin setup-grant row.
pub(crate) const SETUP_GRANT_ID: NumericID = 0;

/// Smallest accepted length of a setup-grant nonce, in characters.
pub(crate) const MIN_SETUP_GRANT_NONCE_LENGTH: usize = 32;

/// Error returned when initialising or updating a `SetupGrant`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum SetupGrantError {
    /// The expiry is not after the instant the grant was built.
    #[error("setup grant expiry must be in the future")]
    ExpiryInPast,
    /// The nonce is shorter than the declared minimum.
    #[error("setup grant nonce must be at least {MIN_SETUP_GRANT_NONCE_LENGTH} characters long")]
    NonceTooShort,
}

/// The single Root Admin setup grant.
///
/// The grant binds a nonce, an expiry and a consumption instant to the pending
/// setup; it deliberately holds no token material, so a copy of the datastore
/// alone neither reveals the token nor verifies a guess.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct SetupGrant {
    /// Instant the grant was consumed, when it has been.
    consumed_at: Option<NaiveDateTime>,
    /// Instant after which the grant is refused.
    expires_at: NaiveDateTime,
    /// Identifier of the single stored row.
    id: NumericID,
    /// Random value the setup token is derived from.
    nonce: String,
}

impl SetupGrant {
    /// Return the instant the grant was consumed, if it has been.
    #[must_use]
    pub(crate) fn consumed_at(&self) -> Option<NaiveDateTime> {
        self.consumed_at
    }

    /// Return the instant after which the grant is refused.
    #[must_use]
    pub(crate) fn expires_at(&self) -> NaiveDateTime {
        self.expires_at
    }

    /// Build a pending grant from a stored row.
    ///
    /// A stored row may already be expired or consumed, so this constructor
    /// checks only the nonce invariant; the use case and the conditional
    /// consume decide whether the grant is still usable.
    ///
    /// # Errors
    ///
    /// Returns [`SetupGrantError::NonceTooShort`] when `nonce` is shorter than
    /// [`MIN_SETUP_GRANT_NONCE_LENGTH`].
    pub(crate) fn from_stored(
        id: NumericID,
        nonce: String,
        expires_at: NaiveDateTime,
        consumed_at: Option<NaiveDateTime>,
    ) -> Result<Self, SetupGrantError> {
        Self::validate_nonce(&nonce)?;
        Ok(Self {
            consumed_at,
            expires_at,
            id,
            nonce,
        })
    }

    /// Return the identifier of the single stored row.
    #[must_use]
    pub(crate) fn id(&self) -> NumericID {
        self.id
    }

    /// Return whether the grant is still usable at `now`.
    #[must_use]
    pub(crate) fn is_live(&self, now: NaiveDateTime) -> bool {
        self.consumed_at.is_none() && self.expires_at > now
    }

    /// Return the random value the setup token is derived from.
    #[must_use]
    pub(crate) fn nonce(&self) -> &str {
        &self.nonce
    }

    /// Return a renewed copy of this grant with a fresh nonce and expiry.
    ///
    /// The stored identifier is kept so the replacement targets the same row.
    ///
    /// # Errors
    ///
    /// Returns [`SetupGrantError::NonceTooShort`] when `nonce` is shorter than
    /// [`MIN_SETUP_GRANT_NONCE_LENGTH`], and
    /// [`SetupGrantError::ExpiryInPast`] when `expires_at` is not after `now`.
    pub(crate) fn renew(
        &self,
        nonce: String,
        expires_at: NaiveDateTime,
        now: NaiveDateTime,
    ) -> Result<Self, SetupGrantError> {
        if expires_at <= now {
            return Err(SetupGrantError::ExpiryInPast);
        }
        Self::from_stored(self.id, nonce, expires_at, None)
    }

    /// Initialise a new pending grant.
    ///
    /// # Errors
    ///
    /// Returns [`SetupGrantError::NonceTooShort`] when `nonce` is shorter than
    /// [`MIN_SETUP_GRANT_NONCE_LENGTH`], and
    /// [`SetupGrantError::ExpiryInPast`] when `expires_at` is not after `now`.
    pub(crate) fn try_new(
        nonce: String,
        expires_at: NaiveDateTime,
        now: NaiveDateTime,
    ) -> Result<Self, SetupGrantError> {
        if expires_at <= now {
            return Err(SetupGrantError::ExpiryInPast);
        }
        Self::from_stored(SETUP_GRANT_ID, nonce, expires_at, None)
    }

    /// Validate `nonce`.
    ///
    /// # Errors
    ///
    /// Returns [`SetupGrantError::NonceTooShort`] when `nonce` is shorter than
    /// [`MIN_SETUP_GRANT_NONCE_LENGTH`].
    fn validate_nonce(nonce: &str) -> Result<(), SetupGrantError> {
        if nonce.chars().count() < MIN_SETUP_GRANT_NONCE_LENGTH {
            return Err(SetupGrantError::NonceTooShort);
        }
        Ok(())
    }
}

impl Debug for SetupGrant {
    /// Redact the nonce: only the identifiers and instants stay readable.
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("SetupGrant")
            .field("consumed_at", &self.consumed_at)
            .field("expires_at", &self.expires_at)
            .field("id", &self.id)
            .field("nonce", &"**")
            .finish_non_exhaustive()
    }
}
