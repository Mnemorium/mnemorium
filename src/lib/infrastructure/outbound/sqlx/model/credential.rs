use std::fmt::{Debug, Formatter, Result as FmtResult};

use chrono::NaiveDateTime;

use crate::domain::NumericID;

/// Data model for the `credential` table.
#[derive(Clone, sqlx::FromRow)]
pub(crate) struct Credential {
    /// Primary key of the `credential` row.
    pub credential_id: NumericID,
    /// Argon2 hash of the user's password.
    pub password_hash: String,
    /// Instant the hash was last written.
    pub updated_at: NaiveDateTime,
}

impl Debug for Credential {
    /// Redact the password hash (`OBS-004`): it never reaches a log sink, while
    /// the credential identifier and timestamp stay readable.
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("Credential")
            .field("credential_id", &self.credential_id)
            .field("password_hash", &"**")
            .field("updated_at", &self.updated_at)
            .finish_non_exhaustive()
    }
}
