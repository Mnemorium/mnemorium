use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::authorization::authorize;
use crate::application::port::{GetVideoCommand, GetVideoError, GetVideoResponse, GetVideoUseCase};
use crate::domain::model::{Action, ResourceKind, ResourceRef};
use crate::domain::port::{
    AssetUnitOfWork, FileFilter, FileRepository as _, IdentityUnitOfWork, LibraryUnitOfWork,
    UnitOfWork as _, UnitOfWorkFactory, UserFilter, UserRepository as _, UserUnitOfWork,
    VideoFilter, VideoRepository as _,
};

/// Use case implementation for fetching one typed video.
pub(crate) struct GetVideo<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> GetVideo<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> GetVideoUseCase for GetVideo<F>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork + IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: GetVideoCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetVideoResponse, GetVideoError>> + Send + 'future>>
    {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

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

                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::File, file.id()),
                    Action::Stream,
                    None,
                )
                .await
                .map_err(|_| GetVideoError::NoSuchVideo)?;

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
        MockAudioRepository, MockFileRepository, MockImageRepository, MockUserRepository,
        MockVideoRepository, RepositoryError,
    };

    use crate::test_helpers::*;
    use allure_cargotest::allure_test;
    use std::error::Error;

    type UseCase = GetVideo<TestFactory>;

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
            resources: no_resources(),
            users,
            videos,
            ..TestUnitOfWork::default()
        })
    }

    #[allure_test]
    #[tokio::test]
    async fn a_library_video_is_readable_and_linked() -> Result<(), Box<dyn Error>> {
        allure.epic("Video");
        allure.feature("REQ-VIDEO-003");
        allure.story("SCN-VIDEO-003-A");

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
        let use_case: UseCase = GetVideo::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(GetVideoCommand::new(9, 7)).await?;

        // Assert
        assert_eq!(response.id(), 7);
        assert_eq!(response.file_id(), 100);
        assert_eq!(response.width(), 640);
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn a_video_repository_failure_is_unknown() -> Result<(), Box<dyn Error>> {
        allure.epic("Video");
        allure.feature("REQ-VIDEO-003");
        allure.story("SCN-VIDEO-003-B");

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
        let use_case: UseCase = GetVideo::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetVideoCommand::new(9, 7)).await;

        // Assert
        assert!(matches!(result, Err(GetVideoError::Unknown(_))));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn an_unknown_video_is_concealed() -> Result<(), Box<dyn Error>> {
        allure.epic("Video");
        allure.feature("REQ-VIDEO-003");
        allure.story("SCN-VIDEO-003-B");

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
        let use_case: UseCase = GetVideo::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetVideoCommand::new(9, 99)).await;

        // Assert
        assert!(matches!(result, Err(GetVideoError::NoSuchVideo)));
        Ok(())
    }
}
