use std::sync::Arc;

use crate::application::port::begin_upload::BeginUploadUseCase;
use crate::application::port::complete_upload::CompleteUploadUseCase;
use crate::application::port::get_asset_content::GetAssetContentUseCase;
use crate::application::port::get_file::GetFileUseCase;
use crate::application::port::get_image::GetImageUseCase;
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

    /// Build the use case completing an upload session.
    fn complete_upload(&self) -> Arc<dyn CompleteUploadUseCase>;

    /// Build the use case reading the content of one asset.
    fn get_asset_content(&self) -> Arc<dyn GetAssetContentUseCase>;

    /// Build the use case fetching one stored file.
    fn get_file(&self) -> Arc<dyn GetFileUseCase>;

    /// Build the use case fetching one typed image.
    fn get_image(&self) -> Arc<dyn GetImageUseCase>;

    /// Build the use case fetching the state of an upload session.
    fn get_upload(&self) -> Arc<dyn GetUploadUseCase>;

    /// Build the use case writing one chunk of an upload session.
    fn write_upload_chunk(&self) -> Arc<dyn WriteUploadChunkUseCase>;
}
