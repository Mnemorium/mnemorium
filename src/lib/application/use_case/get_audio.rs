use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tracing::warn;

use crate::application::port::{GetAudioCommand, GetAudioError, GetAudioResponse, GetAudioUseCase};
use crate::domain::port::{
    AssetUnitOfWork, AudioFilter, AudioRepository as _, ContentLinkProvider, FileFilter,
    FileRepository as _, IdentityUnitOfWork, LibraryUnitOfWork, PermissionFilter,
    PermissionRepository as _, UnitOfWork as _, UnitOfWorkFactory, UserFilter, UserRepository as _,
    UserUnitOfWork,
};
use crate::domain::service::{Authorization, Decision};

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
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| GetAudioError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetAudioError::NoSuchCaller)?;
                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| GetAudioError::Unknown(error.into()))?;
                let audio = unit_of_work
                    .audios()
                    .search(&AudioFilter {
                        id: Some(command.audio_id()),
                        ..AudioFilter::default()
                    })
                    .await
                    .map_err(|error| GetAudioError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetAudioError::NoSuchAudio)?;
                let file = unit_of_work
                    .files()
                    .search(&FileFilter {
                        id: Some(audio.file_id()),
                        ..FileFilter::default()
                    })
                    .await
                    .map_err(|error| GetAudioError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetAudioError::NoSuchAudio)?;

                if let Decision::Deny(denial) =
                    Authorization::decide_asset(&caller, &permissions, &file)
                {
                    warn!(
                        target: "security",
                        event = "authz_failed",
                        actor = caller.id(),
                        action = %denial.action().verb(),
                        target = %denial.action().target(),
                        "denied an action"
                    );
                    return Err(GetAudioError::NoSuchAudio);
                }

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
    use crate::domain::model::{AssetOwner, Audio, ProtectedResource, ResourceKind, ResourceRef};

    use crate::domain::port::{
        ContentLink, MockAudioRepository, MockContentLinkProvider, MockFileRepository,
        MockImageRepository, MockUserRepository, MockVideoRepository, RepositoryError,
    };

    use crate::test_helpers::*;
    use std::error::Error;

    type UseCase = GetAudio<TestFactory, MockContentLinkProvider>;

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

    fn link() -> ContentLink {
        ContentLink::new(4_102_444_800, "signature".to_owned())
    }

    #[tokio::test]
    async fn an_album_track_is_readable_and_linked() -> Result<(), Box<dyn Error>> {
        // Arrange: caller 9 reads the track of album 5.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9)?;
        let mut files = MockFileRepository::new();
        let stored_file =
            sample_file(100, 3, "audio/mpeg")?.with_owner(Some(AssetOwner::Resource(
                ProtectedResource::new(ResourceRef::new(ResourceKind::MusicAlbum, 5), None, true),
            )));
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
        audios
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_audio]) }));
        let harness = harness(
            users,
            files,
            MockImageRepository::new(),
            audios,
            MockVideoRepository::new(),
        );
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
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9)?;
        let mut audios = MockAudioRepository::new();
        audios
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness(
            users,
            MockFileRepository::new(),
            MockImageRepository::new(),
            audios,
            MockVideoRepository::new(),
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
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9)?;
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

    #[tokio::test]
    async fn an_orphan_audio_is_concealed_from_a_non_owner() -> Result<(), Box<dyn Error>> {
        // Arrange: no album holds the recording, so only the file owner may read it.
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
        audios
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_audio]) }));
        let harness = harness(
            users,
            files,
            MockImageRepository::new(),
            audios,
            MockVideoRepository::new(),
        );
        let use_case: UseCase = GetAudio::new(
            Arc::clone(&harness.factory),
            Arc::new(MockContentLinkProvider::new()),
        );

        // Act
        let result = use_case.execute(GetAudioCommand::new(9, 7)).await;

        // Assert
        assert!(matches!(result, Err(GetAudioError::NoSuchAudio)));
        Ok(())
    }
}
