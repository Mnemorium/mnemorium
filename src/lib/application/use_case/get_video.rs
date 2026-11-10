use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::{GetVideoCommand, GetVideoError, GetVideoResponse, GetVideoUseCase};
use crate::application::use_case::asset_access;
use crate::application::use_case::asset_access::AssetAccessError;
use crate::domain::port::{
    AssetUnitOfWork, ContentLinkProvider, IdentityUnitOfWork, LibraryUnitOfWork, UnitOfWork as _,
    UnitOfWorkFactory, UserUnitOfWork,
};

/// Use case implementation for fetching one typed video.
pub(crate) struct GetVideo<F, L> {
    /// Adapter minting the playback credential.
    content_link: Arc<L>,
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, L: ContentLinkProvider> GetVideo<F, L> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>, content_link: Arc<L>) -> Self {
        Self {
            content_link,
            unit_of_work_factory,
        }
    }
}

impl<F, L> GetVideoUseCase for GetVideo<F, L>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork + IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
    L: ContentLinkProvider,
{
    fn execute<'future>(
        &'future self,
        command: GetVideoCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetVideoResponse, GetVideoError>> + Send + 'future>>
    {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);
        let content_link = Arc::clone(&self.content_link);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| GetVideoError::Unknown(error.into()))?;

            let result = async {
                let (video, _accessed) = asset_access::access_video(
                    &mut unit_of_work,
                    command.caller_id(),
                    command.video_id(),
                )
                .await
                .map_err(|error| match error {
                    AssetAccessError::NoSuchAsset => GetVideoError::NoSuchVideo,
                    AssetAccessError::NoSuchCaller => GetVideoError::NoSuchCaller,
                    AssetAccessError::Unknown(source) => GetVideoError::Unknown(source),
                })?;
                let link = content_link
                    .mint(video.file_id())
                    .await
                    .map_err(|error| GetVideoError::Unknown(error.into()))?;
                Ok(GetVideoResponse::new(
                    video.id(),
                    video.codec().to_owned(),
                    video.color_id().to_owned(),
                    video.duration_ms(),
                    video.frame_count(),
                    video.height(),
                    video.width(),
                    video.scan_type(),
                    video.file_id(),
                    link,
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| GetVideoError::Unknown(error.into()))?;
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
    use crate::domain::model::{AssetScope, ScanType, Video};

    use crate::domain::port::{
        ContentLink, MockAudioRepository, MockContentLinkProvider, MockFileRepository,
        MockImageRepository, MockMusicAlbumRepository, MockUserRepository, MockVideoRepository,
        RepositoryError,
    };

    use crate::test_helpers::*;
    use std::error::Error;

    type UseCase = GetVideo<TestFactory, MockContentLinkProvider>;

    fn harness(
        users: MockUserRepository,
        files: MockFileRepository,
        images: MockImageRepository,
        audios: MockAudioRepository,
        videos: MockVideoRepository,
    ) -> AssetFactoryHarness {
        AssetFactoryHarness::new(TestUnitOfWork {
            audios,
            files,
            images,
            music_albums: MockMusicAlbumRepository::new(),
            permissions: no_permissions(),
            users,
            videos,
            ..TestUnitOfWork::default()
        })
    }

    #[tokio::test]
    async fn a_library_video_is_readable_and_linked() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9)?;
        let mut files = MockFileRepository::new();
        let stored_file = sample_file(100, 3, "video/mp4")?;
        files
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_file]) }));
        let mut images = MockImageRepository::new();
        images
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut audios = MockAudioRepository::new();
        audios
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut videos = MockVideoRepository::new();
        let stored_video = Video::try_new(
            7,
            1_000.0,
            "h264".to_owned(),
            30,
            640,
            480,
            "YCbCr".to_owned(),
            ScanType::Progressive,
            100,
            Some(AssetScope::Library),
        )?;
        videos.expect_search().times(2).returning(move |_| {
            let stored = stored_video.clone();
            Box::pin(async move { Ok(vec![stored]) })
        });
        let harness = harness(users, files, images, audios, videos);
        let mut content_link = MockContentLinkProvider::new();
        content_link
            .expect_mint()
            .times(1)
            .withf(|file_id| *file_id == 100)
            .returning(|_| {
                Box::pin(async { Ok(ContentLink::new(4_102_444_800, "signature".to_owned())) })
            });
        let use_case: UseCase = GetVideo::new(Arc::clone(&harness.factory), Arc::new(content_link));

        // Act
        let response = use_case.execute(GetVideoCommand::new(9, 7)).await?;

        // Assert
        assert_eq!(response.id(), 7);
        assert_eq!(response.file_id(), 100);
        assert_eq!(response.width(), 640);
        Ok(())
    }

    #[tokio::test]
    async fn a_video_repository_failure_is_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut videos = MockVideoRepository::new();
        videos
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness(
            MockUserRepository::new(),
            MockFileRepository::new(),
            MockImageRepository::new(),
            MockAudioRepository::new(),
            videos,
        );
        let use_case: UseCase = GetVideo::new(
            Arc::clone(&harness.factory),
            Arc::new(MockContentLinkProvider::new()),
        );

        // Act
        let result = use_case.execute(GetVideoCommand::new(9, 7)).await;

        // Assert
        assert!(matches!(result, Err(GetVideoError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    async fn an_unknown_video_is_concealed() -> Result<(), Box<dyn Error>> {
        // Arrange: the video is loaded before the caller, so no caller read runs.
        let users = MockUserRepository::new();
        let mut videos = MockVideoRepository::new();
        videos
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness(
            users,
            MockFileRepository::new(),
            MockImageRepository::new(),
            MockAudioRepository::new(),
            videos,
        );
        let use_case: UseCase = GetVideo::new(
            Arc::clone(&harness.factory),
            Arc::new(MockContentLinkProvider::new()),
        );

        // Act
        let result = use_case.execute(GetVideoCommand::new(9, 99)).await;

        // Assert
        assert!(matches!(result, Err(GetVideoError::NoSuchVideo)));
        Ok(())
    }
}
