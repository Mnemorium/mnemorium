use std::path::PathBuf;
use std::sync::Arc;

use arc_swap::ArcSwap;
use tokio::sync::Semaphore;

use crate::application::port::asset_use_case_factory::AssetUseCaseFactory;
use crate::application::port::begin_upload::BeginUploadUseCase;
use crate::application::port::complete_upload::CompleteUploadUseCase;
use crate::application::port::get_upload::GetUploadUseCase;
use crate::application::port::write_upload_chunk::WriteUploadChunkUseCase;
use crate::application::use_case::begin_upload::BeginUpload;
use crate::application::use_case::complete_upload::CompleteUpload;
use crate::application::use_case::get_upload::GetUpload;
use crate::application::use_case::write_upload_chunk::WriteUploadChunk;
use crate::domain::model::configuration::Configuration;
use crate::infrastructure::outbound::file_system::file_storage::FileSystemStorage;
use crate::infrastructure::outbound::media_probe::LocalMediaProbe;
use crate::infrastructure::outbound::sha2::content_hasher::Sha2ContentHasher;
use crate::infrastructure::outbound::sqlx::unit_of_work::SqlxUnitOfWorkFactory;

/// Builds the Asset use cases from the live configuration.
///
/// Every accessor reads the current configuration for the chunk size and the
/// upload expiry, so a runtime configuration change is picked up by the next
/// request. The file storage adapter is stateless and rebuilt per accessor from
/// the startup-captured storage root.
pub struct RuntimeAssetUseCaseFactory {
    /// Live application configuration.
    configuration: Arc<ArcSwap<Configuration>>,
    /// Permit pool bounding concurrent media probes, owned by the composition root.
    probe_permits: Arc<Semaphore>,
    /// Root directory holding the upload and file folders.
    storage_root: PathBuf,
    /// Factory opening the unit of work wrapping the Asset use cases.
    unit_of_work_factory: Arc<SqlxUnitOfWorkFactory>,
}

impl RuntimeAssetUseCaseFactory {
    /// Build a fresh file storage adapter for one use case.
    fn file_storage() -> Arc<FileSystemStorage> {
        Arc::new(FileSystemStorage::new())
    }

    /// Create a new factory.
    #[must_use]
    pub fn new(
        configuration: Arc<ArcSwap<Configuration>>,
        storage_root: PathBuf,
        unit_of_work_factory: Arc<SqlxUnitOfWorkFactory>,
        probe_permits: Arc<Semaphore>,
    ) -> Self {
        Self {
            configuration,
            probe_permits,
            storage_root,
            unit_of_work_factory,
        }
    }
}

impl AssetUseCaseFactory for RuntimeAssetUseCaseFactory {
    fn begin_upload(&self) -> Arc<dyn BeginUploadUseCase> {
        let live = self.configuration.load();
        Arc::new(BeginUpload::new(
            Arc::clone(&self.unit_of_work_factory),
            Self::file_storage(),
            self.storage_root.clone(),
            live.asset().upload().chunk_size_bytes(),
            live.asset().upload().expiry_seconds(),
            live.asset().upload().max_file_size_bytes(),
        ))
    }

    fn complete_upload(&self) -> Arc<dyn CompleteUploadUseCase> {
        let live = self.configuration.load();
        Arc::new(CompleteUpload::new(
            Arc::clone(&self.unit_of_work_factory),
            Self::file_storage(),
            Arc::new(LocalMediaProbe::with_permits(Arc::clone(
                &self.probe_permits,
            ))),
            self.storage_root.clone(),
            live.asset().upload().expiry_seconds(),
        ))
    }

    fn get_upload(&self) -> Arc<dyn GetUploadUseCase> {
        let live = self.configuration.load();
        Arc::new(GetUpload::new(
            Arc::clone(&self.unit_of_work_factory),
            Self::file_storage(),
            self.storage_root.clone(),
            live.asset().upload().expiry_seconds(),
        ))
    }

    fn write_upload_chunk(&self) -> Arc<dyn WriteUploadChunkUseCase> {
        let live = self.configuration.load();
        Arc::new(WriteUploadChunk::new(
            Arc::clone(&self.unit_of_work_factory),
            Self::file_storage(),
            Sha2ContentHasher,
            self.storage_root.clone(),
            live.asset().upload().expiry_seconds(),
        ))
    }
}
