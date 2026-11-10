use std::collections::HashMap;

use sqlx::{QueryBuilder, Sqlite, Transaction};

use crate::domain::NumericID;
use crate::domain::model::{ChunkBitmap, IntegrityHash, Upload};
use crate::domain::port::{RepositoryError, UploadFilter, UploadRepository};

use super::error_mapping::emit_catalogued_failure;
use super::model::Upload as SqlxUpload;

/// Repository persisting uploads, backed by a `SQLite` transaction.
pub(crate) struct SqlxUploadRepository<'transaction> {
    /// Transaction the repository reads from and writes to.
    transaction: &'transaction mut Transaction<'static, Sqlite>,
}

impl<'transaction> SqlxUploadRepository<'transaction> {
    /// Delete every recorded chunk row of the upload identified by `id`.
    async fn delete_chunks(&mut self, id: NumericID) -> Result<bool, RepositoryError> {
        let result = sqlx::query("DELETE FROM upload_chunk WHERE upload_id = ?")
            .bind(id)
            .execute(&mut **self.transaction)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Create a new repository bound to `transaction`.
    #[must_use]
    pub(crate) fn new(transaction: &'transaction mut Transaction<'static, Sqlite>) -> Self {
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
                is_verifying,
                sealed_at,
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

    async fn delete(&mut self, filter: &UploadFilter) -> Result<bool, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new("DELETE FROM upload");
        let mut first = true;

        if let Some(id) = filter.upload_id {
            push_filter(&mut first, &mut builder, "upload_id = ", id);
        }
        if let Some(user_id) = filter.user_id {
            push_filter(&mut first, &mut builder, "user_id = ", user_id);
        }
        if let Some(is_finished) = filter.is_finished {
            push_filter(&mut first, &mut builder, "is_finished = ", is_finished);
        }
        if let Some(is_verifying) = filter.is_verifying {
            push_filter(&mut first, &mut builder, "is_verifying = ", is_verifying);
        }
        if let Some(created_at) = filter.created_at {
            push_filter(&mut first, &mut builder, "created_at <= ", created_at);
        }
        if let Some(sealed_at) = filter.sealed_at {
            push_filter(&mut first, &mut builder, "sealed_at <= ", sealed_at);
        }

        let result = builder.build().execute(&mut **self.transaction).await?;
        Ok(result.rows_affected() > 0)
    }

    async fn finish(&mut self, id: NumericID) -> Result<Option<Upload>, RepositoryError> {
        let persisted = sqlx::query_as::<_, SqlxUpload>(
            "UPDATE upload
            SET is_finished = 1, is_verifying = 0, sealed_at = NULL
            WHERE upload_id = ?1
              AND is_finished = 0
              AND is_verifying = 1
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
                is_verifying,
                sealed_at,
                created_at",
        )
        .bind(id)
        .fetch_optional(&mut **self.transaction)
        .await?;

        if let Some(row) = persisted {
            // The session's chunk rows are removed in the same transaction: a
            // finished session reports an all-ones bitmap by definition, so no
            // chunk row is read after this point.
            let _chunks_removed = self.delete_chunks(id).await?;
            return Ok(Some(domain_upload(row, &[])?));
        }

        // The guarded update matched no row. Distinguish a missing upload from
        // a live one: a finished upload means another writer won the transition,
        // an unfinished one means the upload is not sealed or not complete yet.
        let state = sqlx::query_as::<_, (bool, bool)>(
            "SELECT is_finished, is_verifying FROM upload WHERE upload_id = ?",
        )
        .bind(id)
        .fetch_optional(&mut **self.transaction)
        .await?;
        // The adapter owns the failure log for the outcome it classified
        // (`OBS-002`): an already-finished upload is a lost race for the
        // once-only transition and a genuine dependency fault, whereas an
        // unsealed or incomplete one is the expected constraint. Only the
        // stable `kind` and `operation` are logged, never the row (`OBS-003`).
        match state {
            None => Ok(None),
            Some((true, _)) => {
                emit_catalogued_failure("concurrency_conflict", "finish", false);
                Err(RepositoryError::ConcurrencyConflict)
            }
            Some((false, _)) => {
                emit_catalogued_failure("conflict", "finish", true);
                Err(RepositoryError::Conflict)
            }
        }
    }

    async fn save(&mut self, upload: Upload) -> Result<Upload, RepositoryError> {
        let exists = sqlx::query_scalar::<_, i64>("SELECT 1 FROM upload WHERE upload_id = ?")
            .bind(upload.upload_id())
            .fetch_optional(&mut **self.transaction)
            .await?
            .is_some();
        if !exists {
            return Err(RepositoryError::OperationFailed);
        }

        let row = sqlx::query_as::<_, SqlxUpload>(
            "INSERT INTO upload (
                upload_id,
                user_id,
                file_name,
                file_size,
                mime_type_id,
                chunk_size,
                integrity_hash,
                is_finished,
                is_verifying,
                sealed_at,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT (upload_id) DO UPDATE SET
                user_id = excluded.user_id,
                file_name = excluded.file_name,
                file_size = excluded.file_size,
                mime_type_id = excluded.mime_type_id,
                chunk_size = excluded.chunk_size,
                integrity_hash = excluded.integrity_hash,
                is_finished = CASE
                    WHEN upload.is_finished = 1 THEN 1
                    ELSE excluded.is_finished
                END,
                is_verifying = excluded.is_verifying,
                sealed_at = excluded.sealed_at
            RETURNING
                upload_id,
                user_id,
                file_name,
                file_size,
                mime_type_id,
                chunk_size,
                integrity_hash,
                is_finished,
                is_verifying,
                sealed_at,
                created_at",
        )
        .bind(upload.upload_id())
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
        .bind(upload.is_verifying())
        .bind(upload.sealed_at())
        .bind(upload.created_at())
        .fetch_one(&mut **self.transaction)
        .await?;

        // Rebuild the chunk rows from the bitmap the aggregate holds. The row is
        // written first so the insert trigger sees the new seal state: a sealed
        // or finished session keeps its existing chunk rows, and only an open
        // session is rewritten.
        let mut chunk_numbers = Vec::new();
        if !upload.is_finished() {
            let total_chunks = upload.chunk_bitmap().total_chunks();
            for (byte_index, byte) in upload.chunk_bitmap().as_bytes().iter().enumerate() {
                for bit in 0..8u32 {
                    if byte & (1u8 << bit) == 0 {
                        continue;
                    }
                    let chunk_number = byte_index
                        .saturating_mul(8)
                        .saturating_add(usize::try_from(bit).unwrap_or(0));
                    if chunk_number < total_chunks {
                        chunk_numbers.push(chunk_number);
                    }
                }
            }
        }
        if !upload.is_finished() && !upload.is_verifying() {
            self.delete_chunks(upload.upload_id()).await?;
            for &chunk_number in &chunk_numbers {
                let persisted_chunk_number = to_i64(
                    u64::try_from(chunk_number).unwrap_or(u64::MAX),
                    "upload chunk_number does not fit in i64",
                )?;
                sqlx::query(
                    "INSERT INTO upload_chunk (upload_id, chunk_number)
                    VALUES (?1, ?2)
                    ON CONFLICT (upload_id, chunk_number) DO NOTHING",
                )
                .bind(upload.upload_id())
                .bind(persisted_chunk_number)
                .execute(&mut **self.transaction)
                .await?;
            }
        }

        domain_upload(row, &chunk_numbers)
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
                is_verifying,
                sealed_at,
                created_at
            FROM upload",
        );
        let mut first = true;

        if let Some(id) = filter.upload_id {
            push_filter(&mut first, &mut builder, "upload_id = ", id);
        }
        if let Some(user_id) = filter.user_id {
            push_filter(&mut first, &mut builder, "user_id = ", user_id);
        }
        if let Some(is_finished) = filter.is_finished {
            push_filter(&mut first, &mut builder, "is_finished = ", is_finished);
        }
        if let Some(is_verifying) = filter.is_verifying {
            push_filter(&mut first, &mut builder, "is_verifying = ", is_verifying);
        }
        if let Some(created_at) = filter.created_at {
            push_filter(&mut first, &mut builder, "created_at <= ", created_at);
        }
        if let Some(sealed_at) = filter.sealed_at {
            push_filter(&mut first, &mut builder, "sealed_at <= ", sealed_at);
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

        if let Some(id) = filter.upload_id {
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
        if let Some(is_finished) = filter.is_finished {
            push_filter(
                &mut first_chunk,
                &mut chunk_builder,
                "upload.is_finished = ",
                is_finished,
            );
        }
        if let Some(is_verifying) = filter.is_verifying {
            push_filter(
                &mut first_chunk,
                &mut chunk_builder,
                "upload.is_verifying = ",
                is_verifying,
            );
        }
        if let Some(created_at) = filter.created_at {
            push_filter(
                &mut first_chunk,
                &mut chunk_builder,
                "upload.created_at <= ",
                created_at,
            );
        }
        if let Some(sealed_at) = filter.sealed_at {
            push_filter(
                &mut first_chunk,
                &mut chunk_builder,
                "upload.sealed_at <= ",
                sealed_at,
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
///
/// A finished session removed its chunk rows at finish, so it reports an
/// all-ones bitmap: every chunk was received by definition.
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
    let mut chunk_bitmap = if row.is_finished {
        ChunkBitmap::try_complete(total_chunks)
    } else {
        ChunkBitmap::try_new(total_chunks)
    }
    .map_err(|_| RepositoryError::DataIntegrityViolation)?;
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
        row.is_verifying,
        row.sealed_at,
        row.created_at,
    )
    .map_err(|_| RepositoryError::DataIntegrityViolation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::outbound::sqlx::test_helpers::begin_transaction;
    use crate::test_helpers::*;
    use rstest::rstest;
    use std::error::Error;
    use tracing_test::traced_test;

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
            false,
            None,
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
                upload_id: Some(persisted.upload_id()),
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
        let mut aggregate = repository
            .search(&UploadFilter {
                upload_id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?
            .into_iter()
            .next()
            .ok_or("the upload must still exist")?;
        aggregate.record_chunk(0)?;
        aggregate.record_chunk(3)?;
        aggregate.record_chunk(9)?;
        repository.save(aggregate).await?;
        let found = repository
            .search(&UploadFilter {
                upload_id: Some(created.upload_id()),
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
        let mut aggregate = repository
            .search(&UploadFilter {
                upload_id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?
            .into_iter()
            .next()
            .ok_or("the upload must still exist")?;
        aggregate.record_chunk(0)?;
        repository.save(aggregate).await?;
        let mut again = repository
            .search(&UploadFilter {
                upload_id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?
            .into_iter()
            .next()
            .ok_or("the upload must still exist")?;
        again.record_chunk(0)?;
        repository.save(again).await?;
        let found = repository
            .search(&UploadFilter {
                upload_id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?;

        // Assert
        let persisted = found.first().ok_or("the upload must still exist")?;
        assert!(persisted.chunk_bitmap().is_received(0)?);
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn finish_requires_every_chunk_and_persists_the_new_status() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxUploadRepository::new(&mut transaction);
        let created = repository.create(upload(0, 1, 8, &[], false)?).await?;

        // Act: finishing an incomplete, unsealed upload matches no row.
        let incomplete = repository.finish(created.upload_id()).await;
        let mut aggregate = repository
            .search(&UploadFilter {
                upload_id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?
            .into_iter()
            .next()
            .ok_or("the upload must still exist")?;
        aggregate.record_chunk(0)?;
        aggregate.record_chunk(1)?;
        repository.save(aggregate).await?;
        let mut sealed = repository
            .search(&UploadFilter {
                upload_id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?
            .into_iter()
            .next()
            .ok_or("the upload must still exist")?;
        sealed.seal(chrono::Utc::now().naive_utc());
        repository.save(sealed).await?;
        let finished = repository
            .finish(created.upload_id())
            .await?
            .ok_or("the upload must still exist")?;
        let found = repository
            .search(&UploadFilter {
                upload_id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?;

        // Assert
        assert!(matches!(incomplete, Err(RepositoryError::Conflict)));
        assert!(finished.is_finished());
        assert!(!finished.is_verifying());
        assert!(finished.sealed_at().is_none());
        assert!(finished.chunk_bitmap().is_received(0)?);
        assert!(finished.chunk_bitmap().is_received(1)?);
        assert_eq!(found.first(), Some(&finished));
        // The finished session's chunk rows are removed in the same transaction.
        let chunks: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM upload_chunk WHERE upload_id = ?")
                .bind(created.upload_id())
                .fetch_one(&mut *transaction)
                .await?;
        assert_eq!(chunks, 0);
        // An unfinished upload is the expected not-complete-yet constraint, so
        // the adapter logs it once at `warn`, never as a dependency fault.
        logs_assert(|lines| {
            let count = lines.iter().filter(|line| line.contains("WARN")).count();
            (count == 1).then_some(()).ok_or_else(|| {
                format!("the incomplete finish must emit exactly one warn event (found {count})")
            })
        });
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("event=\"constraint_rejected\""))
                .count();
            (count == 1)
                .then_some(())
                .ok_or_else(|| format!(
                    "the incomplete finish must emit exactly one constraint_rejected event (found {count})"
                ))
        });
        assert!(
            !logs_contain("ERROR"),
            "the incomplete finish must not be logged as a dependency fault"
        );
        assert!(
            logs_contain("security"),
            "the event must target the reserved security target"
        );
        assert!(
            logs_contain("kind=\"conflict\""),
            "the event must carry the stable kind"
        );
        assert!(
            logs_contain("operation=\"finish\""),
            "the event must name the owning operation"
        );
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn finish_twice_returns_concurrency_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxUploadRepository::new(&mut transaction);
        let created = repository.create(upload(0, 1, 8, &[], false)?).await?;
        let mut aggregate = repository
            .search(&UploadFilter {
                upload_id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?
            .into_iter()
            .next()
            .ok_or("the upload must still exist")?;
        aggregate.record_chunk(0)?;
        aggregate.record_chunk(1)?;
        repository.save(aggregate).await?;
        let mut sealed = repository
            .search(&UploadFilter {
                upload_id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?
            .into_iter()
            .next()
            .ok_or("the upload must still exist")?;
        sealed.seal(chrono::Utc::now().naive_utc());
        repository.save(sealed).await?;
        repository.finish(created.upload_id()).await?;

        // Act: the second writer loses the once-only transition.
        let result = repository.finish(created.upload_id()).await;

        // Assert
        assert!(matches!(result, Err(RepositoryError::ConcurrencyConflict)));
        // Losing the once-only transition to another writer is a genuine
        // dependency fault, so the adapter logs it once at `error`.
        logs_assert(|lines| {
            let count = lines.iter().filter(|line| line.contains("ERROR")).count();
            (count == 1).then_some(()).ok_or_else(|| {
                format!("the lost transition must emit exactly one error event (found {count})")
            })
        });
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("event=\"port_fault\""))
                .count();
            (count == 1).then_some(()).ok_or_else(|| {
                format!(
                    "the lost transition must emit exactly one port_fault event (found {count})"
                )
            })
        });
        assert!(
            !logs_contain("WARN"),
            "the lost transition must not be logged as an expected constraint"
        );
        assert!(
            logs_contain("security"),
            "the event must target the reserved security target"
        );
        assert!(
            logs_contain("kind=\"concurrency_conflict\""),
            "the event must carry the stable kind"
        );
        assert!(
            logs_contain("operation=\"finish\""),
            "the event must name the owning operation"
        );
        Ok(())
    }

    #[tokio::test]
    async fn record_chunk_on_finished_upload_returns_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxUploadRepository::new(&mut transaction);
        let created = repository.create(upload(0, 1, 8, &[], false)?).await?;
        let mut aggregate = repository
            .search(&UploadFilter {
                upload_id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?
            .into_iter()
            .next()
            .ok_or("the upload must still exist")?;
        aggregate.record_chunk(0)?;
        aggregate.record_chunk(1)?;
        repository.save(aggregate).await?;
        let mut sealed = repository
            .search(&UploadFilter {
                upload_id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?
            .into_iter()
            .next()
            .ok_or("the upload must still exist")?;
        sealed.seal(chrono::Utc::now().naive_utc());
        repository.save(sealed).await?;
        repository.finish(created.upload_id()).await?;

        // Act: the finished-guard trigger rejects a late chunk.
        let result =
            sqlx::query("INSERT INTO upload_chunk (upload_id, chunk_number) VALUES (?1, 0)")
                .bind(created.upload_id())
                .execute(&mut *transaction)
                .await
                .map_err(RepositoryError::from);

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
            false,
            None,
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
    #[case::is_verifying_out_of_range(
        "INSERT INTO upload (
            user_id, file_name, file_size, mime_type_id, chunk_size, integrity_hash, is_verifying
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
        let mut aggregate = repository
            .search(&UploadFilter {
                upload_id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?
            .into_iter()
            .next()
            .ok_or("the upload must still exist")?;
        aggregate.record_chunk(0)?;
        repository.save(aggregate).await?;

        // Act
        let deleted = repository
            .delete(&UploadFilter {
                upload_id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?;

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
        let deleted = repository
            .delete(&UploadFilter {
                upload_id: Some(404),
                ..UploadFilter::default()
            })
            .await?;

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

    #[tokio::test]
    async fn seal_transitions_a_pending_upload() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxUploadRepository::new(&mut transaction);
        let created = repository.create(upload(0, 1, 8, &[], false)?).await?;
        let sealed_at = chrono::Utc::now().naive_utc();

        // Act
        let mut aggregate = repository
            .search(&UploadFilter {
                upload_id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?
            .into_iter()
            .next()
            .ok_or("the upload must still exist")?;
        aggregate.seal(sealed_at);
        let sealed = repository.save(aggregate).await?;
        let found = repository
            .search(&UploadFilter {
                upload_id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?;

        // Assert
        let persisted = found.first().ok_or("the upload must still exist")?;
        assert!(sealed.is_verifying());
        assert_eq!(sealed.sealed_at(), Some(sealed_at));
        assert!(persisted.is_verifying());
        assert_eq!(persisted.sealed_at(), Some(sealed_at));
        Ok(())
    }

    #[tokio::test]
    async fn unseal_clears_the_seal() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxUploadRepository::new(&mut transaction);
        let created = repository.create(upload(0, 1, 8, &[], false)?).await?;
        let mut aggregate = repository
            .search(&UploadFilter {
                upload_id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?
            .into_iter()
            .next()
            .ok_or("the upload must still exist")?;
        aggregate.seal(chrono::Utc::now().naive_utc());
        repository.save(aggregate).await?;

        // Act
        let mut unsealed = repository
            .search(&UploadFilter {
                upload_id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?
            .into_iter()
            .next()
            .ok_or("the upload must still exist")?;
        unsealed.unseal();
        repository.save(unsealed).await?;
        let mut again = repository
            .search(&UploadFilter {
                upload_id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?
            .into_iter()
            .next()
            .ok_or("the upload must still exist")?;
        again.unseal();
        repository.save(again).await?;
        let found = repository
            .search(&UploadFilter {
                upload_id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?;

        // Assert
        let persisted = found.first().ok_or("the upload must still exist")?;
        assert!(!persisted.is_verifying());
        assert!(persisted.sealed_at().is_none());
        Ok(())
    }

    #[tokio::test]
    async fn record_chunk_on_sealed_upload_returns_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxUploadRepository::new(&mut transaction);
        let created = repository.create(upload(0, 1, 8, &[], false)?).await?;
        let mut aggregate = repository
            .search(&UploadFilter {
                upload_id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?
            .into_iter()
            .next()
            .ok_or("the upload must still exist")?;
        aggregate.record_chunk(0)?;
        repository.save(aggregate).await?;
        let mut sealed = repository
            .search(&UploadFilter {
                upload_id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?
            .into_iter()
            .next()
            .ok_or("the upload must still exist")?;
        sealed.seal(chrono::Utc::now().naive_utc());
        repository.save(sealed).await?;

        // Act: the not-writable trigger rejects a chunk during verification.
        let result =
            sqlx::query("INSERT INTO upload_chunk (upload_id, chunk_number) VALUES (?1, 1)")
                .bind(created.upload_id())
                .execute(&mut *transaction)
                .await
                .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(result, Err(RepositoryError::Conflict)));
        Ok(())
    }

    #[tokio::test]
    async fn delete_chunks_removes_the_recorded_rows() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxUploadRepository::new(&mut transaction);
        let created = repository.create(upload(0, 1, 8, &[], false)?).await?;
        let mut aggregate = repository
            .search(&UploadFilter {
                upload_id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?
            .into_iter()
            .next()
            .ok_or("the upload must still exist")?;
        aggregate.record_chunk(0)?;
        aggregate.record_chunk(1)?;
        repository.save(aggregate).await?;

        // Act
        let removed = repository.delete_chunks(created.upload_id()).await?;
        let found = repository
            .search(&UploadFilter {
                upload_id: Some(created.upload_id()),
                ..UploadFilter::default()
            })
            .await?;

        // Assert
        let persisted = found.first().ok_or("the upload must still exist")?;
        assert!(removed);
        assert!(!persisted.chunk_bitmap().is_received(0)?);
        assert!(!persisted.chunk_bitmap().is_received(1)?);
        Ok(())
    }

    #[tokio::test]
    async fn search_filters_created_at_for_pending_unsealed_uploads() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let (expired_id, pending_id) = {
            let mut repository = SqlxUploadRepository::new(&mut transaction);
            let expired = repository.create(upload(0, 1, 8, &[], false)?).await?;
            let pending = repository.create(upload(0, 1, 8, &[], false)?).await?;
            (expired.upload_id(), pending.upload_id())
        };
        let old = chrono::Utc::now().naive_utc() - chrono::Duration::hours(2);
        sqlx::query("UPDATE upload SET created_at = ?1 WHERE upload_id = ?2")
            .bind(old)
            .bind(expired_id)
            .execute(&mut *transaction)
            .await?;
        let mut repository = SqlxUploadRepository::new(&mut transaction);
        let mut pending = repository
            .search(&UploadFilter {
                upload_id: Some(pending_id),
                ..UploadFilter::default()
            })
            .await?
            .into_iter()
            .next()
            .ok_or("the pending upload must still exist")?;
        pending.seal(chrono::Utc::now().naive_utc());
        repository.save(pending).await?;

        // Act
        let found = repository
            .search(&UploadFilter {
                created_at: Some(chrono::Utc::now().naive_utc() - chrono::Duration::hours(1)),
                is_finished: Some(false),
                is_verifying: Some(false),
                ..UploadFilter::default()
            })
            .await?;

        // Assert: only the pending, unsealed upload is past its expiry.
        assert_eq!(found.len(), 1);
        assert_eq!(found.first().map(Upload::upload_id), Some(expired_id));
        Ok(())
    }

    #[tokio::test]
    async fn search_filters_sealed_at_for_stale_seals() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxUploadRepository::new(&mut transaction);
        let stale = repository.create(upload(0, 1, 8, &[], false)?).await?;
        let fresh = repository.create(upload(0, 1, 8, &[], false)?).await?;
        let old = chrono::Utc::now().naive_utc() - chrono::Duration::hours(1);
        let mut stale_aggregate = repository
            .search(&UploadFilter {
                upload_id: Some(stale.upload_id()),
                ..UploadFilter::default()
            })
            .await?
            .into_iter()
            .next()
            .ok_or("the stale upload must still exist")?;
        stale_aggregate.seal(old);
        repository.save(stale_aggregate).await?;
        let mut fresh_aggregate = repository
            .search(&UploadFilter {
                upload_id: Some(fresh.upload_id()),
                ..UploadFilter::default()
            })
            .await?
            .into_iter()
            .next()
            .ok_or("the fresh upload must still exist")?;
        fresh_aggregate.seal(chrono::Utc::now().naive_utc());
        repository.save(fresh_aggregate).await?;

        // Act
        let found = repository
            .search(&UploadFilter {
                sealed_at: Some(chrono::Utc::now().naive_utc() - chrono::Duration::minutes(5)),
                is_finished: Some(false),
                ..UploadFilter::default()
            })
            .await?;

        // Assert
        assert_eq!(found.len(), 1);
        assert_eq!(
            found.first().map(Upload::upload_id),
            Some(stale.upload_id())
        );
        Ok(())
    }
}
