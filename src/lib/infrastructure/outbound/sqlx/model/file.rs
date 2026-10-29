use chrono::NaiveDate;
use chrono::NaiveDateTime;

use crate::domain::alias::NumericID;

/// Data model for the `file` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct File {
    #[sqlx(primary_key)]
    pub file_id: NumericID,
    pub integrity_hash: Option<String>,
    pub is_public: bool,
    pub mime_type_id: String,
    pub modified_at: NaiveDateTime,
    pub origin: String,
    pub path: String,
    pub size_bytes: i64,
    pub uploaded_at: NaiveDate,
    pub user_id: NumericID,
}
