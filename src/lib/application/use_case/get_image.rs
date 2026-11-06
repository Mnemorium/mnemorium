use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::get_image::{
    GetImageCommand, GetImageError, GetImageResponse, GetImageUseCase,
};
use crate::application::use_case::asset_access;
use crate::application::use_case::asset_access::AssetAccessError;
use crate::domain::port::asset_unit_of_work::AssetUnitOfWork;
use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::unit_of_work::{UnitOfWork as _, UnitOfWorkFactory};
use crate::domain::port::user_unit_of_work::UserUnitOfWork;

/// Use case implementation for fetching one typed image.
pub(crate) struct GetImage<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> GetImage<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> GetImageUseCase for GetImage<F>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork + IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: GetImageCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetImageResponse, GetImageError>> + Send + 'future>>
    {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| GetImageError::Unknown(error.into()))?;

            let result = async {
                let (image, _accessed) = asset_access::access_image(
                    &mut unit_of_work,
                    command.caller_id(),
                    command.image_id(),
                )
                .await
                .map_err(|error| match error {
                    AssetAccessError::NoSuchAsset => GetImageError::NoSuchImage,
                    AssetAccessError::NoSuchCaller => GetImageError::NoSuchCaller,
                    AssetAccessError::Unknown(source) => GetImageError::Unknown(source),
                })?;
                Ok(GetImageResponse::new(
                    image.image_id(),
                    image.name().to_owned(),
                    image.width_px(),
                    image.height_px(),
                    image.orientation(),
                    image.created_at(),
                    image.file_id(),
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| GetImageError::Unknown(error.into()))?;
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
    use crate::domain::model::image::{Image, Orientation};
    use crate::domain::model::scope::Scope as AssetScope;
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

    type UseCase = GetImage<TestFactory>;

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
            20,
            Orientation::Portrait,
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
    async fn get_image_projects_the_typed_representation() -> Result<(), Box<dyn Error>> {
        // Arrange: a library image readable through `library.read`.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9)?;
        let mut images = MockImageRepository::new();
        let stored_image = image(19, 100, Some(AssetScope::Library))?;
        images.expect_search().times(2).returning(move |_| {
            let stored = stored_image.clone();
            Box::pin(async move { Ok(vec![stored]) })
        });
        let mut files = MockFileRepository::new();
        let stored_file = file(100, 3)?;
        files.expect_search().times(1).returning(move |_| {
            let stored = stored_file.clone();
            Box::pin(async move { Ok(vec![stored]) })
        });
        let mut audios = MockAudioRepository::new();
        let mut videos = MockVideoRepository::new();
        empty_media(&mut audios, &mut videos);
        let mut galleries = MockGalleryRepository::new();
        galleries.expect_search_items().times(0);
        let mut playlists = MockMusicPlaylistRepository::new();
        playlists.expect_search().times(0);
        let harness = harness(users, files, images, audios, videos, galleries, playlists);
        let use_case: UseCase = GetImage::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(GetImageCommand::new(9, 19)).await?;

        // Assert
        assert_eq!(response.id(), 19);
        assert_eq!(response.file_id(), 100);
        assert_eq!(response.width_px(), 10);
        assert_eq!(response.height_px(), 20);
        assert_eq!(response.orientation(), Orientation::Portrait);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_image_unknown_image_returns_no_such_image() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut images = MockImageRepository::new();
        images
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness(
            MockUserRepository::new(),
            MockFileRepository::new(),
            images,
            MockAudioRepository::new(),
            MockVideoRepository::new(),
            MockGalleryRepository::new(),
            MockMusicPlaylistRepository::new(),
        );
        let use_case: UseCase = GetImage::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetImageCommand::new(9, 999)).await;

        // Assert
        assert!(matches!(result, Err(GetImageError::NoSuchImage)));
        Ok(())
    }

    #[tokio::test]
    async fn get_image_of_a_private_gallery_is_concealed_from_a_non_owner()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9)?;
        let mut images = MockImageRepository::new();
        let stored_image = image(19, 100, Some(AssetScope::User))?;
        images.expect_search().times(2).returning(move |_| {
            let stored = stored_image.clone();
            Box::pin(async move { Ok(vec![stored]) })
        });
        let mut files = MockFileRepository::new();
        let stored_file = file(100, 3)?;
        files.expect_search().times(1).returning(move |_| {
            let stored = stored_file.clone();
            Box::pin(async move { Ok(vec![stored]) })
        });
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
        let use_case: UseCase = GetImage::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetImageCommand::new(9, 19)).await;

        // Assert
        assert!(matches!(result, Err(GetImageError::NoSuchImage)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_image_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange: the image exists, but its caller does not.
        let mut images = MockImageRepository::new();
        let stored_image = image(19, 100, None)?;
        images
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_image]) }));
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness(
            users,
            MockFileRepository::new(),
            images,
            MockAudioRepository::new(),
            MockVideoRepository::new(),
            MockGalleryRepository::new(),
            MockMusicPlaylistRepository::new(),
        );
        let use_case: UseCase = GetImage::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetImageCommand::new(9, 19)).await;

        // Assert
        assert!(matches!(result, Err(GetImageError::NoSuchCaller)));
        Ok(())
    }
    #[tokio::test]
    async fn get_image_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange: the caller load fails after the image resolves.
        let mut images = MockImageRepository::new();
        let stored_image = image(19, 100, None)?;
        images
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_image]) }));
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness(
            users,
            MockFileRepository::new(),
            images,
            MockAudioRepository::new(),
            MockVideoRepository::new(),
            MockGalleryRepository::new(),
            MockMusicPlaylistRepository::new(),
        );
        let use_case: UseCase = GetImage::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetImageCommand::new(9, 19)).await;

        // Assert
        assert!(matches!(result, Err(GetImageError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
