use crate::domain::alias::NumericID;

/// Row for an album track, positioned by the medium that holds it.
///
/// Read from `music_track` joined to `music_medium`: the join is what scopes
/// the query to album tracks, since a row attached to a playlist has no medium.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MusicTrackRow {
    /// Identifier of the medium the track is positioned by.
    pub music_medium_id: NumericID,
    /// Identifier of the recording the track plays.
    pub music_recording_id: NumericID,
    /// Unique identifier of the track.
    #[sqlx(primary_key)]
    pub music_track_id: NumericID,
    /// Position of the track within its medium.
    pub track_index: i64,
}
