use sqlx::QueryBuilder;
use sqlx::Sqlite;
use sqlx::Transaction;

use crate::domain::alias::NumericID;
use crate::domain::model::video::ScanType;
use crate::domain::model::video::Video;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::video_repository::VideoFilter;
use crate::domain::port::video_repository::VideoRepository;

use super::model::video::ScanType as SqlxScanType;
use super::model::video::Video as SqlxVideo;
use super::query_filter::push_filter;

/// Repository persisting videos, backed by a `SQLite` transaction.
pub struct SqlxVideoRepository<'transaction> {
    /// Transaction the repository reads from and writes to.
    transaction: &'transaction mut Transaction<'static, Sqlite>,
}

impl<'transaction> SqlxVideoRepository<'transaction> {
    /// Create a new repository bound to `transaction`.
    #[must_use]
    pub fn new(transaction: &'transaction mut Transaction<'static, Sqlite>) -> Self {
        Self { transaction }
    }
}

impl VideoRepository for SqlxVideoRepository<'_> {
    async fn create(&mut self, video: Video) -> Result<Video, RepositoryError> {
        let row = sqlx::query_as::<_, SqlxVideo>(
            "INSERT INTO video (
                duration_ms,
                codec,
                frame_count,
                width,
                height,
                color_id,
                scan_type,
                file_id
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            RETURNING
                video_id,
                duration_ms,
                codec,
                frame_count,
                width,
                height,
                color_id,
                scan_type,
                file_id",
        )
        .bind(video.duration_ms())
        .bind(video.codec())
        .bind(video.frame_count())
        .bind(video.width())
        .bind(video.height())
        .bind(video.color_id())
        .bind(sqlx_scan_type(video.scan_type()))
        .bind(video.file_id())
        .fetch_one(&mut **self.transaction)
        .await?;

        domain_video(row)
    }

    async fn delete(&mut self, id: NumericID) -> Result<bool, RepositoryError> {
        let result = sqlx::query("DELETE FROM video WHERE video_id = ?")
            .bind(id)
            .execute(&mut **self.transaction)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn get(&mut self, id: NumericID) -> Result<Option<Video>, RepositoryError> {
        let row = sqlx::query_as::<_, SqlxVideo>(
            "SELECT
                video_id,
                duration_ms,
                codec,
                frame_count,
                width,
                height,
                color_id,
                scan_type,
                file_id
            FROM video
            WHERE video_id = ?",
        )
        .bind(id)
        .fetch_optional(&mut **self.transaction)
        .await?;

        row.map(domain_video).transpose()
    }

    async fn search(&mut self, filter: &VideoFilter) -> Result<Vec<Video>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT
                video_id,
                duration_ms,
                codec,
                frame_count,
                width,
                height,
                color_id,
                scan_type,
                file_id
            FROM video",
        );
        let mut first = true;

        if let Some(codec) = filter.codec.as_deref() {
            push_filter(&mut first, &mut builder, "codec = ", codec);
        }
        if let Some(color_id) = filter.color_id.as_deref() {
            push_filter(&mut first, &mut builder, "color_id = ", color_id);
        }
        if let Some(file_id) = filter.file_id {
            push_filter(&mut first, &mut builder, "file_id = ", file_id);
        }
        if let Some(id) = filter.video_id {
            push_filter(&mut first, &mut builder, "video_id = ", id);
        }
        if let Some(scan_type) = filter.scan_type {
            push_filter(
                &mut first,
                &mut builder,
                "scan_type = ",
                sqlx_scan_type(scan_type),
            );
        }

        let rows = builder
            .build_query_as::<SqlxVideo>()
            .fetch_all(&mut **self.transaction)
            .await?;

        rows.into_iter().map(domain_video).collect()
    }
}

/// Map the domain scan type to its persisted representation.
fn sqlx_scan_type(scan_type: ScanType) -> SqlxScanType {
    match scan_type {
        ScanType::Interlaced => SqlxScanType::Interlaced,
        ScanType::Mbaff => SqlxScanType::Mbaff,
        ScanType::Paff => SqlxScanType::Paff,
        ScanType::Progressive => SqlxScanType::Progressive,
    }
}

/// Map a persisted video row back to the domain model.
fn domain_video(row: SqlxVideo) -> Result<Video, RepositoryError> {
    let scan_type = match row.scan_type {
        SqlxScanType::Interlaced => ScanType::Interlaced,
        SqlxScanType::Mbaff => ScanType::Mbaff,
        SqlxScanType::Paff => ScanType::Paff,
        SqlxScanType::Progressive => ScanType::Progressive,
    };

    Video::try_new(
        row.video_id,
        row.duration_ms,
        row.codec,
        row.frame_count,
        row.width,
        row.height,
        row.color_id,
        scan_type,
        row.file_id,
    )
    .map_err(|_| RepositoryError::DataIntegrityViolation)
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use rstest::rstest;
    use sqlx::Sqlite;
    use sqlx::Transaction;
    use sqlx::sqlite::SqlitePoolOptions;

    use crate::domain::model::video::ScanType;
    use crate::domain::model::video::Video;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::video_repository::VideoFilter;
    use crate::domain::port::video_repository::VideoRepository as _;

    use super::SqlxVideoRepository;

    /// Open a migrated in-memory database and begin one transaction.
    async fn begin_transaction() -> Result<Transaction<'static, Sqlite>, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        pool.begin().await
    }

    /// Seed a credential and a standard user so foreign keys resolve.
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

    /// Seed a file owned by `user_id` with a per-file unique digest.
    async fn seed_file(
        transaction: &mut Transaction<'static, Sqlite>,
        file_id: i64,
        user_id: i64,
        path: &str,
        mime_type_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO file (file_id, path, user_id, is_public, mime_type_id, integrity_hash)
             VALUES (?1, ?2, ?3, 0, ?4, ?5)",
        )
        .bind(file_id)
        .bind(path)
        .bind(user_id)
        .bind(mime_type_id)
        .bind(format!("{file_id:064x}"))
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Build a domain video.
    fn video(video_id: i64, file_id: i64, scan_type: ScanType) -> Result<Video, Box<dyn Error>> {
        Ok(Video::try_new(
            video_id,
            1000.0,
            "H264".to_owned(),
            30,
            640,
            480,
            "YCbCr".to_owned(),
            scan_type,
            file_id,
        )?)
    }

    #[tokio::test]
    async fn create_assigns_identifier_and_round_trips() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 11, 1, "files/clip.mp4", "video/mp4").await?;
        let pending = video(0, 11, ScanType::Progressive)?;

        // Act
        let mut repository = SqlxVideoRepository::new(&mut transaction);
        let persisted = repository.create(pending).await?;
        let found = repository.get(persisted.video_id()).await?;

        // Assert
        assert_ne!(
            persisted.video_id(),
            0,
            "a new video must receive a real identifier"
        );
        assert_eq!(persisted.scan_type(), ScanType::Progressive);
        assert_eq!(persisted.color_id(), "YCbCr");
        assert_eq!(persisted.file_id(), 11);
        assert_eq!(found, Some(persisted));
        Ok(())
    }

    #[tokio::test]
    async fn get_missing_video_returns_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;

        // Act
        let mut repository = SqlxVideoRepository::new(&mut transaction);
        let missing = repository.get(999).await?;

        // Assert
        assert!(missing.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn create_duplicate_file_returns_already_exist() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 11, 1, "files/clip.mp4", "video/mp4").await?;
        let mut repository = SqlxVideoRepository::new(&mut transaction);
        repository
            .create(video(0, 11, ScanType::Progressive)?)
            .await?;

        // Act
        let result = repository.create(video(0, 11, ScanType::Interlaced)?).await;

        // Assert: uq_video_file_id.
        assert!(matches!(result, Err(RepositoryError::AlreadyExist)));
        Ok(())
    }

    #[tokio::test]
    async fn create_missing_color_returns_data_integrity_violation() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 11, 1, "files/clip.mp4", "video/mp4").await?;
        let pending = Video::try_new(
            0,
            1000.0,
            "H264".to_owned(),
            30,
            640,
            480,
            "not-a-colour".to_owned(),
            ScanType::Progressive,
            11,
        )?;

        // Act
        let mut repository = SqlxVideoRepository::new(&mut transaction);
        let result = repository.create(pending).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn delete_reports_matched_and_missing_rows() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 11, 1, "files/clip.mp4", "video/mp4").await?;
        let mut repository = SqlxVideoRepository::new(&mut transaction);
        let stored = repository.create(video(0, 11, ScanType::Mbaff)?).await?;

        // Act
        let deleted = repository.delete(stored.video_id()).await?;
        let missing = repository.delete(stored.video_id()).await?;

        // Assert
        assert!(deleted);
        assert!(!missing);
        Ok(())
    }

    #[tokio::test]
    async fn search_filters_by_codec_color_and_scan_type() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 11, 1, "files/clip.mp4", "video/mp4").await?;
        let mut repository = SqlxVideoRepository::new(&mut transaction);
        let stored = repository.create(video(0, 11, ScanType::Paff)?).await?;

        // Act
        let by_codec = repository
            .search(&VideoFilter {
                codec: Some("H264".to_owned()),
                ..VideoFilter::default()
            })
            .await?;
        let by_color = repository
            .search(&VideoFilter {
                color_id: Some("YCbCr".to_owned()),
                ..VideoFilter::default()
            })
            .await?;
        let by_scan_type = repository
            .search(&VideoFilter {
                scan_type: Some(ScanType::Paff),
                ..VideoFilter::default()
            })
            .await?;
        let by_file = repository
            .search(&VideoFilter {
                file_id: Some(11),
                ..VideoFilter::default()
            })
            .await?;

        // Assert
        assert_eq!(by_codec.first(), Some(&stored));
        assert_eq!(by_color.first(), Some(&stored));
        assert_eq!(by_scan_type.first(), Some(&stored));
        assert_eq!(by_file.first(), Some(&stored));
        Ok(())
    }

    #[rstest]
    #[case::duration_ms_negative(
        "INSERT INTO video (
            video_id, duration_ms, codec, frame_count, width, height, color_id, scan_type, file_id
         )
         VALUES (1, -1.0, 'H264', 30, 10, 10, 'YCbCr', 'PROGRESSIVE', 11)"
    )]
    #[case::frame_count_not_positive(
        "INSERT INTO video (
            video_id, duration_ms, codec, frame_count, width, height, color_id, scan_type, file_id
         )
         VALUES (1, 1000.0, 'H264', 0, 10, 10, 'YCbCr', 'PROGRESSIVE', 11)"
    )]
    #[case::width_not_positive(
        "INSERT INTO video (
            video_id, duration_ms, codec, frame_count, width, height, color_id, scan_type, file_id
         )
         VALUES (1, 1000.0, 'H264', 30, 0, 10, 'YCbCr', 'PROGRESSIVE', 11)"
    )]
    #[case::height_not_positive(
        "INSERT INTO video (
            video_id, duration_ms, codec, frame_count, width, height, color_id, scan_type, file_id
         )
         VALUES (1, 1000.0, 'H264', 30, 10, 0, 'YCbCr', 'PROGRESSIVE', 11)"
    )]
    #[case::scan_type_not_known(
        "INSERT INTO video (
            video_id, duration_ms, codec, frame_count, width, height, color_id, scan_type, file_id
         )
         VALUES (1, 1000.0, 'H264', 30, 10, 10, 'YCbCr', 'WEIRD', 11)"
    )]
    #[tokio::test]
    async fn raw_insert_violating_a_check_returns_data_integrity_violation(
        #[case] statement: &'static str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 11, 1, "files/clip.mp4", "video/mp4").await?;

        // Act
        let result = sqlx::query(statement)
            .execute(&mut *transaction)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::DataIntegrityViolation)),
            "statement must violate a check or foreign key: {statement}"
        );
        Ok(())
    }
}
