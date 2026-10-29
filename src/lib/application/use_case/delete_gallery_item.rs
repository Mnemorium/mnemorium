use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use crate::application::port::delete_gallery_item::DeleteGalleryItemCommand;
use crate::application::port::delete_gallery_item::DeleteGalleryItemError;
use crate::application::port::delete_gallery_item::DeleteGalleryItemUseCase;
use crate::application::security_event;
use crate::domain::model::gallery_item::GalleryItemMedia;
use crate::domain::model::permission::Action;
use crate::domain::port::asset_unit_of_work::AssetUnitOfWork;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::file_repository::FileFilter;
use crate::domain::port::file_repository::FileRepository as _;
use crate::domain::port::file_storage::FileStorage;
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

/// Use case implementation for deleting one item of a gallery.
pub struct DeleteGalleryItem<F, S> {
    /// Storage adapter kept for the removal path. Byte deletion is disabled
    /// (`TODO(ingest)`), so the field is held but not read.
    _file_storage: Arc<S>,
    /// Factory opening the unit of work wrapping the deletion.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, S: FileStorage> DeleteGalleryItem<F, S> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>, file_storage: Arc<S>) -> Self {
        Self {
            _file_storage: file_storage,
            unit_of_work_factory,
        }
    }
}

impl<F, S> DeleteGalleryItemUseCase for DeleteGalleryItem<F, S>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork + IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
    S: FileStorage,
{
    fn execute<'future>(
        &'future self,
        command: DeleteGalleryItemCommand,
    ) -> Pin<Box<dyn Future<Output = Result<(), DeleteGalleryItemError>> + Send + 'future>> {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| DeleteGalleryItemError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| DeleteGalleryItemError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(DeleteGalleryItemError::NoSuchCaller)?;

                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| DeleteGalleryItemError::Unknown(error.into()))?;

                let mut gallery = unit_of_work
                    .galleries()
                    .search(&GalleryFilter {
                        gallery_id: Some(command.gallery_id()),
                        ..GalleryFilter::default()
                    })
                    .await
                    .map_err(|error| DeleteGalleryItemError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(DeleteGalleryItemError::NoSuchGallery)?;

                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::GalleryModerate, &gallery)
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(if denial.conceals() {
                        DeleteGalleryItemError::NoSuchGallery
                    } else {
                        DeleteGalleryItemError::Forbidden
                    });
                }

                let detail = unit_of_work
                    .galleries()
                    .search_items(&GalleryItemFilter {
                        gallery_id: Some(command.gallery_id()),
                        gallery_item_id: Some(command.gallery_item_id()),
                        ..GalleryItemFilter::default()
                    })
                    .await
                    .map_err(|error| DeleteGalleryItemError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(DeleteGalleryItemError::NoSuchItem)?;
                let media = detail.item().media();

                let deleted = unit_of_work
                    .galleries()
                    .delete_item(detail.item().gallery_item_id())
                    .await
                    .map_err(|error| DeleteGalleryItemError::Unknown(error.into()))?;
                if !deleted {
                    return Err(DeleteGalleryItemError::NoSuchItem);
                }
                let deleted_media = match media {
                    GalleryItemMedia::Image(image_id) => {
                        unit_of_work.images().delete(image_id).await
                    }
                    GalleryItemMedia::Video(video_id) => {
                        unit_of_work.videos().delete(video_id).await
                    }
                }
                .map_err(|error| DeleteGalleryItemError::Unknown(error.into()))?;
                if !deleted_media {
                    // The medium vanished between the item read and the delete;
                    // the destructive delete has still succeeded.
                }

                // The backing file is Asset-owned and its foreign keys from the
                // media tables do not cascade, so the deletion is orchestrated
                // here (`ARCH-003`, `ARCH-004`, `PERS-020`). The row joins this
                // unit of work; the bytes are removed once it commits.
                let stored = unit_of_work
                    .files()
                    .search(&FileFilter {
                        id: Some(detail.file_id()),
                        ..FileFilter::default()
                    })
                    .await
                    .map_err(|error| DeleteGalleryItemError::Unknown(error.into()))?
                    .into_iter()
                    .next();

                // The gallery predicate authorizes removing the item and its
                // medium. The backing file belongs to the user who uploaded it —
                // a `file` is unique per `(user_id, integrity_hash)` — so its row
                // and bytes are destroyed only for that user or the Root Admin,
                // the destruction side of the own-media rule the add path
                // enforces. Any other caller's removal still commits, keeping the
                // file.
                let removable = stored.filter(|file| {
                    Authorization::decide(&caller, &permissions, Action::FileUse, file).is_allowed()
                });
                let file_path = match removable.as_ref() {
                    Some(file) => {
                        // A `file` is shared per `(user_id, integrity_hash)`, so
                        // another media row may still reference it; the delete is
                        // confirmed safe first (`ARCH-004`, `PERS-020`). The image
                        // probe runs first; a hit short-circuits the video probe.
                        let still_referenced = !unit_of_work
                            .images()
                            .search(&ImageFilter {
                                file_id: Some(file.id()),
                                ..ImageFilter::default()
                            })
                            .await
                            .map_err(|error| DeleteGalleryItemError::Unknown(error.into()))?
                            .is_empty()
                            || !unit_of_work
                                .videos()
                                .search(&VideoFilter {
                                    file_id: Some(file.id()),
                                    ..VideoFilter::default()
                                })
                                .await
                                .map_err(|error| DeleteGalleryItemError::Unknown(error.into()))?
                                .is_empty();
                        if still_referenced {
                            None
                        } else {
                            match unit_of_work.files().delete(file.id()).await {
                                Ok(true) => Some(PathBuf::from(file.path())),
                                // The row was already gone — the removal that
                                // deleted it owns the bytes — or the datastore
                                // rejected the delete because a reference this
                                // guard cannot see survived (the Asset-owned
                                // `audio.file_id`): the file is kept, bytes
                                // included (`STY-RUST-016`).
                                Ok(false) | Err(RepositoryError::DataIntegrityViolation) => None,
                                Err(error) => {
                                    return Err(DeleteGalleryItemError::Unknown(error.into()));
                                }
                            }
                        }
                    }
                    None => None,
                };

                gallery.touch(Utc::now().naive_utc());
                unit_of_work
                    .galleries()
                    .save(gallery)
                    .await
                    .map_err(|error| DeleteGalleryItemError::Unknown(error.into()))?;

                Ok(file_path)
            }
            .await;

            match result {
                Ok(file_path) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| DeleteGalleryItemError::Unknown(error.into()))?;
                    // TODO(ingest): the path now addresses the operator's media
                    // mount, and a scanned file was never the server's to
                    // remove. Byte deletion is intentionally disabled; only the
                    // recorded reference is removed. Re-enable a guarded removal
                    // once a deletion policy consults `file.origin`.
                    drop(file_path);
                    Ok(())
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

    use crate::application::port::delete_gallery_item::DeleteGalleryItemCommand;
    use crate::application::port::delete_gallery_item::DeleteGalleryItemError;
    use crate::application::port::delete_gallery_item::DeleteGalleryItemUseCase as _;
    use crate::domain::model::file::File;
    use crate::domain::model::file::Origin;
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
    use crate::domain::port::file_storage::MockFileStorage;
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

    use super::DeleteGalleryItem;

    /// Stored path of the fixture file, relative to the storage root.
    const FILE_PATH: &str = "files/1_photo.png";

    /// Identifier of the standard user most tests act as.
    const CALLER_ID: i64 = 4;

    type UseCase = DeleteGalleryItem<TestFactory, MockFileStorage>;

    fn gallery(gallery_id: i64, user_id: Option<i64>, name: &str, is_public: bool) -> Gallery {
        let now = NaiveDateTime::default();
        Gallery::new(gallery_id, user_id, name.to_owned(), is_public, now, now)
    }

    fn detail(
        gallery_item_id: i64,
        gallery_id: i64,
        media: GalleryItemMedia,
    ) -> Result<GalleryItemDetail, Box<dyn Error>> {
        let (image_id, video_id) = match media {
            GalleryItemMedia::Image(id) => (Some(id), None),
            GalleryItemMedia::Video(id) => (None, Some(id)),
        };
        Ok(GalleryItemDetail::new(
            GalleryItem::try_new(
                gallery_item_id,
                gallery_id,
                image_id,
                video_id,
                gallery_item_id,
                NaiveDateTime::default(),
            )?,
            100,
            format!("item{gallery_item_id}"),
        ))
    }

    /// Build the backing `file` row the item's detail names, owned by `user_id`.
    fn backing_file(user_id: i64) -> Result<File, Box<dyn Error>> {
        Ok(File::try_new(
            100,
            FILE_PATH.to_owned(),
            user_id,
            false,
            "image/png".to_owned(),
            NaiveDate::from_ymd_opt(2026, 10, 16).ok_or("invalid fixture date")?,
            0,
            NaiveDate::from_ymd_opt(2026, 10, 16)
                .ok_or("invalid fixture date")?
                .and_hms_opt(0, 0, 0)
                .ok_or("invalid fixture time")?,
            Some(IntegrityHash::try_new(DIGEST.to_owned())?),
            Some(Origin::Uploaded),
        )?)
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

    /// Build the use case over `harness`, with `file_storage` removing the bytes.
    fn use_case(harness: &GalleryFactoryHarness, file_storage: MockFileStorage) -> UseCase {
        DeleteGalleryItem::new(Arc::clone(&harness.factory), Arc::new(file_storage))
    }

    /// Expect `owner_id`'s own, unreferenced backing file to be loaded and deleted.
    ///
    /// The caller owns `file` and the medium was deleted earlier in the unit of
    /// work, so no `image` or `video` row references it any more.
    fn expect_removable_backing_file(
        images: &mut MockImageRepository,
        videos: &mut MockVideoRepository,
        files: &mut MockFileRepository,
        owner_id: i64,
    ) -> Result<(), Box<dyn Error>> {
        images
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        videos
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let stored = backing_file(owner_id)?;
        files
            .expect_search()
            .times(1)
            .withf(|filter| filter.id == Some(100))
            .return_once(move |_| {
                let value = stored.clone();
                Box::pin(async move { Ok(vec![value]) })
            });
        files
            .expect_delete()
            .times(1)
            .withf(|id| *id == 100)
            .returning(|_| Box::pin(async { Ok(true) }));
        Ok(())
    }

    /// Expect no byte deletion: the mount belongs to the operator.
    fn expect_backing_bytes(file_storage: &mut MockFileStorage) {
        file_storage.expect_delete_upload_file().times(0);
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

    #[tokio::test]
    async fn delete_gallery_item_owner_deletes_image_and_item() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        let found = detail(42, 3, GalleryItemMedia::Image(7))?;
        galleries
            .expect_search_items()
            .times(1)
            .withf(|filter| filter.gallery_id == Some(3) && filter.gallery_item_id == Some(42))
            .return_once(move |_| {
                let value = found.clone();
                Box::pin(async move { Ok(vec![value]) })
            });
        galleries
            .expect_delete_item()
            .times(1)
            .withf(|id| *id == 42)
            .returning(|_| Box::pin(async { Ok(true) }));
        galleries
            .expect_save()
            .times(1)
            .withf(|saved| saved.last_modified_at() > NaiveDateTime::default())
            .returning(|saved| Box::pin(async move { Ok(saved) }));
        let mut images = MockImageRepository::new();
        images
            .expect_delete()
            .times(1)
            .withf(|id| *id == 7)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut videos = MockVideoRepository::new();
        let mut files = MockFileRepository::new();
        expect_removable_backing_file(&mut images, &mut videos, &mut files, 4)?;
        let mut file_storage = MockFileStorage::new();
        expect_backing_bytes(&mut file_storage);
        let harness = harness_with(users, galleries, images, videos, files);
        let use_case = use_case(&harness, file_storage);

        // Act
        let result = use_case
            .execute(DeleteGalleryItemCommand::new(4, 3, 42))
            .await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_storage_failure_still_succeeds() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        let found = detail(42, 3, GalleryItemMedia::Image(7))?;
        galleries
            .expect_search_items()
            .times(1)
            .return_once(move |_| {
                let value = found.clone();
                Box::pin(async move { Ok(vec![value]) })
            });
        galleries
            .expect_delete_item()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        galleries
            .expect_save()
            .times(1)
            .returning(|saved| Box::pin(async move { Ok(saved) }));
        let mut images = MockImageRepository::new();
        images
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut videos = MockVideoRepository::new();
        let mut files = MockFileRepository::new();
        expect_removable_backing_file(&mut images, &mut videos, &mut files, 4)?;
        // The mount belongs to the operator: no byte deletion may occur.
        let mut file_storage = MockFileStorage::new();
        file_storage.expect_delete_upload_file().times(0);
        let harness = harness_with(users, galleries, images, videos, files);
        let use_case = use_case(&harness, file_storage);

        // Act
        let result = use_case
            .execute(DeleteGalleryItemCommand::new(4, 3, 42))
            .await;

        // Assert: the reference is removed, the content is not, and the row
        // deletion commits.
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_any_caller_deletes_public_item() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(2, Some(3), "Public", true));
        let found = detail(42, 2, GalleryItemMedia::Video(8))?;
        galleries
            .expect_search_items()
            .times(1)
            .return_once(move |_| {
                let value = found.clone();
                Box::pin(async move { Ok(vec![value]) })
            });
        galleries
            .expect_delete_item()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        galleries
            .expect_save()
            .times(1)
            .returning(|saved| Box::pin(async move { Ok(saved) }));
        let mut videos = MockVideoRepository::new();
        videos
            .expect_delete()
            .times(1)
            .withf(|id| *id == 8)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut images = MockImageRepository::new();
        let mut files = MockFileRepository::new();
        // The caller's own medium sits in another user's public gallery, so the
        // own-media predicate lets the removal destroy the file.
        expect_removable_backing_file(&mut images, &mut videos, &mut files, 9)?;
        let mut file_storage = MockFileStorage::new();
        expect_backing_bytes(&mut file_storage);
        let harness = harness_with(users, galleries, images, videos, files);
        let use_case = use_case(&harness, file_storage);

        // Act
        let result = use_case
            .execute(DeleteGalleryItemCommand::new(9, 2, 42))
            .await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_non_owner_keeps_the_backing_file() -> Result<(), Box<dyn Error>> {
        // Arrange: the item is in another user's public gallery, so the caller
        // may remove it, but the backing file belongs to that other user.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(2, Some(3), "Public", true));
        let found = detail(42, 2, GalleryItemMedia::Image(7))?;
        galleries
            .expect_search_items()
            .times(1)
            .return_once(move |_| {
                let value = found.clone();
                Box::pin(async move { Ok(vec![value]) })
            });
        galleries
            .expect_delete_item()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        galleries
            .expect_save()
            .times(1)
            .returning(|saved| Box::pin(async move { Ok(saved) }));
        let mut images = MockImageRepository::new();
        images
            .expect_delete()
            .times(1)
            .withf(|id| *id == 7)
            .returning(|_| Box::pin(async { Ok(true) }));
        // The file is not the caller's, so the reference guard never runs.
        images.expect_search().times(0);
        let mut videos = MockVideoRepository::new();
        videos.expect_search().times(0);
        let mut files = MockFileRepository::new();
        let stored = backing_file(3)?;
        files
            .expect_search()
            .times(1)
            .withf(|filter| filter.id == Some(100))
            .return_once(move |_| {
                let value = stored.clone();
                Box::pin(async move { Ok(vec![value]) })
            });
        files.expect_delete().times(0);
        let mut file_storage = MockFileStorage::new();
        file_storage.expect_delete_upload_file().times(0);
        let harness = harness_with(users, galleries, images, videos, files);
        let use_case = use_case(&harness, file_storage);

        // Act
        let result = use_case
            .execute(DeleteGalleryItemCommand::new(9, 2, 42))
            .await;

        // Assert: the item and its medium are gone, the file's row and bytes are
        // kept, and the removal still commits.
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_root_admin_deletes_other_users_private_item()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the Root Admin may remove an item of another user's private
        // gallery and destroy the file that user owns.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 0, Role::Admin)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        let found = detail(42, 3, GalleryItemMedia::Image(7))?;
        galleries
            .expect_search_items()
            .times(1)
            .return_once(move |_| {
                let value = found.clone();
                Box::pin(async move { Ok(vec![value]) })
            });
        galleries
            .expect_delete_item()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        galleries
            .expect_save()
            .times(1)
            .returning(|saved| Box::pin(async move { Ok(saved) }));
        let mut images = MockImageRepository::new();
        images
            .expect_delete()
            .times(1)
            .withf(|id| *id == 7)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut videos = MockVideoRepository::new();
        let mut files = MockFileRepository::new();
        expect_removable_backing_file(&mut images, &mut videos, &mut files, 4)?;
        let mut file_storage = MockFileStorage::new();
        expect_backing_bytes(&mut file_storage);
        let harness = harness_with(users, galleries, images, videos, files);
        let use_case = use_case(&harness, file_storage);

        // Act
        let result = use_case
            .execute(DeleteGalleryItemCommand::new(0, 3, 42))
            .await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_backing_file_failure_returns_unknown() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        let found = detail(42, 3, GalleryItemMedia::Image(7))?;
        galleries
            .expect_search_items()
            .times(1)
            .return_once(move |_| {
                let value = found.clone();
                Box::pin(async move { Ok(vec![value]) })
            });
        galleries
            .expect_delete_item()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        galleries.expect_save().times(0);
        let mut images = MockImageRepository::new();
        images
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        images
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut videos = MockVideoRepository::new();
        videos
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut files = MockFileRepository::new();
        let stored = backing_file(CALLER_ID)?;
        files.expect_search().times(1).return_once(move |_| {
            let value = stored.clone();
            Box::pin(async move { Ok(vec![value]) })
        });
        files
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let mut file_storage = MockFileStorage::new();
        file_storage.expect_delete_upload_file().times(0);
        let harness = harness_with(users, galleries, images, videos, files);
        let use_case = use_case(&harness, file_storage);

        // Act
        let result = use_case
            .execute(DeleteGalleryItemCommand::new(4, 3, 42))
            .await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryItemError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_keeps_a_file_an_image_still_references()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the item's own image is deleted, but another image still
        // references the shared file, so its row and bytes are kept.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        let found = detail(42, 3, GalleryItemMedia::Image(7))?;
        galleries
            .expect_search_items()
            .times(1)
            .return_once(move |_| {
                let value = found.clone();
                Box::pin(async move { Ok(vec![value]) })
            });
        galleries
            .expect_delete_item()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        galleries
            .expect_save()
            .times(1)
            .returning(|saved| Box::pin(async move { Ok(saved) }));
        let mut images = MockImageRepository::new();
        images
            .expect_delete()
            .times(1)
            .withf(|id| *id == 7)
            .returning(|_| Box::pin(async { Ok(true) }));
        let referencing = Image::try_new(
            1,
            "other.png".to_owned(),
            10,
            10,
            Orientation::Square,
            NaiveDate::from_ymd_opt(2026, 10, 16).ok_or("invalid fixture date")?,
            100,
        )?;
        images.expect_search().times(1).return_once(move |_| {
            let value = referencing.clone();
            Box::pin(async move { Ok(vec![value]) })
        });
        let mut videos = MockVideoRepository::new();
        // The image reference is found first, so the video probe never runs.
        videos.expect_search().times(0);
        let mut files = MockFileRepository::new();
        let stored = backing_file(4)?;
        files.expect_search().times(1).return_once(move |_| {
            let value = stored.clone();
            Box::pin(async move { Ok(vec![value]) })
        });
        files.expect_delete().times(0);
        let mut file_storage = MockFileStorage::new();
        file_storage.expect_delete_upload_file().times(0);
        let harness = harness_with(users, galleries, images, videos, files);
        let use_case = use_case(&harness, file_storage);

        // Act
        let result = use_case
            .execute(DeleteGalleryItemCommand::new(4, 3, 42))
            .await;

        // Assert: the removal commits, keeping the shared file.
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_keeps_a_file_a_video_still_references()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the item's own image is deleted, but a video still references
        // the shared file, so its row and bytes are kept.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        let found = detail(42, 3, GalleryItemMedia::Image(7))?;
        galleries
            .expect_search_items()
            .times(1)
            .return_once(move |_| {
                let value = found.clone();
                Box::pin(async move { Ok(vec![value]) })
            });
        galleries
            .expect_delete_item()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        galleries
            .expect_save()
            .times(1)
            .returning(|saved| Box::pin(async move { Ok(saved) }));
        let mut images = MockImageRepository::new();
        images
            .expect_delete()
            .times(1)
            .withf(|id| *id == 7)
            .returning(|_| Box::pin(async { Ok(true) }));
        images
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut videos = MockVideoRepository::new();
        let referencing = Video::try_new(
            2,
            1_000.0,
            "H264".to_owned(),
            24,
            640,
            480,
            "YCbCr".to_owned(),
            ScanType::Progressive,
            100,
        )?;
        videos.expect_search().times(1).return_once(move |_| {
            let value = referencing.clone();
            Box::pin(async move { Ok(vec![value]) })
        });
        let mut files = MockFileRepository::new();
        let stored = backing_file(4)?;
        files.expect_search().times(1).return_once(move |_| {
            let value = stored.clone();
            Box::pin(async move { Ok(vec![value]) })
        });
        files.expect_delete().times(0);
        let mut file_storage = MockFileStorage::new();
        file_storage.expect_delete_upload_file().times(0);
        let harness = harness_with(users, galleries, images, videos, files);
        let use_case = use_case(&harness, file_storage);

        // Act
        let result = use_case
            .execute(DeleteGalleryItemCommand::new(4, 3, 42))
            .await;

        // Assert: the removal commits, keeping the shared file.
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_missing_file_row_removes_no_bytes() -> Result<(), Box<dyn Error>> {
        // Arrange: the file row is already gone when the delete runs, so the
        // removal that deleted it owns the bytes and none are removed here.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        let found = detail(42, 3, GalleryItemMedia::Image(7))?;
        galleries
            .expect_search_items()
            .times(1)
            .return_once(move |_| {
                let value = found.clone();
                Box::pin(async move { Ok(vec![value]) })
            });
        galleries
            .expect_delete_item()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        galleries
            .expect_save()
            .times(1)
            .returning(|saved| Box::pin(async move { Ok(saved) }));
        let mut images = MockImageRepository::new();
        images
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        images
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut videos = MockVideoRepository::new();
        videos
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut files = MockFileRepository::new();
        let stored = backing_file(4)?;
        files.expect_search().times(1).return_once(move |_| {
            let value = stored.clone();
            Box::pin(async move { Ok(vec![value]) })
        });
        files
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(false) }));
        let mut file_storage = MockFileStorage::new();
        file_storage.expect_delete_upload_file().times(0);
        let harness = harness_with(users, galleries, images, videos, files);
        let use_case = use_case(&harness, file_storage);

        // Act
        let result = use_case
            .execute(DeleteGalleryItemCommand::new(4, 3, 42))
            .await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_rejected_file_delete_keeps_the_file() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the datastore rejects the delete because a reference the guard
        // cannot see survives (`audio.file_id` today has no port to query); the
        // file is kept and the removal still commits.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        let found = detail(42, 3, GalleryItemMedia::Image(7))?;
        galleries
            .expect_search_items()
            .times(1)
            .return_once(move |_| {
                let value = found.clone();
                Box::pin(async move { Ok(vec![value]) })
            });
        galleries
            .expect_delete_item()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        galleries
            .expect_save()
            .times(1)
            .returning(|saved| Box::pin(async move { Ok(saved) }));
        let mut images = MockImageRepository::new();
        images
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        images
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut videos = MockVideoRepository::new();
        videos
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut files = MockFileRepository::new();
        let stored = backing_file(4)?;
        files.expect_search().times(1).return_once(move |_| {
            let value = stored.clone();
            Box::pin(async move { Ok(vec![value]) })
        });
        files
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::DataIntegrityViolation) }));
        let mut file_storage = MockFileStorage::new();
        file_storage.expect_delete_upload_file().times(0);
        let harness = harness_with(users, galleries, images, videos, files);
        let use_case = use_case(&harness, file_storage);

        // Act
        let result = use_case
            .execute(DeleteGalleryItemCommand::new(4, 3, 42))
            .await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_reference_probe_failure_returns_unknown()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the image reference probe fails, so the removal rolls back
        // rather than leaking or wrongly deleting the shared file.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        let found = detail(42, 3, GalleryItemMedia::Image(7))?;
        galleries
            .expect_search_items()
            .times(1)
            .return_once(move |_| {
                let value = found.clone();
                Box::pin(async move { Ok(vec![value]) })
            });
        galleries
            .expect_delete_item()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        galleries.expect_save().times(0);
        let mut images = MockImageRepository::new();
        images
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        images
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let mut videos = MockVideoRepository::new();
        videos.expect_search().times(0);
        let mut files = MockFileRepository::new();
        let stored = backing_file(4)?;
        files.expect_search().times(1).return_once(move |_| {
            let value = stored.clone();
            Box::pin(async move { Ok(vec![value]) })
        });
        files.expect_delete().times(0);
        let mut file_storage = MockFileStorage::new();
        file_storage.expect_delete_upload_file().times(0);
        let harness = harness_with(users, galleries, images, videos, files);
        let use_case = use_case(&harness, file_storage);

        // Act
        let result = use_case
            .execute(DeleteGalleryItemCommand::new(4, 3, 42))
            .await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryItemError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_other_private_caller_returns_no_such_gallery()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        galleries.expect_delete_item().times(0);
        let harness = harness_with(
            users,
            galleries,
            MockImageRepository::new(),
            MockVideoRepository::new(),
            MockFileRepository::new(),
        );
        let use_case = use_case(&harness, MockFileStorage::new());

        // Act
        let result = use_case
            .execute(DeleteGalleryItemCommand::new(9, 3, 42))
            .await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryItemError::NoSuchGallery)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_unknown_item_returns_no_such_item() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        galleries
            .expect_search_items()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        galleries.expect_delete_item().times(0);
        let harness = harness_with(
            users,
            galleries,
            MockImageRepository::new(),
            MockVideoRepository::new(),
            MockFileRepository::new(),
        );
        let use_case = use_case(&harness, MockFileStorage::new());

        // Act
        let result = use_case
            .execute(DeleteGalleryItemCommand::new(4, 3, 42))
            .await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryItemError::NoSuchItem)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_unknown_gallery_returns_no_such_gallery()
    -> Result<(), Box<dyn Error>> {
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
        let use_case = use_case(&harness, MockFileStorage::new());

        // Act
        let result = use_case
            .execute(DeleteGalleryItemCommand::new(4, 999, 42))
            .await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryItemError::NoSuchGallery)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_unknown_caller_returns_no_such_caller()
    -> Result<(), Box<dyn Error>> {
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
        let use_case = use_case(&harness, MockFileStorage::new());

        // Act
        let result = use_case
            .execute(DeleteGalleryItemCommand::new(999, 3, 42))
            .await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryItemError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        galleries
            .expect_search_items()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(
            users,
            galleries,
            MockImageRepository::new(),
            MockVideoRepository::new(),
            MockFileRepository::new(),
        );
        let use_case = use_case(&harness, MockFileStorage::new());

        // Act
        let result = use_case
            .execute(DeleteGalleryItemCommand::new(4, 3, 42))
            .await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryItemError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
        let found = detail(42, 3, GalleryItemMedia::Image(7))?;
        galleries
            .expect_search_items()
            .times(1)
            .return_once(move |_| {
                let value = found.clone();
                Box::pin(async move { Ok(vec![value]) })
            });
        galleries
            .expect_delete_item()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        galleries
            .expect_save()
            .times(1)
            .returning(|saved| Box::pin(async move { Ok(saved) }));
        let mut images = MockImageRepository::new();
        images
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut videos = MockVideoRepository::new();
        let mut files = MockFileRepository::new();
        expect_removable_backing_file(&mut images, &mut videos, &mut files, 4)?;
        let harness = harness_with(users, galleries, images, videos, files);
        harness.commit_fails.store(true, Ordering::SeqCst);
        // The commit fails, so the rows are discarded and the bytes are never
        // removed: the storage adapter is not reached.
        let use_case = use_case(&harness, MockFileStorage::new());

        // Act
        let result = use_case
            .execute(DeleteGalleryItemCommand::new(4, 3, 42))
            .await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryItemError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }
}
