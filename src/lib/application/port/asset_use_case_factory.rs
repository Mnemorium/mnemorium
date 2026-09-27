use std::sync::Arc;

use crate::application::port::begin_upload::BeginUploadUseCase;
use crate::application::port::check_upload::CheckUploadUseCase;
use crate::application::port::complete_upload::CompleteUploadUseCase;
use crate::application::port::get_upload::GetUploadUseCase;
use crate::application::port::write_upload_chunk::WriteUploadChunkUseCase;

/// Builds the use cases of the Asset bounded context on demand.
///
/// The inbound layer depends on this port instead of a pre-built use case, so
/// that every request gets a use case bound to the current configuration.
#[cfg_attr(test, mockall::automock)]
pub trait AssetUseCaseFactory: Send + Sync {
    /// Build the use case beginning an upload session.
    fn begin_upload(&self) -> Arc<dyn BeginUploadUseCase>;

    /// Build the use case checking for an existing file.
    fn check_upload(&self) -> Arc<dyn CheckUploadUseCase>;

    /// Build the use case completing an upload session.
    fn complete_upload(&self) -> Arc<dyn CompleteUploadUseCase>;

    /// Build the use case fetching the state of an upload session.
    fn get_upload(&self) -> Arc<dyn GetUploadUseCase>;

    /// Build the use case writing one chunk of an upload session.
    fn write_upload_chunk(&self) -> Arc<dyn WriteUploadChunkUseCase>;
}
