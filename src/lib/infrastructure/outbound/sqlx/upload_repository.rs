use std::collections::HashMap;

use sqlx::QueryBuilder;
use sqlx::Sqlite;
use sqlx::Transaction;

use crate::domain::alias::NumericID;
use crate::domain::model::integrity_hash::IntegrityHash;
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
                integrity_hash,
                is_finished
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            RETURNING
                upload_id,
                user_id,
                file_name,
                file_size,
                mime_type_id,
                chunk_size,
                integrity_hash,
                is_finished,
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
        .bind(upload.integrity_hash().as_str())
        .bind(upload.is_finished())
        .fetch_one(&mut **self.transaction)
        .await?;

        domain_upload(row, &[])
    }

    async fn delete(&mut self, id: NumericID) -> Result<bool, RepositoryError> {
        let result = sqlx::query("DELETE FROM upload WHERE upload_id = ?")
            .bind(id)
            .execute(&mut **self.transaction)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn finish(&mut self, id: NumericID) -> Result<Option<Upload>, RepositoryError> {
        let persisted = sqlx::query_as::<_, SqlxUpload>(
            "UPDATE upload
            SET is_finished = 1
            WHERE upload_id = ?1
              AND is_finished = 0
              AND (SELECT COUNT(*) FROM upload_chunk WHERE upload_id = ?1)
                  = ((file_size - 1) / chunk_size + 1)
            RETURNING
                upload_id,
                user_id,
                file_name,
                file_size,
                mime_type_id,
                chunk_size,
                integrity_hash,
                is_finished,
                created_at",
        )
        .bind(id)
        .fetch_optional(&mut **self.transaction)
        .await?;

        if let Some(row) = persisted {
            let chunk_numbers = self.chunk_numbers(id).await?;
            return Ok(Some(domain_upload(row, &chunk_numbers)?));
        }

        // The guarded update matched no row. Distinguish a missing upload from
        // a live one: a finished upload means another writer won the transition,
        // an unfinished one means the upload is not complete yet.
        let state =
            sqlx::query_scalar::<_, i64>("SELECT is_finished FROM upload WHERE upload_id = ?")
                .bind(id)
                .fetch_optional(&mut **self.transaction)
                .await?;
        match state {
            None => Ok(None),
            Some(1) => Err(RepositoryError::ConcurrencyConflict),
            Some(_) => Err(RepositoryError::Conflict),
        }
    }

    async fn record_chunk(
        &mut self,
        id: NumericID,
        chunk_number: u64,
    ) -> Result<(), RepositoryError> {
        let persisted_chunk_number =
            to_i64(chunk_number, "upload chunk_number does not fit in i64")?;
        sqlx::query(
            "INSERT INTO upload_chunk (upload_id, chunk_number)
            VALUES (?1, ?2)
            ON CONFLICT (upload_id, chunk_number) DO NOTHING",
        )
        .bind(id)
        .bind(persisted_chunk_number)
        .execute(&mut **self.transaction)
        .await?;

        Ok(())
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
                integrity_hash,
                is_finished,
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

        if rows.is_empty() {
            return Ok(Vec::new());
        }

        // Fetch the chunk rows of every matched upload in one statement, keyed
        // by upload identifier, so rebuilding the bitmap does not issue a query
        // per upload.
        let mut chunk_builder = QueryBuilder::<Sqlite>::new(
            "SELECT
                upload_chunk.upload_id,
                upload_chunk.chunk_number
            FROM upload_chunk
            JOIN upload ON upload.upload_id = upload_chunk.upload_id",
        );
        let mut first_chunk = true;

        if let Some(id) = filter.id {
            push_filter(
                &mut first_chunk,
                &mut chunk_builder,
                "upload.upload_id = ",
                id,
            );
        }
        if let Some(user_id) = filter.user_id {
            push_filter(
                &mut first_chunk,
                &mut chunk_builder,
                "upload.user_id = ",
                user_id,
            );
        }
        chunk_builder.push(" ORDER BY upload_chunk.upload_id, upload_chunk.chunk_number");

        let chunk_rows = chunk_builder
            .build_query_as::<(NumericID, i64)>()
            .fetch_all(&mut **self.transaction)
            .await?;

        let mut chunks_by_upload: HashMap<NumericID, Vec<usize>> = HashMap::new();
        for (upload_id, chunk_number) in chunk_rows {
            let chunk_number_usize = usize::try_from(chunk_number).map_err(|error| {
                RepositoryError::Unknown(
                    anyhow::anyhow!(error).context("upload chunk_number is negative"),
                )
            })?;
            chunks_by_upload
                .entry(upload_id)
                .or_default()
                .push(chunk_number_usize);
        }

        rows.into_iter()
            .map(|row| {
                let found = chunks_by_upload.get(&row.upload_id);
                let chunk_numbers: &[usize] = match found {
                    Some(numbers) => numbers.as_slice(),
                    None => &[],
                };
                domain_upload(row, chunk_numbers)
            })
            .collect()
    }
}

impl SqlxUploadRepository<'_> {
    /// Return the received chunk numbers of `id`, ordered.
    async fn chunk_numbers(&mut self, id: NumericID) -> Result<Vec<usize>, RepositoryError> {
        let rows: Vec<i64> = sqlx::query_scalar(
            "SELECT chunk_number FROM upload_chunk WHERE upload_id = ? ORDER BY chunk_number",
        )
        .bind(id)
        .fetch_all(&mut **self.transaction)
        .await?;

        rows.into_iter()
            .map(|chunk_number| {
                usize::try_from(chunk_number).map_err(|error| {
                    RepositoryError::Unknown(
                        anyhow::anyhow!(error).context("upload chunk_number is negative"),
                    )
                })
            })
            .collect()
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

/// Map a persisted upload row back to the domain model, rebuilding its chunk
/// bitmap from the `chunk_numbers` recorded for it.
fn domain_upload(row: SqlxUpload, chunk_numbers: &[usize]) -> Result<Upload, RepositoryError> {
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
    let mut chunk_bitmap =
        ChunkBitmap::try_new(total_chunks).map_err(|_| RepositoryError::DataIntegrityViolation)?;
    for &chunk_number in chunk_numbers {
        chunk_bitmap
            .mark_received(chunk_number)
            .map_err(|_| RepositoryError::DataIntegrityViolation)?;
    }
    let integrity_hash = IntegrityHash::try_new(row.integrity_hash)
        .map_err(|_| RepositoryError::DataIntegrityViolation)?;

    Upload::try_new(
        row.upload_id,
        row.user_id,
        row.file_name,
        file_size,
        row.mime_type_id,
        chunk_size,
        integrity_hash,
        chunk_bitmap,
        row.is_finished,
        row.created_at,
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

    use crate::domain::model::integrity_hash::IntegrityHash;
    use crate::domain::model::upload::ChunkBitmap;
    use crate::domain::model::upload::Upload;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::upload_repository::UploadFilter;
    use crate::domain::port::upload_repository::UploadRepository as _;

    use super::SqlxUploadRepository;

    /// A 64-character hexadecimal digest, valid for `integrity_hash`.
    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
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
            IntegrityHash::try_new(DIGEST.to_owned())?,
            bitmap,
            is_finished,
            chrono::Utc::now().naive_utc(),
        )?)
    }

    #[tokio::test]
    async fn create_assigns_final_identifier_and_round_trips() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let pending = upload(0, 1, 10, &[], false)?;

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
        assert_eq!(persisted.integrity_hash().as_str(), DIGEST);
        assert!(!persisted.is_finished());
        assert_eq!(found.first(), Some(&persisted));
        Ok(())
    }

    #[tokio::test]
    async fn record_chunk_round_trips_chunk_bits() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxUploadRepository::new(&mut transaction);
        // 10 chunks over 40 bytes: bits 0, 3 and 9 are set across two bytes.
        let created = repository.create(upload(0, 1, 40, &[], false)?).await?;

        // Act
        for chunk_number in [0, 3, 9] {
            repository
                .record_chunk(created.upload_id(), chunk_number)
                .await?;
        }
        let found = repository
            .search(&UploadFilter {
                id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?;
        let persisted = found.first().ok_or("the upload must still exist")?;

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
    async fn record_chunk_is_idempotent() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxUploadRepository::new(&mut transaction);
        let created = repository.create(upload(0, 1, 8, &[], false)?).await?;

        // Act: recording the same chunk twice is a no-op.
        repository.record_chunk(created.upload_id(), 0).await?;
        repository.record_chunk(created.upload_id(), 0).await?;
        let found = repository
            .search(&UploadFilter {
                id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?;

        // Assert
        let persisted = found.first().ok_or("the upload must still exist")?;
        assert!(persisted.chunk_bitmap().is_received(0)?);
        Ok(())
    }

    #[tokio::test]
    async fn finish_requires_every_chunk_and_persists_the_new_status() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxUploadRepository::new(&mut transaction);
        let created = repository.create(upload(0, 1, 8, &[], false)?).await?;

        // Act: finishing an incomplete upload matches no row.
        let incomplete = repository.finish(created.upload_id()).await;
        for chunk_number in [0, 1] {
            repository
                .record_chunk(created.upload_id(), chunk_number)
                .await?;
        }
        let finished = repository
            .finish(created.upload_id())
            .await?
            .ok_or("the upload must still exist")?;
        let found = repository
            .search(&UploadFilter {
                id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?;

        // Assert
        assert!(matches!(incomplete, Err(RepositoryError::Conflict)));
        assert!(finished.is_finished());
        assert!(finished.chunk_bitmap().is_received(0)?);
        assert!(finished.chunk_bitmap().is_received(1)?);
        assert_eq!(found.first(), Some(&finished));
        Ok(())
    }

    #[tokio::test]
    async fn finish_twice_returns_concurrency_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxUploadRepository::new(&mut transaction);
        let created = repository.create(upload(0, 1, 8, &[], false)?).await?;
        repository.record_chunk(created.upload_id(), 0).await?;
        repository.record_chunk(created.upload_id(), 1).await?;
        repository.finish(created.upload_id()).await?;

        // Act: the second writer loses the once-only transition.
        let result = repository.finish(created.upload_id()).await;

        // Assert
        assert!(matches!(result, Err(RepositoryError::ConcurrencyConflict)));
        Ok(())
    }

    #[tokio::test]
    async fn record_chunk_on_finished_upload_returns_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxUploadRepository::new(&mut transaction);
        let created = repository.create(upload(0, 1, 8, &[], false)?).await?;
        repository.record_chunk(created.upload_id(), 0).await?;
        repository.record_chunk(created.upload_id(), 1).await?;
        repository.finish(created.upload_id()).await?;

        // Act: the finished-guard trigger rejects a late chunk.
        let result = repository.record_chunk(created.upload_id(), 0).await;

        // Assert
        assert!(matches!(result, Err(RepositoryError::Conflict)));
        Ok(())
    }

    #[tokio::test]
    async fn finish_missing_upload_returns_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxUploadRepository::new(&mut transaction);

        // Act
        let result = repository.finish(404).await;

        // Assert
        assert!(matches!(result, Ok(None)));
        Ok(())
    }

    #[tokio::test]
    async fn create_missing_user_returns_data_integrity_violation() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;

        // Act
        let mut repository = SqlxUploadRepository::new(&mut transaction);
        let result = repository.create(upload(0, 999, 8, &[], false)?).await;

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
            IntegrityHash::try_new(DIGEST.to_owned())?,
            chunk_bitmap,
            false,
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

    #[rstest]
    #[case::file_size_zero(
        "INSERT INTO upload (
            user_id, file_name, file_size, mime_type_id, chunk_size, integrity_hash
         ) VALUES (1, 'clip.mp4', 0, 'video/mp4', 4, '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef')"
    )]
    #[case::chunk_size_zero(
        "INSERT INTO upload (
            user_id, file_name, file_size, mime_type_id, chunk_size, integrity_hash
         ) VALUES (1, 'clip.mp4', 4, 'video/mp4', 0, '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef')"
    )]
    #[case::integrity_hash_length(
        "INSERT INTO upload (
            user_id, file_name, file_size, mime_type_id, chunk_size, integrity_hash
         ) VALUES (1, 'clip.mp4', 4, 'video/mp4', 4, 'too-short')"
    )]
    #[case::is_finished_out_of_range(
        "INSERT INTO upload (
            user_id, file_name, file_size, mime_type_id, chunk_size, integrity_hash, is_finished
         ) VALUES (1, 'clip.mp4', 4, 'video/mp4', 4, '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef', 2)"
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
    async fn delete_existing_upload_returns_true() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxUploadRepository::new(&mut transaction);
        let created = repository.create(upload(0, 1, 8, &[], false)?).await?;
        repository.record_chunk(created.upload_id(), 0).await?;

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
        repository.create(upload(0, 1, 8, &[], false)?).await?;
        repository.create(upload(0, 2, 8, &[], false)?).await?;

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
