use sqlx::{QueryBuilder, Sqlite, Transaction};

use crate::domain::model::Credential;
use crate::domain::port::{CredentialFilter, CredentialRepository, RepositoryError};

use super::model::Credential as SqlxCredential;

/// Repository persisting credentials, backed by a `SQLite` transaction.
pub(crate) struct SqlxCredentialRepository<'transaction> {
    /// Transaction the repository reads from and writes to.
    transaction: &'transaction mut Transaction<'static, Sqlite>,
}

impl<'transaction> SqlxCredentialRepository<'transaction> {
    /// Create a new repository bound to `transaction`.
    #[must_use]
    pub(crate) fn new(transaction: &'transaction mut Transaction<'static, Sqlite>) -> Self {
        Self { transaction }
    }
}

impl CredentialRepository for SqlxCredentialRepository<'_> {
    async fn create(&mut self, credential: Credential) -> Result<Credential, RepositoryError> {
        let row = sqlx::query_as::<_, SqlxCredential>(
            "INSERT INTO credential (password_hash, updated_at)
             VALUES (?1, ?2)
             RETURNING credential_id, password_hash, updated_at",
        )
        .bind(credential.password_hash())
        .bind(credential.updated_at())
        .fetch_one(&mut **self.transaction)
        .await?;

        domain_credential(row)
    }

    async fn delete(&mut self, filter: &CredentialFilter) -> Result<bool, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new("DELETE FROM credential");
        if let Some(id) = filter.id {
            builder.push(" WHERE credential_id = ");
            builder.push_bind(id);
        }

        let result = builder.build().execute(&mut **self.transaction).await?;
        Ok(result.rows_affected() > 0)
    }

    async fn save(&mut self, credential: Credential) -> Result<Credential, RepositoryError> {
        let row = sqlx::query_as::<_, SqlxCredential>(
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
        .fetch_one(&mut **self.transaction)
        .await?;

        domain_credential(row)
    }

    async fn search(
        &mut self,
        filter: &CredentialFilter,
    ) -> Result<Vec<Credential>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT credential_id, password_hash, updated_at FROM credential",
        );
        if let Some(id) = filter.id {
            builder.push(" WHERE credential_id = ");
            builder.push_bind(id);
        }

        let rows = builder
            .build_query_as::<SqlxCredential>()
            .fetch_all(&mut **self.transaction)
            .await?;

        rows.into_iter().map(domain_credential).collect()
    }
}

/// Map a persisted credential row back to the domain model.
fn domain_credential(row: SqlxCredential) -> Result<Credential, RepositoryError> {
    Credential::try_new(row.credential_id, row.password_hash, row.updated_at)
        .map_err(|_| RepositoryError::DataIntegrityViolation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::CredentialError;
    use crate::domain::port::PasswordHasher as _;
    use crate::infrastructure::outbound::argon2::Argon2PasswordHasher;

    use crate::infrastructure::outbound::sqlx::test_helpers::begin_transaction;
    use allure_cargotest::allure_test;
    use chrono::NaiveDateTime;
    use std::error::Error;

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

    async fn seed_credential(
        transaction: &mut Transaction<'static, Sqlite>,
        id: i64,
        password_hash: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO credential (credential_id, password_hash) VALUES (?1, ?2)")
            .bind(id)
            .bind(password_hash)
            .execute(&mut **transaction)
            .await?;
        Ok(())
    }

    #[tokio::test]
    async fn save_then_search_round_trips_credential() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let expected = credential(5, "hash-alice", updated_at("2026-01-01 12:00:00")?)?;

        // Act
        let mut repository = SqlxCredentialRepository::new(&mut transaction);
        let persisted = repository.save(expected.clone()).await?;
        let found = repository
            .search(&CredentialFilter {
                id: Some(5),
                ..CredentialFilter::default()
            })
            .await?;

        // Assert
        assert_eq!(persisted.id(), 5);
        assert_eq!(persisted.password_hash(), "hash-alice");
        assert_eq!(persisted.updated_at(), expected.updated_at());
        assert_eq!(found.first(), Some(&expected));
        Ok(())
    }

    #[tokio::test]
    async fn save_explicit_id_updates_existing_credential() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let first = credential(5, "hash-old", updated_at("2026-01-01 12:00:00")?)?;
        let second = credential(5, "hash-new", updated_at("2026-02-01 12:00:00")?)?;

        // Act
        let mut repository = SqlxCredentialRepository::new(&mut transaction);
        repository.save(first).await?;
        let persisted = repository.save(second.clone()).await?;
        let found = repository
            .search(&CredentialFilter {
                id: Some(5),
                ..CredentialFilter::default()
            })
            .await?;

        // Assert
        assert_eq!(persisted.password_hash(), "hash-new");
        assert_eq!(found.first(), Some(&second));
        Ok(())
    }

    #[tokio::test]
    async fn create_assigns_final_identifier() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let pending = credential(0, "hash-zero", updated_at("2026-03-01 12:00:00")?)?;

        // Act
        let mut repository = SqlxCredentialRepository::new(&mut transaction);
        let persisted = repository.create(pending).await?;
        let found = repository
            .search(&CredentialFilter {
                id: Some(persisted.id()),
                ..CredentialFilter::default()
            })
            .await?;

        // Assert
        assert_ne!(
            persisted.id(),
            0,
            "a new credential must receive a real identifier"
        );
        assert_eq!(found.first(), Some(&persisted));
        Ok(())
    }

    #[tokio::test]
    async fn save_duplicate_password_hash_returns_already_exist() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let first = credential(1, "shared-hash", updated_at("2026-01-01 12:00:00")?)?;
        let second = credential(2, "shared-hash", updated_at("2026-01-01 12:00:00")?)?;

        // Act
        let mut repository = SqlxCredentialRepository::new(&mut transaction);
        repository.save(first).await?;
        let result = repository.save(second).await;

        // Assert
        assert!(matches!(result, Err(RepositoryError::AlreadyExist)));
        Ok(())
    }

    #[tokio::test]
    async fn search_missing_credential_returns_empty() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;

        // Act
        let mut repository = SqlxCredentialRepository::new(&mut transaction);
        let found = repository
            .search(&CredentialFilter {
                id: Some(404),
                ..CredentialFilter::default()
            })
            .await?;

        // Assert
        assert!(
            found.is_empty(),
            "a missing credential must not be an error"
        );
        Ok(())
    }

    #[tokio::test]
    async fn delete_missing_credential_returns_false() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;

        // Act
        let mut repository = SqlxCredentialRepository::new(&mut transaction);
        let deleted = repository
            .delete(&CredentialFilter {
                id: Some(404),
                ..CredentialFilter::default()
            })
            .await?;

        // Assert
        assert!(!deleted);
        Ok(())
    }

    #[tokio::test]
    async fn delete_existing_credential_returns_true() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_credential(&mut transaction, 7, "hash-delete").await?;

        // Act
        let mut repository = SqlxCredentialRepository::new(&mut transaction);
        let deleted = repository
            .delete(&CredentialFilter {
                id: Some(7),
                ..CredentialFilter::default()
            })
            .await?;

        // Assert
        assert!(deleted);
        Ok(())
    }

    #[tokio::test]
    async fn delete_referenced_credential_returns_data_integrity_violation()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_credential(&mut transaction, 1, "hash-referenced").await?;
        sqlx::query(
            "INSERT INTO user (user_id, role, username, credential_id)
             VALUES (1, 'STANDARD', 'alice', 1)",
        )
        .execute(&mut *transaction)
        .await?;

        // Act
        let mut repository = SqlxCredentialRepository::new(&mut transaction);
        let result = repository
            .delete(&CredentialFilter {
                id: Some(1),
                ..CredentialFilter::default()
            })
            .await;

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
        let mut transaction = begin_transaction().await?;

        // Act
        let result =
            sqlx::query("INSERT INTO credential (credential_id, password_hash) VALUES (99, NULL)")
                .execute(&mut *transaction)
                .await;

        // Assert
        let mapped = result.map_err(RepositoryError::from);
        assert!(matches!(
            mapped,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn creation_and_rotation_store_only_a_password_hash() -> Result<(), Box<dyn Error>> {
        allure.epic("Authentication");
        allure.feature("REQ-AUTHENTICATION-006");
        allure.story("SCN-AUTHENTICATION-006-A");

        // Arrange: the same hasher the creation and rotation use cases hash with.
        let hasher = Argon2PasswordHasher::new(vec![7; 32]);
        let pending_password = "C0rrect!Horse";
        let replacement_password = "N3w!Secret";
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxCredentialRepository::new(&mut transaction);

        // Act: creation stores the hash of the first password ...
        let created = repository
            .create(credential(
                0,
                &hasher.hash_password(pending_password).await?,
                updated_at("2026-01-01 12:00:00")?,
            )?)
            .await?;
        // ... and rotation replaces it with the hash of the second.
        let rotated = repository
            .save(credential(
                created.id(),
                &hasher.hash_password(replacement_password).await?,
                updated_at("2026-02-01 12:00:00")?,
            )?)
            .await?;

        // Assert
        assert_ne!(created.password_hash(), pending_password);
        assert_ne!(rotated.password_hash(), replacement_password);
        assert!(
            hasher
                .verify_password(replacement_password, rotated.password_hash())
                .await?
        );
        Ok(())
    }

    #[tokio::test]
    async fn insert_without_updated_at_uses_current_timestamp() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;

        // Act
        sqlx::query("INSERT INTO credential (credential_id, password_hash) VALUES (60, 'hash-ts')")
            .execute(&mut *transaction)
            .await?;
        let mut repository = SqlxCredentialRepository::new(&mut transaction);
        let found = repository
            .search(&CredentialFilter {
                id: Some(60),
                ..CredentialFilter::default()
            })
            .await?;

        // Assert
        let credential = found
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("expected the seeded credential"))?;
        assert!(
            credential.updated_at()
                < NaiveDateTime::parse_from_str("2030-01-01 00:00:00", "%F %T")?,
            "the default timestamp must be applied by the database"
        );
        Ok(())
    }
}
