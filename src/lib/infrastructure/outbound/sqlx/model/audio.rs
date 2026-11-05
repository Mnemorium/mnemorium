use crate::domain::alias::NumericID;

/// Data model for the `audio` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct Audio {
    /// Primary key of the `audio` row.
    #[sqlx(primary_key)]
    pub audio_id: NumericID,
    /// Bits per sample, when the file reports it.
    pub bit_depth: Option<i64>,
    /// Block size in bytes, when the file reports it.
    pub block_size: Option<i64>,
    /// Codec name.
    pub codec: String,
    /// Duration in milliseconds.
    pub duration_ms: f64,
    /// Identifier of the `file` row holding the encoded audio.
    pub file_id: NumericID,
    /// Track name.
    pub name: String,
    /// Sample rate in hertz.
    pub sample_rate_hz: f64,
}
