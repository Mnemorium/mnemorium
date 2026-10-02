use sqlx::Sqlite;
use sqlx::Transaction;

use crate::domain::port::error::RepositoryError;
use crate::domain::port::mime_type_repository::MimeTypeRepository;

/// Repository querying mime types, backed by a `SQLite` transaction.
pub struct SqlxMimeTypeRepository<'transaction> {
    /// Transaction the repository reads from.
    transaction: &'transaction mut Transaction<'static, Sqlite>,
}

impl<'transaction> SqlxMimeTypeRepository<'transaction> {
    /// Create a new repository bound to `transaction`.
    #[must_use]
    pub fn new(transaction: &'transaction mut Transaction<'static, Sqlite>) -> Self {
        Self { transaction }
    }
}

impl MimeTypeRepository for SqlxMimeTypeRepository<'_> {
    async fn exists(&mut self, mime_type_id: &str) -> Result<bool, RepositoryError> {
        let count =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM mime_type WHERE mime_type_id = ?")
                .bind(mime_type_id)
                .fetch_one(&mut **self.transaction)
                .await?;

        Ok(count > 0)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use sqlx::Sqlite;
    use sqlx::Transaction;
    use sqlx::sqlite::SqlitePoolOptions;

    use crate::domain::port::mime_type_repository::MimeTypeRepository as _;

    use super::SqlxMimeTypeRepository;

    async fn begin_transaction() -> Result<Transaction<'static, Sqlite>, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        pool.begin().await
    }

    #[tokio::test]
    async fn exists_returns_true_for_a_seeded_mime_type() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxMimeTypeRepository::new(&mut transaction);

        // Act
        let exists = repository.exists("video/mp4").await?;

        // Assert
        assert!(exists);
        Ok(())
    }

    #[tokio::test]
    async fn exists_returns_false_for_an_unknown_mime_type() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxMimeTypeRepository::new(&mut transaction);

        // Act
        let exists = repository.exists("application/unknown").await?;

        // Assert
        assert!(!exists);
        Ok(())
    }
}
