use chrono::NaiveDate;

use crate::domain::alias::NumericID;

/// Read-model row for a music album joined to its genres and track count.
///
/// Assembled from `music_album` plus its `music_album_genre` and `music_track`
/// rows; `genres` is the comma-separated list of genre identifiers and
/// `track_count` the number of tracks the album holds across its mediums.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MusicAlbumSummary {
    /// Comma-separated genre identifiers of the album, ordered by identifier.
    pub genres: String,
    /// Unique identifier of the album.
    #[sqlx(primary_key)]
    pub music_album_id: NumericID,
    /// Identifier of the medium the album holds, when it holds one.
    pub music_medium_id: Option<NumericID>,
    /// Name of the album.
    pub name: String,
    /// Stored production type of the album, when set.
    pub production_type: Option<String>,
    /// Date of the release of the album.
    pub release_date: NaiveDate,
    /// Number of tracks the album contains.
    pub track_count: i64,
}
