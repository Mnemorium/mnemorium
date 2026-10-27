use chrono::NaiveDate;

use crate::domain::alias::NumericID;

/// Row for a show with one of its seasons, one episode of that season and the
/// video backing that episode.
///
/// Assembled from `series` left-joined to its `series_genre`, `season`,
/// `episode` and `video` rows; `genres` is the comma-separated list of genre
/// identifiers ordered by identifier. The season, episode and video columns are
/// absent when the show holds no season, when the season holds no episode, or
/// when the stored episode has no readable video. Indexes are read as `INTEGER`
/// and converted by the repository so a value outside the domain range is
/// reported as a data-integrity failure.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SeriesRow {
    /// Date of the first airing of the episode, when the season holds one.
    pub air_date: Option<NaiveDate>,
    /// Codec of the video, when the season holds an episode.
    pub codec: Option<String>,
    /// Identifier of the colour space of the video, when the season holds an
    /// episode.
    pub color_id: Option<String>,
    /// Country of origin of the show.
    pub country_of_origin: String,
    /// Identifier of the file holding the show's artwork, when set.
    pub cover_file_id: Option<NumericID>,
    /// Duration of the video, in milliseconds, when the season holds an
    /// episode.
    pub duration_ms: Option<f64>,
    /// Index of the episode within its season, when the season holds one.
    pub episode_index: Option<i64>,
    /// Name of the episode, when the season holds one.
    pub episode_name: Option<String>,
    /// Plot of the episode, when the season holds one.
    pub episode_plot: Option<String>,
    /// Identifier of the backing file, when the season holds an episode.
    pub file_id: Option<NumericID>,
    /// Date of the first airing of the show.
    pub first_air_date: NaiveDate,
    /// Number of frames in the video, when the season holds an episode.
    pub frame_count: Option<i64>,
    /// Comma-separated genre identifiers of the show, ordered by identifier.
    pub genres: String,
    /// Height of the video, in pixels, when the season holds an episode.
    pub height: Option<i64>,
    /// Name of the show.
    pub name: String,
    /// Plot of the show.
    pub plot: String,
    /// Stored scan type of the video, when the season holds an episode.
    pub scan_type: Option<String>,
    /// Identifier of the file holding the season's artwork, when set.
    pub season_cover_file_id: Option<NumericID>,
    /// Identifier of the season, when the show holds one.
    pub season_id: Option<NumericID>,
    /// Index of the season within its show, when the show holds one.
    pub season_index: Option<i64>,
    /// Name of the season, when the show holds one and the season is named.
    pub season_name: Option<String>,
    /// Unique identifier of the show.
    #[sqlx(primary_key)]
    pub series_id: NumericID,
    /// Unique identifier of the video, when the season holds an episode.
    pub video_id: Option<NumericID>,
    /// Width of the video, in pixels, when the season holds an episode.
    pub width: Option<i64>,
}
