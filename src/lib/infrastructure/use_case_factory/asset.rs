use std::sync::Arc;

use crate::application::port::{
    AssetUseCaseFactory, BeginUploadUseCase, CancelUploadUseCase, CompleteUploadUseCase,
    GetAssetContentUseCase, GetAudioUseCase, GetFileUseCase, GetImageUseCase, GetUploadUseCase,
    GetVideoUseCase, ReapUploadsUseCase, WriteUploadChunkUseCase,
};
use crate::application::use_case::{
    BeginUpload, CancelUpload, CompleteUpload, GetAssetContent, GetAudio, GetFile, GetImage,
    GetUpload, GetVideo, ReapUploads, WriteUploadChunk,
};
use crate::domain::model::Configuration;
use crate::infrastructure::outbound::LocalMediaProbe;
use crate::infrastructure::outbound::file_system::FileSystemStorage;
use crate::infrastructure::outbound::sha2::Sha2ContentHasher;
use crate::infrastructure::outbound::sqlx::SqlxUnitOfWorkFactory;

use tokio::sync::Semaphore;

/// Builds the Asset use cases from the live configuration.
///
/// Every accessor reads the current configuration for the upload expiry, so a
/// runtime configuration change is picked up by the next request. The chunk size
/// is a startup-only setting (`STY-RUST-076`): it is snapshotted when the factory
/// is built, because the router fixes its body limit from the same value at
/// startup. The file storage adapter is stateless and rebuilt per accessor.
pub(crate) struct RuntimeAssetUseCaseFactory {
    /// Chunk size fixed when the factory was built, in bytes.
    chunk_size_bytes: u64,
    /// Live application configuration.
    configuration: Arc<Configuration>,
    /// Permit pool bounding concurrent media probes, owned by the composition root.
    probe_permits: Arc<Semaphore>,
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
    pub(crate) fn new(
        configuration: Arc<Configuration>,
        unit_of_work_factory: Arc<SqlxUnitOfWorkFactory>,
        probe_permits: Arc<Semaphore>,
    ) -> Self {
        Self {
            chunk_size_bytes: configuration.asset().upload().chunk_size_bytes(),
            configuration,
            probe_permits,
            unit_of_work_factory,
        }
    }
}

impl AssetUseCaseFactory for RuntimeAssetUseCaseFactory {
    fn begin_upload(&self) -> Arc<dyn BeginUploadUseCase> {
        let live = Arc::clone(&self.configuration);
        Arc::new(BeginUpload::new(
            Arc::clone(&self.unit_of_work_factory),
            Self::file_storage(),
            self.chunk_size_bytes,
            live.asset().upload().expiry_seconds(),
            live.asset().upload().max_file_size_bytes(),
        ))
    }

    fn cancel_upload(&self) -> Arc<dyn CancelUploadUseCase> {
        let live = Arc::clone(&self.configuration);
        Arc::new(CancelUpload::new(
            Arc::clone(&self.unit_of_work_factory),
            Self::file_storage(),
            live.asset().upload().expiry_seconds(),
        ))
    }

    fn complete_upload(&self) -> Arc<dyn CompleteUploadUseCase> {
        let live = Arc::clone(&self.configuration);
        Arc::new(CompleteUpload::new(
            Arc::clone(&self.unit_of_work_factory),
            Self::file_storage(),
            Arc::new(LocalMediaProbe::with_permits(Arc::clone(
                &self.probe_permits,
            ))),
            live.asset().upload().expiry_seconds(),
        ))
    }

    fn get_asset_content(&self) -> Arc<dyn GetAssetContentUseCase> {
        Arc::new(GetAssetContent::new(
            Arc::clone(&self.unit_of_work_factory),
            Self::file_storage(),
        ))
    }

    fn get_audio(&self) -> Arc<dyn GetAudioUseCase> {
        Arc::new(GetAudio::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn get_file(&self) -> Arc<dyn GetFileUseCase> {
        Arc::new(GetFile::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn get_image(&self) -> Arc<dyn GetImageUseCase> {
        Arc::new(GetImage::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn get_upload(&self) -> Arc<dyn GetUploadUseCase> {
        let live = Arc::clone(&self.configuration);
        Arc::new(GetUpload::new(
            Arc::clone(&self.unit_of_work_factory),
            Self::file_storage(),
            live.asset().upload().expiry_seconds(),
        ))
    }

    fn get_video(&self) -> Arc<dyn GetVideoUseCase> {
        Arc::new(GetVideo::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn reap_uploads(&self) -> Arc<dyn ReapUploadsUseCase> {
        let live = Arc::clone(&self.configuration);
        Arc::new(ReapUploads::new(
            Arc::clone(&self.unit_of_work_factory),
            Self::file_storage(),
            live.asset().upload().expiry_seconds(),
            live.asset().upload().reap_batch_size(),
        ))
    }

    fn write_upload_chunk(&self) -> Arc<dyn WriteUploadChunkUseCase> {
        let live = Arc::clone(&self.configuration);
        Arc::new(WriteUploadChunk::new(
            Arc::clone(&self.unit_of_work_factory),
            Self::file_storage(),
            Sha2ContentHasher,
            live.asset().upload().expiry_seconds(),
        ))
    }
}
