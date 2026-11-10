use sqlx::{QueryBuilder, Sqlite, Transaction};

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

    async fn save(&mut self, file: File) -> Result<File, RepositoryError> {
        let row = sqlx::query_as::<_, SqlxFile>(
            "INSERT INTO file (file_id, path, is_public, integrity_hash)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (file_id) DO UPDATE SET
                path = excluded.path,
                is_public = excluded.is_public,
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
        .bind(file.is_public())
        .bind(file.integrity_hash().map(IntegrityHash::as_str))
        .fetch_optional(&mut **self.transaction)
        .await?
        .ok_or(RepositoryError::OperationFailed)?;

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
    use crate::infrastructure::outbound::sqlx::test_helpers::begin_transaction;
    use crate::test_helpers::*;
    use chrono::{NaiveDate, NaiveDateTime};
    use rstest::rstest;
    use std::error::Error;

    /// Seed a credential and a user so foreign keys resolve.
    async fn seed_user(
        transaction: &mut Transaction<'static, Sqlite>,
        user_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO credential (credential_id, password_hash) VALUES (?1, ?2)")
            .bind(user_id)
            .bind(format!("hash-{user_id}"))
            .execute(&mut **transaction)
            .await?;
        sqlx::query(
            "INSERT INTO user (user_id, role, username, email, credential_id)
             VALUES (?1, 'STANDARD', ?2, NULL, ?1)",
        )
        .bind(user_id)
        .bind(format!("user{user_id}"))
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

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

    /// Seed a scanned file with a known identifier.
    async fn seed_file(
        transaction: &mut Transaction<'static, Sqlite>,
        file_id: i64,
        path: &str,
        user_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO file (file_id, path, user_id, is_public, mime_type_id, size_bytes, modified_at)
             VALUES (?1, ?2, ?3, 0, 'video/mp4', 10, '2026-09-26 10:00:00')",
        )
        .bind(file_id)
        .bind(path)
        .bind(user_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// The owner of the file matching `file_id`.
    async fn owner_of(
        transaction: &mut Transaction<'static, Sqlite>,
        file_id: i64,
    ) -> Result<Option<AssetOwner>, Box<dyn Error>> {
        let mut repository = SqlxFileRepository::new(transaction);
        let found = repository
            .search(&FileFilter {
                id: Some(file_id),
                ..FileFilter::default()
            })
            .await?;
        Ok(found.first().and_then(File::owner))
    }

    fn protected(
        kind: ResourceKind,
        id: i64,
        owner_id: Option<i64>,
        is_public: bool,
    ) -> AssetOwner {
        AssetOwner::Resource(ProtectedResource::new(
            ResourceRef::new(kind, id),
            owner_id,
            is_public,
        ))
    }

    /// Seed an image row naming `file_id`.
    async fn seed_image(
        transaction: &mut Transaction<'static, Sqlite>,
        image_id: i64,
        file_id: i64,
        scope: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO image (image_id, name, width_px, height_px, orientation, created_at, scope, file_id)
             VALUES (?1, 'art', 10, 10, 'SQUARE', '2026-09-26', ?2, ?3)",
        )
        .bind(image_id)
        .bind(scope)
        .bind(file_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Seed a video row naming `file_id`.
    async fn seed_video(
        transaction: &mut Transaction<'static, Sqlite>,
        video_id: i64,
        file_id: i64,
        scope: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO video (video_id, duration_ms, codec, frame_count, width, height, color_id, scan_type, scope, file_id)
             VALUES (?1, 1000, 'h264', 30, 640, 480, 'sRGB', 'PROGRESSIVE', ?2, ?3)",
        )
        .bind(video_id)
        .bind(scope)
        .bind(file_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn search_leaves_an_unreferenced_file_without_an_owner() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 100, "files/1_orphan.mp4", 1).await?;

        // Act
        let owner = owner_of(&mut transaction, 100).await?;

        // Assert
        assert_eq!(owner, None);
        Ok(())
    }

    #[tokio::test]
    async fn search_resolves_library_art() -> Result<(), Box<dyn Error>> {
        // Arrange: a LIBRARY image belongs to no single aggregate.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 100, "art/1.png", 1).await?;
        seed_image(&mut transaction, 1, 100, Some("LIBRARY")).await?;

        // Act
        let owner = owner_of(&mut transaction, 100).await?;

        // Assert
        assert_eq!(owner, Some(AssetOwner::Library));
        Ok(())
    }

    #[tokio::test]
    async fn search_resolves_the_gallery_that_holds_an_image() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 100, "files/1_item.png", 1).await?;
        sqlx::query(
            "INSERT INTO gallery (gallery_id, name, created_at, last_modified_at, is_public, user_id)
             VALUES (2, 'Private', '2026-09-26', '2026-09-26', 0, 1)",
        )
        .execute(&mut *transaction)
        .await?;
        seed_image(&mut transaction, 1, 100, Some("USER")).await?;
        sqlx::query(
            "INSERT INTO gallery_item (gallery_item_id, gallery_id, image_id, item_index)
             VALUES (1, 2, 1, 0)",
        )
        .execute(&mut *transaction)
        .await?;

        // Act
        let owner = owner_of(&mut transaction, 100).await?;

        // Assert
        assert_eq!(
            owner,
            Some(protected(ResourceKind::Gallery, 2, Some(1), false))
        );
        Ok(())
    }

    #[tokio::test]
    async fn search_resolves_the_playlist_that_uses_an_image_as_cover() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 100, "files/1_cover.png", 1).await?;
        sqlx::query(
            "INSERT INTO music_playlist (music_playlist_id, name, user_id, is_public, cover_image_id)
             VALUES (4, 'Mix', 1, 1, NULL)",
        )
        .execute(&mut *transaction)
        .await?;
        seed_image(&mut transaction, 1, 100, Some("USER")).await?;
        sqlx::query("UPDATE music_playlist SET cover_image_id = 1 WHERE music_playlist_id = 4")
            .execute(&mut *transaction)
            .await?;

        // Act
        let owner = owner_of(&mut transaction, 100).await?;

        // Assert
        assert_eq!(
            owner,
            Some(protected(ResourceKind::MusicPlaylist, 4, Some(1), true))
        );
        Ok(())
    }

    #[tokio::test]
    async fn search_resolves_the_film_that_names_a_video() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 100, "files/1_film.mp4", 1).await?;
        seed_video(&mut transaction, 3, 100, None).await?;
        sqlx::query(
            "INSERT INTO movie (movie_id, name, plot, video_id) VALUES (5, 'Film', 'Plot', 3)",
        )
        .execute(&mut *transaction)
        .await?;

        // Act
        let owner = owner_of(&mut transaction, 100).await?;

        // Assert
        assert_eq!(owner, Some(protected(ResourceKind::Movie, 5, None, true)));
        Ok(())
    }

    #[tokio::test]
    async fn search_resolves_the_series_that_holds_an_episode() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 100, "files/1_episode.mp4", 1).await?;
        sqlx::query("INSERT INTO series (series_id, name, plot) VALUES (6, 'Show', 'Plot')")
            .execute(&mut *transaction)
            .await?;
        sqlx::query("INSERT INTO season (season_id, series_id) VALUES (7, 6)")
            .execute(&mut *transaction)
            .await?;
        seed_video(&mut transaction, 3, 100, None).await?;
        sqlx::query("INSERT INTO episode (episode_id, season_id, episode_index, name, plot, video_id) VALUES (8, 7, 0, 'Pilot', 'Plot', 3)")
            .execute(&mut *transaction)
            .await?;

        // Act
        let owner = owner_of(&mut transaction, 100).await?;

        // Assert
        assert_eq!(owner, Some(protected(ResourceKind::Series, 6, None, true)));
        Ok(())
    }

    #[tokio::test]
    async fn search_resolves_the_album_that_holds_an_audio() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 100, "files/1_track.flac", 1).await?;
        sqlx::query("INSERT INTO music_group (music_group_id, stage_name) VALUES (9, 'Artist')")
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            "INSERT INTO music_album (music_album_id, name, music_group_id) VALUES (5, 'Album', 9)",
        )
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO music_medium (music_medium_id, music_album_id, type) VALUES (11, 5, 'DIGITAL')",
        )
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO audio (audio_id, name, codec, file_id) VALUES (7, 'Track', 'flac', 100)",
        )
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO music_recording (music_recording_id, name, audio_id) VALUES (12, 'Track', 7)",
        )
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO music_track (music_track_id, track_index, music_medium_id, music_recording_id)
             VALUES (13, 0, 11, 12)",
        )
        .execute(&mut *transaction)
        .await?;

        // Act
        let owner = owner_of(&mut transaction, 100).await?;

        // Assert
        assert_eq!(
            owner,
            Some(protected(ResourceKind::MusicAlbum, 5, None, true))
        );
        Ok(())
    }
}
