use chrono::NaiveDate;

use crate::domain::alias::NumericID;

/// Read-model row for a music track joined to its medium, recording, audio and
/// backing file.
///
/// `name` is the recording name, falling back to the audio name; `file_id` is
/// the identifier of the backing file the recording's audio references.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MusicTrackDetail {
    /// Identifier of the backing file.
    pub file_id: NumericID,
    /// Date of the first release of the recording.
    pub first_release_date: NaiveDate,
    /// ISRC code of the recording, when set.
    pub isrc_code: Option<String>,
    /// Index of the medium the track is positioned on.
    pub medium_index: u32,
    /// Identifier of the album the track belongs to.
    pub music_album_id: NumericID,
    /// Identifier of the medium the track is positioned by.
    pub music_medium_id: NumericID,
    /// Identifier of the recording the track plays.
    pub music_recording_id: NumericID,
    /// Unique identifier of the track.
    #[sqlx(primary_key)]
    pub music_track_id: NumericID,
    /// Name of the track: the recording name, or the audio name as a fallback.
    pub name: String,
    /// Position of the track within its medium.
    pub track_index: i64,
}
