use sqlx::QueryBuilder;
use sqlx::Sqlite;
use sqlx::Transaction;

use crate::domain::model::file::File;
use crate::domain::model::integrity_hash::IntegrityHash;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::file_repository::FileFilter;
use crate::domain::port::file_repository::FileRepository;

use super::model::file::File as SqlxFile;

/// Repository persisting files, backed by a `SQLite` transaction.
pub struct SqlxFileRepository<'transaction> {
    /// Transaction the repository reads from and writes to.
    transaction: &'transaction mut Transaction<'static, Sqlite>,
}

impl<'transaction> SqlxFileRepository<'transaction> {
    /// Create a new repository bound to `transaction`.
    #[must_use]
    pub fn new(transaction: &'transaction mut Transaction<'static, Sqlite>) -> Self {
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
                integrity_hash
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            RETURNING
                file_id,
                path,
                user_id,
                is_public,
                mime_type_id,
                uploaded_at,
                integrity_hash",
        )
        .bind(file.path())
        .bind(file.user_id())
        .bind(file.is_public())
        .bind(file.mime_type_id())
        .bind(file.uploaded_at())
        .bind(file.integrity_hash().as_str())
        .fetch_one(&mut **self.transaction)
        .await?;

        domain_file(row)
    }

    async fn search(&mut self, filter: &FileFilter) -> Result<Vec<File>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT
                file_id,
                path,
                user_id,
                is_public,
                mime_type_id,
                uploaded_at,
                integrity_hash
            FROM file",
        );
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
        if let Some(user_id) = filter.user_id {
            push_filter(&mut first, &mut builder, "user_id = ", user_id);
        }

        let rows = builder
            .build_query_as::<SqlxFile>()
            .fetch_all(&mut **self.transaction)
            .await?;

        rows.into_iter().map(domain_file).collect()
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
    let integrity_hash = IntegrityHash::try_new(row.integrity_hash)
        .map_err(|_| RepositoryError::DataIntegrityViolation)?;
    File::try_new(
        row.file_id,
        row.path,
        row.user_id,
        row.is_public,
        row.mime_type_id,
        row.uploaded_at,
        integrity_hash,
    )
    .map_err(|_| RepositoryError::DataIntegrityViolation)
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use chrono::NaiveDate;
    use rstest::rstest;
    use sqlx::Sqlite;
    use sqlx::Transaction;
    use sqlx::sqlite::SqlitePoolOptions;

    use crate::domain::model::file::File;
    use crate::domain::model::integrity_hash::IntegrityHash;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::file_repository::FileFilter;
    use crate::domain::port::file_repository::FileRepository as _;

    use super::SqlxFileRepository;

    /// A 64-character hexadecimal digest, valid for `integrity_hash`.
    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

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
            IntegrityHash::try_new(integrity_hash.to_owned())?,
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
        assert_eq!(persisted.integrity_hash().as_str(), DIGEST);
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
        assert_eq!(stored.integrity_hash().as_str(), DIGEST);
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
            IntegrityHash::try_new(DIGEST.to_owned())?,
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

    #[rstest]
    #[case::integrity_hash_length(
        "INSERT INTO file (path, user_id, is_public, mime_type_id, integrity_hash)
         VALUES ('files/short.mp4', 1, 0, 'video/mp4', 'too-short')"
    )]
    #[case::is_public_out_of_range(
        "INSERT INTO file (path, user_id, is_public, mime_type_id, integrity_hash)
         VALUES ('files/public.mp4', 1, 2, 'video/mp4', '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef')"
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
}
