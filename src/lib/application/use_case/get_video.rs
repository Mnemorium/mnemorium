use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::{GetVideoCommand, GetVideoError, GetVideoResponse, GetVideoUseCase};
use crate::application::security_event;
use crate::domain::port::{
    AssetUnitOfWork, ContentLinkProvider, FileFilter, FileRepository as _, IdentityUnitOfWork,
    LibraryUnitOfWork, PermissionFilter, PermissionRepository as _, UnitOfWork as _,
    UnitOfWorkFactory, UserFilter, UserRepository as _, UserUnitOfWork, VideoFilter,
    VideoRepository as _,
};
use crate::domain::service::{Authorization, Decision};

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
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| GetVideoError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetVideoError::NoSuchCaller)?;
                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| GetVideoError::Unknown(error.into()))?;
                let video = unit_of_work
                    .videos()
                    .search(&VideoFilter {
                        id: Some(command.video_id()),
                        ..VideoFilter::default()
                    })
                    .await
                    .map_err(|error| GetVideoError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetVideoError::NoSuchVideo)?;
                let file = unit_of_work
                    .files()
                    .search(&FileFilter {
                        id: Some(video.file_id()),
                        ..FileFilter::default()
                    })
                    .await
                    .map_err(|error| GetVideoError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetVideoError::NoSuchVideo)?;

                if let Decision::Deny(denial) =
                    Authorization::decide_asset(&caller, &permissions, &file)
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(GetVideoError::NoSuchVideo);
                }

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
    use crate::domain::model::{
        AssetOwner, AssetScope, ProtectedResource, ResourceKind, ResourceRef, ScanType, Video,
    };

    use crate::domain::port::{
        ContentLink, MockAudioRepository, MockContentLinkProvider, MockFileRepository,
        MockImageRepository, MockUserRepository, MockVideoRepository, RepositoryError,
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
        let stored_file = sample_file(100, 3, "video/mp4")?.with_owner(Some(AssetOwner::Resource(
            ProtectedResource::new(ResourceRef::new(ResourceKind::Movie, 5), None, true),
        )));
        files
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_file]) }));
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
        videos
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_video]) }));
        let harness = harness(
            users,
            files,
            MockImageRepository::new(),
            MockAudioRepository::new(),
            videos,
        );
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
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9)?;
        let mut videos = MockVideoRepository::new();
        videos
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
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
        let result = use_case.execute(GetVideoCommand::new(9, 7)).await;

        // Assert
        assert!(matches!(result, Err(GetVideoError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    async fn an_unknown_video_is_concealed() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9)?;
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

    #[tokio::test]
    async fn a_private_gallery_video_is_concealed_from_a_non_owner() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9)?;
        let mut files = MockFileRepository::new();
        let stored_file = sample_file(100, 3, "video/mp4")?.with_owner(Some(AssetOwner::Resource(
            ProtectedResource::new(ResourceRef::new(ResourceKind::Gallery, 2), Some(3), false),
        )));
        files
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_file]) }));
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
            Some(AssetScope::User),
        )?;
        videos
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_video]) }));
        let harness = harness(
            users,
            files,
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
        assert!(matches!(result, Err(GetVideoError::NoSuchVideo)));
        Ok(())
    }
}
