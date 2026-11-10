use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::{GetFileCommand, GetFileError, GetFileResponse, GetFileUseCase};
use crate::application::security_event;
use crate::domain::model::{Audio, Image, Video};
use crate::domain::port::{
    AssetUnitOfWork, AudioFilter, AudioRepository as _, ContentLinkProvider, FileFilter,
    FileRepository as _, IdentityUnitOfWork, ImageFilter, ImageRepository as _, LibraryUnitOfWork,
    PermissionFilter, PermissionRepository as _, UnitOfWork as _, UnitOfWorkFactory, UserFilter,
    UserRepository as _, UserUnitOfWork, VideoFilter, VideoRepository as _,
};
use crate::domain::service::{Authorization, Decision};

/// Use case implementation for fetching one stored file.
pub(crate) struct GetFile<F, L> {
    /// Adapter minting the playback credential.
    content_link: Arc<L>,
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, L: ContentLinkProvider> GetFile<F, L> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>, content_link: Arc<L>) -> Self {
        Self {
            content_link,
            unit_of_work_factory,
        }
    }
}

impl<F, L> GetFileUseCase for GetFile<F, L>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork + IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
    L: ContentLinkProvider,
{
    fn execute<'future>(
        &'future self,
        command: GetFileCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetFileResponse, GetFileError>> + Send + 'future>> {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);
        let content_link = Arc::clone(&self.content_link);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| GetFileError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| GetFileError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetFileError::NoSuchCaller)?;
                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| GetFileError::Unknown(error.into()))?;
                let file = unit_of_work
                    .files()
                    .search(&FileFilter {
                        id: Some(command.file_id()),
                        ..FileFilter::default()
                    })
                    .await
                    .map_err(|error| GetFileError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetFileError::NoSuchFile)?;

                if let Decision::Deny(denial) =
                    Authorization::decide_asset(&caller, &permissions, &file)
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(GetFileError::NoSuchFile);
                }

                // The media rows backing the file are response data, not an
                // authorization input.
                let image = unit_of_work
                    .images()
                    .search(&ImageFilter {
                        file_id: Some(file.id()),
                        ..ImageFilter::default()
                    })
                    .await
                    .map_err(|error| GetFileError::Unknown(error.into()))?
                    .into_iter()
                    .next();
                let audio = unit_of_work
                    .audios()
                    .search(&AudioFilter {
                        file_id: Some(file.id()),
                        ..AudioFilter::default()
                    })
                    .await
                    .map_err(|error| GetFileError::Unknown(error.into()))?
                    .into_iter()
                    .next();
                let video = unit_of_work
                    .videos()
                    .search(&VideoFilter {
                        file_id: Some(file.id()),
                        ..VideoFilter::default()
                    })
                    .await
                    .map_err(|error| GetFileError::Unknown(error.into()))?
                    .into_iter()
                    .next();

                let link = content_link
                    .mint(file.id())
                    .await
                    .map_err(|error| GetFileError::Unknown(error.into()))?;
                Ok(GetFileResponse::new(
                    file.id(),
                    file.path().to_owned(),
                    file.mime_type_id().to_owned(),
                    file.size_bytes(),
                    file.uploaded_at(),
                    file.modified_at(),
                    file.origin(),
                    file.is_public(),
                    file.user_id(),
                    image.as_ref().map(Image::image_id),
                    audio.as_ref().map(Audio::id),
                    video.as_ref().map(Video::id),
                    link,
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| GetFileError::Unknown(error.into()))?;
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
    use crate::domain::NumericID;

    use crate::domain::model::{
        AssetOwner, AssetScope, File, Image, Orientation, ProtectedResource, ResourceKind,
        ResourceRef, Role, User,
    };

    use crate::domain::port::{
        ContentLink, MockAudioRepository, MockContentLinkProvider, MockFileRepository,
        MockImageRepository, MockUserRepository, MockVideoRepository, RepositoryError,
    };

    use crate::test_helpers::*;
    use chrono::{NaiveDate, NaiveDateTime};
    use std::error::Error;
    use std::sync::atomic::Ordering;

    type UseCase = GetFile<TestFactory, MockContentLinkProvider>;

    /// A signer accepting any number of mints.
    fn signer() -> MockContentLinkProvider {
        let mut signer = MockContentLinkProvider::new();
        signer.expect_mint().times(0..).returning(|_| {
            Box::pin(async { Ok(ContentLink::new(4_102_444_800, "signature".to_owned())) })
        });
        signer
    }

    fn file(
        id: NumericID,
        owner_id: NumericID,
        owner: Option<AssetOwner>,
    ) -> Result<File, Box<dyn Error>> {
        Ok(File::try_new(
            id,
            format!("{id}.png"),
            owner_id,
            false,
            "image/png".to_owned(),
            NaiveDate::default(),
            10,
            NaiveDateTime::default(),
            None,
            None,
        )?
        .with_owner(owner))
    }

    fn resource(
        kind: ResourceKind,
        id: NumericID,
        owner_id: Option<NumericID>,
        is_public: bool,
    ) -> AssetOwner {
        AssetOwner::Resource(ProtectedResource::new(
            ResourceRef::new(kind, id),
            owner_id,
            is_public,
        ))
    }

    fn gallery(id: NumericID, owner_id: NumericID, is_public: bool) -> AssetOwner {
        resource(ResourceKind::Gallery, id, Some(owner_id), is_public)
    }

    fn playlist(id: NumericID, owner_id: NumericID, is_public: bool) -> AssetOwner {
        resource(ResourceKind::MusicPlaylist, id, Some(owner_id), is_public)
    }

    fn image(
        id: NumericID,
        file_id: NumericID,
        scope: Option<AssetScope>,
    ) -> Result<Image, Box<dyn Error>> {
        Ok(Image::try_new(
            id,
            format!("{id}.png"),
            10,
            10,
            Orientation::Square,
            NaiveDate::default(),
            file_id,
            scope,
        )?)
    }

    fn caller(id: NumericID, role: Role) -> Result<User, Box<dyn Error>> {
        Ok(User::try_new(
            id,
            "caller".to_owned(),
            None,
            id.max(1),
            role,
        )?)
    }

    /// Build the fixture around the mocked repositories.
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

    fn expect_file(files: &mut MockFileRepository, file_id: NumericID, stored: File) {
        files
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(file_id))
            .return_once(move |_| Box::pin(async move { Ok(vec![stored]) }));
    }

    fn expect_image(images: &mut MockImageRepository, stored: Image) {
        images
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored]) }));
    }

    fn empty_media(audios: &mut MockAudioRepository, videos: &mut MockVideoRepository) {
        audios
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        videos
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
    }

    #[tokio::test]
    async fn get_file_library_art_follows_library_read() -> Result<(), Box<dyn Error>> {
        // Arrange: a catalogue image shared by the library.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9)?;
        let mut files = MockFileRepository::new();
        expect_file(&mut files, 100, file(100, 3, Some(AssetOwner::Library))?);
        let mut images = MockImageRepository::new();
        expect_image(&mut images, image(19, 100, Some(AssetScope::Library))?);
        let mut audios = MockAudioRepository::new();
        let mut videos = MockVideoRepository::new();
        empty_media(&mut audios, &mut videos);
        let harness = harness(users, files, images, audios, videos);
        let use_case: UseCase = GetFile::new(Arc::clone(&harness.factory), Arc::new(signer()));

        // Act
        let response = use_case.execute(GetFileCommand::new(9, 100)).await?;

        // Assert
        assert_eq!(response.id(), 100);
        assert_eq!(response.owner_id(), 3);
        assert_eq!(response.image_id(), Some(19));
        assert_eq!(response.mime_type_id(), "image/png");
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_file_private_gallery_image_is_concealed_from_a_non_owner()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the image belongs to a private gallery owned by caller 3.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9)?;
        let mut files = MockFileRepository::new();
        expect_file(&mut files, 100, file(100, 3, Some(gallery(2, 3, false)))?);
        let images = MockImageRepository::new();
        let audios = MockAudioRepository::new();
        let videos = MockVideoRepository::new();
        let harness = harness(users, files, images, audios, videos);
        let use_case: UseCase = GetFile::new(Arc::clone(&harness.factory), Arc::new(signer()));

        // Act
        let result = use_case.execute(GetFileCommand::new(9, 100)).await;

        // Assert: an unreadable image is indistinguishable from a missing file.
        assert!(matches!(result, Err(GetFileError::NoSuchFile)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_file_private_gallery_image_is_readable_by_the_owner() -> Result<(), Box<dyn Error>>
    {
        // Arrange: caller 3 owns the private gallery holding the image.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 3)?;
        let mut files = MockFileRepository::new();
        expect_file(&mut files, 100, file(100, 3, Some(gallery(2, 3, false)))?);
        let mut images = MockImageRepository::new();
        expect_image(&mut images, image(19, 100, Some(AssetScope::User))?);
        let mut audios = MockAudioRepository::new();
        let mut videos = MockVideoRepository::new();
        empty_media(&mut audios, &mut videos);
        let harness = harness(users, files, images, audios, videos);
        let use_case: UseCase = GetFile::new(Arc::clone(&harness.factory), Arc::new(signer()));

        // Act
        let response = use_case.execute(GetFileCommand::new(3, 100)).await?;

        // Assert
        assert_eq!(response.id(), 100);
        Ok(())
    }

    #[tokio::test]
    async fn get_file_playlist_cover_follows_playlist_read() -> Result<(), Box<dyn Error>> {
        // Arrange: the image is the cover of a private playlist owned by 3.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 3)?;
        let mut files = MockFileRepository::new();
        expect_file(&mut files, 100, file(100, 3, Some(playlist(4, 3, false)))?);
        let mut images = MockImageRepository::new();
        expect_image(&mut images, image(19, 100, Some(AssetScope::User))?);
        let mut audios = MockAudioRepository::new();
        let mut videos = MockVideoRepository::new();
        empty_media(&mut audios, &mut videos);
        let harness = harness(users, files, images, audios, videos);
        let use_case: UseCase = GetFile::new(Arc::clone(&harness.factory), Arc::new(signer()));

        // Act
        let response = use_case.execute(GetFileCommand::new(3, 100)).await?;

        // Assert
        assert_eq!(response.id(), 100);
        Ok(())
    }

    #[tokio::test]
    async fn get_file_orphan_asset_is_concealed_from_a_non_owner() -> Result<(), Box<dyn Error>> {
        // Arrange: a user-scoped image no gallery or playlist references.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9)?;
        let mut files = MockFileRepository::new();
        expect_file(&mut files, 100, file(100, 3, None)?);
        let images = MockImageRepository::new();
        let audios = MockAudioRepository::new();
        let videos = MockVideoRepository::new();
        let harness = harness(users, files, images, audios, videos);
        let use_case: UseCase = GetFile::new(Arc::clone(&harness.factory), Arc::new(signer()));

        // Act
        let result = use_case.execute(GetFileCommand::new(9, 100)).await;

        // Assert
        assert!(matches!(result, Err(GetFileError::NoSuchFile)));
        Ok(())
    }

    #[tokio::test]
    async fn get_file_root_admin_reads_a_private_asset() -> Result<(), Box<dyn Error>> {
        // Arrange: the Root Admin reads an image of another caller's private gallery.
        let mut users = MockUserRepository::new();
        let root = caller(0, Role::Admin)?;
        users
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![root]) }));
        let mut files = MockFileRepository::new();
        expect_file(&mut files, 100, file(100, 3, Some(gallery(2, 3, false)))?);
        let mut images = MockImageRepository::new();
        expect_image(&mut images, image(19, 100, Some(AssetScope::User))?);
        let mut audios = MockAudioRepository::new();
        let mut videos = MockVideoRepository::new();
        empty_media(&mut audios, &mut videos);
        let harness = harness(users, files, images, audios, videos);
        let use_case: UseCase = GetFile::new(Arc::clone(&harness.factory), Arc::new(signer()));

        // Act
        let response = use_case.execute(GetFileCommand::new(0, 100)).await?;

        // Assert
        assert_eq!(response.id(), 100);
        Ok(())
    }

    #[tokio::test]
    async fn get_file_unknown_file_returns_no_such_file() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9)?;
        let mut files = MockFileRepository::new();
        files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness(
            users,
            files,
            MockImageRepository::new(),
            MockAudioRepository::new(),
            MockVideoRepository::new(),
        );
        let use_case: UseCase = GetFile::new(Arc::clone(&harness.factory), Arc::new(signer()));

        // Act
        let result = use_case.execute(GetFileCommand::new(9, 999)).await;

        // Assert
        assert!(matches!(result, Err(GetFileError::NoSuchFile)));
        Ok(())
    }

    #[tokio::test]
    async fn get_file_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness(
            users,
            MockFileRepository::new(),
            MockImageRepository::new(),
            MockAudioRepository::new(),
            MockVideoRepository::new(),
        );
        let use_case: UseCase = GetFile::new(Arc::clone(&harness.factory), Arc::new(signer()));

        // Act
        let result = use_case.execute(GetFileCommand::new(9, 100)).await;

        // Assert
        assert!(matches!(result, Err(GetFileError::NoSuchCaller)));
        Ok(())
    }

    #[tokio::test]
    async fn get_file_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9)?;
        let mut files = MockFileRepository::new();
        files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness(
            users,
            files,
            MockImageRepository::new(),
            MockAudioRepository::new(),
            MockVideoRepository::new(),
        );
        let use_case: UseCase = GetFile::new(Arc::clone(&harness.factory), Arc::new(signer()));

        // Act
        let result = use_case.execute(GetFileCommand::new(9, 100)).await;

        // Assert
        assert!(matches!(result, Err(GetFileError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_file_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9)?;
        let mut files = MockFileRepository::new();
        expect_file(&mut files, 100, file(100, 3, Some(AssetOwner::Library))?);
        let mut images = MockImageRepository::new();
        expect_image(&mut images, image(19, 100, Some(AssetScope::Library))?);
        let mut audios = MockAudioRepository::new();
        let mut videos = MockVideoRepository::new();
        empty_media(&mut audios, &mut videos);
        let harness = harness(users, files, images, audios, videos);
        harness.commit_fails.store(true, Ordering::SeqCst);
        let use_case: UseCase = GetFile::new(Arc::clone(&harness.factory), Arc::new(signer()));

        // Act
        let result = use_case.execute(GetFileCommand::new(9, 100)).await;

        // Assert
        assert!(matches!(result, Err(GetFileError::Unknown(_))));
        Ok(())
    }
}
