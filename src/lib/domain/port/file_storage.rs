use std::future::Future;
use std::path::Path;

use crate::domain::model::integrity_hash::IntegrityHash;
use crate::domain::model::integrity_hash::SHA256_HEX_LENGTH;
use crate::domain::port::error::StorageError;

/// Port for storing uploaded file content on a storage backend.
///
/// The port covers the chunked upload flow: create a staging file for an upload
/// session, write chunks into it at their offset, promote it to its final path
/// once complete, and delete it when the session is abandoned.
///
/// The adapter knows nothing about the media-library layout: every method takes
/// the filesystem location it operates on. Composing those locations (the upload
/// and file folders, the naming scheme) is the caller's responsibility.
#[cfg_attr(test, mockall::automock)]
pub trait FileStorage: Send + Sync {
    /// Write `chunk` at `offset` bytes from the start of the file at `path`.
    ///
    /// Writing the same chunk at the same offset is idempotent.
    fn add_chunk(
        &self,
        path: &Path,
        offset: u64,
        chunk: Vec<u8>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    /// Create the directory at `path`, including any missing parent.
    ///
    /// The operation is idempotent: a directory that already exists is a
    /// success.
    fn create_directory(
        &self,
        path: &Path,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    /// Create the file at `path`, preallocated to `file_size` bytes.
    ///
    /// The caller has already created the upload row. A file that already exists
    /// at `path` is a leftover from an interrupted begin and is reclaimed, so it
    /// cannot wedge the identifier.
    fn create_upload_file(
        &self,
        path: &Path,
        file_size: u64,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    /// Delete the file at `path`.
    ///
    /// A missing file is treated as success.
    fn delete_upload_file(
        &self,
        path: &Path,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    /// Return whether a directory exists at `path`.
    ///
    /// A missing directory is `Ok(false)`, not an error.
    fn directory_exist(
        &self,
        path: &Path,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send;

    /// Stream the file at `path` and return its integrity hash.
    ///
    /// The adapter reads the file in bounded chunks; the whole content is never
    /// loaded into memory.
    fn integrity_hash(
        &self,
        path: &Path,
    ) -> impl Future<Output = Result<IntegrityHash<SHA256_HEX_LENGTH>, StorageError>> + Send;

    /// Move the file at `staged` to `final_path`.
    fn promote(
        &self,
        staged: &Path,
        final_path: &Path,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    /// Move the file at `final_path` back to `staged`.
    ///
    /// Compensates a completion that failed after `promote`, so the upload can
    /// be retried and no final file is orphaned. A missing final file is
    /// treated as success.
    fn restore(
        &self,
        final_path: &Path,
        staged: &Path,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;
}
