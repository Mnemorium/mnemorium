use std::future::Future;
use std::pin::Pin;

/// One change observed under the library root.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MediaChange {
    /// A file appeared at the relative path.
    Created(String),
    /// A file no longer exists at the relative path.
    Removed(String),
    /// A file moved from `from` to `to`.
    Renamed {
        /// The recorded relative path.
        from: String,
        /// The new relative path.
        to: String,
    },
}

/// Error returned when the change source cannot deliver an event.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MediaChangeSourceError {
    /// The change queue overflowed, so changes were lost.
    #[error("the change queue overflowed")]
    Overflow,
    /// An unexpected or unmapped error occurred.
    #[error("an unexpected or unmapped error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
    /// The watcher reported an error while observing the library root.
    #[error("the library root watcher reported an error")]
    WatchError,
    /// The watch could not be registered; the startup scan remains the
    /// mechanism of record.
    #[error("the library root could not be watched")]
    WatchFailed,
}

/// Inbound port delivering the library root's changes to the runtime.
///
/// The source never blocks the watcher thread: an adapter hands each change to
/// an internal queue that [`MediaChangeSource::changes`] drains.
#[cfg_attr(test, mockall::automock)]
pub trait MediaChangeSource: Send + Sync {
    /// Return the next observed change, waiting until one is available.
    ///
    /// An overflow is reported once as an error and does not end the
    /// subscription: the startup scan repairs whatever the lost events missed.
    ///
    /// # Errors
    ///
    /// Returns [`MediaChangeSourceError::Overflow`] when the queue overflowed,
    /// and [`MediaChangeSourceError::WatchFailed`] or
    /// [`MediaChangeSourceError::Unknown`] when the source cannot deliver.
    fn changes<'future>(
        &'future self,
    ) -> Pin<Box<dyn Future<Output = Result<MediaChange, MediaChangeSourceError>> + Send + 'future>>;
}
