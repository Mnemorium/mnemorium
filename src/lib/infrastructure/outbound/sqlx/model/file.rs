use chrono::{NaiveDate, NaiveDateTime};

use crate::domain::alias::NumericID;

/// Data model for the `file` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct File {
    /// Primary key of the `file` row.
    #[sqlx(primary_key)]
    pub file_id: NumericID,
    /// Content hash, when the upload recorded one.
    pub integrity_hash: Option<String>,
    /// Whether every user may see the file.
    pub is_public: bool,
    /// Identifier of the `mime_type` row.
    pub mime_type_id: String,
    /// Instant the underlying file was last modified.
    pub modified_at: NaiveDateTime,
    /// Where the file came from.
    pub origin: String,
    /// Path relative to the library or staging root.
    pub path: String,
    /// Size in bytes.
    pub size_bytes: i64,
    /// Date the file was uploaded.
    pub uploaded_at: NaiveDate,
    /// Identifier of the `user` row that owns the file.
    pub user_id: NumericID,
}
