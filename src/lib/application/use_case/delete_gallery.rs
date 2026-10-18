use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::delete_gallery::DeleteGalleryCommand;
use crate::application::port::delete_gallery::DeleteGalleryError;
use crate::application::port::delete_gallery::DeleteGalleryUseCase;
use crate::application::security_event;
use crate::application::use_case::library_access;
use crate::domain::model::gallery_item::GalleryItemMedia;
use crate::domain::port::asset_unit_of_work::AssetUnitOfWork;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::file_repository::FileFilter;
use crate::domain::port::file_repository::FileRepository as _;
use crate::domain::port::file_storage::FileStorage;
use crate::domain::port::gallery_repository::GalleryFilter;
use crate::domain::port::gallery_repository::GalleryItemFilter;
use crate::domain::port::gallery_repository::GalleryRepository as _;
use crate::domain::port::image_repository::ImageFilter;
use crate::domain::port::image_repository::ImageRepository as _;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::port::video_repository::VideoFilter;
use crate::domain::port::video_repository::VideoRepository as _;

/// Use case implementation for deleting a gallery and its items.
pub struct DeleteGallery<F, S> {
    /// Storage adapter removing each backing file's bytes.
    file_storage: Arc<S>,
    /// Root directory holding the upload and file folders.
    root: PathBuf,
    /// Factory opening the unit of work wrapping the deletion.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, S: FileStorage> DeleteGallery<F, S> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>, file_storage: Arc<S>, root: PathBuf) -> Self {
        Self {
            file_storage,
            root,
            unit_of_work_factory,
        }
    }
}

impl<F, S> DeleteGalleryUseCase for DeleteGallery<F, S>
where
    F: UnitOfWorkFactory,
    F::Uow: LibraryUnitOfWork + UserUnitOfWork + AssetUnitOfWork,
    S: FileStorage,
{
    fn execute<'future>(
        &'future self,
        command: DeleteGalleryCommand,
    ) -> Pin<Box<dyn Future<Output = Result<(), DeleteGalleryError>> + Send + 'future>> {
        let file_storage = Arc::clone(&self.file_storage);
        let root = self.root.clone();
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| DeleteGalleryError::Unknown(error.into()))?;

            let result = async {
                let caller = library_access::load_caller(&mut unit_of_work, command.caller_id())
                    .await
                    .map_err(|error| DeleteGalleryError::Unknown(error.into()))?
                    .ok_or(DeleteGalleryError::NoSuchCaller)?;

                let gallery = unit_of_work
                    .galleries()
                    .search(&GalleryFilter {
                        gallery_id: Some(command.gallery_id()),
                        ..GalleryFilter::default()
                    })
                    .await
                    .map_err(|error| DeleteGalleryError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(DeleteGalleryError::NoSuchGallery)?;

                if gallery.is_system() {
                    return Err(DeleteGalleryError::DefaultGallery);
                }
                if !gallery.is_owned_by(caller.id()) {
                    security_event::authorization_failed(caller.id(), "delete", "gallery");
                    return Err(DeleteGalleryError::Forbidden);
                }

                let items = unit_of_work
                    .galleries()
                    .search_items(&GalleryItemFilter {
                        gallery_id: Some(command.gallery_id()),
                        ..GalleryItemFilter::default()
                    })
                    .await
                    .map_err(|error| DeleteGalleryError::Unknown(error.into()))?;
                let mut removed_paths = Vec::new();
                for detail in items {
                    unit_of_work
                        .galleries()
                        .delete_item(detail.item().gallery_item_id())
                        .await
                        .map_err(|error| DeleteGalleryError::Unknown(error.into()))?;
                    let deleted = match detail.item().media() {
                        GalleryItemMedia::Image(image_id) => {
                            unit_of_work.images().delete(image_id).await
                        }
                        GalleryItemMedia::Video(video_id) => {
                            unit_of_work.videos().delete(video_id).await
                        }
                    }
                    .map_err(|error| DeleteGalleryError::Unknown(error.into()))?;
                    if !deleted {
                        // The medium vanished between the item read and the
                        // delete; the destructive delete has still succeeded.
                    }

                    // The backing file is Asset-owned and its foreign keys from
                    // the media tables do not cascade, so the deletion is
                    // orchestrated here (`ARCH-003`, `ARCH-004`, `PERS-020`). The
                    // row joins this unit of work; the bytes are removed once it
                    // commits.
                    let stored = unit_of_work
                        .files()
                        .search(&FileFilter {
                            id: Some(detail.file_id()),
                            ..FileFilter::default()
                        })
                        .await
                        .map_err(|error| DeleteGalleryError::Unknown(error.into()))?
                        .into_iter()
                        .next();

                    // The gallery predicate authorizes removing the item and its
                    // medium. The backing file belongs to the user who uploaded
                    // it — a `file` is unique per `(user_id, integrity_hash)` — so
                    // its row and bytes are destroyed only for that user or the
                    // Root Admin, the own-media rule the add path enforces. Any
                    // other caller's removal still commits, keeping the file.
                    let removable = stored
                        .filter(|file| caller.is_root_admin() || caller.id() == file.user_id());
                    if let Some(file) = removable {
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
                            .map_err(|error| DeleteGalleryError::Unknown(error.into()))?
                            .is_empty()
                            || !unit_of_work
                                .videos()
                                .search(&VideoFilter {
                                    file_id: Some(file.id()),
                                    ..VideoFilter::default()
                                })
                                .await
                                .map_err(|error| DeleteGalleryError::Unknown(error.into()))?
                                .is_empty();
                        if !still_referenced {
                            match unit_of_work.files().delete(file.id()).await {
                                Ok(true) => removed_paths.push(PathBuf::from(file.path())),
                                // The row was already gone — the removal that
                                // deleted it owns the bytes — or the datastore
                                // rejected the delete because a reference this
                                // guard cannot see survived (the Asset-owned
                                // `audio.file_id`): the file is kept, bytes
                                // included (`STY-RUST-016`).
                                Ok(false) | Err(RepositoryError::DataIntegrityViolation) => {}
                                Err(error) => {
                                    return Err(DeleteGalleryError::Unknown(error.into()));
                                }
                            }
                        }
                    }
                }

                let deleted = unit_of_work
                    .galleries()
                    .delete(command.gallery_id())
                    .await
                    .map_err(|error| DeleteGalleryError::Unknown(error.into()))?;
                if !deleted {
                    return Err(DeleteGalleryError::NoSuchGallery);
                }

                Ok(removed_paths)
            }
            .await;

            match result {
                Ok(removed_paths) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| DeleteGalleryError::Unknown(error.into()))?;
                    // The rows have committed; the bytes cannot join the
                    // transaction, so they are removed best-effort afterwards.
                    for stored_path in removed_paths {
                        if file_storage
                            .delete_upload_file(&root.join(stored_path))
                            .await
                            .is_err()
                        {
                            // The file-storage adapter owns the failure log
                            // (`OBS-002`); the committed deletion stands, so the
                            // removal is not failed.
                        }
                    }
                    Ok(())
                }
                Err(error) => {
                    if unit_of_work.rollback().await.is_err() {
                        // The unit-of-work adapter owns the rollback-failure log (OBS-002).
                    }
                    Err(error)
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use chrono::NaiveDate;
    use chrono::NaiveDateTime;

    use crate::application::port::delete_gallery::DeleteGalleryCommand;
    use crate::application::port::delete_gallery::DeleteGalleryError;
    use crate::application::port::delete_gallery::DeleteGalleryUseCase as _;
    use crate::domain::model::file::File;
    use crate::domain::model::gallery::Gallery;
    use crate::domain::model::gallery_item::GalleryItem;
    use crate::domain::model::gallery_item::GalleryItemMedia;
    use crate::domain::model::image::Image;
    use crate::domain::model::image::Orientation;
    use crate::domain::model::integrity_hash::IntegrityHash;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::error::StorageError;
    use crate::domain::port::file_repository::MockFileRepository;
    use crate::domain::port::file_storage::MockFileStorage;
    use crate::domain::port::gallery_repository::GalleryItemDetail;
    use crate::domain::port::gallery_repository::MockGalleryRepository;
    use crate::domain::port::image_repository::MockImageRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::domain::port::video_repository::MockVideoRepository;
    use crate::test_helpers::GalleryFactoryHarness;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::TestUnitOfWork;

    use super::DeleteGallery;

    /// A 64-character hexadecimal digest, valid for `integrity_hash`.
    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    /// Storage root the use case composes the bytes path against.
    const ROOT: &str = "/storage";

    type UseCase = DeleteGallery<TestFactory, MockFileStorage>;

    fn gallery(
        gallery_id: i64,
        user_id: Option<i64>,
        name: &str,
        is_public: bool,
    ) -> Result<Gallery, Box<dyn Error>> {
        let now = NaiveDateTime::default();
        Ok(Gallery::try_new(
            gallery_id,
            user_id,
            name.to_owned(),
            is_public,
            now,
            now,
        )?)
    }

    fn item_detail(
        gallery_item_id: i64,
        gallery_id: i64,
        media: GalleryItemMedia,
        file_id: i64,
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
            file_id,
            format!("item{gallery_item_id}"),
        ))
    }

    /// Build a backing `file` row owned by `user_id` at `path`.
    fn backing_file(file_id: i64, user_id: i64, path: &str) -> Result<File, Box<dyn Error>> {
        Ok(File::try_new(
            file_id,
            path.to_owned(),
            user_id,
            false,
            "image/png".to_owned(),
            NaiveDate::from_ymd_opt(2026, 10, 16).ok_or("invalid fixture date")?,
            IntegrityHash::try_new(DIGEST.to_owned())?,
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
        DeleteGallery::new(
            Arc::clone(&harness.factory),
            Arc::new(file_storage),
            PathBuf::from(ROOT),
        )
    }

    /// Expect no media row to reference the file the guard probes.
    fn expect_no_media_references(
        images: &mut MockImageRepository,
        videos: &mut MockVideoRepository,
    ) {
        images
            .expect_search()
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        videos
            .expect_search()
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
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
    async fn delete_gallery_owner_deletes_items_media_files_and_gallery()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        let details = vec![
            item_detail(1, 3, GalleryItemMedia::Image(7), 100)?,
            item_detail(2, 3, GalleryItemMedia::Video(8), 200)?,
        ];
        galleries
            .expect_search_items()
            .times(1)
            .return_once(move |_| {
                let found = details.clone();
                Box::pin(async move { Ok(found) })
            });
        galleries
            .expect_delete_item()
            .times(2)
            .returning(|_| Box::pin(async { Ok(true) }));
        galleries
            .expect_delete()
            .times(1)
            .withf(|id| *id == 3)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut images = MockImageRepository::new();
        images
            .expect_delete()
            .times(1)
            .withf(|id| *id == 7)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut videos = MockVideoRepository::new();
        videos
            .expect_delete()
            .times(1)
            .withf(|id| *id == 8)
            .returning(|_| Box::pin(async { Ok(true) }));
        // Both media rows are deleted first, so neither file is referenced.
        expect_no_media_references(&mut images, &mut videos);
        let mut files = MockFileRepository::new();
        let first = backing_file(100, 4, "files/1_photo.png")?;
        let second = backing_file(200, 4, "files/2_clip.mp4")?;
        files
            .expect_search()
            .times(1)
            .withf(|filter| filter.id == Some(100))
            .return_once(move |_| {
                let value = first.clone();
                Box::pin(async move { Ok(vec![value]) })
            });
        files
            .expect_search()
            .times(1)
            .withf(|filter| filter.id == Some(200))
            .return_once(move |_| {
                let value = second.clone();
                Box::pin(async move { Ok(vec![value]) })
            });
        files
            .expect_delete()
            .times(2)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_delete_upload_file()
            .times(2)
            .withf(|path| {
                path == Path::new("/storage/files/1_photo.png")
                    || path == Path::new("/storage/files/2_clip.mp4")
            })
            .returning(|_| Box::pin(async { Ok(()) }));
        let harness = harness_with(users, galleries, images, videos, files);
        let use_case = use_case(&harness, file_storage);

        // Act
        let result = use_case.execute(DeleteGalleryCommand::new(4, 3)).await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_keeps_a_file_the_owner_does_not_own() -> Result<(), Box<dyn Error>> {
        // Arrange: the gallery owner may delete the gallery, but the item's
        // backing file belongs to another user, so its row and bytes are kept
        // (the own-media rule the add path enforces).
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", true)?);
        let found = item_detail(1, 3, GalleryItemMedia::Image(7), 100)?;
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
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut images = MockImageRepository::new();
        images
            .expect_delete()
            .times(1)
            .withf(|id| *id == 7)
            .returning(|_| Box::pin(async { Ok(true) }));
        // The file is not the caller's, so the reference guard never runs.
        let mut videos = MockVideoRepository::new();
        videos.expect_search().times(0);
        let mut files = MockFileRepository::new();
        let stored = backing_file(100, 9, "files/1_photo.png")?;
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
        let result = use_case.execute(DeleteGalleryCommand::new(4, 3)).await;

        // Assert: the gallery and its item are gone, the file is kept, and the
        // removal still commits.
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_keeps_a_file_a_surviving_medium_references()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the item's own medium is deleted, but another image still
        // references the shared file, so its row and bytes are kept.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        let found = item_detail(1, 3, GalleryItemMedia::Image(7), 100)?;
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
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut images = MockImageRepository::new();
        images
            .expect_delete()
            .times(1)
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
        // The image reference is found first, so the video probe never runs.
        let mut videos = MockVideoRepository::new();
        videos.expect_search().times(0);
        let mut files = MockFileRepository::new();
        let stored = backing_file(100, 4, "files/1_photo.png")?;
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
        let result = use_case.execute(DeleteGalleryCommand::new(4, 3)).await;

        // Assert: the removal commits, keeping the shared file.
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_missing_file_row_removes_no_bytes() -> Result<(), Box<dyn Error>> {
        // Arrange: the file row is already gone when the delete runs, so the
        // removal that deleted it owns the bytes and none are removed here.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        let found = item_detail(1, 3, GalleryItemMedia::Image(7), 100)?;
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
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut images = MockImageRepository::new();
        images
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut videos = MockVideoRepository::new();
        expect_no_media_references(&mut images, &mut videos);
        let mut files = MockFileRepository::new();
        let stored = backing_file(100, 4, "files/1_photo.png")?;
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
        let result = use_case.execute(DeleteGalleryCommand::new(4, 3)).await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_rejected_file_delete_keeps_the_file() -> Result<(), Box<dyn Error>> {
        // Arrange: the datastore rejects the delete because a reference the guard
        // cannot see survives (`audio.file_id` today has no port to query); the
        // file is kept and the removal still commits.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        let found = item_detail(1, 3, GalleryItemMedia::Image(7), 100)?;
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
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut images = MockImageRepository::new();
        images
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut videos = MockVideoRepository::new();
        expect_no_media_references(&mut images, &mut videos);
        let mut files = MockFileRepository::new();
        let stored = backing_file(100, 4, "files/1_photo.png")?;
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
        let result = use_case.execute(DeleteGalleryCommand::new(4, 3)).await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_backing_file_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange: the file-row delete fails, so the removal rolls back rather
        // than deleting the gallery and leaving the file's row and bytes.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        let found = item_detail(1, 3, GalleryItemMedia::Image(7), 100)?;
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
        galleries.expect_delete().times(0);
        let mut images = MockImageRepository::new();
        images
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut videos = MockVideoRepository::new();
        expect_no_media_references(&mut images, &mut videos);
        let mut files = MockFileRepository::new();
        let stored = backing_file(100, 4, "files/1_photo.png")?;
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
        let result = use_case.execute(DeleteGalleryCommand::new(4, 3)).await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_reference_probe_failure_returns_unknown() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the image reference probe fails, so the removal rolls back
        // rather than leaking or wrongly deleting the shared file.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        let found = item_detail(1, 3, GalleryItemMedia::Image(7), 100)?;
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
        galleries.expect_delete().times(0);
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
        let stored = backing_file(100, 4, "files/1_photo.png")?;
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
        let result = use_case.execute(DeleteGalleryCommand::new(4, 3)).await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_storage_failure_still_succeeds() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        let found = item_detail(1, 3, GalleryItemMedia::Image(7), 100)?;
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
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut images = MockImageRepository::new();
        images
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut videos = MockVideoRepository::new();
        expect_no_media_references(&mut images, &mut videos);
        let mut files = MockFileRepository::new();
        let stored = backing_file(100, 4, "files/1_photo.png")?;
        files.expect_search().times(1).return_once(move |_| {
            let value = stored.clone();
            Box::pin(async move { Ok(vec![value]) })
        });
        files
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_delete_upload_file()
            .times(1)
            .returning(|_| Box::pin(async { Err(StorageError::OperationFailed) }));
        let harness = harness_with(users, galleries, images, videos, files);
        let use_case = use_case(&harness, file_storage);

        // Act
        let result = use_case.execute(DeleteGalleryCommand::new(4, 3)).await;

        // Assert: the rows have committed, so a failure removing the bytes does
        // not fail the removal — the file-storage adapter logs it (OBS-002).
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_default_gallery_returns_default_gallery() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Admin)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(0, None, "Default", true)?);
        galleries.expect_search_items().times(0);
        galleries.expect_delete().times(0);
        let harness = harness_with(
            users,
            galleries,
            MockImageRepository::new(),
            MockVideoRepository::new(),
            MockFileRepository::new(),
        );
        let use_case = use_case(&harness, MockFileStorage::new());

        // Act
        let result = use_case.execute(DeleteGalleryCommand::new(1, 0)).await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryError::DefaultGallery)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_admin_not_owner_returns_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Admin)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        galleries.expect_delete().times(0);
        let harness = harness_with(
            users,
            galleries,
            MockImageRepository::new(),
            MockVideoRepository::new(),
            MockFileRepository::new(),
        );
        let use_case = use_case(&harness, MockFileStorage::new());

        // Act
        let result = use_case.execute(DeleteGalleryCommand::new(1, 3)).await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryError::Forbidden)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_unknown_gallery_returns_no_such_gallery() -> Result<(), Box<dyn Error>>
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
        let use_case = use_case(&harness, MockFileStorage::new());

        // Act
        let result = use_case.execute(DeleteGalleryCommand::new(4, 999)).await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryError::NoSuchGallery)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
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
        let result = use_case.execute(DeleteGalleryCommand::new(999, 3)).await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_dependency_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
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
        let result = use_case.execute(DeleteGalleryCommand::new(4, 3)).await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        galleries
            .expect_search_items()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        galleries
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let harness = harness_with(
            users,
            galleries,
            MockImageRepository::new(),
            MockVideoRepository::new(),
            MockFileRepository::new(),
        );
        harness.commit_fails.store(true, Ordering::SeqCst);
        let use_case = use_case(&harness, MockFileStorage::new());

        // Act
        let result = use_case.execute(DeleteGalleryCommand::new(4, 3)).await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }
}
