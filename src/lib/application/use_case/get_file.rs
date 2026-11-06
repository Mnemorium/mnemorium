use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::get_file::{
    GetFileCommand, GetFileError, GetFileResponse, GetFileUseCase,
};
use crate::application::use_case::asset_access;
use crate::application::use_case::asset_access::AssetAccessError;
use crate::domain::model::audio::Audio;
use crate::domain::model::image::Image;
use crate::domain::model::video::Video;
use crate::domain::port::asset_unit_of_work::AssetUnitOfWork;
use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::unit_of_work::{UnitOfWork as _, UnitOfWorkFactory};
use crate::domain::port::user_unit_of_work::UserUnitOfWork;

/// Use case implementation for fetching one stored file.
pub(crate) struct GetFile<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> GetFile<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> GetFileUseCase for GetFile<F>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork + IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: GetFileCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetFileResponse, GetFileError>> + Send + 'future>> {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| GetFileError::Unknown(error.into()))?;

            let result = async {
                let accessed = asset_access::access_file(
                    &mut unit_of_work,
                    command.caller_id(),
                    command.file_id(),
                )
                .await
                .map_err(|error| match error {
                    AssetAccessError::NoSuchAsset => GetFileError::NoSuchFile,
                    AssetAccessError::NoSuchCaller => GetFileError::NoSuchCaller,
                    AssetAccessError::Unknown(source) => GetFileError::Unknown(source),
                })?;
                let file = &accessed.file;
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
                    accessed.image.as_ref().map(Image::image_id),
                    accessed.audio.as_ref().map(Audio::id),
                    accessed.video.as_ref().map(Video::video_id),
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
    use crate::domain::alias::NumericID;
    use crate::domain::model::file::File;
    use crate::domain::model::gallery::Gallery;
    use crate::domain::model::gallery_item::GalleryItem;
    use crate::domain::model::image::Orientation;
    use crate::domain::model::music_playlist::MusicPlaylist;
    use crate::domain::model::scope::Scope as AssetScope;
    use crate::domain::model::user::{Role, User};
    use crate::domain::port::audio_repository::MockAudioRepository;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::file_repository::MockFileRepository;
    use crate::domain::port::gallery_repository::{GalleryItemDetail, MockGalleryRepository};
    use crate::domain::port::image_repository::MockImageRepository;
    use crate::domain::port::music_playlist_repository::MockMusicPlaylistRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::domain::port::video_repository::MockVideoRepository;
    use crate::test_helpers::*;
    use chrono::{NaiveDate, NaiveDateTime};
    use std::error::Error;
    use std::sync::atomic::Ordering;

    type UseCase = GetFile<TestFactory>;

    fn file(id: NumericID, owner_id: NumericID) -> Result<File, Box<dyn Error>> {
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
        )?)
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

    fn gallery(id: NumericID, owner_id: NumericID, is_public: bool) -> Gallery {
        let now = NaiveDateTime::default();
        Gallery::new(
            id,
            Some(owner_id),
            format!("Gallery {id}"),
            is_public,
            now,
            now,
        )
    }

    fn gallery_item(
        gallery_id: NumericID,
        image_id: NumericID,
    ) -> Result<GalleryItemDetail, Box<dyn Error>> {
        Ok(GalleryItemDetail::new(
            GalleryItem::try_new(
                1,
                gallery_id,
                Some(image_id),
                None,
                0,
                NaiveDateTime::default(),
            )?,
            image_id,
            "item".to_owned(),
        ))
    }

    fn playlist(id: NumericID, owner_id: NumericID, is_public: bool) -> MusicPlaylist {
        let now = NaiveDateTime::default();
        MusicPlaylist::new(
            id,
            format!("Playlist {id}"),
            now,
            now,
            owner_id,
            is_public,
            Some(19),
            Vec::new(),
        )
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
        galleries: MockGalleryRepository,
        playlists: MockMusicPlaylistRepository,
    ) -> AssetFactoryHarness {
        AssetFactoryHarness::new(TestUnitOfWork {
            audios,
            files,
            galleries,
            images,
            music_playlists: playlists,
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
        expect_file(&mut files, 100, file(100, 3)?);
        let mut images = MockImageRepository::new();
        expect_image(&mut images, image(19, 100, Some(AssetScope::Library))?);
        let mut audios = MockAudioRepository::new();
        let mut videos = MockVideoRepository::new();
        empty_media(&mut audios, &mut videos);
        let mut galleries = MockGalleryRepository::new();
        galleries.expect_search_items().times(0);
        let mut playlists = MockMusicPlaylistRepository::new();
        playlists.expect_search().times(0);
        let harness = harness(users, files, images, audios, videos, galleries, playlists);
        let use_case: UseCase = GetFile::new(Arc::clone(&harness.factory));

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
        expect_file(&mut files, 100, file(100, 3)?);
        let mut images = MockImageRepository::new();
        expect_image(&mut images, image(19, 100, Some(AssetScope::User))?);
        let mut audios = MockAudioRepository::new();
        let mut videos = MockVideoRepository::new();
        empty_media(&mut audios, &mut videos);
        let mut galleries = MockGalleryRepository::new();
        let item = gallery_item(2, 19)?;
        galleries
            .expect_search_items()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![item]) }));
        galleries
            .expect_search()
            .times(1)
            .return_once(|_| Box::pin(async { Ok(vec![gallery(2, 3, false)]) }));
        let mut playlists = MockMusicPlaylistRepository::new();
        playlists.expect_search().times(0);
        let harness = harness(users, files, images, audios, videos, galleries, playlists);
        let use_case: UseCase = GetFile::new(Arc::clone(&harness.factory));

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
        expect_file(&mut files, 100, file(100, 3)?);
        let mut images = MockImageRepository::new();
        expect_image(&mut images, image(19, 100, Some(AssetScope::User))?);
        let mut audios = MockAudioRepository::new();
        let mut videos = MockVideoRepository::new();
        empty_media(&mut audios, &mut videos);
        let mut galleries = MockGalleryRepository::new();
        let item = gallery_item(2, 19)?;
        galleries
            .expect_search_items()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![item]) }));
        galleries
            .expect_search()
            .times(1)
            .return_once(|_| Box::pin(async { Ok(vec![gallery(2, 3, false)]) }));
        let mut playlists = MockMusicPlaylistRepository::new();
        playlists.expect_search().times(0);
        let harness = harness(users, files, images, audios, videos, galleries, playlists);
        let use_case: UseCase = GetFile::new(Arc::clone(&harness.factory));

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
        expect_file(&mut files, 100, file(100, 3)?);
        let mut images = MockImageRepository::new();
        expect_image(&mut images, image(19, 100, Some(AssetScope::User))?);
        let mut audios = MockAudioRepository::new();
        let mut videos = MockVideoRepository::new();
        empty_media(&mut audios, &mut videos);
        let mut galleries = MockGalleryRepository::new();
        galleries
            .expect_search_items()
            .times(1)
            .return_once(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut playlists = MockMusicPlaylistRepository::new();
        playlists
            .expect_search()
            .times(1)
            .withf(|filter| filter.cover_image_id == Some(19))
            .return_once(|_| Box::pin(async { Ok(vec![playlist(4, 3, false)]) }));
        let harness = harness(users, files, images, audios, videos, galleries, playlists);
        let use_case: UseCase = GetFile::new(Arc::clone(&harness.factory));

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
        expect_file(&mut files, 100, file(100, 3)?);
        let mut images = MockImageRepository::new();
        expect_image(&mut images, image(19, 100, Some(AssetScope::User))?);
        let mut audios = MockAudioRepository::new();
        let mut videos = MockVideoRepository::new();
        empty_media(&mut audios, &mut videos);
        let mut galleries = MockGalleryRepository::new();
        galleries
            .expect_search_items()
            .times(1)
            .return_once(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut playlists = MockMusicPlaylistRepository::new();
        playlists
            .expect_search()
            .times(1)
            .return_once(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness(users, files, images, audios, videos, galleries, playlists);
        let use_case: UseCase = GetFile::new(Arc::clone(&harness.factory));

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
        expect_file(&mut files, 100, file(100, 3)?);
        let mut images = MockImageRepository::new();
        expect_image(&mut images, image(19, 100, Some(AssetScope::User))?);
        let mut audios = MockAudioRepository::new();
        let mut videos = MockVideoRepository::new();
        empty_media(&mut audios, &mut videos);
        let mut galleries = MockGalleryRepository::new();
        let item = gallery_item(2, 19)?;
        galleries
            .expect_search_items()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![item]) }));
        galleries
            .expect_search()
            .times(1)
            .return_once(|_| Box::pin(async { Ok(vec![gallery(2, 3, false)]) }));
        let mut playlists = MockMusicPlaylistRepository::new();
        playlists.expect_search().times(0);
        let harness = harness(users, files, images, audios, videos, galleries, playlists);
        let use_case: UseCase = GetFile::new(Arc::clone(&harness.factory));

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
            MockGalleryRepository::new(),
            MockMusicPlaylistRepository::new(),
        );
        let use_case: UseCase = GetFile::new(Arc::clone(&harness.factory));

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
            MockGalleryRepository::new(),
            MockMusicPlaylistRepository::new(),
        );
        let use_case: UseCase = GetFile::new(Arc::clone(&harness.factory));

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
            MockGalleryRepository::new(),
            MockMusicPlaylistRepository::new(),
        );
        let use_case: UseCase = GetFile::new(Arc::clone(&harness.factory));

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
        expect_file(&mut files, 100, file(100, 3)?);
        let mut images = MockImageRepository::new();
        expect_image(&mut images, image(19, 100, Some(AssetScope::Library))?);
        let mut audios = MockAudioRepository::new();
        let mut videos = MockVideoRepository::new();
        empty_media(&mut audios, &mut videos);
        let mut galleries = MockGalleryRepository::new();
        galleries.expect_search_items().times(0);
        let mut playlists = MockMusicPlaylistRepository::new();
        playlists.expect_search().times(0);
        let harness = harness(users, files, images, audios, videos, galleries, playlists);
        harness.commit_fails.store(true, Ordering::SeqCst);
        let use_case: UseCase = GetFile::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetFileCommand::new(9, 100)).await;

        // Assert
        assert!(matches!(result, Err(GetFileError::Unknown(_))));
        Ok(())
    }
}
