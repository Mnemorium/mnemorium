use std::sync::Arc;

use arc_swap::ArcSwap;

use crate::application::port::asset_use_case_factory::AssetUseCaseFactory;
use crate::application::port::begin_upload::BeginUploadUseCase;
use crate::application::port::check_upload::CheckUploadUseCase;
use crate::application::port::complete_upload::CompleteUploadUseCase;
use crate::application::port::get_upload::GetUploadUseCase;
use crate::application::port::write_upload_chunk::WriteUploadChunkUseCase;
use crate::application::use_case::begin_upload::BeginUpload;
use crate::application::use_case::check_upload::CheckUpload;
use crate::application::use_case::complete_upload::CompleteUpload;
use crate::application::use_case::get_upload::GetUpload;
use crate::application::use_case::write_upload_chunk::WriteUploadChunk;
use crate::domain::model::configuration::Configuration;
use crate::infrastructure::outbound::file_system::file_storage::FileSystemStorage;
use crate::infrastructure::outbound::sqlx::unit_of_work::SqlxUnitOfWorkFactory;

/// Builds the Asset use cases from the live configuration.
///
/// Every accessor reads the current configuration for the chunk size and the
/// upload expiry, so a runtime configuration change is picked up by the next
/// request.
pub struct RuntimeAssetUseCaseFactory {
    /// Live application configuration.
    configuration: Arc<ArcSwap<Configuration>>,
    /// Storage adapter backing the chunked upload flow.
    file_storage: Arc<FileSystemStorage>,
    /// Factory opening the unit of work wrapping the Asset use cases.
    unit_of_work_factory: Arc<SqlxUnitOfWorkFactory>,
}

impl RuntimeAssetUseCaseFactory {
    /// Create a new factory.
    #[must_use]
    pub fn new(
        configuration: Arc<ArcSwap<Configuration>>,
        file_storage: Arc<FileSystemStorage>,
        unit_of_work_factory: Arc<SqlxUnitOfWorkFactory>,
    ) -> Self {
        Self {
            configuration,
            file_storage,
            unit_of_work_factory,
        }
    }
}

impl AssetUseCaseFactory for RuntimeAssetUseCaseFactory {
    fn begin_upload(&self) -> Arc<dyn BeginUploadUseCase> {
        let live = self.configuration.load();
        Arc::new(BeginUpload::new(
            Arc::clone(&self.unit_of_work_factory),
            Arc::clone(&self.file_storage),
            live.asset().upload().chunk_size_bytes(),
            live.asset().upload().expiry_seconds(),
        ))
    }

    fn check_upload(&self) -> Arc<dyn CheckUploadUseCase> {
        Arc::new(CheckUpload::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn complete_upload(&self) -> Arc<dyn CompleteUploadUseCase> {
        let live = self.configuration.load();
        Arc::new(CompleteUpload::new(
            Arc::clone(&self.unit_of_work_factory),
            Arc::clone(&self.file_storage),
            live.asset().upload().expiry_seconds(),
        ))
    }

    fn get_upload(&self) -> Arc<dyn GetUploadUseCase> {
        let live = self.configuration.load();
        Arc::new(GetUpload::new(
            Arc::clone(&self.unit_of_work_factory),
            Arc::clone(&self.file_storage),
            live.asset().upload().expiry_seconds(),
        ))
    }

    fn write_upload_chunk(&self) -> Arc<dyn WriteUploadChunkUseCase> {
        let live = self.configuration.load();
        Arc::new(WriteUploadChunk::new(
            Arc::clone(&self.unit_of_work_factory),
            Arc::clone(&self.file_storage),
            live.asset().upload().expiry_seconds(),
        ))
    }
}
