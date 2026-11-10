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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_debug_redacts_the_password_hash_but_keeps_the_id() {
        // Arrange: a `credential` row whose stored hash is a unique sentinel.
        let row = Credential {
            credential_id: 7,
            password_hash: "LEAK_PASSWORD_HASH".to_owned(),
            updated_at: NaiveDateTime::default(),
        };

        // Act
        let debug = format!("{row:?}");

        // Assert
        assert!(
            !debug.contains("LEAK_PASSWORD_HASH"),
            "Debug output must not contain the password hash, got: {debug}"
        );
        assert!(
            debug.contains("credential_id: 7"),
            "Debug output must still contain the credential identifier, got: {debug}"
        );
    }
}
