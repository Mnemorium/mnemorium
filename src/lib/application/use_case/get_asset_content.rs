use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::get_asset_content::{
    AssetTarget, GetAssetContentCommand, GetAssetContentError, GetAssetContentResponse,
    GetAssetContentUseCase,
};
use crate::application::use_case::asset_access;
use crate::application::use_case::asset_access::AssetAccessError;
use crate::application::use_case::upload_layout::media_path;
use crate::domain::port::asset_unit_of_work::AssetUnitOfWork;
use crate::domain::port::file_storage::FileStorage;
use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::unit_of_work::{UnitOfWork as _, UnitOfWorkFactory};
use crate::domain::port::user_unit_of_work::UserUnitOfWork;

/// Use case implementation for reading the content of one asset.
///
/// The asset is authorized inside the unit of work; only a readable asset's
/// bytes are read from storage, so a denied caller never reaches the file and
/// cannot observe its existence through timing or size (`API-044`).
pub(crate) struct GetAssetContent<F, S> {
    /// Storage adapter reading the stored bytes.
    file_storage: Arc<S>,
    /// Factory opening the unit of work wrapping the authorization.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, S: FileStorage> GetAssetContent<F, S> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>, file_storage: Arc<S>) -> Self {
        Self {
            file_storage,
            unit_of_work_factory,
        }
    }
}

impl<F, S> GetAssetContentUseCase for GetAssetContent<F, S>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork + IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
    S: FileStorage,
{
    fn execute<'future>(
        &'future self,
        command: GetAssetContentCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<GetAssetContentResponse, GetAssetContentError>>
                + Send
                + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);
        let file_storage = Arc::clone(&self.file_storage);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| GetAssetContentError::Unknown(error.into()))?;

            let result = async {
                let file = match command.target() {
                    AssetTarget::File(file_id) => {
                        asset_access::access_file(&mut unit_of_work, command.caller_id(), file_id)
                            .await
                            .map_err(|error| match error {
                                AssetAccessError::NoSuchAsset => GetAssetContentError::NoSuchAsset,
                                AssetAccessError::NoSuchCaller => {
                                    GetAssetContentError::NoSuchCaller
                                }
                                AssetAccessError::Unknown(source) => {
                                    GetAssetContentError::Unknown(source)
                                }
                            })?
                            .file
                    }
                    AssetTarget::Image(image_id) => {
                        asset_access::access_image(&mut unit_of_work, command.caller_id(), image_id)
                            .await
                            .map_err(|error| match error {
                                AssetAccessError::NoSuchAsset => GetAssetContentError::NoSuchAsset,
                                AssetAccessError::NoSuchCaller => {
                                    GetAssetContentError::NoSuchCaller
                                }
                                AssetAccessError::Unknown(source) => {
                                    GetAssetContentError::Unknown(source)
                                }
                            })?
                            .1
                            .file
                    }
                };
                Ok(file)
            }
            .await;

            let file = match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| GetAssetContentError::Unknown(error.into()))?;
                    value
                }
                Err(error) => {
                    // The unit-of-work adapter owns the rollback-failure log (OBS-002).
                    let _rollback = unit_of_work.rollback().await;
                    return Err(error);
                }
            };

            let path = media_path(file.path());
            let bytes = file_storage
                .read(&path)
                .await
                .map_err(|error| GetAssetContentError::Unknown(error.into()))?
                .ok_or(GetAssetContentError::NoSuchAsset)?;
            Ok(GetAssetContentResponse::new(
                file.mime_type_id().to_owned(),
                bytes,
            ))
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
    use crate::domain::port::file_storage::MockFileStorage;
    use crate::domain::port::gallery_repository::{GalleryItemDetail, MockGalleryRepository};
    use crate::domain::port::image_repository::MockImageRepository;
    use crate::domain::port::music_playlist_repository::MockMusicPlaylistRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::domain::port::video_repository::MockVideoRepository;
    use crate::test_helpers::*;
    use chrono::{NaiveDate, NaiveDateTime};
    use std::error::Error;
    use std::sync::atomic::Ordering;

    type UseCase = GetAssetContent<TestFactory, MockFileStorage>;

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
    async fn content_of_a_readable_file_carries_its_media_type() -> Result<(), Box<dyn Error>> {
        // Arrange: a library image readable through `library.read`.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9)?;
        let mut files = MockFileRepository::new();
        let stored_file = file(100, 3)?;
        files
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_file]) }));
        let mut images = MockImageRepository::new();
        let stored_image = image(19, 100, Some(AssetScope::Library))?;
        images
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_image]) }));
        let mut audios = MockAudioRepository::new();
        let mut videos = MockVideoRepository::new();
        empty_media(&mut audios, &mut videos);
        let mut galleries = MockGalleryRepository::new();
        galleries.expect_search_items().times(0);
        let mut playlists = MockMusicPlaylistRepository::new();
        playlists.expect_search().times(0);
        let harness = harness(users, files, images, audios, videos, galleries, playlists);
        let mut storage = MockFileStorage::new();
        storage
            .expect_read()
            .times(1)
            .withf(|path| path.ends_with("100.png"))
            .returning(|_| Box::pin(async { Ok(Some(vec![0x89, 0x50, 0x4e, 0x47])) }));
        let use_case: UseCase =
            GetAssetContent::new(Arc::clone(&harness.factory), Arc::new(storage));

        // Act
        let response = use_case
            .execute(GetAssetContentCommand::new(9, AssetTarget::File(100)))
            .await?;

        // Assert
        assert_eq!(response.mime_type_id(), "image/png");
        assert_eq!(response.bytes(), &[0x89, 0x50, 0x4e, 0x47]);
        Ok(())
    }

    #[tokio::test]
    async fn content_of_an_unreadable_asset_reads_no_bytes() -> Result<(), Box<dyn Error>> {
        // Arrange: another caller's private gallery image.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9)?;
        let mut files = MockFileRepository::new();
        let stored_file = file(100, 3)?;
        files
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_file]) }));
        let mut images = MockImageRepository::new();
        let stored_image = image(19, 100, Some(AssetScope::User))?;
        images
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_image]) }));
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
        let mut storage = MockFileStorage::new();
        storage.expect_read().times(0);
        let use_case: UseCase =
            GetAssetContent::new(Arc::clone(&harness.factory), Arc::new(storage));

        // Act
        let result = use_case
            .execute(GetAssetContentCommand::new(9, AssetTarget::File(100)))
            .await;

        // Assert
        assert!(matches!(result, Err(GetAssetContentError::NoSuchAsset)));
        Ok(())
    }

    #[tokio::test]
    async fn content_of_an_image_resolves_its_backing_file() -> Result<(), Box<dyn Error>> {
        // Arrange: an image address resolves the file before reading it.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9)?;
        let mut files = MockFileRepository::new();
        let stored_file = file(100, 3)?;
        files
            .expect_search()
            .times(1)
            .withf(|filter| filter.id == Some(100))
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_file]) }));
        let mut images = MockImageRepository::new();
        let stored_image = image(19, 100, Some(AssetScope::Library))?;
        images.expect_search().times(2).returning(move |_| {
            let stored = stored_image.clone();
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
        let mut storage = MockFileStorage::new();
        storage
            .expect_read()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(vec![1, 2, 3])) }));
        let use_case: UseCase =
            GetAssetContent::new(Arc::clone(&harness.factory), Arc::new(storage));

        // Act
        let response = use_case
            .execute(GetAssetContentCommand::new(9, AssetTarget::Image(19)))
            .await?;

        // Assert
        assert_eq!(response.bytes(), &[1, 2, 3]);
        Ok(())
    }

    #[tokio::test]
    async fn content_missing_from_storage_returns_no_such_asset() -> Result<(), Box<dyn Error>> {
        // Arrange: the owner may read the row, but the bytes do not exist.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 3)?;
        let mut files = MockFileRepository::new();
        let stored_file = file(100, 3)?;
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
        let mut videos = MockVideoRepository::new();
        empty_media(&mut audios, &mut videos);
        let harness = harness(
            users,
            files,
            images,
            audios,
            videos,
            MockGalleryRepository::new(),
            MockMusicPlaylistRepository::new(),
        );
        let mut storage = MockFileStorage::new();
        storage
            .expect_read()
            .times(1)
            .returning(|_| Box::pin(async { Ok(None) }));
        let use_case: UseCase =
            GetAssetContent::new(Arc::clone(&harness.factory), Arc::new(storage));

        // Act
        let result = use_case
            .execute(GetAssetContentCommand::new(3, AssetTarget::File(100)))
            .await;

        // Assert
        assert!(matches!(result, Err(GetAssetContentError::NoSuchAsset)));
        Ok(())
    }
    #[tokio::test]
    async fn content_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange: the file load fails, so no byte is read.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 3)?;
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
        let mut storage = MockFileStorage::new();
        storage.expect_read().times(0);
        let use_case: UseCase =
            GetAssetContent::new(Arc::clone(&harness.factory), Arc::new(storage));

        // Act
        let result = use_case
            .execute(GetAssetContentCommand::new(3, AssetTarget::File(100)))
            .await;

        // Assert
        assert!(matches!(result, Err(GetAssetContentError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
