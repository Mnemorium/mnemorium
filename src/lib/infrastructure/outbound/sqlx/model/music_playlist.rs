use chrono::NaiveDate;
use chrono::NaiveDateTime;

use crate::domain::alias::NumericID;

/// Row for a music playlist with one of its track placements, the recording the
/// placement references and that recording's backing file.
///
/// Assembled from `music_playlist` left-joined to `music_track`,
/// `music_recording`, `audio` and `file`. The track and recording columns are
/// absent when the playlist holds no placement. The track index is read as
/// `INTEGER` and converted by the repository so a value outside the domain range
/// is reported as a data-integrity failure.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MusicPlaylistRow {
    /// Identifier of the audio backing the recording, when the playlist
    /// sequences a recording.
    pub audio_id: Option<NumericID>,
    /// Identifier of the file holding the playlist's artwork, when set.
    pub cover_file_id: Option<NumericID>,
    /// Date and time of the creation of the playlist.
    pub created_at: NaiveDateTime,
    /// Identifier of the backing file, when the playlist sequences a recording.
    pub file_id: Option<NumericID>,
    /// Date of the first release of the recording, when the playlist sequences
    /// a recording.
    pub first_release_date: Option<NaiveDate>,
    /// Whether the playlist is visible to every user.
    pub is_public: bool,
    /// ISRC code of the recording, when set.
    pub isrc_code: Option<String>,
    /// Date and time of the last modification of the playlist.
    pub last_modified_at: NaiveDateTime,
    /// Unique identifier of the playlist.
    #[sqlx(primary_key)]
    pub music_playlist_id: NumericID,
    /// Identifier of the recording, when the playlist sequences one.
    pub music_recording_id: Option<NumericID>,
    /// Name of the playlist.
    pub name: String,
    /// Name of the recording, when set.
    pub recording_name: Option<String>,
    /// Index of the recording within the playlist, when the playlist sequences
    /// one.
    pub track_index: Option<i64>,
    /// Identifier of the user owning the playlist.
    pub user_id: NumericID,
}
