use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;

/// Command to fetch one medium of a music album.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetAlbumMediumCommand {
    /// Identifier of the album the medium belongs to.
    album_id: NumericID,
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Album-scoped index of the medium to fetch.
    medium_index: u8,
}

impl GetAlbumMediumCommand {
    /// Return the identifier of the album the medium belongs to.
    #[must_use]
    pub fn album_id(&self) -> NumericID {
        self.album_id
    }

    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the album-scoped index of the medium to fetch.
    #[must_use]
    pub fn medium_index(&self) -> u8 {
        self.medium_index
    }

    /// Create a new fetch-album-medium command.
    #[must_use]
    pub fn new(caller_id: NumericID, album_id: NumericID, medium_index: u8) -> Self {
        Self {
            album_id,
            caller_id,
            medium_index,
        }
    }
}

/// Response of a successful medium lookup: one medium of an album.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetAlbumMediumResponse {
    /// Identifier of the album the medium belongs to.
    album_id: NumericID,
    /// Album-scoped index of the medium.
    medium_index: u8,
    /// Stored type of the medium (`CD`).
    medium_type: String,
    /// Number of recordings the medium sequences.
    track_count: usize,
}

impl GetAlbumMediumResponse {
    /// Return the identifier of the album the medium belongs to.
    #[must_use]
    pub fn album_id(&self) -> NumericID {
        self.album_id
    }

    /// Return the album-scoped index of the medium.
    #[must_use]
    pub fn medium_index(&self) -> u8 {
        self.medium_index
    }

    /// Return the stored type of the medium.
    #[must_use]
    pub fn medium_type(&self) -> &str {
        &self.medium_type
    }

    /// Create a new fetch-album-medium response.
    #[must_use]
    pub fn new(
        album_id: NumericID,
        medium_index: u8,
        medium_type: String,
        track_count: usize,
    ) -> Self {
        Self {
            album_id,
            medium_index,
            medium_type,
            track_count,
        }
    }

    /// Return the number of recordings the medium sequences.
    #[must_use]
    pub fn track_count(&self) -> usize {
        self.track_count
    }
}

/// Error returned when fetching one medium of an album.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GetAlbumMediumError {
    /// No album matches the requested identifier.
    #[error("a music album with this identifier does not exist")]
    NoSuchAlbum,
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// No medium of the album matches the requested index.
    #[error("a music medium with this index does not exist in this album")]
    NoSuchMedium,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for fetching one medium of a music album.
#[cfg_attr(test, mockall::automock)]
pub trait GetAlbumMediumUseCase: Send + Sync {
    /// Fetch the requested medium, scoped to its album.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn GetAlbumMediumUseCase>`. `Box` keeps the future on the heap at
    /// a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: GetAlbumMediumCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<GetAlbumMediumResponse, GetAlbumMediumError>>
                + Send
                + 'future,
        >,
    >;
}
