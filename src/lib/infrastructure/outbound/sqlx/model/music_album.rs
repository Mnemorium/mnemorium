use chrono::NaiveDate;

use crate::domain::alias::NumericID;

/// Row for a music album with one of its mediums and one recording the medium
/// sequences, with the recording's backing file.
///
/// Assembled from `music_album` left-joined to its `music_medium`,
/// `music_track`, `music_recording`, `audio` and `file` rows; `genres` is the
/// comma-separated list of genre identifiers. The medium and recording columns
/// are absent when the album holds no medium, or when the medium sequences no
/// recording. Indexes are read as `INTEGER` and converted by the repository so
/// a value outside the domain range is reported as a data-integrity failure.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MusicAlbumRow {
    /// Identifier of the audio backing the recording, when the medium
    /// sequences a recording.
    pub audio_id: Option<NumericID>,
    /// Identifier of the file holding the album's artwork, when set.
    pub cover_file_id: Option<NumericID>,
    /// Identifier of the backing file, when the medium sequences a recording.
    pub file_id: Option<NumericID>,
    /// Date of the first release of the recording, when the medium sequences a
    /// recording.
    pub first_release_date: Option<NaiveDate>,
    /// Comma-separated genre identifiers of the album, ordered by identifier.
    pub genres: String,
    /// ISRC code of the recording, when set.
    pub isrc_code: Option<String>,
    /// Index of the medium within its album, when the album holds one.
    pub medium_index: Option<i64>,
    /// Stored type of the medium, when the album holds one.
    pub medium_type: Option<String>,
    /// Unique identifier of the album.
    #[sqlx(primary_key)]
    pub music_album_id: NumericID,
    /// Identifier of the medium, when the album holds one.
    pub music_medium_id: Option<NumericID>,
    /// Identifier of the recording, when the medium sequences one.
    pub music_recording_id: Option<NumericID>,
    /// Name of the album.
    pub name: String,
    /// Stored production type of the album, when set.
    pub production_type: Option<String>,
    /// Name of the recording, when set.
    pub recording_name: Option<String>,
    /// Date of the release of the album.
    pub release_date: NaiveDate,
    /// Index of the recording within the medium, when the medium sequences one.
    pub track_index: Option<i64>,
}
