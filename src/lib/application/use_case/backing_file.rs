//! Asset-side cleanup of a removed gallery medium.
//!
//! A gallery item and its medium belong to the Library context, while the file
//! backing the medium belongs to the Asset context. The cross-context foreign
//! keys (`image.file_id`, `video.file_id`) do not cascade, so the application
//! layer orchestrates the deletion (`ARCH-003`, `ARCH-004`, `PERS-020`).
//!
//! The `file` row joins the unit of work that deletes the Library rows, so every
//! row commits together (`STY-RUST-040`). The bytes on the storage backend cannot
//! join that transaction: they are removed best-effort once the unit of work has
//! committed, and a storage failure never rolls the committed rows back.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use crate::domain::model::file::File;
use crate::domain::port::asset_unit_of_work::AssetUnitOfWork;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::file_repository::FileRepository as _;
use crate::domain::port::file_storage::FileStorage;
use crate::domain::port::image_repository::ImageFilter;
use crate::domain::port::image_repository::ImageRepository as _;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::video_repository::VideoFilter;
use crate::domain::port::video_repository::VideoRepository as _;

/// Delete the `file` row of `file` and return its stored path.
///
/// The caller has loaded `file` and has already decided the file may be
/// destroyed. A `file` is shared per `(user_id, integrity_hash)`, so the delete
/// is confirmed safe first: when another `image` or `video` row still references
/// the file, the row is kept and the outcome is `None`. A delete the datastore
/// still rejects — a reference this guard cannot see, such as the Asset-owned
/// `audio.file_id` or a row inserted concurrently — is the same shared-file
/// outcome (`ARCH-004`, `PERS-020`, `STY-RUST-016`).
///
/// The returned path is the stored, root-relative path of the deleted file. The
/// caller passes it to [`remove_bytes`] once the unit of work has committed.
///
/// # Errors
///
/// Returns the repository error when a media query fails or the delete fails for
/// a reason other than a rejected reference.
#[cfg_attr(
    not(test),
    expect(
        clippy::single_call_fn,
        reason = "extracted for the Delete a Gallery use case to reuse; this change's only caller calls it once"
    )
)]
pub(crate) async fn remove_row<U>(
    unit_of_work: &mut U,
    file: &File,
) -> Result<Option<PathBuf>, RepositoryError>
where
    U: AssetUnitOfWork + LibraryUnitOfWork,
{
    if !unit_of_work
        .images()
        .search(&ImageFilter {
            file_id: Some(file.id()),
            ..ImageFilter::default()
        })
        .await?
        .is_empty()
    {
        return Ok(None);
    }
    if !unit_of_work
        .videos()
        .search(&VideoFilter {
            file_id: Some(file.id()),
            ..VideoFilter::default()
        })
        .await?
        .is_empty()
    {
        return Ok(None);
    }

    match unit_of_work.files().delete(file.id()).await {
        Ok(true) => Ok(Some(PathBuf::from(file.path()))),
        // The row was already gone — the removal that deleted it owns the bytes —
        // or the datastore rejected the delete because a reference this guard
        // cannot see survived (the Asset-owned `audio.file_id`, or a row inserted
        // concurrently): the file is kept, bytes included.
        Ok(false) | Err(RepositoryError::DataIntegrityViolation) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Remove the bytes at `path` under `root`, best-effort.
///
/// The rows have already committed when this runs, so a storage failure does not
/// roll the deletion back: the file-storage adapter owns its failure log and the
/// deletion stands (`OBS-002`, `STY-RUST-016`). `None` — nothing was deleted —
/// is a no-op.
#[cfg_attr(
    not(test),
    expect(
        clippy::single_call_fn,
        reason = "extracted for the Delete a Gallery use case to reuse; this change's only caller calls it once"
    )
)]
pub(crate) async fn remove_bytes<S>(file_storage: &Arc<S>, root: &Path, path: Option<PathBuf>)
where
    S: FileStorage,
{
    let Some(stored_path) = path else {
        return;
    };
    if file_storage
        .delete_upload_file(&root.join(stored_path))
        .await
        .is_err()
    {
        // The file-storage adapter owns the failure log (`OBS-002`).
        // TODO(reaper): a crash between the commit and this call orphans the
        // bytes; a background reaper removes them.
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::Arc;

    use chrono::NaiveDate;

    use crate::domain::model::file::File;
    use crate::domain::model::image::Image;
    use crate::domain::model::image::Orientation;
    use crate::domain::model::integrity_hash::IntegrityHash;
    use crate::domain::model::video::ScanType;
    use crate::domain::model::video::Video;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::error::StorageError;
    use crate::domain::port::file_repository::MockFileRepository;
    use crate::domain::port::file_storage::MockFileStorage;
    use crate::domain::port::gallery_repository::MockGalleryRepository;
    use crate::domain::port::image_repository::MockImageRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::domain::port::video_repository::MockVideoRepository;
    use crate::test_helpers::TestUow;
    use crate::test_helpers::gallery_unit_of_work;

    use super::remove_bytes;
    use super::remove_row;

    /// A 64-character hexadecimal digest, valid for `integrity_hash`.
    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    /// Storage root a test composes the bytes path against.
    const ROOT: &str = "/storage";

    /// Stored path of the fixture file, relative to the storage root.
    const FILE_PATH: &str = "files/1_photo.png";

    /// Build a stored file with `file_id` and `path`.
    fn file(file_id: i64, path: &str) -> Result<File, Box<dyn Error>> {
        Ok(File::try_new(
            file_id,
            path.to_owned(),
            1,
            false,
            "image/png".to_owned(),
            NaiveDate::from_ymd_opt(2026, 10, 16).ok_or("invalid fixture date")?,
            IntegrityHash::try_new(DIGEST.to_owned())?,
        )?)
    }

    /// A unit of work whose Library and Asset repositories are mocked.
    fn unit_of_work(
        images: MockImageRepository,
        videos: MockVideoRepository,
        files: MockFileRepository,
    ) -> TestUow {
        let (unit_of_work, _committed, _rolled_back) = gallery_unit_of_work(
            MockGalleryRepository::new(),
            images,
            videos,
            MockUserRepository::new(),
            files,
        );
        unit_of_work
    }

    #[tokio::test]
    async fn remove_row_deletes_an_unreferenced_file_and_returns_its_path()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut images = MockImageRepository::new();
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
        files
            .expect_delete()
            .times(1)
            .withf(|id| *id == 7)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut uow = unit_of_work(images, videos, files);
        let stored = file(7, FILE_PATH)?;

        // Act
        let removed = remove_row(&mut uow, &stored).await?;

        // Assert
        assert_eq!(removed, Some(PathBuf::from(FILE_PATH)));
        Ok(())
    }

    #[tokio::test]
    async fn remove_row_keeps_a_file_an_image_still_references() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut images = MockImageRepository::new();
        let referencing = Image::try_new(
            1,
            "photo.png".to_owned(),
            10,
            10,
            Orientation::Square,
            NaiveDate::from_ymd_opt(2026, 10, 16).ok_or("invalid fixture date")?,
            7,
        )?;
        images.expect_search().times(1).return_once(move |_| {
            let value = referencing.clone();
            Box::pin(async move { Ok(vec![value]) })
        });
        let mut videos = MockVideoRepository::new();
        videos.expect_search().times(0);
        let mut files = MockFileRepository::new();
        files.expect_delete().times(0);
        let mut uow = unit_of_work(images, videos, files);
        let stored = file(7, FILE_PATH)?;

        // Act
        let removed = remove_row(&mut uow, &stored).await?;

        // Assert
        assert_eq!(removed, None);
        Ok(())
    }

    #[tokio::test]
    async fn remove_row_keeps_a_file_a_video_still_references() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut images = MockImageRepository::new();
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
            7,
        )?;
        videos.expect_search().times(1).return_once(move |_| {
            let value = referencing.clone();
            Box::pin(async move { Ok(vec![value]) })
        });
        let mut files = MockFileRepository::new();
        files.expect_delete().times(0);
        let mut uow = unit_of_work(images, videos, files);
        let stored = file(7, FILE_PATH)?;

        // Act
        let removed = remove_row(&mut uow, &stored).await?;

        // Assert
        assert_eq!(removed, None);
        Ok(())
    }

    #[tokio::test]
    async fn remove_row_concurrently_deleted_file_returns_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut images = MockImageRepository::new();
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
        files
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(false) }));
        let mut uow = unit_of_work(images, videos, files);
        let stored = file(7, FILE_PATH)?;

        // Act
        let removed = remove_row(&mut uow, &stored).await?;

        // Assert
        assert_eq!(removed, None);
        Ok(())
    }

    #[tokio::test]
    async fn remove_row_rejected_delete_keeps_the_file() -> Result<(), Box<dyn Error>> {
        // Arrange: the datastore rejects the delete because a reference the guard
        // cannot see survives (`audio.file_id` today has no port to query).
        let mut images = MockImageRepository::new();
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
        files
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::DataIntegrityViolation) }));
        let mut uow = unit_of_work(images, videos, files);
        let stored = file(7, FILE_PATH)?;

        // Act
        let removed = remove_row(&mut uow, &stored).await?;

        // Assert
        assert_eq!(removed, None);
        Ok(())
    }

    #[tokio::test]
    async fn remove_row_repository_failure_returns_the_error() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut images = MockImageRepository::new();
        images
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let mut videos = MockVideoRepository::new();
        videos.expect_search().times(0);
        let mut files = MockFileRepository::new();
        files.expect_delete().times(0);
        let mut uow = unit_of_work(images, videos, files);
        let stored = file(7, FILE_PATH)?;

        // Act
        let result = remove_row(&mut uow, &stored).await;

        // Assert
        assert!(matches!(result, Err(RepositoryError::OperationFailed)));
        Ok(())
    }

    #[tokio::test]
    async fn remove_row_delete_failure_returns_the_error() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut images = MockImageRepository::new();
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
        files
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let mut uow = unit_of_work(images, videos, files);
        let stored = file(7, FILE_PATH)?;

        // Act
        let result = remove_row(&mut uow, &stored).await;

        // Assert
        assert!(matches!(result, Err(RepositoryError::OperationFailed)));
        Ok(())
    }

    #[tokio::test]
    async fn remove_bytes_deletes_the_file_under_the_root() {
        // Arrange
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_delete_upload_file()
            .times(1)
            .withf(|path| path == Path::new("/storage/files/1_photo.png"))
            .returning(|_| Box::pin(async { Ok(()) }));

        // Act & Assert
        remove_bytes(
            &Arc::new(file_storage),
            Path::new(ROOT),
            Some(PathBuf::from(FILE_PATH)),
        )
        .await;
    }

    #[tokio::test]
    async fn remove_bytes_absent_path_is_a_no_op() {
        // Arrange
        let mut file_storage = MockFileStorage::new();
        file_storage.expect_delete_upload_file().times(0);

        // Act & Assert
        remove_bytes(&Arc::new(file_storage), Path::new(ROOT), None).await;
    }

    #[tokio::test]
    async fn remove_bytes_storage_failure_is_swallowed() {
        // Arrange
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_delete_upload_file()
            .times(1)
            .returning(|_| Box::pin(async { Err(StorageError::OperationFailed) }));

        // Act & Assert
        remove_bytes(
            &Arc::new(file_storage),
            Path::new(ROOT),
            Some(PathBuf::from(FILE_PATH)),
        )
        .await;
    }
}
