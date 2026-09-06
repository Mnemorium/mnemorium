use sqlx::SqlitePool;

use crate::domain::alias::NumericID;
use crate::domain::model::credential::Credential;
use crate::domain::port::credential_repository::CredentialRepository;
use crate::domain::port::error::RepositoryError;

use super::model::credential::Credential as SqlxCredential;

/// Repository persisting credentials, backed by `SQLite`.
pub struct SqlxCredentialRepository {
    /// Connection pool to the `SQLite` database.
    pool: SqlitePool,
}

impl SqlxCredentialRepository {
    /// Create a new repository bound to `pool`.
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl CredentialRepository for SqlxCredentialRepository {
    async fn delete(&self, id: NumericID) -> Result<bool, RepositoryError> {
        let result = sqlx::query("DELETE FROM credential WHERE credential_id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn find(&self, id: NumericID) -> Result<Option<Credential>, RepositoryError> {
        let row = sqlx::query_as::<_, SqlxCredential>(
            "SELECT credential_id, password_hash, updated_at
             FROM credential
             WHERE credential_id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(domain_credential).transpose()
    }

    async fn save(&self, credential: Credential) -> Result<Credential, RepositoryError> {
        let row = if credential.id() == 0 {
            sqlx::query_as::<_, SqlxCredential>(
                "INSERT INTO credential (password_hash, updated_at)
                 VALUES (?1, ?2)
                 RETURNING credential_id, password_hash, updated_at",
            )
            .bind(credential.password_hash())
            .bind(credential.updated_at())
            .fetch_one(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, SqlxCredential>(
                "INSERT INTO credential (credential_id, password_hash, updated_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT (credential_id) DO UPDATE SET
                     password_hash = excluded.password_hash,
                     updated_at = excluded.updated_at
                 RETURNING credential_id, password_hash, updated_at",
            )
            .bind(credential.id())
            .bind(credential.password_hash())
            .bind(credential.updated_at())
            .fetch_one(&self.pool)
            .await?
        };

        domain_credential(row)
    }
}

/// Map a persisted credential row back to the domain model.
fn domain_credential(row: SqlxCredential) -> Result<Credential, RepositoryError> {
    Credential::try_new(row.credential_id, row.password_hash, row.updated_at)
        .map_err(|_| RepositoryError::DataIntegrityViolation)
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use chrono::NaiveDateTime;
    use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};

    use crate::domain::model::credential::Credential;
    use crate::domain::model::credential::CredentialError;
    use crate::domain::port::credential_repository::CredentialRepository as _;
    use crate::domain::port::error::RepositoryError;

    use super::SqlxCredentialRepository;

    fn updated_at(value: &str) -> Result<NaiveDateTime, Box<dyn Error>> {
        Ok(NaiveDateTime::parse_from_str(value, "%F %T")?)
    }

    fn credential(
        id: i64,
        password_hash: &str,
        updated_at: NaiveDateTime,
    ) -> Result<Credential, CredentialError> {
        Credential::try_new(id, password_hash.to_owned(), updated_at)
    }

    async fn repo() -> Result<SqlxCredentialRepository, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(SqlxCredentialRepository::new(pool))
    }

    async fn seed_credential(
        pool: &SqlitePool,
        id: i64,
        password_hash: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO credential (credential_id, password_hash) VALUES (?1, ?2)")
            .bind(id)
            .bind(password_hash)
            .execute(pool)
            .await?;
        Ok(())
    }

    #[tokio::test]
    async fn save_then_find_round_trips_credential() -> Result<(), Box<dyn Error>> {
        // Arrange
        let repository = repo().await?;
        let expected = credential(5, "hash-alice", updated_at("2026-01-01 12:00:00")?)?;

        // Act
        let persisted = repository.save(expected.clone()).await?;
        let found = repository.find(5).await?;

        // Assert
        assert_eq!(persisted.id(), 5);
        assert_eq!(persisted.password_hash(), "hash-alice");
        assert_eq!(persisted.updated_at(), expected.updated_at());
        assert_eq!(found.as_ref(), Some(&expected));
        Ok(())
    }

    #[tokio::test]
    async fn save_explicit_id_updates_existing_credential() -> Result<(), Box<dyn Error>> {
        // Arrange
        let repository = repo().await?;
        let first = credential(5, "hash-old", updated_at("2026-01-01 12:00:00")?)?;
        let second = credential(5, "hash-new", updated_at("2026-02-01 12:00:00")?)?;

        // Act
        repository.save(first).await?;
        let persisted = repository.save(second.clone()).await?;
        let found = repository.find(5).await?;

        // Assert
        assert_eq!(persisted.password_hash(), "hash-new");
        assert_eq!(found.as_ref(), Some(&second));
        Ok(())
    }

    #[tokio::test]
    async fn save_with_zero_id_assigns_final_identifier() -> Result<(), Box<dyn Error>> {
        // Arrange
        let repository = repo().await?;
        let pending = credential(0, "hash-zero", updated_at("2026-03-01 12:00:00")?)?;

        // Act
        let persisted = repository.save(pending).await?;
        let found = repository.find(persisted.id()).await?;

        // Assert
        assert_ne!(
            persisted.id(),
            0,
            "a new credential must receive a real identifier"
        );
        assert_eq!(found.as_ref(), Some(&persisted));
        Ok(())
    }

    #[tokio::test]
    async fn save_duplicate_password_hash_returns_already_exist() -> Result<(), Box<dyn Error>> {
        // Arrange
        let repository = repo().await?;
        let first = credential(1, "shared-hash", updated_at("2026-01-01 12:00:00")?)?;
        let second = credential(2, "shared-hash", updated_at("2026-01-01 12:00:00")?)?;

        // Act
        repository.save(first).await?;
        let result = repository.save(second).await;

        // Assert
        assert!(matches!(result, Err(RepositoryError::AlreadyExist)));
        Ok(())
    }

    #[tokio::test]
    async fn find_missing_credential_returns_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let repository = repo().await?;

        // Act
        let found = repository.find(404).await?;

        // Assert
        assert!(found.is_none(), "a missing credential must not be an error");
        Ok(())
    }

    #[tokio::test]
    async fn delete_missing_credential_returns_false() -> Result<(), Box<dyn Error>> {
        // Arrange
        let repository = repo().await?;

        // Act
        let deleted = repository.delete(404).await?;

        // Assert
        assert!(!deleted);
        Ok(())
    }

    #[tokio::test]
    async fn delete_existing_credential_returns_true() -> Result<(), Box<dyn Error>> {
        // Arrange
        let repository = repo().await?;
        seed_credential(&repository.pool, 7, "hash-delete").await?;

        // Act
        let deleted = repository.delete(7).await?;

        // Assert
        assert!(deleted);
        Ok(())
    }

    #[tokio::test]
    async fn delete_referenced_credential_returns_data_integrity_violation()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let repository = repo().await?;
        seed_credential(&repository.pool, 1, "hash-referenced").await?;
        sqlx::query(
            "INSERT INTO user (user_id, role, username, credential_id)
             VALUES (1, 'STANDARD', 'alice', 1)",
        )
        .execute(&repository.pool)
        .await?;

        // Act
        let result = repository.delete(1).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn insert_null_password_hash_returns_data_integrity_violation()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let repository = repo().await?;

        // Act
        let result =
            sqlx::query("INSERT INTO credential (credential_id, password_hash) VALUES (99, NULL)")
                .execute(&repository.pool)
                .await;

        // Assert
        let mapped = result.map_err(RepositoryError::from);
        assert!(matches!(
            mapped,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn insert_without_updated_at_uses_current_timestamp() -> Result<(), Box<dyn Error>> {
        // Arrange
        let repository = repo().await?;

        // Act
        sqlx::query("INSERT INTO credential (credential_id, password_hash) VALUES (60, 'hash-ts')")
            .execute(&repository.pool)
            .await?;
        let found = repository.find(60).await?;

        // Assert
        let credential = found.ok_or_else(|| anyhow::anyhow!("expected the seeded credential"))?;
        assert!(
            credential.updated_at()
                < NaiveDateTime::parse_from_str("2030-01-01 00:00:00", "%F %T")?,
            "the default timestamp must be applied by the database"
        );
        Ok(())
    }
}
