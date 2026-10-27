use chrono::NaiveDate;

use crate::domain::alias::NumericID;

/// Row for a film with the video that backs it.
///
/// Assembled from a `movie` row joined to its `video` row; `genres` is the
/// comma-separated list of genre identifiers ordered by identifier. The video
/// columns are read as stored values so the repository can report a value the
/// domain cannot hold as a data-integrity failure.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MovieRow {
    /// Codec of the video.
    pub codec: String,
    /// Identifier of the colour space of the video.
    pub color_id: String,
    /// Country of origin of the film.
    pub country_of_origin: String,
    /// Duration of the video, in milliseconds.
    pub duration_ms: f64,
    /// Identifier of the backing file.
    pub file_id: NumericID,
    /// Number of frames in the video.
    pub frame_count: i64,
    /// Comma-separated genre identifiers of the film, ordered by identifier.
    pub genres: String,
    /// Height of the video, in pixels.
    pub height: i64,
    /// Unique identifier of the film.
    #[sqlx(primary_key)]
    pub movie_id: NumericID,
    /// Name of the film.
    pub name: String,
    /// Plot of the film.
    pub plot: String,
    /// Date of the release of the film.
    pub release_date: NaiveDate,
    /// Stored scan type of the video.
    pub scan_type: String,
    /// Unique identifier of the video.
    pub video_id: NumericID,
    /// Width of the video, in pixels.
    pub width: i64,
}
