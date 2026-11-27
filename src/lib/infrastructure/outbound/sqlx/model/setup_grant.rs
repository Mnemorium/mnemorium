use std::fmt::{Debug, Formatter, Result as FmtResult};

use chrono::NaiveDateTime;

use crate::domain::NumericID;

/// Data model for the `root_admin_setup` table.
#[derive(Clone, sqlx::FromRow)]
pub(crate) struct SetupGrant {
    /// Instant the grant was consumed, when it has been.
    pub consumed_at: Option<NaiveDateTime>,
    /// Instant after which the grant is refused.
    pub expires_at: NaiveDateTime,
    /// Random value the setup token is derived from.
    pub nonce: String,
    /// Primary key of the singleton row.
    pub root_admin_setup_id: NumericID,
}

impl Debug for SetupGrant {
    /// Redact the nonce; only the identifier and instants stay readable.
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("SetupGrant")
            .field("consumed_at", &self.consumed_at)
            .field("expires_at", &self.expires_at)
            .field("nonce", &"**")
            .field("root_admin_setup_id", &self.root_admin_setup_id)
            .finish_non_exhaustive()
    }
}
