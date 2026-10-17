use std::future::Future;

use chrono::NaiveDate;

use crate::domain::alias::NumericID;
use crate::domain::model::music_album::ProductionType;
use crate::domain::model::music_track::MusicTrack;
use crate::domain::port::error::RepositoryError;

/// Search filters for [`MusicAlbumRepository::search`]. Every field is optional;
/// an all-`None` filter returns every album.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct MusicAlbumFilter {
    /// Filter on the album identifier.
    pub id: Option<NumericID>,
    /// Filter on the album name.
    pub name: Option<String>,
}

/// Search filters for [`MusicAlbumRepository::search_tracks`]. Every field is
/// optional; an all-`None` filter returns every album track.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct MusicTrackFilter {
    /// Filter on the identifier of the positioning medium.
    pub music_medium_id: Option<NumericID>,
    /// Filter on the track identifier.
    pub music_track_id: Option<NumericID>,
}

/// Port for querying `MusicAlbum` and its tracks.
#[cfg_attr(test, mockall::automock)]
pub trait MusicAlbumRepository: Send + Sync {
    /// Search albums matching `filter`, returned as `Vec<MusicAlbumSummary>`.
    ///
    /// Returns an empty list when no album matches; a missing match is a valid
    /// outcome, not an error.
    fn search(
        &mut self,
        filter: &MusicAlbumFilter,
    ) -> impl Future<Output = Result<Vec<MusicAlbumSummary>, RepositoryError>> + Send;

    /// Search the tracks matching `filter`, returned as `Vec<MusicTrackDetail>`.
    ///
    /// An album's tracks are ordered by ascending medium index, then ascending
    /// track index. Rows attached to a playlist rather than a medium are out of
    /// scope and never returned. A missing match is a valid outcome, not an
    /// error.
    fn search_tracks(
        &mut self,
        filter: &MusicTrackFilter,
    ) -> impl Future<Output = Result<Vec<MusicTrackDetail>, RepositoryError>> + Send;
}

/// A `MusicAlbum` projection with its aggregated genres and track count.
///
/// A read model assembled from `music_album` plus its `music_album_genre` and
/// `music_track` rows, carrying the fields the album representation needs that
/// the `MusicAlbum` aggregate does not hold.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MusicAlbumSummary {
    /// Genres of the album, ordered by genre identifier.
    genres: Vec<String>,
    /// Unique identifier of the album.
    id: NumericID,
    /// Identifier of the medium the album holds, when it holds one.
    music_medium_id: Option<NumericID>,
    /// Name of the album.
    name: String,
    /// Production type of the album, when set.
    production_type: Option<ProductionType>,
    /// Date of the release of the album.
    release_date: NaiveDate,
    /// Number of tracks the album contains, across its mediums.
    track_count: usize,
}

impl MusicAlbumSummary {
    /// Return the genres of the album, ordered by genre identifier.
    #[must_use]
    pub fn genres(&self) -> &[String] {
        &self.genres
    }

    /// Return the unique identifier.
    #[must_use]
    pub fn id(&self) -> NumericID {
        self.id
    }

    /// Return the identifier of the album's medium, if it holds one.
    #[must_use]
    pub fn music_medium_id(&self) -> Option<NumericID> {
        self.music_medium_id
    }

    /// Return the name of the album.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Create a new album summary.
    #[must_use]
    pub fn new(
        id: NumericID,
        music_medium_id: Option<NumericID>,
        name: String,
        release_date: NaiveDate,
        production_type: Option<ProductionType>,
        genres: Vec<String>,
        track_count: usize,
    ) -> Self {
        Self {
            genres,
            id,
            music_medium_id,
            name,
            production_type,
            release_date,
            track_count,
        }
    }

    /// Return the production type, if any.
    #[must_use]
    pub fn production_type(&self) -> Option<ProductionType> {
        self.production_type
    }

    /// Return the date of the release.
    #[must_use]
    pub fn release_date(&self) -> NaiveDate {
        self.release_date
    }

    /// Return the number of tracks the album contains.
    #[must_use]
    pub fn track_count(&self) -> usize {
        self.track_count
    }
}

/// A `MusicTrack` together with the metadata of its recording and backing file.
///
/// A read model assembled from `music_track` joined to its medium, recording,
/// audio and file, carrying the fields the track representation needs that the
/// [`MusicTrack`] entity does not hold.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MusicTrackDetail {
    /// Identifier of the album the track belongs to.
    album_id: NumericID,
    /// Identifier of the backing file.
    file_id: NumericID,
    /// Date of the first release of the recording.
    first_release_date: NaiveDate,
    /// ISRC code of the recording, when set.
    isrc_code: Option<String>,
    /// Index of the medium the track is positioned on.
    medium_index: u32,
    /// Name of the track: the recording name, or the audio name as a fallback.
    name: String,
    /// The music track itself.
    track: MusicTrack,
}

impl MusicTrackDetail {
    /// Return the identifier of the album the track belongs to.
    #[must_use]
    pub fn album_id(&self) -> NumericID {
        self.album_id
    }

    /// Return the identifier of the backing file.
    #[must_use]
    pub fn file_id(&self) -> NumericID {
        self.file_id
    }

    /// Return the date of the first release of the recording.
    #[must_use]
    pub fn first_release_date(&self) -> NaiveDate {
        self.first_release_date
    }

    /// Return the ISRC code of the recording, if any.
    #[must_use]
    pub fn isrc_code(&self) -> Option<&str> {
        self.isrc_code.as_deref()
    }

    /// Return the index of the medium the track is positioned on.
    #[must_use]
    pub fn medium_index(&self) -> u32 {
        self.medium_index
    }

    /// Return the name of the track.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Create a new track detail.
    #[must_use]
    pub fn new(
        track: MusicTrack,
        album_id: NumericID,
        medium_index: u32,
        file_id: NumericID,
        name: String,
        isrc_code: Option<String>,
        first_release_date: NaiveDate,
    ) -> Self {
        Self {
            album_id,
            file_id,
            first_release_date,
            isrc_code,
            medium_index,
            name,
            track,
        }
    }

    /// Return the music track.
    #[must_use]
    pub fn track(&self) -> &MusicTrack {
        &self.track
    }
}

#[cfg(test)]
impl<T: MusicAlbumRepository + ?Sized> MusicAlbumRepository for &mut T {
    fn search(
        &mut self,
        filter: &MusicAlbumFilter,
    ) -> impl Future<Output = Result<Vec<MusicAlbumSummary>, RepositoryError>> + Send {
        (**self).search(filter)
    }

    fn search_tracks(
        &mut self,
        filter: &MusicTrackFilter,
    ) -> impl Future<Output = Result<Vec<MusicTrackDetail>, RepositoryError>> + Send {
        (**self).search_tracks(filter)
    }
}
