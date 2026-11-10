use chrono::NaiveDateTime;

use crate::domain::NumericID;

/// Data model for the `upload` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct Upload {
    /// Size of each upload chunk, in bytes.
    pub chunk_size: i64,
    /// Instant the upload session was created.
    pub created_at: NaiveDateTime,
    /// Original file name.
    pub file_name: String,
    /// Declared size of the file, in bytes.
    pub file_size: i64,
    /// Content hash the completed upload must match.
    pub integrity_hash: String,
    /// Whether every chunk arrived.
    pub is_finished: bool,
    /// Whether a completion is verifying the staged content.
    pub is_verifying: bool,
    /// Identifier of the `mime_type` row.
    pub mime_type_id: String,
    /// Instant the current verification sealed the session, if any.
    pub sealed_at: Option<NaiveDateTime>,
    /// Primary key of the `upload` row.
    #[sqlx(primary_key)]
    pub upload_id: NumericID,
    /// Identifier of the `user` row that owns the upload.
    pub user_id: NumericID,
}
