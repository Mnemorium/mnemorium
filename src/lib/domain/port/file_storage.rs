use std::future::Future;

use crate::domain::alias::NumericID;
use crate::domain::port::error::StorageError;

/// Port for storing uploaded file content on a storage backend.
///
/// The port covers the chunked upload flow: create a staging file for an upload
/// session, write chunks into it at their offset, promote it to its final path
/// once complete, and delete it when the session is abandoned. The adapter owns
/// the concrete layout on disk.
#[cfg_attr(test, mockall::automock)]
pub trait FileStorage: Send + Sync {
    /// Write `chunk` at `offset` bytes from the start of the staging file of the
    /// upload identified by `upload_id`.
    ///
    /// Writing the same chunk at the same offset is idempotent.
    fn add_chunk(
        &self,
        upload_id: NumericID,
        offset: u64,
        chunk: Vec<u8>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    /// Stream the staging file of the upload identified by `upload_id` and
    /// return its lowercase hexadecimal MD5 digest.
    ///
    /// The adapter reads the file in bounded chunks; the whole content is never
    /// loaded into memory.
    fn checksum(
        &self,
        upload_id: NumericID,
    ) -> impl Future<Output = Result<String, StorageError>> + Send;

    /// Create the staging file of the upload identified by `upload_id`,
    /// preallocated to `file_size` bytes.
    ///
    /// The adapter only touches the filesystem; the caller has already created
    /// the upload row.
    fn create_upload_file(
        &self,
        upload_id: NumericID,
        file_size: u64,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    /// Delete the staging file of the upload identified by `upload_id`.
    ///
    /// A missing staging file is treated as success.
    fn delete_upload_file(
        &self,
        upload_id: NumericID,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    /// Move the staging file of the upload identified by `upload_id` to its
    /// final path, returning the relative final path.
    ///
    /// The final name is derived from `upload_id` and the original `file_name`.
    fn promote(
        &self,
        upload_id: NumericID,
        file_name: &str,
    ) -> impl Future<Output = Result<String, StorageError>> + Send;
}
