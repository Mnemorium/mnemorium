use chrono::NaiveDateTime;

use crate::domain::alias::NumericID;

/// Data model for the `upload` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Upload {
    pub chunk_bitmap: Vec<u8>,
    pub chunk_size: i64,
    pub created_at: NaiveDateTime,
    pub file_name: String,
    pub file_size: i64,
    pub is_finished: bool,
    pub md5_integrity: String,
    pub mime_type_id: String,
    #[sqlx(primary_key)]
    pub upload_id: NumericID,
    pub user_id: NumericID,
    pub version: i64,
}
