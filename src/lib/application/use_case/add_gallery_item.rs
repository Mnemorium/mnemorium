use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use crate::application::port::add_gallery_item::AddGalleryItemCommand;
use crate::application::port::add_gallery_item::AddGalleryItemError;
use crate::application::port::add_gallery_item::AddGalleryItemResponse;
use crate::application::port::add_gallery_item::AddGalleryItemUseCase;
use crate::application::security_event;
use crate::domain::model::gallery_item::GalleryItem;
use crate::domain::model::gallery_item::GalleryItemMedia;
use crate::domain::model::permission::Action;
use crate::domain::port::asset_unit_of_work::AssetUnitOfWork;
use crate::domain::port::file_repository::FileFilter;
use crate::domain::port::file_repository::FileRepository as _;
use crate::domain::port::gallery_repository::GalleryFilter;
use crate::domain::port::gallery_repository::GalleryItemFilter;
use crate::domain::port::gallery_repository::GalleryRepository as _;
use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
use crate::domain::port::image_repository::ImageFilter;
use crate::domain::port::image_repository::ImageRepository as _;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::permission_repository::PermissionFilter;
use crate::domain::port::permission_repository::PermissionRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::port::video_repository::VideoFilter;
use crate::domain::port::video_repository::VideoRepository as _;
use crate::domain::service::authorization::Authorization;
use crate::domain::service::authorization::Decision;

/// Use case implementation for adding one of the caller's media to a gallery.
pub struct AddGalleryItem<F> {
    /// Factory opening the unit of work wrapping the addition.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> AddGalleryItem<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
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
                .begin()
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

                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| AddGalleryItemError::Unknown(error.into()))?;

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

                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::GalleryModerate, &gallery)
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(if denial.conceals() {
                        AddGalleryItemError::NoSuchGallery
                    } else {
                        AddGalleryItemError::Forbidden
                    });
                }

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
                        (
                            GalleryItemMedia::Video(video.video_id()),
                            video.file_id(),
                            None,
                        )
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

                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::FileUse, &file)
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(AddGalleryItemError::NotOwnedMedia);
                }

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
                    .next_item_index(command.gallery_id())
                    .await
                    .map_err(|error| AddGalleryItemError::Unknown(error.into()))?;
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
                let persisted = unit_of_work
                    .galleries()
                    .add_item(item)
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
    use std::error::Error;
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use chrono::NaiveDate;
    use chrono::NaiveDateTime;

    use crate::application::port::add_gallery_item::AddGalleryItemCommand;
    use crate::application::port::add_gallery_item::AddGalleryItemError;
    use crate::application::port::add_gallery_item::AddGalleryItemResponse;
    use crate::application::port::add_gallery_item::AddGalleryItemUseCase as _;
    use crate::domain::model::file::File;
    use crate::domain::model::gallery::Gallery;
    use crate::domain::model::gallery_item::GalleryItem;
    use crate::domain::model::gallery_item::GalleryItemMedia;
    use crate::domain::model::image::Image;
    use crate::domain::model::image::Orientation;
    use crate::domain::model::integrity_hash::IntegrityHash;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::model::video::ScanType;
    use crate::domain::model::video::Video;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::file_repository::MockFileRepository;
    use crate::domain::port::gallery_repository::GalleryItemDetail;
    use crate::domain::port::gallery_repository::MockGalleryRepository;
    use crate::domain::port::image_repository::MockImageRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::domain::port::video_repository::MockVideoRepository;
    use crate::test_helpers::DIGEST;
    use crate::test_helpers::GalleryFactoryHarness;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::TestUnitOfWork;
    use crate::test_helpers::no_permissions;

    use super::AddGalleryItem;

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
            IntegrityHash::try_new(DIGEST.to_owned())?,
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
            permissions: no_permissions(),
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
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
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

    #[expect(
        clippy::single_call_fn,
        reason = "the video expectation is named after the mock it configures"
    )]
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

    #[tokio::test]
    async fn add_gallery_item_owner_adds_own_image() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        expect_unassigned(&mut galleries);
        galleries
            .expect_next_item_index()
            .times(1)
            .withf(|id| *id == 3)
            .returning(|_| Box::pin(async { Ok(3) }));
        galleries
            .expect_add_item()
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

    #[tokio::test]
    async fn add_gallery_item_owner_adds_own_video_named_after_file_path()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(2, Some(3), "Public", true));
        expect_unassigned(&mut galleries);
        galleries
            .expect_next_item_index()
            .times(1)
            .returning(|_| Box::pin(async { Ok(0) }));
        galleries.expect_add_item().times(1).return_once(|_| {
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

    #[tokio::test]
    async fn add_gallery_item_unknown_video_returns_no_such_media() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        galleries.expect_add_item().times(0);
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

    #[tokio::test]
    async fn add_gallery_item_non_root_admin_cannot_add_other_users_media()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Admin)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Public", true));
        galleries.expect_add_item().times(0);
        let mut images = MockImageRepository::new();
        expect_image(&mut images, 100, image(7, 100, "sunset.png")?);
        let mut files = MockFileRepository::new();
        expect_file(&mut files, 100, file(100, 9, "files/sunset.png")?);
        let harness = harness_with(users, galleries, images, MockVideoRepository::new(), files);
        let use_case: UseCase = AddGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(AddGalleryItemCommand::new(
                1,
                3,
                GalleryItemMedia::Image(100),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(AddGalleryItemError::NotOwnedMedia)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn add_gallery_item_root_admin_adds_other_users_media() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 0, Role::Admin)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        expect_unassigned(&mut galleries);
        galleries
            .expect_next_item_index()
            .times(1)
            .returning(|_| Box::pin(async { Ok(0) }));
        galleries.expect_add_item().times(1).return_once(|_| {
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

    #[tokio::test]
    async fn add_gallery_item_non_owner_media_returns_not_owned_media() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(2, Some(3), "Public", true));
        galleries.expect_add_item().times(0);
        let mut images = MockImageRepository::new();
        expect_image(&mut images, 100, image(7, 100, "sunset.png")?);
        let mut files = MockFileRepository::new();
        expect_file(&mut files, 100, file(100, 9, "files/sunset.png")?);
        let harness = harness_with(users, galleries, images, MockVideoRepository::new(), files);
        let use_case: UseCase = AddGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(AddGalleryItemCommand::new(
                4,
                2,
                GalleryItemMedia::Image(100),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(AddGalleryItemError::NotOwnedMedia)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn add_gallery_item_other_user_private_gallery_returns_no_such_gallery()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        galleries.expect_add_item().times(0);
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
            .execute(AddGalleryItemCommand::new(9, 3, GalleryItemMedia::Image(7)))
            .await;

        // Assert
        assert!(matches!(result, Err(AddGalleryItemError::NoSuchGallery)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn add_gallery_item_non_root_admin_private_gallery_returns_no_such_gallery()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Admin)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        galleries.expect_add_item().times(0);
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
            .execute(AddGalleryItemCommand::new(1, 3, GalleryItemMedia::Image(7)))
            .await;

        // Assert
        assert!(matches!(result, Err(AddGalleryItemError::NoSuchGallery)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn add_gallery_item_duplicate_media_returns_already_assigned()
    -> Result<(), Box<dyn Error>> {
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
        galleries.expect_add_item().times(0);
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

    #[tokio::test]
    async fn add_gallery_item_unknown_gallery_returns_no_such_gallery() -> Result<(), Box<dyn Error>>
    {
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

    #[tokio::test]
    async fn add_gallery_item_unknown_media_returns_no_such_media() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        galleries.expect_add_item().times(0);
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

    #[tokio::test]
    async fn add_gallery_item_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>>
    {
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

    #[tokio::test]
    async fn add_gallery_item_dependency_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        galleries.expect_add_item().times(0);
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

    #[tokio::test]
    async fn add_gallery_item_missing_backing_file_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        galleries.expect_add_item().times(0);
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

    #[tokio::test]
    async fn add_gallery_item_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        expect_unassigned(&mut galleries);
        galleries
            .expect_next_item_index()
            .times(1)
            .returning(|_| Box::pin(async { Ok(0) }));
        galleries.expect_add_item().times(1).return_once(|_| {
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
