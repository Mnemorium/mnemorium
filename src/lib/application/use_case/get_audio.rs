use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::get_audio::{
    GetAudioCommand, GetAudioError, GetAudioResponse, GetAudioUseCase,
};
use crate::application::use_case::asset_access;
use crate::application::use_case::asset_access::AssetAccessError;
use crate::domain::port::asset_unit_of_work::AssetUnitOfWork;
use crate::domain::port::content_link::ContentLinkProvider;
use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::unit_of_work::{UnitOfWork as _, UnitOfWorkFactory};
use crate::domain::port::user_unit_of_work::UserUnitOfWork;

/// Use case implementation for fetching one typed audio.
pub(crate) struct GetAudio<F, L> {
    /// Adapter minting the playback credential.
    content_link: Arc<L>,
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, L: ContentLinkProvider> GetAudio<F, L> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>, content_link: Arc<L>) -> Self {
        Self {
            content_link,
            unit_of_work_factory,
        }
    }
}

impl<F, L> GetAudioUseCase for GetAudio<F, L>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork + IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
    L: ContentLinkProvider,
{
    fn execute<'future>(
        &'future self,
        command: GetAudioCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetAudioResponse, GetAudioError>> + Send + 'future>>
    {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);
        let content_link = Arc::clone(&self.content_link);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| GetAudioError::Unknown(error.into()))?;

            let result = async {
                let (audio, _accessed) = asset_access::access_audio(
                    &mut unit_of_work,
                    command.caller_id(),
                    command.audio_id(),
                )
                .await
                .map_err(|error| match error {
                    AssetAccessError::NoSuchAsset => GetAudioError::NoSuchAudio,
                    AssetAccessError::NoSuchCaller => GetAudioError::NoSuchCaller,
                    AssetAccessError::Unknown(source) => GetAudioError::Unknown(source),
                })?;
                let link = content_link
                    .mint(audio.file_id())
                    .await
                    .map_err(|error| GetAudioError::Unknown(error.into()))?;
                Ok(GetAudioResponse::new(
                    audio.id(),
                    audio.name().to_owned(),
                    audio.duration_ms(),
                    audio.codec().to_owned(),
                    audio.sample_rate_hz(),
                    audio.bit_depth(),
                    audio.block_size(),
                    audio.file_id(),
                    link,
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| GetAudioError::Unknown(error.into()))?;
                    Ok(value)
                }
                Err(error) => {
                    // The unit-of-work adapter owns the rollback-failure log (OBS-002).
                    let _rollback = unit_of_work.rollback().await;
                    Err(error)
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::audio::Audio;
    use crate::domain::port::audio_repository::MockAudioRepository;
    use crate::domain::port::content_link::{ContentLink, MockContentLinkProvider};
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::file_repository::MockFileRepository;
    use crate::domain::port::image_repository::MockImageRepository;
    use crate::domain::port::music_album_repository::MockMusicAlbumRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::domain::port::video_repository::MockVideoRepository;
    use crate::test_helpers::*;
    use std::error::Error;

    type UseCase = GetAudio<TestFactory, MockContentLinkProvider>;

    fn harness(
        users: MockUserRepository,
        files: MockFileRepository,
        images: MockImageRepository,
        audios: MockAudioRepository,
        videos: MockVideoRepository,
        albums: MockMusicAlbumRepository,
    ) -> AssetFactoryHarness {
        AssetFactoryHarness::new(TestUnitOfWork {
            audios,
            files,
            images,
            music_albums: albums,
            permissions: no_permissions(),
            users,
            videos,
            ..TestUnitOfWork::default()
        })
    }

    fn link() -> ContentLink {
        ContentLink::new(4_102_444_800, "signature".to_owned())
    }

    #[tokio::test]
    async fn an_album_track_is_readable_and_linked() -> Result<(), Box<dyn Error>> {
        // Arrange: caller 9 reads the track of album 5.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9)?;
        let mut files = MockFileRepository::new();
        let stored_file = sample_file(100, 3, "audio/mpeg")?;
        files
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_file]) }));
        let mut audios = MockAudioRepository::new();
        let stored_audio = Audio::try_new(
            7,
            "Track".to_owned(),
            None,
            "mp3".to_owned(),
            None,
            None,
            None,
            100,
        )?;
        audios.expect_search().times(2).returning(move |_| {
            let stored = stored_audio.clone();
            Box::pin(async move { Ok(vec![stored]) })
        });
        let mut images = MockImageRepository::new();
        images
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut videos = MockVideoRepository::new();
        videos
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut albums = MockMusicAlbumRepository::new();
        albums.expect_find_album_by_audio().times(1).returning(|_| {
            let album = sample_album(5, Vec::new()).map_err(|_| RepositoryError::OperationFailed);
            Box::pin(async move { album.map(Some) })
        });
        let harness = harness(users, files, images, audios, videos, albums);
        let mut content_link = MockContentLinkProvider::new();
        content_link
            .expect_mint()
            .times(1)
            .withf(|file_id| *file_id == 100)
            .returning(|_| Box::pin(async { Ok(link()) }));
        let use_case: UseCase = GetAudio::new(Arc::clone(&harness.factory), Arc::new(content_link));

        // Act
        let response = use_case.execute(GetAudioCommand::new(9, 7)).await?;

        // Assert
        assert_eq!(response.id(), 7);
        assert_eq!(response.file_id(), 100);
        assert_eq!(response.content_link(), &link());
        Ok(())
    }

    #[tokio::test]
    async fn an_audio_repository_failure_is_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut audios = MockAudioRepository::new();
        audios
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness(
            MockUserRepository::new(),
            MockFileRepository::new(),
            MockImageRepository::new(),
            audios,
            MockVideoRepository::new(),
            MockMusicAlbumRepository::new(),
        );
        let use_case: UseCase = GetAudio::new(
            Arc::clone(&harness.factory),
            Arc::new(MockContentLinkProvider::new()),
        );

        // Act
        let result = use_case.execute(GetAudioCommand::new(9, 7)).await;

        // Assert
        assert!(matches!(result, Err(GetAudioError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    async fn an_unknown_audio_is_concealed() -> Result<(), Box<dyn Error>> {
        // Arrange: the audio is loaded before the caller, so no caller read runs.
        let users = MockUserRepository::new();
        let mut audios = MockAudioRepository::new();
        audios
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness(
            users,
            MockFileRepository::new(),
            MockImageRepository::new(),
            audios,
            MockVideoRepository::new(),
            MockMusicAlbumRepository::new(),
        );
        let use_case: UseCase = GetAudio::new(
            Arc::clone(&harness.factory),
            Arc::new(MockContentLinkProvider::new()),
        );

        // Act
        let result = use_case.execute(GetAudioCommand::new(9, 99)).await;

        // Assert
        assert!(matches!(result, Err(GetAudioError::NoSuchAudio)));
        Ok(())
    }
}
