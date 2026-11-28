use sqlx::{QueryBuilder, Sqlite, Transaction};

use crate::domain::NumericID;
use crate::domain::model::{
    AssetOwner, File, IntegrityHash, Origin, ProtectedResource, ResourceKind, ResourceRef,
};
use crate::domain::port::{FileFilter, FileRepository, RepositoryError};

use super::model::{File as SqlxFile, FileWithOwner as SqlxFileWithOwner};

/// Repository persisting files, backed by a `SQLite` transaction.
pub(crate) struct SqlxFileRepository<'transaction> {
    /// Transaction the repository reads from and writes to.
    transaction: &'transaction mut Transaction<'static, Sqlite>,
}

impl<'transaction> SqlxFileRepository<'transaction> {
    /// Create a new repository bound to `transaction`.
    #[must_use]
    pub(crate) fn new(transaction: &'transaction mut Transaction<'static, Sqlite>) -> Self {
        Self { transaction }
    }
}

impl FileRepository for SqlxFileRepository<'_> {
    async fn clear_integrity_hash(&mut self, id: NumericID) -> Result<bool, RepositoryError> {
        let result = sqlx::query("UPDATE file SET integrity_hash = NULL WHERE file_id = ?1")
            .bind(id)
            .execute(&mut **self.transaction)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn create(&mut self, file: File) -> Result<File, RepositoryError> {
        let row = sqlx::query_as::<_, SqlxFile>(
            "INSERT INTO file (
                path,
                user_id,
                is_public,
                mime_type_id,
                uploaded_at,
                size_bytes,
                modified_at,
                origin,
                integrity_hash
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            RETURNING
                file_id,
                path,
                user_id,
                is_public,
                mime_type_id,
                uploaded_at,
                size_bytes,
                modified_at,
                origin,
                integrity_hash",
        )
        .bind(file.path())
        .bind(file.user_id())
        .bind(file.is_public())
        .bind(file.mime_type_id())
        .bind(file.uploaded_at())
        .bind(file.size_bytes())
        .bind(file.modified_at())
        .bind(file.origin().as_str())
        .bind(file.integrity_hash().map(IntegrityHash::as_str))
        .fetch_one(&mut **self.transaction)
        .await?;

        domain_file(row)
    }

    async fn delete(&mut self, filter: &FileFilter) -> Result<bool, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new("DELETE FROM file");
        let mut first = true;

        if let Some(id) = filter.id {
            push_filter(&mut first, &mut builder, "file_id = ", id);
        }
        if let Some(integrity_hash) = filter.integrity_hash.as_deref() {
            push_filter(
                &mut first,
                &mut builder,
                "integrity_hash = ",
                integrity_hash,
            );
        }
        if let Some(path) = filter.path.as_deref() {
            push_filter(&mut first, &mut builder, "path = ", path);
        }
        if let Some(user_id) = filter.user_id {
            push_filter(&mut first, &mut builder, "user_id = ", user_id);
        }

        let result = builder.build().execute(&mut **self.transaction).await?;
        Ok(result.rows_affected() > 0)
    }

    async fn reassign_owner(
        &mut self,
        id: NumericID,
        new_owner: NumericID,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query("UPDATE file SET user_id = ?1 WHERE file_id = ?2")
            .bind(new_owner)
            .bind(id)
            .execute(&mut **self.transaction)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn save(&mut self, file: File) -> Result<File, RepositoryError> {
        let row = sqlx::query_as::<_, SqlxFile>(
            "INSERT INTO file (
                file_id,
                path,
                user_id,
                is_public,
                mime_type_id,
                uploaded_at,
                size_bytes,
                modified_at,
                origin,
                integrity_hash
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT (file_id) DO UPDATE SET
                path = excluded.path,
                user_id = excluded.user_id,
                is_public = excluded.is_public,
                mime_type_id = excluded.mime_type_id,
                uploaded_at = excluded.uploaded_at,
                size_bytes = excluded.size_bytes,
                modified_at = excluded.modified_at,
                origin = excluded.origin,
                integrity_hash = excluded.integrity_hash
            RETURNING
                file_id,
                path,
                user_id,
                is_public,
                mime_type_id,
                uploaded_at,
                size_bytes,
                modified_at,
                origin,
                integrity_hash",
        )
        .bind(file.id())
        .bind(file.path())
        .bind(file.user_id())
        .bind(file.is_public())
        .bind(file.mime_type_id())
        .bind(file.uploaded_at())
        .bind(file.size_bytes())
        .bind(file.modified_at())
        .bind(file.origin().as_str())
        .bind(file.integrity_hash().map(IntegrityHash::as_str))
        .fetch_one(&mut **self.transaction)
        .await?;

        domain_file(row)
    }

    async fn search(&mut self, filter: &FileFilter) -> Result<Vec<File>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT
                f.file_id,
                f.path,
                f.user_id,
                f.is_public,
                f.mime_type_id,
                f.uploaded_at,
                f.size_bytes,
                f.modified_at,
                f.origin,
                f.integrity_hash,
                CASE
                    WHEN image.scope = 'LIBRARY' THEN 'LIBRARY'
                    WHEN image.scope = 'USER'
                        AND gallery_item.gallery_id IS NOT NULL THEN 'GALLERY'
                    WHEN image.scope = 'USER'
                        AND music_playlist.music_playlist_id IS NOT NULL
                        THEN 'MUSIC_PLAYLIST'
                    WHEN video.scope = 'LIBRARY' THEN 'LIBRARY'
                    WHEN video.scope = 'USER'
                        AND gallery_item_video.gallery_id IS NOT NULL
                        THEN 'GALLERY'
                    WHEN video.scope IS NULL
                        AND movie.movie_id IS NOT NULL THEN 'MOVIE'
                    WHEN video.scope IS NULL
                        AND series.series_id IS NOT NULL THEN 'SERIES'
                    WHEN music_album.music_album_id IS NOT NULL THEN 'MUSIC_ALBUM'
                END AS owner_kind,
                COALESCE(
                    gallery_item.gallery_id,
                    music_playlist.music_playlist_id,
                    gallery_item_video.gallery_id,
                    movie.movie_id,
                    series.series_id,
                    music_album.music_album_id
                ) AS owner_id,
                COALESCE(
                    gallery.user_id,
                    music_playlist.user_id,
                    gallery_video.user_id
                ) AS owner_user_id,
                COALESCE(
                    gallery.is_public,
                    music_playlist.is_public,
                    gallery_video.is_public,
                    1
                ) AS owner_is_public
            FROM file AS f
            LEFT JOIN image ON image.file_id = f.file_id
            LEFT JOIN gallery_item ON gallery_item.image_id = image.image_id
            LEFT JOIN gallery ON gallery.gallery_id = gallery_item.gallery_id
            LEFT JOIN music_playlist
                ON music_playlist.cover_image_id = image.image_id
            LEFT JOIN video ON video.file_id = f.file_id
            LEFT JOIN gallery_item AS gallery_item_video
                ON gallery_item_video.video_id = video.video_id
            LEFT JOIN gallery AS gallery_video
                ON gallery_video.gallery_id = gallery_item_video.gallery_id
            LEFT JOIN movie ON movie.video_id = video.video_id
            LEFT JOIN episode ON episode.video_id = video.video_id
            LEFT JOIN season ON season.season_id = episode.season_id
            LEFT JOIN series ON series.series_id = season.series_id
            LEFT JOIN audio ON audio.file_id = f.file_id
            LEFT JOIN music_album ON music_album.music_album_id = (
                SELECT music_medium.music_album_id
                FROM music_recording
                JOIN music_track
                    ON music_track.music_recording_id
                        = music_recording.music_recording_id
                JOIN music_medium
                    ON music_medium.music_medium_id = music_track.music_medium_id
                WHERE music_recording.audio_id = audio.audio_id
                LIMIT 1
            )",
        );
        let mut first = true;

        if let Some(id) = filter.id {
            push_filter(&mut first, &mut builder, "f.file_id = ", id);
        }
        if let Some(integrity_hash) = filter.integrity_hash.as_deref() {
            push_filter(
                &mut first,
                &mut builder,
                "f.integrity_hash = ",
                integrity_hash,
            );
        }
        if let Some(path) = filter.path.as_deref() {
            push_filter(&mut first, &mut builder, "f.path = ", path);
        }
        if let Some(user_id) = filter.user_id {
            push_filter(&mut first, &mut builder, "f.user_id = ", user_id);
        }
        // A collection pages this read, so the order must be total: the
        // identifier is the one key every file row carries.
        builder.push(" ORDER BY f.file_id");

        let rows = builder
            .build_query_as::<SqlxFileWithOwner>()
            .fetch_all(&mut **self.transaction)
            .await?;

        rows.into_iter().map(domain_file_with_owner).collect()
    }
}

/// Append a `WHERE`/`AND`-separated query filter condition to `builder`.
fn push_filter<'value, T>(
    first: &mut bool,
    builder: &mut QueryBuilder<Sqlite>,
    clause: &'static str,
    value: T,
) where
    T: sqlx::Encode<'value, Sqlite> + sqlx::Type<Sqlite>,
{
    if *first {
        builder.push(" WHERE ");
        *first = false;
    } else {
        builder.push(" AND ");
    }
    builder.push(clause);
    builder.push_bind(value);
}

/// Map a persisted file row back to the domain model.
fn domain_file(row: SqlxFile) -> Result<File, RepositoryError> {
    let integrity_hash = row
        .integrity_hash
        .map(|digest| {
            IntegrityHash::try_new(digest).map_err(|_| RepositoryError::DataIntegrityViolation)
        })
        .transpose()?;
    let origin =
        Origin::parse(row.origin.as_str()).ok_or(RepositoryError::DataIntegrityViolation)?;
    File::try_new(
        row.file_id,
        row.path,
        row.user_id,
        row.is_public,
        row.mime_type_id,
        row.uploaded_at,
        row.size_bytes,
        row.modified_at,
        integrity_hash,
        Some(origin),
    )
    .map_err(|_| RepositoryError::DataIntegrityViolation)
}

/// Map a persisted file row and its owner back to the domain model.
fn domain_file_with_owner(row: SqlxFileWithOwner) -> Result<File, RepositoryError> {
    let owner = match row.owner_kind.as_deref() {
        // An unreferenced file is an orphan asset.
        None => None,
        // Shared catalogue art belongs to the library, not to one aggregate; the
        // owner identifier the join carries is meaningless for it.
        Some("LIBRARY") => Some(AssetOwner::Library),
        Some(kind) => {
            let resource_kind =
                ResourceKind::from_wire(kind).ok_or(RepositoryError::DataIntegrityViolation)?;
            let id = row
                .owner_id
                .ok_or(RepositoryError::DataIntegrityViolation)?;
            Some(AssetOwner::Resource(ProtectedResource::new(
                ResourceRef::new(resource_kind, id),
                row.owner_user_id,
                row.owner_is_public.unwrap_or(true),
            )))
        }
    };
    Ok(domain_file(row.file)?.with_owner(owner))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::port::{UnitOfWork as _, UnitOfWorkFactory as _};
    use crate::infrastructure::outbound::sqlx::SqlxUnitOfWorkFactory;
    use crate::infrastructure::outbound::sqlx::test_helpers::begin_transaction;
    use crate::infrastructure::outbound::sqlx::test_helpers::*;
    use crate::test_helpers::*;
    use chrono::{NaiveDate, NaiveDateTime};
    use rstest::{fixture, rstest};
    use std::error::Error;
    use std::sync::Arc;
    use tokio::sync::Barrier;
    use tokio::task::JoinSet;

    /// `SQLite` extended result code for a writer that found the lock held
    /// (`SQLITE_BUSY`).
    const SQLITE_BUSY: &str = "5";

    /// How many units of work contend for the single `SQLite` writer.
    const WRITERS: usize = 8;

    /// Build a pending file with the given digest, path and owner.
    fn file(
        id: i64,
        path: &str,
        user_id: i64,
        integrity_hash: &str,
    ) -> Result<File, Box<dyn Error>> {
        Ok(File::try_new(
            id,
            path.to_owned(),
            user_id,
            false,
            "video/mp4".to_owned(),
            NaiveDate::from_ymd_opt(2026, 9, 26).ok_or("invalid fixture date")?,
            1024,
            modified_at()?,
            Some(IntegrityHash::try_new(integrity_hash.to_owned())?),
            Some(Origin::Uploaded),
        )?)
    }

    /// Build a scanned file with no digest.
    fn scanned(id: i64, path: &str, user_id: i64) -> Result<File, Box<dyn Error>> {
        Ok(File::try_new(
            id,
            path.to_owned(),
            user_id,
            false,
            "video/mp4".to_owned(),
            NaiveDate::from_ymd_opt(2026, 9, 26).ok_or("invalid fixture date")?,
            2048,
            modified_at()?,
            None,
            None,
        )?)
    }

    /// Fixture modification instant.
    #[fixture]
    fn modified_at() -> Result<NaiveDateTime, Box<dyn Error>> {
        Ok(NaiveDateTime::parse_from_str(
            "2026-09-26 10:00:00",
            "%Y-%m-%d %H:%M:%S",
        )?)
    }

    #[tokio::test]
    async fn create_assigns_final_identifier_and_round_trips() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let pending = file(0, "files/1_clip.mp4", 1, DIGEST)?;

        // Act
        let mut repository = SqlxFileRepository::new(&mut transaction);
        let persisted = repository.create(pending).await?;
        let found = repository
            .search(&FileFilter {
                id: Some(persisted.id()),
                ..FileFilter::default()
            })
            .await?;

        // Assert
        assert_ne!(
            persisted.id(),
            0,
            "a new file must receive a real identifier"
        );
        assert_eq!(persisted.path(), "files/1_clip.mp4");
        assert_eq!(persisted.user_id(), 1);
        assert!(!persisted.is_public());
        assert_eq!(persisted.mime_type_id(), "video/mp4");
        assert_eq!(
            persisted.integrity_hash().map(IntegrityHash::as_str),
            Some(DIGEST)
        );
        assert_eq!(found.first(), Some(&persisted));
        Ok(())
    }

    #[tokio::test]
    async fn search_filters_by_digest_and_user() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_user(&mut transaction, 2).await?;
        let mut repository = SqlxFileRepository::new(&mut transaction);
        repository
            .create(file(0, "files/1_clip.mp4", 1, DIGEST)?)
            .await?;
        repository
            .create(file(
                0,
                "files/2_other.mp4",
                2,
                "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            )?)
            .await?;

        // Act
        let by_digest = repository
            .search(&FileFilter {
                integrity_hash: Some(DIGEST.to_owned()),
                ..FileFilter::default()
            })
            .await?;
        let by_user = repository
            .search(&FileFilter {
                user_id: Some(2),
                ..FileFilter::default()
            })
            .await?;
        let all = repository.search(&FileFilter::default()).await?;

        // Assert
        assert_eq!(by_digest.len(), 1);
        assert_eq!(by_digest.first().map(File::user_id), Some(1));
        assert_eq!(by_user.len(), 1);
        assert_eq!(by_user.first().map(File::path), Some("files/2_other.mp4"));
        assert_eq!(all.len(), 2);
        Ok(())
    }

    #[tokio::test]
    async fn delete_reports_matched_and_missing_rows() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxFileRepository::new(&mut transaction);
        let stored = repository
            .create(file(0, "files/1_clip.mp4", 1, DIGEST)?)
            .await?;

        // Act
        let deleted = repository
            .delete(&FileFilter {
                id: Some(stored.id()),
                ..FileFilter::default()
            })
            .await?;
        let missing = repository
            .delete(&FileFilter {
                id: Some(stored.id()),
                ..FileFilter::default()
            })
            .await?;

        // Assert
        assert!(deleted);
        assert!(!missing);
        assert!(repository.search(&FileFilter::default()).await?.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn delete_referenced_file_returns_data_integrity_violation() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        sqlx::query(
            "INSERT INTO file (
                file_id, path, user_id, is_public, mime_type_id, size_bytes, modified_at,
                integrity_hash
             )
             VALUES (10, 'files/1_photo.png', 1, 0, 'image/png', 10, '2026-10-06 00:00:00', ?1)",
        )
        .bind(DIGEST)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO image (image_id, name, width_px, height_px, orientation, created_at, file_id)
             VALUES (1, 'photo.png', 10, 10, 'SQUARE', '2026-10-06', 10)",
        )
        .execute(&mut *transaction)
        .await?;

        // Act
        let mut repository = SqlxFileRepository::new(&mut transaction);
        let result = repository
            .delete(&FileFilter {
                id: Some(10),
                ..FileFilter::default()
            })
            .await;

        // Assert: `fk_image_file` crosses a bounded-context boundary and does
        // not cascade (`ARCH-004`, `PERS-020`), so the delete is rejected.
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn search_filters_by_id_returns_only_the_matching_file() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxFileRepository::new(&mut transaction);
        let first = repository
            .create(file(0, "files/1_clip.mp4", 1, DIGEST)?)
            .await?;
        repository
            .create(file(
                0,
                "files/1_other.mp4",
                1,
                "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            )?)
            .await?;

        // Act
        let found = repository
            .search(&FileFilter {
                id: Some(first.id()),
                ..FileFilter::default()
            })
            .await?;

        // Assert: a media removal addresses the row it read the stored path from,
        // so the identifier filter must select exactly that row.
        assert_eq!(found, vec![first]);
        Ok(())
    }

    #[tokio::test]
    async fn search_returns_files_in_identifier_order() -> Result<(), Box<dyn Error>> {
        // Arrange: the paths sort against the identifier order, so the assertion
        // proves the order rather than the insertion sequence.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxFileRepository::new(&mut transaction);
        let first = repository
            .create(file(0, "files/z_last.png", 1, DIGEST)?)
            .await?;
        let second = repository
            .create(file(
                0,
                "files/a_first.png",
                1,
                "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            )?)
            .await?;

        // Act
        let found = repository.search(&FileFilter::default()).await?;

        // Assert: the stored-file collection pages this read, so its order is
        // total and independent of the path.
        assert_eq!(
            found.iter().map(File::id).collect::<Vec<NumericID>>(),
            vec![first.id(), second.id()]
        );
        Ok(())
    }

    #[tokio::test]
    async fn create_duplicate_digest_for_same_user_returns_already_exist()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxFileRepository::new(&mut transaction);
        repository
            .create(file(0, "files/1_clip.mp4", 1, DIGEST)?)
            .await?;

        // Act
        let result = repository
            .create(file(0, "files/1_other.mp4", 1, DIGEST)?)
            .await;

        // Assert
        assert!(matches!(result, Err(RepositoryError::AlreadyExist)));
        Ok(())
    }

    #[tokio::test]
    async fn create_duplicate_digest_for_different_user_is_allowed() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_user(&mut transaction, 2).await?;
        let mut repository = SqlxFileRepository::new(&mut transaction);
        repository
            .create(file(0, "files/1_clip.mp4", 1, DIGEST)?)
            .await?;

        // Act
        let stored = repository
            .create(file(0, "files/2_clip.mp4", 2, DIGEST)?)
            .await?;

        // Assert
        assert_eq!(stored.user_id(), 2);
        assert_eq!(
            stored.integrity_hash().map(IntegrityHash::as_str),
            Some(DIGEST)
        );
        Ok(())
    }

    #[tokio::test]
    async fn create_duplicate_path_returns_already_exist() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_user(&mut transaction, 2).await?;
        let mut repository = SqlxFileRepository::new(&mut transaction);
        repository
            .create(file(0, "files/clip.mp4", 1, DIGEST)?)
            .await?;

        // Act
        let result = repository
            .create(file(
                0,
                "files/clip.mp4",
                2,
                "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            )?)
            .await;

        // Assert
        assert!(matches!(result, Err(RepositoryError::AlreadyExist)));
        Ok(())
    }

    #[tokio::test]
    async fn create_missing_user_returns_data_integrity_violation() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;

        // Act
        let mut repository = SqlxFileRepository::new(&mut transaction);
        let result = repository
            .create(file(0, "files/1_clip.mp4", 999, DIGEST)?)
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn create_missing_mime_type_returns_data_integrity_violation()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let pending = File::try_new(
            0,
            "files/1_clip.mp4".to_owned(),
            1,
            false,
            "application/unknown".to_owned(),
            NaiveDate::from_ymd_opt(2026, 9, 26).ok_or("invalid fixture date")?,
            1024,
            modified_at()?,
            Some(IntegrityHash::try_new(DIGEST.to_owned())?),
            Some(Origin::Uploaded),
        )?;

        // Act
        let mut repository = SqlxFileRepository::new(&mut transaction);
        let result = repository.create(pending).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn create_and_read_back_a_scanned_file_without_digest() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let pending = scanned(0, "found/clip.mp4", 1)?;

        // Act
        let mut repository = SqlxFileRepository::new(&mut transaction);
        let persisted = repository.create(pending).await?;
        let found = repository
            .search(&FileFilter {
                id: Some(persisted.id()),
                ..FileFilter::default()
            })
            .await?;

        // Assert: the nullable digest and the size and instant round-trip.
        assert!(persisted.integrity_hash().is_none());
        assert_eq!(persisted.origin(), Origin::Scanned);
        assert_eq!(persisted.size_bytes(), 2048);
        assert_eq!(found.first(), Some(&persisted));
        Ok(())
    }

    #[tokio::test]
    async fn save_moves_an_existing_record_to_a_new_path() -> Result<(), Box<dyn Error>> {
        // Arrange: a scanned file already recorded under its old location.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file_v3(&mut transaction, 100, "found/clip.mp4", 1).await?;
        let mut repository = SqlxFileRepository::new(&mut transaction);
        let mut record = repository
            .search(&FileFilter {
                id: Some(100),
                ..FileFilter::default()
            })
            .await?
            .into_iter()
            .next()
            .ok_or("the seeded file was not found")?;
        record.set_path("found/renamed.mp4".to_owned())?;

        // Act
        let saved = repository.save(record).await?;
        let found = repository
            .search(&FileFilter {
                id: Some(100),
                ..FileFilter::default()
            })
            .await?;

        // Assert: the location moved and the row kept its other columns.
        assert_eq!(saved.path(), "found/renamed.mp4");
        assert_eq!(found.first().map(File::path), Some("found/renamed.mp4"));
        assert_eq!(found.first().map(File::size_bytes), Some(10));
        Ok(())
    }

    #[tokio::test]
    async fn save_a_missing_record_inserts_it() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let pending = scanned(999, "found/missing.mp4", 1)?;

        // Act: `save` is an upsert by identity.
        let mut repository = SqlxFileRepository::new(&mut transaction);
        let saved = repository.save(pending).await?;
        let found = repository
            .search(&FileFilter {
                id: Some(999),
                ..FileFilter::default()
            })
            .await?;

        // Assert
        assert_eq!(saved.id(), 999);
        assert_eq!(found.first().map(File::path), Some("found/missing.mp4"));
        Ok(())
    }

    #[rstest]
    #[case::integrity_hash_length(
        "INSERT INTO file (path, user_id, is_public, mime_type_id, size_bytes, modified_at, integrity_hash)
         VALUES ('files/short.mp4', 1, 0, 'video/mp4', 1, '2026-10-06 00:00:00', 'too-short')"
    )]
    #[case::is_public_out_of_range(
        "INSERT INTO file (path, user_id, is_public, mime_type_id, size_bytes, modified_at, integrity_hash)
         VALUES ('files/public.mp4', 1, 2, 'video/mp4', 1, '2026-10-06 00:00:00', '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef')"
    )]
    #[case::origin_not_known(
        "INSERT INTO file (path, user_id, is_public, mime_type_id, size_bytes, modified_at, origin, integrity_hash)
         VALUES ('files/origin.mp4', 1, 0, 'video/mp4', 1, '2026-10-06 00:00:00', 'BORROWED', NULL)"
    )]
    #[tokio::test]
    async fn create_violating_a_check_returns_data_integrity_violation(
        #[case] statement: &'static str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;

        // Act & Assert
        let result = sqlx::query(statement).execute(&mut *transaction).await;
        let mapped = result.map_err(RepositoryError::from);
        assert!(
            matches!(mapped, Err(RepositoryError::DataIntegrityViolation)),
            "statement must violate a check constraint: {statement}"
        );
        Ok(())
    }
    #[tokio::test]
    async fn reassign_owner_moves_the_file_to_the_new_owner() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 0).await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxFileRepository::new(&mut transaction);
        let stored = repository
            .create(file(0, "files/1_clip.mp4", 1, DIGEST)?)
            .await?;

        // Act
        let reassigned = repository.reassign_owner(stored.id(), 0).await?;
        let found = repository
            .search(&FileFilter {
                id: Some(stored.id()),
                ..FileFilter::default()
            })
            .await?;

        // Assert
        assert!(reassigned);
        assert_eq!(found.first().map(File::user_id), Some(0));
        Ok(())
    }

    #[tokio::test]
    async fn reassign_owner_of_a_missing_file_returns_false() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 0).await?;

        // Act
        let mut repository = SqlxFileRepository::new(&mut transaction);
        let reassigned = repository.reassign_owner(404, 0).await?;

        // Assert
        assert!(!reassigned);
        Ok(())
    }

    #[tokio::test]
    async fn clear_integrity_hash_removes_the_digest() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxFileRepository::new(&mut transaction);
        let stored = repository
            .create(file(0, "files/1_clip.mp4", 1, DIGEST)?)
            .await?;

        // Act
        let cleared = repository.clear_integrity_hash(stored.id()).await?;
        let found = repository
            .search(&FileFilter {
                id: Some(stored.id()),
                ..FileFilter::default()
            })
            .await?;

        // Assert
        assert!(cleared);
        assert_eq!(
            found.first().and_then(File::integrity_hash),
            None,
            "the cleared row must carry no digest"
        );
        Ok(())
    }

    #[tokio::test]
    async fn clear_integrity_hash_of_a_missing_file_returns_false() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;

        // Act
        let mut repository = SqlxFileRepository::new(&mut transaction);
        let cleared = repository.clear_integrity_hash(404).await?;

        // Assert
        assert!(!cleared);
        Ok(())
    }
    #[tokio::test]
    async fn deleting_file_referenced_by_media_is_rejected() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;

        // Act
        let result = sqlx::query("DELETE FROM file WHERE file_id = 1")
            .execute(&pool)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn begin_write_holds_the_write_lock_before_the_first_statement()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let (_tmp, pool) = disk_pool(2).await?;
        let unit_of_work = SqlxUnitOfWorkFactory::new(pool.clone())
            .begin_write()
            .await?;

        // Act
        let blocked = try_write_lock(&pool).await;

        // Assert
        let code = blocked.err().and_then(sqlite_code);
        assert_eq!(
            code.as_deref(),
            Some(SQLITE_BUSY),
            "a write-intent unit of work must hold the write lock from BEGIN"
        );
        unit_of_work.rollback().await?;
        pool.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn begin_leaves_the_write_lock_to_the_write_intent_opener() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let (_tmp, pool) = disk_pool(2).await?;
        let unit_of_work = SqlxUnitOfWorkFactory::new(pool.clone()).begin().await?;

        // Act
        let locked = try_write_lock(&pool).await;

        // Assert
        assert!(
            locked.is_ok(),
            "a read-only unit of work must not hold the write lock"
        );
        unit_of_work.rollback().await?;
        pool.close().await;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn concurrent_read_then_write_units_of_work_all_commit() -> Result<(), Box<dyn Error>> {
        // Arrange
        let (_tmp, pool) = disk_pool(u32::try_from(WRITERS)?).await?;
        seed_media(&pool).await?;
        let factory = Arc::new(SqlxUnitOfWorkFactory::new(pool.clone()));
        let barrier = Arc::new(Barrier::new(WRITERS));

        // Act
        let mut tasks = (0..WRITERS)
            .map(|index| {
                let task_factory = Arc::clone(&factory);
                let task_barrier = Arc::clone(&barrier);
                let path = format!("concurrent-{index}.png");
                async move { read_then_write(&task_factory, path, task_barrier).await }
            })
            .collect::<JoinSet<Result<(), String>>>();
        let failures = joined_failures(&mut tasks).await;
        let persisted: i64 =
            sqlx::query_scalar("SELECT count(*) FROM file WHERE path LIKE 'concurrent-%'")
                .fetch_one(&pool)
                .await?;

        // Assert
        assert!(
            failures.is_empty(),
            "concurrent units of work reported {} failure(s): {}",
            failures.len(),
            failures.join("; ")
        );
        assert_eq!(
            persisted,
            i64::try_from(WRITERS)?,
            "every concurrent unit of work must commit its own file"
        );
        pool.close().await;
        Ok(())
    }
}
