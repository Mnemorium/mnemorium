use crate::application::authorization::authorize;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use crate::application::port::{
    AddGalleryItemCommand, AddGalleryItemError, AddGalleryItemResponse, AddGalleryItemUseCase,
};
use crate::domain::model::{
    Action, GalleryItem, GalleryItemMedia, ResourceKind, ResourceRef, Visibility,
};
use crate::domain::port::{
    AssetUnitOfWork, FileFilter, FileRepository as _, GalleryFilter, GalleryItemFilter,
    GalleryRepository as _, IdentityUnitOfWork, ImageFilter, ImageRepository as _,
    LibraryUnitOfWork, NewResource, RepositoryError, ResourceRepository as _, UnitOfWork as _,
    UnitOfWorkFactory, UserFilter, UserRepository as _, UserUnitOfWork, VideoFilter,
    VideoRepository as _,
};

/// Use case implementation for adding one of the caller's media to a gallery.
pub(crate) struct AddGalleryItem<F> {
    /// Factory opening the unit of work wrapping the addition.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> AddGalleryItem<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> AddGalleryItemUseCase for AddGalleryItem<F>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork + IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: AddGalleryItemCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<AddGalleryItemResponse, AddGalleryItemError>>
                + Send
                + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| AddGalleryItemError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| AddGalleryItemError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(AddGalleryItemError::NoSuchCaller)?;

                let mut gallery = unit_of_work
                    .galleries()
                    .search(&GalleryFilter {
                        gallery_id: Some(command.gallery_id()),
                        ..GalleryFilter::default()
                    })
                    .await
                    .map_err(|error| AddGalleryItemError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(AddGalleryItemError::NoSuchGallery)?;

                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::Gallery, gallery.gallery_id()),
                    Action::CreateChild,
                    None,
                )
                .await
                .map_err(|error| {
                    if error.conceals() {
                        AddGalleryItemError::NoSuchGallery
                    } else {
                        AddGalleryItemError::Forbidden
                    }
                })?;

                let (media, file_id, image_name) = match command.media() {
                    GalleryItemMedia::Image(image_id) => {
                        let image = unit_of_work
                            .images()
                            .search(&ImageFilter {
                                file_id: Some(image_id),
                                ..ImageFilter::default()
                            })
                            .await
                            .map_err(|error| AddGalleryItemError::Unknown(error.into()))?
                            .into_iter()
                            .next()
                            .ok_or(AddGalleryItemError::NoSuchMedia)?;
                        (
                            GalleryItemMedia::Image(image.image_id()),
                            image.file_id(),
                            Some(image.name().to_owned()),
                        )
                    }
                    GalleryItemMedia::Video(video_id) => {
                        let video = unit_of_work
                            .videos()
                            .search(&VideoFilter {
                                file_id: Some(video_id),
                                ..VideoFilter::default()
                            })
                            .await
                            .map_err(|error| AddGalleryItemError::Unknown(error.into()))?
                            .into_iter()
                            .next()
                            .ok_or(AddGalleryItemError::NoSuchMedia)?;
                        (GalleryItemMedia::Video(video.id()), video.file_id(), None)
                    }
                };

                let file = unit_of_work
                    .files()
                    .search(&FileFilter {
                        id: Some(file_id),
                        ..FileFilter::default()
                    })
                    .await
                    .map_err(|error| AddGalleryItemError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or_else(|| {
                        AddGalleryItemError::Unknown(anyhow::anyhow!(
                            "the medium has no backing file"
                        ))
                    })?;
                let name = image_name.unwrap_or_else(|| file.path().to_owned());

                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::File, file.id()),
                    Action::View,
                    None,
                )
                .await
                .map_err(|_| AddGalleryItemError::NotOwnedMedia)?;

                let already_assigned = !unit_of_work
                    .galleries()
                    .search_items(&GalleryItemFilter {
                        media: Some(media),
                        ..GalleryItemFilter::default()
                    })
                    .await
                    .map_err(|error| AddGalleryItemError::Unknown(error.into()))?
                    .is_empty();
                if already_assigned {
                    return Err(AddGalleryItemError::AlreadyAssigned);
                }

                let now = Utc::now().naive_utc();
                let item_index = unit_of_work
                    .galleries()
                    .search_items(&GalleryItemFilter {
                        gallery_id: Some(command.gallery_id()),
                        ..GalleryItemFilter::default()
                    })
                    .await
                    .map_err(|error| AddGalleryItemError::Unknown(error.into()))?
                    .iter()
                    .map(|detail| detail.item().item_index())
                    .max()
                    .map_or(0, |index| index.saturating_add(1));
                let (image_id, video_id) = match media {
                    GalleryItemMedia::Image(id) => (Some(id), None),
                    GalleryItemMedia::Video(id) => (None, Some(id)),
                };
                let item = GalleryItem::try_new(
                    0,
                    command.gallery_id(),
                    image_id,
                    video_id,
                    item_index,
                    now,
                )
                .map_err(|error| AddGalleryItemError::Unknown(anyhow::Error::new(error)))?;
                let persisted =
                    unit_of_work
                        .galleries()
                        .save_item(item)
                        .await
                        .map_err(|error| {
                            if matches!(error, RepositoryError::Conflict) {
                                AddGalleryItemError::ScopeConflict
                            } else {
                                AddGalleryItemError::Unknown(error.into())
                            }
                        })?;

                unit_of_work
                    .resources()
                    .create(NewResource {
                        kind: ResourceKind::GalleryItem,
                        entity_id: persisted.gallery_item_id(),
                        owner_id: None,
                        auth_parent: Some(ResourceRef::new(
                            ResourceKind::Gallery,
                            gallery.gallery_id(),
                        )),
                        visibility: Visibility::Private,
                    })
                    .await
                    .map_err(|error| AddGalleryItemError::Unknown(error.into()))?;

                gallery.touch(now);
                unit_of_work
                    .galleries()
                    .save(gallery)
                    .await
                    .map_err(|error| AddGalleryItemError::Unknown(error.into()))?;

                Ok(AddGalleryItemResponse::new(
                    persisted.gallery_item_id(),
                    persisted.gallery_id(),
                    persisted.media(),
                    file_id,
                    name,
                    persisted.item_index(),
                    persisted.added_at(),
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| AddGalleryItemError::Unknown(error.into()))?;
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
        File, Gallery, Image, IntegrityHash, Orientation, Origin, Role, ScanType, User, Video,
    };
    use allure_cargotest::allure_test;

    use crate::domain::port::{
        GalleryItemDetail, MockFileRepository, MockGalleryRepository, MockImageRepository,
        MockUserRepository, MockVideoRepository,
    };

    use crate::test_helpers::*;
    use chrono::{NaiveDate, NaiveDateTime};
    use std::error::Error;
    use std::sync::atomic::Ordering;

    type UseCase = AddGalleryItem<TestFactory>;

    fn gallery(gallery_id: i64, user_id: Option<i64>, name: &str, is_public: bool) -> Gallery {
        let now = NaiveDateTime::default();
        Gallery::new(gallery_id, user_id, name.to_owned(), is_public, now, now)
    }

    fn image(image_id: i64, file_id: i64, name: &str) -> Result<Image, Box<dyn Error>> {
        Ok(Image::try_new(
            image_id,
            name.to_owned(),
            640,
            480,
            Orientation::Landscape,
            NaiveDate::from_ymd_opt(2026, 1, 1).unwrap_or_default(),
            file_id,
            None,
        )?)
    }

    fn file(file_id: i64, user_id: i64, path: &str) -> Result<File, Box<dyn Error>> {
        Ok(File::try_new(
            file_id,
            path.to_owned(),
            user_id,
            false,
            "image/png".to_owned(),
            NaiveDate::from_ymd_opt(2026, 1, 1).unwrap_or_default(),
            0,
            NaiveDate::from_ymd_opt(2026, 1, 1)
                .unwrap_or_default()
                .and_hms_opt(0, 0, 0)
                .unwrap_or_default(),
            Some(IntegrityHash::try_new(DIGEST.to_owned())?),
            Some(Origin::Uploaded),
        )?)
    }

    fn persisted_item(
        gallery_item_id: i64,
        gallery_id: i64,
        media: GalleryItemMedia,
        item_index: i64,
    ) -> Result<GalleryItem, RepositoryError> {
        let (image_id, video_id) = match media {
            GalleryItemMedia::Image(id) => (Some(id), None),
            GalleryItemMedia::Video(id) => (None, Some(id)),
        };
        GalleryItem::try_new(
            gallery_item_id,
            gallery_id,
            image_id,
            video_id,
            item_index,
            NaiveDateTime::default(),
        )
        .map_err(|_| RepositoryError::OperationFailed)
    }

    fn harness_with(
        users: MockUserRepository,
        galleries: MockGalleryRepository,
        images: MockImageRepository,
        videos: MockVideoRepository,
        files: MockFileRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            resources: no_resources(),
            files,
            galleries,
            images,
            users,
            videos,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
    }

    fn expect_caller(
        users: &mut MockUserRepository,
        caller_id: i64,
        role: Role,
    ) -> Result<(), Box<dyn Error>> {
        let caller = User::try_new(caller_id, "caller".to_owned(), None, caller_id, role)?;
        users
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(caller_id))
            .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
        Ok(())
    }

    fn expect_gallery(galleries: &mut MockGalleryRepository, gallery: Gallery) {
        let gallery_id = gallery.gallery_id();
        galleries
            .expect_search()
            .times(1)
            .withf(move |filter| filter.gallery_id == Some(gallery_id))
            .return_once(move |_| Box::pin(async move { Ok(vec![gallery]) }));
    }

    fn expect_unassigned(galleries: &mut MockGalleryRepository) {
        galleries
            .expect_search_items()
            .times(1)
            .withf(|filter| filter.media.is_some())
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
    }

    /// Expect the per-gallery item read to return `items`, whose largest item
    /// index seeds the next index.
    fn expect_item_index(galleries: &mut MockGalleryRepository, items: Vec<GalleryItemDetail>) {
        galleries
            .expect_search_items()
            .times(1)
            .withf(|filter| filter.media.is_none())
            .returning(move |_| {
                let details = items.clone();
                Box::pin(async move { Ok(details) })
            });
    }

    fn expect_saved_gallery(galleries: &mut MockGalleryRepository) {
        galleries
            .expect_save()
            .times(1)
            .withf(|saved| saved.last_modified_at() > NaiveDateTime::default())
            .returning(|saved| Box::pin(async move { Ok(saved) }));
    }

    fn expect_image(images: &mut MockImageRepository, file_id: i64, stored: Image) {
        images
            .expect_search()
            .times(1)
            .withf(move |filter| filter.file_id == Some(file_id))
            .return_once(move |_| Box::pin(async move { Ok(vec![stored]) }));
    }

    fn expect_video(videos: &mut MockVideoRepository, file_id: i64, stored: Video) {
        videos
            .expect_search()
            .times(1)
            .withf(move |filter| filter.file_id == Some(file_id))
            .return_once(move |_| Box::pin(async move { Ok(vec![stored]) }));
    }

    fn expect_file(files: &mut MockFileRepository, file_id: i64, stored: File) {
        files
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(file_id))
            .return_once(move |_| Box::pin(async move { Ok(vec![stored]) }));
    }

    #[allure_test]
    #[tokio::test]
    async fn add_gallery_item_owner_adds_own_image() -> Result<(), Box<dyn Error>> {
        allure.epic("Gallery");
        allure.feature("REQ-GALLERY-007");
        allure.story("SCN-GALLERY-007-A");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        expect_unassigned(&mut galleries);
        let indexed = GalleryItemDetail::new(
            persisted_item(99, 3, GalleryItemMedia::Image(7), 2)?,
            100,
            "item".to_owned(),
        );
        expect_item_index(&mut galleries, vec![indexed]);
        galleries
            .expect_save_item()
            .times(1)
            .withf(|item| item.item_index() == 3 && item.gallery_id() == 3)
            .return_once(|_| {
                Box::pin(async { persisted_item(42, 3, GalleryItemMedia::Image(7), 3) })
            });
        expect_saved_gallery(&mut galleries);
        let mut images = MockImageRepository::new();
        expect_image(&mut images, 100, image(7, 100, "sunset.png")?);
        let mut files = MockFileRepository::new();
        expect_file(&mut files, 100, file(100, 4, "files/sunset.png")?);
        let harness = harness_with(users, galleries, images, MockVideoRepository::new(), files);
        let use_case: UseCase = AddGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(AddGalleryItemCommand::new(
                4,
                3,
                GalleryItemMedia::Image(100),
            ))
            .await?;

        // Assert
        assert_eq!(
            response,
            AddGalleryItemResponse::new(
                42,
                3,
                GalleryItemMedia::Image(7),
                100,
                "sunset.png".to_owned(),
                3,
                NaiveDateTime::default(),
            )
        );
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn add_gallery_item_owner_adds_own_video_named_after_file_path()
    -> Result<(), Box<dyn Error>> {
        allure.epic("Gallery");
        allure.feature("REQ-GALLERY-007");
        allure.story("SCN-GALLERY-007-B");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(2, Some(3), "Public", true));
        expect_unassigned(&mut galleries);
        expect_item_index(&mut galleries, Vec::new());
        galleries.expect_save_item().times(1).return_once(|_| {
            Box::pin(async { persisted_item(1, 2, GalleryItemMedia::Video(8), 0) })
        });
        expect_saved_gallery(&mut galleries);
        let stored_video = Video::try_new(
            8,
            1000.0,
            "h264".to_owned(),
            24,
            640,
            480,
            "yuv420p".to_owned(),
            ScanType::Progressive,
            200,
            None,
        )?;
        let mut videos = MockVideoRepository::new();
        expect_video(&mut videos, 200, stored_video);
        let mut files = MockFileRepository::new();
        expect_file(&mut files, 200, file(200, 4, "files/clip.mp4")?);
        let harness = harness_with(users, galleries, MockImageRepository::new(), videos, files);
        let use_case: UseCase = AddGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(AddGalleryItemCommand::new(
                4,
                2,
                GalleryItemMedia::Video(200),
            ))
            .await?;

        // Assert
        assert_eq!(response.media(), GalleryItemMedia::Video(8));
        assert_eq!(response.name(), "files/clip.mp4");
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn add_gallery_item_unknown_video_returns_no_such_media() -> Result<(), Box<dyn Error>> {
        allure.epic("Gallery");
        allure.feature("REQ-GALLERY-007");
        allure.story("SCN-GALLERY-007-E");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        galleries.expect_save_item().times(0);
        let mut videos = MockVideoRepository::new();
        videos
            .expect_search()
            .times(1)
            .withf(|filter| filter.file_id == Some(999))
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(
            users,
            galleries,
            MockImageRepository::new(),
            videos,
            MockFileRepository::new(),
        );
        let use_case: UseCase = AddGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(AddGalleryItemCommand::new(
                4,
                3,
                GalleryItemMedia::Video(999),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(AddGalleryItemError::NoSuchMedia)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn add_gallery_item_admin_adds_other_users_media() -> Result<(), Box<dyn Error>> {
        allure.epic("Gallery");
        allure.feature("REQ-GALLERY-007");
        allure.story("SCN-GALLERY-007-A");
        // Arrange: `file.use` grants an Admin `ALL`, so an Admin may reference
        // another caller's file (a Standard user may not).
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Admin)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Public", true));
        expect_unassigned(&mut galleries);
        expect_item_index(&mut galleries, Vec::new());
        galleries.expect_save_item().times(1).return_once(|_| {
            Box::pin(async { persisted_item(1, 3, GalleryItemMedia::Image(7), 0) })
        });
        expect_saved_gallery(&mut galleries);
        let mut images = MockImageRepository::new();
        expect_image(&mut images, 100, image(7, 100, "sunset.png")?);
        let mut files = MockFileRepository::new();
        expect_file(&mut files, 100, file(100, 9, "files/sunset.png")?);
        let harness = harness_with(users, galleries, images, MockVideoRepository::new(), files);
        let use_case: UseCase = AddGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(AddGalleryItemCommand::new(
                1,
                3,
                GalleryItemMedia::Image(100),
            ))
            .await?;

        // Assert
        assert_eq!(response.gallery_item_id(), 1);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn add_gallery_item_root_admin_adds_other_users_media() -> Result<(), Box<dyn Error>> {
        allure.epic("Gallery");
        allure.feature("REQ-GALLERY-007");
        allure.story("SCN-GALLERY-007-A");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 0, Role::Admin)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        expect_unassigned(&mut galleries);
        expect_item_index(&mut galleries, Vec::new());
        galleries.expect_save_item().times(1).return_once(|_| {
            Box::pin(async { persisted_item(1, 3, GalleryItemMedia::Image(7), 0) })
        });
        expect_saved_gallery(&mut galleries);
        let mut images = MockImageRepository::new();
        expect_image(&mut images, 100, image(7, 100, "sunset.png")?);
        let mut files = MockFileRepository::new();
        expect_file(&mut files, 100, file(100, 9, "files/sunset.png")?);
        let harness = harness_with(users, galleries, images, MockVideoRepository::new(), files);
        let use_case: UseCase = AddGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(AddGalleryItemCommand::new(
                0,
                3,
                GalleryItemMedia::Image(100),
            ))
            .await?;

        // Assert
        assert_eq!(response.gallery_item_id(), 1);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }
    #[allure_test]
    #[tokio::test]
    async fn add_gallery_item_duplicate_media_returns_already_assigned()
    -> Result<(), Box<dyn Error>> {
        allure.epic("Gallery");
        allure.feature("REQ-GALLERY-007");
        allure.story("SCN-GALLERY-007-D");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let existing = GalleryItemDetail::new(
            persisted_item(5, 8, GalleryItemMedia::Image(7), 0)?,
            100,
            "sunset.png".to_owned(),
        );
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        galleries
            .expect_search_items()
            .times(1)
            .withf(|filter| filter.media == Some(GalleryItemMedia::Image(7)))
            .return_once(move |_| {
                let found = existing.clone();
                Box::pin(async move { Ok(vec![found]) })
            });
        galleries.expect_save_item().times(0);
        let mut images = MockImageRepository::new();
        expect_image(&mut images, 100, image(7, 100, "sunset.png")?);
        let mut files = MockFileRepository::new();
        expect_file(&mut files, 100, file(100, 4, "files/sunset.png")?);
        let harness = harness_with(users, galleries, images, MockVideoRepository::new(), files);
        let use_case: UseCase = AddGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(AddGalleryItemCommand::new(
                4,
                3,
                GalleryItemMedia::Image(100),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(AddGalleryItemError::AlreadyAssigned)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn add_gallery_item_unknown_gallery_returns_no_such_gallery() -> Result<(), Box<dyn Error>>
    {
        allure.epic("Gallery");
        allure.feature("REQ-GALLERY-007");
        allure.story("SCN-GALLERY-007-A");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        galleries
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(
            users,
            galleries,
            MockImageRepository::new(),
            MockVideoRepository::new(),
            MockFileRepository::new(),
        );
        let use_case: UseCase = AddGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(AddGalleryItemCommand::new(
                4,
                999,
                GalleryItemMedia::Image(7),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(AddGalleryItemError::NoSuchGallery)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn add_gallery_item_unknown_media_returns_no_such_media() -> Result<(), Box<dyn Error>> {
        allure.epic("Gallery");
        allure.feature("REQ-GALLERY-007");
        allure.story("SCN-GALLERY-007-E");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        galleries.expect_save_item().times(0);
        let mut images = MockImageRepository::new();
        images
            .expect_search()
            .times(1)
            .withf(|filter| filter.file_id == Some(999))
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(
            users,
            galleries,
            images,
            MockVideoRepository::new(),
            MockFileRepository::new(),
        );
        let use_case: UseCase = AddGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(AddGalleryItemCommand::new(
                4,
                3,
                GalleryItemMedia::Image(999),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(AddGalleryItemError::NoSuchMedia)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn add_gallery_item_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>>
    {
        allure.epic("Gallery");
        allure.feature("REQ-GALLERY-007");
        allure.story("SCN-GALLERY-007-A");
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(
            users,
            MockGalleryRepository::new(),
            MockImageRepository::new(),
            MockVideoRepository::new(),
            MockFileRepository::new(),
        );
        let use_case: UseCase = AddGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(AddGalleryItemCommand::new(
                999,
                3,
                GalleryItemMedia::Image(7),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(AddGalleryItemError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn add_gallery_item_scope_conflict_returns_conflict() -> Result<(), Box<dyn Error>> {
        allure.epic("Gallery");
        allure.feature("REQ-GALLERY-007");
        allure.story("SCN-GALLERY-007-D");
        // Arrange: the repository guard rejects the item because the medium's
        // scope does not admit a gallery.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        expect_unassigned(&mut galleries);
        let indexed = GalleryItemDetail::new(
            persisted_item(99, 3, GalleryItemMedia::Image(7), 2)?,
            100,
            "item".to_owned(),
        );
        expect_item_index(&mut galleries, vec![indexed]);
        galleries
            .expect_save_item()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::Conflict) }));
        let mut images = MockImageRepository::new();
        expect_image(&mut images, 100, image(7, 100, "sunset.png")?);
        let mut files = MockFileRepository::new();
        expect_file(&mut files, 100, file(100, 4, "files/sunset.png")?);
        let harness = harness_with(users, galleries, images, MockVideoRepository::new(), files);
        let use_case: UseCase = AddGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(AddGalleryItemCommand::new(
                4,
                3,
                GalleryItemMedia::Image(100),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(AddGalleryItemError::ScopeConflict)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn add_gallery_item_dependency_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        allure.epic("Gallery");
        allure.feature("REQ-GALLERY-007");
        allure.story("SCN-GALLERY-007-A");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        galleries.expect_save_item().times(0);
        let mut images = MockImageRepository::new();
        images
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(
            users,
            galleries,
            images,
            MockVideoRepository::new(),
            MockFileRepository::new(),
        );
        let use_case: UseCase = AddGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(AddGalleryItemCommand::new(
                4,
                3,
                GalleryItemMedia::Image(100),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(AddGalleryItemError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn add_gallery_item_missing_backing_file_returns_unknown() -> Result<(), Box<dyn Error>> {
        allure.epic("Gallery");
        allure.feature("REQ-GALLERY-007");
        allure.story("SCN-GALLERY-007-E");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        galleries.expect_save_item().times(0);
        let mut images = MockImageRepository::new();
        expect_image(&mut images, 100, image(7, 100, "sunset.png")?);
        let mut files = MockFileRepository::new();
        files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, galleries, images, MockVideoRepository::new(), files);
        let use_case: UseCase = AddGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(AddGalleryItemCommand::new(
                4,
                3,
                GalleryItemMedia::Image(100),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(AddGalleryItemError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn add_gallery_item_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        allure.epic("Gallery");
        allure.feature("REQ-GALLERY-007");
        allure.story("SCN-GALLERY-007-A");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        expect_unassigned(&mut galleries);
        expect_item_index(&mut galleries, Vec::new());
        galleries.expect_save_item().times(1).return_once(|_| {
            Box::pin(async { persisted_item(1, 3, GalleryItemMedia::Image(7), 0) })
        });
        expect_saved_gallery(&mut galleries);
        let mut images = MockImageRepository::new();
        expect_image(&mut images, 100, image(7, 100, "sunset.png")?);
        let mut files = MockFileRepository::new();
        expect_file(&mut files, 100, file(100, 4, "files/sunset.png")?);
        let harness = harness_with(users, galleries, images, MockVideoRepository::new(), files);
        harness.commit_fails.store(true, Ordering::SeqCst);
        let use_case: UseCase = AddGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(AddGalleryItemCommand::new(
                4,
                3,
                GalleryItemMedia::Image(100),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(AddGalleryItemError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }
}
