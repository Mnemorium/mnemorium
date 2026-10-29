//! Read-only discovery of the files under the library root.
//!
//! Kept separate from [`crate::domain::port::file_storage::FileStorage`]: that
//! port is the write path for server-owned content, while this one only reads
//! the operator's mount. Mixing traversal into the write path would give one
//! port two owners and two trust levels.

use std::future::Future;

use chrono::NaiveDateTime;

use crate::domain::port::error::MediaSourceError;

/// One file found under the library root.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MediaEntry {
    /// Instant at which the content was last modified.
    pub modified_at: NaiveDateTime,
    /// Path relative to the library root.
    pub relative_path: String,
    /// Size of the content, in bytes.
    pub size_bytes: i64,
}

/// Port yielding the files under the library root, read-only.
///
/// The port exposes no write, move or delete operation: ingest must never
/// change the content it examines.
#[cfg_attr(test, mockall::automock)]
pub trait MediaSource: Send + Sync {
    /// Return the entry for the file at `relative_path`, or `Ok(None)` when no
    /// file exists there.
    ///
    /// A missing file or an absent root is `Ok(None)`, not an error.
    ///
    /// # Errors
    ///
    /// Returns [`MediaSourceError::OperationFailed`] or
    /// [`MediaSourceError::Unknown`] when the entry exists but its metadata
    /// cannot be read.
    fn stat(
        &self,
        relative_path: &str,
    ) -> impl Future<Output = Result<Option<MediaEntry>, MediaSourceError>> + Send;

    /// Return the files currently under the library root, or `Ok(None)` when
    /// the root itself is absent.
    ///
    /// An absent root is an outcome rather than an error: it usually means the
    /// volume did not mount, which the server reports without failing.
    ///
    /// # Errors
    ///
    /// Returns [`MediaSourceError::OperationFailed`] or
    /// [`MediaSourceError::Unknown`] when the root exists but cannot be walked.
    fn walk(
        &self,
    ) -> impl Future<Output = Result<Option<Vec<MediaEntry>>, MediaSourceError>> + Send;
}
