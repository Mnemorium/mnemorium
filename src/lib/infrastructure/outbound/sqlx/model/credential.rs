use chrono::NaiveDateTime;

use crate::domain::alias::NumericID;

/// Data model for the `credential` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Credential {
    pub credential_id: NumericID,
    pub password_hash: String,
    pub updated_at: NaiveDateTime,
}
