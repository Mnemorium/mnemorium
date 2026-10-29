use crate::domain::alias::NumericID;

/// Data model for the `audio` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Audio {
    #[sqlx(primary_key)]
    pub audio_id: NumericID,
    pub bit_depth: Option<i64>,
    pub block_size: Option<i64>,
    pub codec: String,
    pub duration_ms: f64,
    pub file_id: NumericID,
    pub name: String,
    pub sample_rate_hz: f64,
}
