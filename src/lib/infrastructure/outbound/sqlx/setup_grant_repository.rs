use chrono::NaiveDateTime;
use sqlx::{Sqlite, Transaction};

use crate::domain::model::{SETUP_GRANT_ID, SetupGrant};
use crate::domain::port::{RepositoryError, SetupGrantRepository};

use super::model::SetupGrant as SqlxSetupGrant;

/// Repository persisting the single Root Admin setup grant, backed by a
/// `SQLite` transaction.
pub(crate) struct SqlxSetupGrantRepository<'transaction> {
    /// Transaction the repository reads from and writes to.
    transaction: &'transaction mut Transaction<'static, Sqlite>,
}

impl<'transaction> SqlxSetupGrantRepository<'transaction> {
    /// Create a new repository bound to `transaction`.
    #[must_use]
    pub(crate) fn new(transaction: &'transaction mut Transaction<'static, Sqlite>) -> Self {
        Self { transaction }
    }
}

impl SetupGrantRepository for SqlxSetupGrantRepository<'_> {
    async fn consume(&mut self, now: NaiveDateTime) -> Result<bool, RepositoryError> {
        let result = sqlx::query(
            "UPDATE root_admin_setup SET consumed_at = ?1
             WHERE singleton = ?2 AND consumed_at IS NULL AND expires_at > ?1",
        )
        .bind(now)
        .bind(SETUP_GRANT_ID)
        .execute(&mut **self.transaction)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn create(&mut self, grant: SetupGrant) -> Result<SetupGrant, RepositoryError> {
        let row = sqlx::query_as::<_, SqlxSetupGrant>(
            "INSERT INTO root_admin_setup (nonce, expires_at, consumed_at)
             VALUES (?1, ?2, ?3)
             RETURNING root_admin_setup_id, nonce, expires_at, consumed_at",
        )
        .bind(grant.nonce())
        .bind(grant.expires_at())
        .bind(grant.consumed_at())
        .fetch_one(&mut **self.transaction)
        .await?;

        domain_setup_grant(row)
    }

    async fn find(&mut self) -> Result<Option<SetupGrant>, RepositoryError> {
        let row = sqlx::query_as::<_, SqlxSetupGrant>(
            "SELECT root_admin_setup_id, nonce, expires_at, consumed_at
             FROM root_admin_setup WHERE singleton = ?1",
        )
        .bind(SETUP_GRANT_ID)
        .fetch_optional(&mut **self.transaction)
        .await?;

        row.map(domain_setup_grant).transpose()
    }

    async fn save(&mut self, grant: SetupGrant) -> Result<SetupGrant, RepositoryError> {
        let row = sqlx::query_as::<_, SqlxSetupGrant>(
            "INSERT INTO root_admin_setup (root_admin_setup_id, singleton, nonce, expires_at, consumed_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (root_admin_setup_id) DO UPDATE SET
                 nonce = excluded.nonce,
                 expires_at = excluded.expires_at,
                 consumed_at = excluded.consumed_at
             RETURNING root_admin_setup_id, nonce, expires_at, consumed_at",
        )
        .bind(grant.id())
        .bind(SETUP_GRANT_ID)
        .bind(grant.nonce())
        .bind(grant.expires_at())
        .bind(grant.consumed_at())
        .fetch_one(&mut **self.transaction)
        .await?;

        domain_setup_grant(row)
    }
}

/// Map a persisted setup-grant row back to the domain model.
fn domain_setup_grant(row: SqlxSetupGrant) -> Result<SetupGrant, RepositoryError> {
    SetupGrant::from_stored(
        row.root_admin_setup_id,
        row.nonce,
        row.expires_at,
        row.consumed_at,
    )
    .map_err(|_| RepositoryError::DataIntegrityViolation)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::domain::port::{
        IdentityUnitOfWork as _, UnitOfWork as _, UnitOfWorkFactory as _, UserUnitOfWork as _,
    };
    use crate::infrastructure::outbound::sqlx::{
        SqlxUnitOfWorkFactory,
        test_helpers::{begin_transaction, disk_pool, in_memory_pool, seed_setup_grant},
    };

    use chrono::Utc;
    use std::error::Error;
    use std::sync::Arc;
    use tokio::sync::Barrier;

    fn nonce(seed: char) -> String {
        seed.to_string().repeat(32)
    }

    fn expiry(seconds: i64) -> NaiveDateTime {
        Utc::now()
            .naive_utc()
            .checked_add_signed(chrono::Duration::seconds(seconds))
            .unwrap_or_default()
    }

    #[tokio::test]
    async fn create_then_find_round_trips_a_pending_grant() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let expected = SetupGrant::try_new(nonce('a'), expiry(60), Utc::now().naive_utc())?;

        // Act
        let mut repository = SqlxSetupGrantRepository::new(&mut transaction);
        let stored = repository.create(expected).await?;
        let found = repository.find().await?;

        // Assert
        assert_eq!(stored.nonce(), nonce('a'));
        assert_eq!(found, Some(stored));
        Ok(())
    }

    #[tokio::test]
    async fn save_replaces_the_nonce_and_expiry() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let now = Utc::now().naive_utc();
        let first = SetupGrant::try_new(nonce('a'), expiry(60), now)?;

        // Act
        let mut repository = SqlxSetupGrantRepository::new(&mut transaction);
        let created = repository.create(first).await?;
        let second = created.renew(nonce('b'), expiry(120), now)?;
        let stored = repository.save(second.clone()).await?;
        let found = repository.find().await?;

        // Assert
        assert_eq!(stored, second);
        assert_eq!(found, Some(second));
        Ok(())
    }

    #[tokio::test]
    async fn find_without_a_row_returns_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;

        // Act
        let mut repository = SqlxSetupGrantRepository::new(&mut transaction);
        let found = repository.find().await?;

        // Assert
        assert_eq!(found, None);
        Ok(())
    }

    #[tokio::test]
    async fn consume_an_absent_grant_affects_no_row() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;

        // Act
        let mut repository = SqlxSetupGrantRepository::new(&mut transaction);
        let consumed = repository.consume(Utc::now().naive_utc()).await?;

        // Assert
        assert!(!consumed);
        Ok(())
    }

    #[tokio::test]
    async fn consume_an_expired_grant_affects_no_row() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_setup_grant(&mut transaction, &nonce('a'), expiry(-60), None).await?;

        // Act
        let mut repository = SqlxSetupGrantRepository::new(&mut transaction);
        let consumed = repository.consume(Utc::now().naive_utc()).await?;

        // Assert
        assert!(!consumed);
        let found = repository.find().await?;
        assert_eq!(
            found.and_then(|grant| grant.consumed_at()),
            None,
            "an expired grant must stay unconsumed"
        );
        Ok(())
    }

    #[tokio::test]
    async fn consume_a_consumed_grant_affects_no_row() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let consumed_at = Utc::now().naive_utc();
        seed_setup_grant(&mut transaction, &nonce('a'), expiry(60), Some(consumed_at)).await?;

        // Act
        let mut repository = SqlxSetupGrantRepository::new(&mut transaction);
        let consumed = repository.consume(Utc::now().naive_utc()).await?;

        // Assert
        assert!(!consumed);
        Ok(())
    }

    #[tokio::test]
    async fn consume_a_live_grant_sets_its_consumption_instant() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_setup_grant(&mut transaction, &nonce('a'), expiry(60), None).await?;

        // Act
        let now = Utc::now().naive_utc();
        let mut repository = SqlxSetupGrantRepository::new(&mut transaction);
        let consumed = repository.consume(now).await?;
        let found = repository.find().await?;

        // Assert
        assert!(consumed);
        assert_eq!(found.and_then(|grant| grant.consumed_at()), Some(now));
        Ok(())
    }

    // proves: SCN-AUTHENTICATION-019-A — Business rule
    #[tokio::test]
    async fn the_stored_grant_holds_no_token_material() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let grant = SetupGrant::try_new(nonce('a'), expiry(60), Utc::now().naive_utc())?;

        // Act
        let mut repository = SqlxSetupGrantRepository::new(&mut transaction);
        let stored = repository.create(grant).await?;
        let columns: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_table_info('root_admin_setup')")
                .fetch_all(&mut *transaction)
                .await?;
        let nonce_value: String =
            sqlx::query_scalar("SELECT nonce FROM root_admin_setup WHERE singleton = 0")
                .fetch_one(&mut *transaction)
                .await?;

        // Assert
        assert_eq!(
            columns,
            vec![
                "root_admin_setup_id".to_owned(),
                "singleton".to_owned(),
                "nonce".to_owned(),
                "expires_at".to_owned(),
                "consumed_at".to_owned(),
            ],
            "the grant carries no token column"
        );
        assert_eq!(nonce_value, stored.nonce());
        assert!(
            !columns.iter().any(|column| column.contains("token")),
            "no stored column may name a token"
        );
        Ok(())
    }

    #[tokio::test]
    async fn concurrent_consume_lets_exactly_one_submission_win() -> Result<(), Box<dyn Error>> {
        // Arrange
        let (_tmp, pool) = disk_pool(2).await?;
        let mut seeding = pool.begin().await?;
        seed_setup_grant(&mut seeding, &nonce('a'), expiry(60), None).await?;
        seeding.commit().await?;
        let factory = Arc::new(SqlxUnitOfWorkFactory::new(pool));
        let barrier = Arc::new(Barrier::new(2));

        let attempt = |factory: Arc<SqlxUnitOfWorkFactory>, barrier: Arc<Barrier>| async move {
            barrier.wait().await;
            let mut unit_of_work = factory
                .begin_write()
                .await
                .map_err(|error| error.to_string())?;
            let now = Utc::now().naive_utc();
            let consumed = unit_of_work
                .setup_grants()
                .consume(now)
                .await
                .map_err(|error| error.to_string())?;
            unit_of_work
                .commit()
                .await
                .map_err(|error| error.to_string())?;
            Ok::<bool, String>(consumed)
        };

        // Act
        let (first, second) = tokio::join!(
            attempt(Arc::clone(&factory), Arc::clone(&barrier)),
            attempt(Arc::clone(&factory), Arc::clone(&barrier)),
        );

        // Assert
        let successes = [first, second]
            .into_iter()
            .filter(|result| matches!(result, Ok(true)))
            .count();
        assert_eq!(successes, 1, "exactly one concurrent consume must win");
        Ok(())
    }

    #[tokio::test]
    async fn a_rolled_back_identity_write_persists_nothing() -> Result<(), Box<dyn Error>> {
        // Arrange
        use crate::domain::model::{Credential, Role, User};
        use crate::domain::port::{CredentialRepository as _, UserRepository as _};

        let pool = in_memory_pool().await?;
        let factory = SqlxUnitOfWorkFactory::new(pool.clone());
        let mut unit_of_work = factory.begin_write().await?;
        let now = Utc::now().naive_utc();
        let credential = unit_of_work
            .credentials()
            .create(Credential::try_new(0, "hash".to_owned(), now)?)
            .await?;
        unit_of_work
            .users()
            .save(User::try_new(
                0,
                "root".to_owned(),
                None,
                credential.id(),
                Role::Admin,
            )?)
            .await?;
        unit_of_work
            .setup_grants()
            .create(SetupGrant::try_new(
                "a".repeat(32),
                now.checked_add_signed(chrono::Duration::seconds(60))
                    .ok_or("the test expiry is not representable")?,
                now,
            )?)
            .await?;

        // Act
        unit_of_work.rollback().await?;

        // Assert
        let credentials: i64 = sqlx::query_scalar("SELECT count(*) FROM credential")
            .fetch_one(&pool)
            .await?;
        let users: i64 = sqlx::query_scalar("SELECT count(*) FROM user")
            .fetch_one(&pool)
            .await?;
        let grants: i64 = sqlx::query_scalar("SELECT count(*) FROM root_admin_setup")
            .fetch_one(&pool)
            .await?;
        assert_eq!(credentials, 0, "the credential must be rolled back");
        assert_eq!(users, 0, "the user must be rolled back");
        assert_eq!(grants, 0, "the setup grant must be rolled back");
        Ok(())
    }
}
