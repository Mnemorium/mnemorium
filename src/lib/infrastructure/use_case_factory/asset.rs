use std::sync::Arc;

use crate::application::port::asset_use_case_factory::AssetUseCaseFactory;
use crate::application::port::begin_upload::BeginUploadUseCase;
use crate::application::port::cancel_upload::CancelUploadUseCase;
use crate::application::port::complete_upload::CompleteUploadUseCase;
use crate::application::port::get_asset_content::GetAssetContentUseCase;
use crate::application::port::get_audio::GetAudioUseCase;
use crate::application::port::get_file::GetFileUseCase;
use crate::application::port::get_image::GetImageUseCase;
use crate::application::port::get_upload::GetUploadUseCase;
use crate::application::port::get_video::GetVideoUseCase;
use crate::application::port::reap_uploads::ReapUploadsUseCase;
use crate::application::port::write_upload_chunk::WriteUploadChunkUseCase;
use crate::application::use_case::begin_upload::BeginUpload;
use crate::application::use_case::cancel_upload::CancelUpload;
use crate::application::use_case::complete_upload::CompleteUpload;
use crate::application::use_case::get_asset_content::GetAssetContent;
use crate::application::use_case::get_audio::GetAudio;
use crate::application::use_case::get_file::GetFile;
use crate::application::use_case::get_image::GetImage;
use crate::application::use_case::get_upload::GetUpload;
use crate::application::use_case::get_video::GetVideo;
use crate::application::use_case::reap_uploads::ReapUploads;
use crate::application::use_case::write_upload_chunk::WriteUploadChunk;
use crate::domain::model::configuration::Configuration;
use crate::domain::model::instance_state::InstanceState;
use crate::infrastructure::outbound::file_system::file_storage::FileSystemStorage;
use crate::infrastructure::outbound::jwt::content_link::JwtContentLinkProvider;
use crate::infrastructure::outbound::media_probe::LocalMediaProbe;
use crate::infrastructure::outbound::sha2::content_hasher::Sha2ContentHasher;
use crate::infrastructure::outbound::sqlx::unit_of_work::SqlxUnitOfWorkFactory;
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
    /// Immutable instance identity carrying the signing secret.
    instance_state: Arc<InstanceState>,
    /// Permit pool bounding concurrent media probes, owned by the composition root.
    probe_permits: Arc<Semaphore>,
    /// Factory opening the unit of work wrapping the Asset use cases.
    unit_of_work_factory: Arc<SqlxUnitOfWorkFactory>,
}

impl RuntimeAssetUseCaseFactory {
    /// Build a fresh content-link adapter for one use case.
    fn content_link(&self) -> Arc<JwtContentLinkProvider> {
        Arc::new(JwtContentLinkProvider::new(
            self.instance_state.jwt_secret().to_owned(),
        ))
    }

    /// Build a fresh file storage adapter for one use case.
    fn file_storage() -> Arc<FileSystemStorage> {
        Arc::new(FileSystemStorage::new())
    }

    /// Create a new factory.
    #[must_use]
    pub(crate) fn new(
        configuration: Arc<Configuration>,
        instance_state: Arc<InstanceState>,
        unit_of_work_factory: Arc<SqlxUnitOfWorkFactory>,
        probe_permits: Arc<Semaphore>,
    ) -> Self {
        Self {
            chunk_size_bytes: configuration.asset().upload().chunk_size_bytes(),
            configuration,
            instance_state,
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
        Arc::new(GetAudio::new(
            Arc::clone(&self.unit_of_work_factory),
            self.content_link(),
        ))
    }

    fn get_file(&self) -> Arc<dyn GetFileUseCase> {
        Arc::new(GetFile::new(
            Arc::clone(&self.unit_of_work_factory),
            self.content_link(),
        ))
    }

    fn get_image(&self) -> Arc<dyn GetImageUseCase> {
        Arc::new(GetImage::new(
            Arc::clone(&self.unit_of_work_factory),
            self.content_link(),
        ))
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
        Arc::new(GetVideo::new(
            Arc::clone(&self.unit_of_work_factory),
            self.content_link(),
        ))
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
