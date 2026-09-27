use chrono::NaiveDate;

use crate::domain::alias::NumericID;

/// Data model for the `file` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct File {
    #[sqlx(primary_key)]
    pub file_id: NumericID,
    pub is_public: bool,
    pub md5_integrity: String,
    pub mime_type_id: String,
    pub path: String,
    pub uploaded_at: NaiveDate,
    pub user_id: NumericID,
}
