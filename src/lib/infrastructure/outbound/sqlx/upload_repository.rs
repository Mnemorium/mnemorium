use sqlx::QueryBuilder;
use sqlx::Sqlite;
use sqlx::Transaction;

use crate::domain::alias::NumericID;
use crate::domain::model::upload::ChunkBitmap;
use crate::domain::model::upload::Upload;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::upload_repository::UploadFilter;
use crate::domain::port::upload_repository::UploadRepository;

use super::model::upload::Upload as SqlxUpload;

/// Repository persisting uploads, backed by a `SQLite` transaction.
pub struct SqlxUploadRepository<'transaction> {
    /// Transaction the repository reads from and writes to.
    transaction: &'transaction mut Transaction<'static, Sqlite>,
}

impl<'transaction> SqlxUploadRepository<'transaction> {
    /// Create a new repository bound to `transaction`.
    #[must_use]
    pub fn new(transaction: &'transaction mut Transaction<'static, Sqlite>) -> Self {
        Self { transaction }
    }
}

impl UploadRepository for SqlxUploadRepository<'_> {
    async fn create(&mut self, upload: Upload) -> Result<Upload, RepositoryError> {
        let row = sqlx::query_as::<_, SqlxUpload>(
            "INSERT INTO upload (
                user_id,
                file_name,
                file_size,
                mime_type_id,
                chunk_size,
                md5_integrity,
                chunk_bitmap,
                is_finished,
                version
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            RETURNING
                upload_id,
                user_id,
                file_name,
                file_size,
                mime_type_id,
                chunk_size,
                md5_integrity,
                chunk_bitmap,
                is_finished,
                version,
                created_at",
        )
        .bind(upload.user_id())
        .bind(upload.file_name())
        .bind(to_i64(
            upload.file_size(),
            "upload file_size does not fit in i64",
        )?)
        .bind(upload.mime_type_id())
        .bind(to_i64(
            upload.chunk_size(),
            "upload chunk_size does not fit in i64",
        )?)
        .bind(upload.md5_integrity())
        .bind(upload.chunk_bitmap().as_bytes().to_vec())
        .bind(upload.is_finished())
        .bind(upload.version())
        .fetch_one(&mut **self.transaction)
        .await?;

        domain_upload(row)
    }

    async fn delete(&mut self, id: NumericID) -> Result<bool, RepositoryError> {
        let result = sqlx::query("DELETE FROM upload WHERE upload_id = ?")
            .bind(id)
            .execute(&mut **self.transaction)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn save(&mut self, upload: Upload) -> Result<Upload, RepositoryError> {
        let row = sqlx::query_as::<_, SqlxUpload>(
            "UPDATE upload
            SET
                user_id = ?1,
                file_name = ?2,
                file_size = ?3,
                mime_type_id = ?4,
                chunk_size = ?5,
                md5_integrity = ?6,
                chunk_bitmap = ?7,
                is_finished = ?8,
                version = version + 1
            WHERE upload_id = ?9 AND version = ?10
            RETURNING
                upload_id,
                user_id,
                file_name,
                file_size,
                mime_type_id,
                chunk_size,
                md5_integrity,
                chunk_bitmap,
                is_finished,
                version,
                created_at",
        )
        .bind(upload.user_id())
        .bind(upload.file_name())
        .bind(to_i64(
            upload.file_size(),
            "upload file_size does not fit in i64",
        )?)
        .bind(upload.mime_type_id())
        .bind(to_i64(
            upload.chunk_size(),
            "upload chunk_size does not fit in i64",
        )?)
        .bind(upload.md5_integrity())
        .bind(upload.chunk_bitmap().as_bytes().to_vec())
        .bind(upload.is_finished())
        .bind(upload.upload_id())
        .bind(upload.version())
        .fetch_optional(&mut **self.transaction)
        .await?
        .ok_or(RepositoryError::ConcurrentModification)?;

        domain_upload(row)
    }

    async fn search(&mut self, filter: &UploadFilter) -> Result<Vec<Upload>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT
                upload_id,
                user_id,
                file_name,
                file_size,
                mime_type_id,
                chunk_size,
                md5_integrity,
                chunk_bitmap,
                is_finished,
                version,
                created_at
            FROM upload",
        );
        let mut first = true;

        if let Some(id) = filter.id {
            push_filter(&mut first, &mut builder, "upload_id = ", id);
        }
        if let Some(user_id) = filter.user_id {
            push_filter(&mut first, &mut builder, "user_id = ", user_id);
        }

        let rows = builder
            .build_query_as::<SqlxUpload>()
            .fetch_all(&mut **self.transaction)
            .await?;

        rows.into_iter().map(domain_upload).collect()
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

/// Convert an unsigned value to its persisted `i64` representation.
fn to_i64(value: u64, message: &'static str) -> Result<i64, RepositoryError> {
    i64::try_from(value)
        .map_err(|error| RepositoryError::Unknown(anyhow::anyhow!(error).context(message)))
}

/// Map a persisted upload row back to the domain model.
fn domain_upload(row: SqlxUpload) -> Result<Upload, RepositoryError> {
    let file_size = u64::try_from(row.file_size).map_err(|error| {
        RepositoryError::Unknown(anyhow::anyhow!(error).context("upload file_size is negative"))
    })?;
    let chunk_size = u64::try_from(row.chunk_size).map_err(|error| {
        RepositoryError::Unknown(anyhow::anyhow!(error).context("upload chunk_size is negative"))
    })?;
    let total_chunks = usize::try_from(file_size.div_ceil(chunk_size)).map_err(|error| {
        RepositoryError::Unknown(
            anyhow::anyhow!(error).context("upload total chunk count does not fit in usize"),
        )
    })?;
    let chunk_bitmap = ChunkBitmap::from_bytes(row.chunk_bitmap, total_chunks)
        .map_err(|_| RepositoryError::DataIntegrityViolation)?;

    Upload::try_new(
        row.upload_id,
        row.user_id,
        row.file_name,
        file_size,
        row.mime_type_id,
        chunk_size,
        row.md5_integrity,
        chunk_bitmap,
        row.is_finished,
        row.version,
        row.created_at,
    )
    .map_err(|_| RepositoryError::DataIntegrityViolation)
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use sqlx::Sqlite;
    use sqlx::Transaction;
    use sqlx::sqlite::SqlitePoolOptions;

    use crate::domain::model::upload::ChunkBitmap;
    use crate::domain::model::upload::Upload;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::upload_repository::UploadFilter;
    use crate::domain::port::upload_repository::UploadRepository as _;

    use super::SqlxUploadRepository;

    /// A 32-character hexadecimal digest, valid for `md5_integrity`.
    const DIGEST: &str = "0123456789abcdef0123456789abcdef";
    /// Size of every chunk, in bytes.
    const CHUNK_SIZE: u64 = 4;

    async fn begin_transaction() -> Result<Transaction<'static, Sqlite>, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        pool.begin().await
    }

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

    /// Build a pending upload whose bitmap marks `received` chunks.
    fn upload(
        upload_id: i64,
        user_id: i64,
        file_size: u64,
        received: &[usize],
        is_finished: bool,
        version: i64,
    ) -> Result<Upload, Box<dyn Error>> {
        let total_chunks = usize::try_from(file_size.div_ceil(CHUNK_SIZE))?;
        let mut bitmap = ChunkBitmap::try_new(total_chunks)?;
        for chunk_number in received {
            bitmap.mark_received(*chunk_number)?;
        }
        Ok(Upload::try_new(
            upload_id,
            user_id,
            "clip.mp4".to_owned(),
            file_size,
            "video/mp4".to_owned(),
            CHUNK_SIZE,
            DIGEST.to_owned(),
            bitmap,
            is_finished,
            version,
            chrono::Utc::now().naive_utc(),
        )?)
    }

    #[tokio::test]
    async fn create_assigns_final_identifier_and_round_trips() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let pending = upload(0, 1, 10, &[], false, 0)?;

        // Act
        let mut repository = SqlxUploadRepository::new(&mut transaction);
        let persisted = repository.create(pending).await?;
        let found = repository
            .search(&UploadFilter {
                id: Some(persisted.upload_id()),
                ..UploadFilter::default()
            })
            .await?;

        // Assert
        assert_ne!(
            persisted.upload_id(),
            0,
            "a new upload must receive a real identifier"
        );
        assert_eq!(persisted.user_id(), 1);
        assert_eq!(persisted.file_name(), "clip.mp4");
        assert_eq!(persisted.file_size(), 10);
        assert_eq!(persisted.mime_type_id(), "video/mp4");
        assert_eq!(persisted.chunk_size(), CHUNK_SIZE);
        assert_eq!(persisted.md5_integrity(), DIGEST);
        assert!(!persisted.is_finished());
        assert_eq!(persisted.version(), 0);
        assert_eq!(found.first(), Some(&persisted));
        Ok(())
    }

    #[tokio::test]
    async fn create_round_trips_chunk_bitmap_bits() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        // 10 chunks over 40 bytes: bits 0, 3 and 9 are set across two bytes.
        let pending = upload(0, 1, 40, &[0, 3, 9], false, 0)?;

        // Act
        let mut repository = SqlxUploadRepository::new(&mut transaction);
        let persisted = repository.create(pending).await?;

        // Assert
        assert!(persisted.chunk_bitmap().is_received(0)?);
        assert!(!persisted.chunk_bitmap().is_received(1)?);
        assert!(persisted.chunk_bitmap().is_received(3)?);
        assert!(persisted.chunk_bitmap().is_received(9)?);
        assert_eq!(
            persisted.chunk_bitmap().as_bytes(),
            &[0b0000_1001, 0b0000_0010]
        );
        Ok(())
    }

    #[tokio::test]
    async fn save_bumps_version_and_persists_the_new_status() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxUploadRepository::new(&mut transaction);
        let created = repository.create(upload(0, 1, 8, &[0], false, 0)?).await?;
        let mut updated = created.clone();
        updated.mark_chunk_received(1)?;
        updated.finish();

        // Act
        let saved = repository.save(updated).await?;
        let found = repository
            .search(&UploadFilter {
                id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?;

        // Assert
        assert_eq!(saved.version(), created.version().saturating_add(1));
        assert!(saved.is_finished());
        assert!(saved.chunk_bitmap().is_received(1)?);
        assert_eq!(found.first(), Some(&saved));
        Ok(())
    }

    #[tokio::test]
    async fn save_with_stale_version_returns_concurrent_modification() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxUploadRepository::new(&mut transaction);
        let created = repository.create(upload(0, 1, 8, &[], false, 0)?).await?;
        let mut first_writer = created.clone();
        first_writer.mark_chunk_received(0)?;
        repository.save(first_writer).await?;

        // Act: the second writer still carries the pre-save version.
        let mut second_writer = created;
        second_writer.mark_chunk_received(1)?;
        let result = repository.save(second_writer).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::ConcurrentModification)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn save_missing_upload_returns_concurrent_modification() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxUploadRepository::new(&mut transaction);

        // Act
        let result = repository.save(upload(404, 1, 8, &[], false, 0)?).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::ConcurrentModification)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn create_missing_user_returns_data_integrity_violation() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;

        // Act
        let mut repository = SqlxUploadRepository::new(&mut transaction);
        let result = repository.create(upload(0, 999, 8, &[], false, 0)?).await;

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
        let chunk_bitmap = ChunkBitmap::try_new(2)?;
        let pending = Upload::try_new(
            0,
            1,
            "clip.mp4".to_owned(),
            8,
            "application/unknown".to_owned(),
            CHUNK_SIZE,
            DIGEST.to_owned(),
            chunk_bitmap,
            false,
            0,
            chrono::Utc::now().naive_utc(),
        )?;

        // Act
        let mut repository = SqlxUploadRepository::new(&mut transaction);
        let result = repository.create(pending).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn create_violating_a_check_returns_data_integrity_violation()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let statements = [
            "INSERT INTO upload (
                user_id, file_name, file_size, mime_type_id, chunk_size, md5_integrity, chunk_bitmap
             ) VALUES (1, 'clip.mp4', 0, 'video/mp4', 4, '0123456789abcdef0123456789abcdef', x'00')",
            "INSERT INTO upload (
                user_id, file_name, file_size, mime_type_id, chunk_size, md5_integrity, chunk_bitmap
             ) VALUES (1, 'clip.mp4', 4, 'video/mp4', 0, '0123456789abcdef0123456789abcdef', x'00')",
            "INSERT INTO upload (
                user_id, file_name, file_size, mime_type_id, chunk_size, md5_integrity, chunk_bitmap
             ) VALUES (1, 'clip.mp4', 4, 'video/mp4', 4, 'too-short', x'00')",
            "INSERT INTO upload (
                user_id, file_name, file_size, mime_type_id, chunk_size, md5_integrity, chunk_bitmap, is_finished
             ) VALUES (1, 'clip.mp4', 4, 'video/mp4', 4, '0123456789abcdef0123456789abcdef', x'00', 2)",
            "INSERT INTO upload (
                user_id, file_name, file_size, mime_type_id, chunk_size, md5_integrity, chunk_bitmap, version
             ) VALUES (1, 'clip.mp4', 4, 'video/mp4', 4, '0123456789abcdef0123456789abcdef', x'00', -1)",
        ];

        // Act & Assert
        for statement in statements {
            let result = sqlx::query(statement).execute(&mut *transaction).await;
            let mapped = result.map_err(RepositoryError::from);
            assert!(
                matches!(mapped, Err(RepositoryError::DataIntegrityViolation)),
                "statement must violate a check constraint: {statement}"
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn delete_existing_upload_returns_true() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxUploadRepository::new(&mut transaction);
        let created = repository.create(upload(0, 1, 8, &[], false, 0)?).await?;

        // Act
        let deleted = repository.delete(created.upload_id()).await?;

        // Assert
        assert!(deleted);
        Ok(())
    }

    #[tokio::test]
    async fn delete_missing_upload_returns_false() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;

        // Act
        let mut repository = SqlxUploadRepository::new(&mut transaction);
        let deleted = repository.delete(404).await?;

        // Assert
        assert!(!deleted);
        Ok(())
    }

    #[tokio::test]
    async fn search_filters_by_user_and_returns_all_when_empty() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_user(&mut transaction, 2).await?;
        let mut repository = SqlxUploadRepository::new(&mut transaction);
        repository.create(upload(0, 1, 8, &[], false, 0)?).await?;
        repository.create(upload(0, 2, 8, &[], false, 0)?).await?;

        // Act
        let first_user = repository
            .search(&UploadFilter {
                user_id: Some(1),
                ..UploadFilter::default()
            })
            .await?;
        let all = repository.search(&UploadFilter::default()).await?;

        // Assert
        assert_eq!(first_user.len(), 1);
        assert_eq!(first_user.first().map(Upload::user_id), Some(1));
        assert_eq!(all.len(), 2);
        Ok(())
    }
}
