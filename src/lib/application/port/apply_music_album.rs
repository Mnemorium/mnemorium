use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;
use crate::domain::model::external_ref::ExternalRef;
use crate::domain::model::music_album::MusicAlbum;

/// Command to apply external metadata to a music album.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct ApplyMusicAlbumCommand {
    /// Identifier of the album to write.
    album_id: NumericID,
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Recordings to move into the applied structure; empty leaves every
    /// placement unchanged.
    placements: Vec<AlbumPlacement>,
    /// Reference to the provider's work.
    reference: ExternalRef,
}

impl ApplyMusicAlbumCommand {
    /// Return the identifier of the album to write.
    #[must_use]
    pub(crate) fn album_id(&self) -> NumericID {
        self.album_id
    }

    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Create a new apply command.
    #[must_use]
    pub(crate) fn new(
        caller_id: NumericID,
        album_id: NumericID,
        reference: ExternalRef,
        placements: Vec<AlbumPlacement>,
    ) -> Self {
        Self {
            album_id,
            caller_id,
            placements,
            reference,
        }
    }

    /// Return the placements to apply.
    #[must_use]
    pub(crate) fn placements(&self) -> &[AlbumPlacement] {
        &self.placements
    }

    /// Return the reference to the provider's work.
    #[must_use]
    pub(crate) fn reference(&self) -> &ExternalRef {
        &self.reference
    }
}

/// Response of a successful apply: the stored album.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct ApplyMusicAlbumResponse {
    /// The album that was stored.
    album: MusicAlbum,
}

impl ApplyMusicAlbumResponse {
    /// Return the album that was stored.
    #[must_use]
    pub(crate) fn album(&self) -> &MusicAlbum {
        &self.album
    }

    /// Create a new applied-album response.
    #[must_use]
    pub(crate) fn new(album: MusicAlbum) -> Self {
        Self { album }
    }
}

/// Error returned when applying external metadata to an album.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum ApplyMusicAlbumError {
    /// The write duplicates another album's `(name, group)` identity, or a
    /// placement collides with an occupied destination.
    #[error("the applied metadata conflicts with the stored catalogue")]
    Conflict,
    /// The default album is immutable.
    #[error("the default music album cannot be written")]
    DefaultAlbum,
    /// The caller may not manage the library catalogue.
    #[error("the caller may not manage the library")]
    Forbidden,
    /// A placement names a destination outside the applied medium structure, or
    /// a recording the catalogue does not know.
    #[error("the request names an invalid placement")]
    InvalidPlacement,
    /// The provider reports a work the album apply cannot write.
    #[error("the provider reports a work that cannot be applied to an album")]
    InvalidReference,
    /// No album matches the requested identifier.
    #[error("a music album with this identifier does not exist")]
    NoSuchAlbum,
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// The provider failed while resolving the reference.
    #[error("the music metadata provider failed")]
    ProviderFailure,
    /// No music metadata provider is configured.
    #[error("no music metadata provider is configured")]
    ProviderUnavailable,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
    /// The reference names a provider other than the configured one.
    #[error("the music metadata provider is not configured")]
    UnknownProvider,
    /// The provider does not know the reference.
    #[error("the music metadata provider does not know the reference")]
    UnknownReference,
}

/// One recording the apply places at a destination address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct AlbumPlacement {
    /// Album-scoped index of the destination medium.
    medium_index: u8,
    /// Identifier of the recording to move.
    recording_id: NumericID,
    /// Index of the destination track within the medium.
    track_index: u8,
}

impl AlbumPlacement {
    /// Return the album-scoped index of the destination medium.
    #[must_use]
    pub(crate) fn medium_index(&self) -> u8 {
        self.medium_index
    }

    /// Create a new placement.
    #[must_use]
    pub(crate) fn new(medium_index: u8, track_index: u8, recording_id: NumericID) -> Self {
        Self {
            medium_index,
            recording_id,
            track_index,
        }
    }

    /// Return the identifier of the recording to move.
    #[must_use]
    pub(crate) fn recording_id(&self) -> NumericID {
        self.recording_id
    }

    /// Return the index of the destination track within the medium.
    #[must_use]
    pub(crate) fn track_index(&self) -> u8 {
        self.track_index
    }
}

/// Use case for applying external metadata to a music album.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait ApplyMusicAlbumUseCase: Send + Sync {
    /// Resolve the reference and write the album and its medium structure.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn ApplyMusicAlbumUseCase>`. `Box` keeps the future on the heap at
    /// a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: ApplyMusicAlbumCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ApplyMusicAlbumResponse, ApplyMusicAlbumError>>
                + Send
                + 'future,
        >,
    >;
}
