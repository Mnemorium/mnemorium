use chrono::NaiveDate;

use crate::domain::alias::NumericID;

/// Row for a music album with one of its mediums, that medium's next track and
/// the track's recording and backing file.
///
/// Assembled from `music_album` left-joined to its `music_medium`,
/// `music_track`, `music_recording`, `audio` and `file` rows; `genres` is the
/// comma-separated list of genre identifiers. The medium and recording columns
/// are absent when the album holds no medium, or when the medium holds no
/// track.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MusicAlbumRow {
    /// Identifier of the audio backing the recording, when the medium holds a
    /// track.
    pub audio_id: Option<NumericID>,
    /// Identifier of the backing file, when the medium holds a track.
    pub file_id: Option<NumericID>,
    /// Date of the first release of the recording, when the medium holds a
    /// track.
    pub first_release_date: Option<NaiveDate>,
    /// Comma-separated genre identifiers of the album, ordered by identifier.
    pub genres: String,
    /// ISRC code of the recording, when set.
    pub isrc_code: Option<String>,
    /// Index of the medium within its album, when the album holds one.
    pub medium_index: Option<u32>,
    /// Type of the medium, when the album holds one.
    pub medium_type: Option<String>,
    /// Unique identifier of the album.
    #[sqlx(primary_key)]
    pub music_album_id: NumericID,
    /// Identifier of the medium, when the album holds one.
    pub music_medium_id: Option<NumericID>,
    /// Identifier of the recording, when the medium holds a track.
    pub music_recording_id: Option<NumericID>,
    /// Name of the album.
    pub name: String,
    /// Stored production type of the album, when set.
    pub production_type: Option<String>,
    /// Name of the recording, when set.
    pub recording_name: Option<String>,
    /// Date of the release of the album.
    pub release_date: NaiveDate,
}
