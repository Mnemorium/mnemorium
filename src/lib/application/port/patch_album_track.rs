use std::future::Future;
use std::pin::Pin;

use crate::application::field_update::FieldUpdate;
use crate::domain::alias::NumericID;
use crate::domain::model::music_album::MusicAlbum;

/// Command to edit the shared recording behind a track address.
///
/// An absent field keeps its stored value; a field the request clears drops an
/// optional value.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct PatchAlbumTrackCommand {
    /// Identifier of the album that holds the medium.
    album_id: NumericID,
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Replacement ISRC code, when the request names one; `Clear` drops it.
    isrc_code: FieldUpdate<String>,
    /// Album-scoped index of the medium.
    medium_index: u8,
    /// Replacement name, when the request names one; `Clear` drops it.
    name: FieldUpdate<String>,
    /// Index of the track within the medium.
    track_index: u8,
}

impl PatchAlbumTrackCommand {
    /// Return the identifier of the album that holds the medium.
    #[must_use]
    pub(crate) fn album_id(&self) -> NumericID {
        self.album_id
    }

    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the replacement ISRC code, if the request names one.
    #[must_use]
    pub(crate) fn isrc_code(&self) -> &FieldUpdate<String> {
        &self.isrc_code
    }

    /// Return the album-scoped index of the medium.
    #[must_use]
    pub(crate) fn medium_index(&self) -> u8 {
        self.medium_index
    }

    /// Return the replacement name, if the request names one.
    #[must_use]
    pub(crate) fn name(&self) -> &FieldUpdate<String> {
        &self.name
    }

    /// Create a new patch-track command.
    #[must_use]
    pub(crate) fn new(
        caller_id: NumericID,
        album_id: NumericID,
        medium_index: u8,
        track_index: u8,
        name: FieldUpdate<String>,
        isrc_code: FieldUpdate<String>,
    ) -> Self {
        Self {
            album_id,
            caller_id,
            isrc_code,
            medium_index,
            name,
            track_index,
        }
    }

    /// Return the index of the track within the medium.
    #[must_use]
    pub(crate) fn track_index(&self) -> u8 {
        self.track_index
    }
}

/// Response of a successful track edit: the stored album and the edited
/// recording's address.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct PatchAlbumTrackResponse {
    /// The stored album with its mediums.
    album: MusicAlbum,
    /// Album-scoped index of the medium.
    medium_index: u8,
    /// Index of the track within the medium.
    track_index: u8,
}

impl PatchAlbumTrackResponse {
    /// Return the stored album.
    #[must_use]
    pub(crate) fn album(&self) -> &MusicAlbum {
        &self.album
    }

    /// Return the album-scoped index of the medium.
    #[must_use]
    pub(crate) fn medium_index(&self) -> u8 {
        self.medium_index
    }

    /// Create a new edited-track response.
    #[must_use]
    pub(crate) fn new(album: MusicAlbum, medium_index: u8, track_index: u8) -> Self {
        Self {
            album,
            medium_index,
            track_index,
        }
    }

    /// Return the index of the track within the medium.
    #[must_use]
    pub(crate) fn track_index(&self) -> u8 {
        self.track_index
    }
}

/// Error returned when editing the recording behind a track.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum PatchAlbumTrackError {
    /// The write conflicts with the stored recording's state.
    #[error("the music recording cannot be written")]
    Conflict,
    /// The default album is immutable.
    #[error("the default music album cannot be written")]
    DefaultAlbum,
    /// The caller may not manage the library catalogue.
    #[error("the caller may not manage the library")]
    Forbidden,
    /// The ISRC code is not exactly twelve characters long.
    #[error("the ISRC code must be exactly twelve characters long")]
    InvalidIsrc,
    /// The name is empty.
    #[error("the name must not be empty")]
    InvalidName,
    /// Another recording already carries the ISRC code.
    #[error("a music recording with this ISRC code already exists")]
    IsrcAlreadyExists,
    /// No album matches the requested identifier.
    #[error("a music album with this identifier does not exist")]
    NoSuchAlbum,
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// No medium matches the requested album and index.
    #[error("a music medium with this index does not exist")]
    NoSuchMedium,
    /// No track matches the requested address.
    #[error("a music track with this index does not exist")]
    NoSuchTrack,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for editing the shared recording behind a track address.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait PatchAlbumTrackUseCase: Send + Sync {
    /// Edit the recording placed at the requested address.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn PatchAlbumTrackUseCase>`. `Box` keeps the future on the heap at
    /// a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: PatchAlbumTrackCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PatchAlbumTrackResponse, PatchAlbumTrackError>>
                + Send
                + 'future,
        >,
    >;
}
