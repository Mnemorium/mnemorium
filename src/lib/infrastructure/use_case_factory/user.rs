use std::sync::Arc;

use crate::application::port::get_current_user::GetCurrentUserUseCase;
use crate::application::port::get_user::GetUserUseCase;
use crate::application::port::update_user::UpdateUserUseCase;
use crate::application::port::user_use_case_factory::UserUseCaseFactory;
use crate::application::use_case::get_current_user::GetCurrentUser;
use crate::application::use_case::get_user::GetUser;
use crate::application::use_case::update_user::UpdateUser;
use crate::infrastructure::outbound::sqlx::unit_of_work::SqlxUnitOfWorkFactory;

/// Builds the User use cases over the shared unit of work factory.
pub struct RuntimeUserUseCaseFactory {
    /// Factory opening the unit of work wrapping the User use cases.
    unit_of_work_factory: Arc<SqlxUnitOfWorkFactory>,
}

impl RuntimeUserUseCaseFactory {
    /// Create a new factory.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<SqlxUnitOfWorkFactory>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl UserUseCaseFactory for RuntimeUserUseCaseFactory {
    fn get_current_user(&self) -> Arc<dyn GetCurrentUserUseCase> {
        Arc::new(GetCurrentUser::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn get_user(&self) -> Arc<dyn GetUserUseCase> {
        Arc::new(GetUser::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn update_user(&self) -> Arc<dyn UpdateUserUseCase> {
        Arc::new(UpdateUser::new(Arc::clone(&self.unit_of_work_factory)))
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use sqlx::SqlitePool;
    use sqlx::sqlite::SqlitePoolOptions;

    use crate::application::port::get_current_user::GetCurrentUserCommand;
    use crate::application::port::get_user::GetUserCommand;
    use crate::application::port::update_user::UpdateUserCommand;
    use crate::application::port::user_use_case_factory::UserUseCaseFactory as _;
    use crate::domain::model::user::Role;
    use crate::infrastructure::outbound::sqlx::unit_of_work::SqlxUnitOfWorkFactory;

    use super::RuntimeUserUseCaseFactory;

    /// An in-memory database migrated to the current schema, capped at one
    /// connection so a unit of work owns the only connection.
    async fn migrated_pool() -> Result<SqlitePool, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(pool)
    }

    /// Insert a credential row directly into `pool`.
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

    /// Insert a user row directly into `pool`.
    async fn seed_user(
        pool: &SqlitePool,
        id: i64,
        role: &str,
        username: &str,
        credential_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO user (user_id, role, username, email, credential_id)
             VALUES (?1, ?2, ?3, NULL, ?4)",
        )
        .bind(id)
        .bind(role)
        .bind(username)
        .bind(credential_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Build a runtime factory over `pool`.
    fn factory(pool: &SqlitePool) -> RuntimeUserUseCaseFactory {
        RuntimeUserUseCaseFactory::new(Arc::new(SqlxUnitOfWorkFactory::new(pool.clone())))
    }

    #[tokio::test]
    async fn get_current_user_seeded_user_returns_profile() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = migrated_pool().await?;
        seed_credential(&pool, 1, "alice-hash").await?;
        seed_user(&pool, 1, "STANDARD", "alice", 1).await?;
        let factory = factory(&pool);

        // Act
        let response = factory
            .get_current_user()
            .execute(GetCurrentUserCommand::new(1))
            .await?;

        // Assert
        assert_eq!(response.id(), 1);
        assert_eq!(response.username(), "alice");
        assert_eq!(response.role(), Role::Standard);
        Ok(())
    }

    #[tokio::test]
    async fn get_user_seeded_admin_caller_returns_target_profile() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = migrated_pool().await?;
        seed_credential(&pool, 1, "admin-hash").await?;
        seed_credential(&pool, 2, "brad-hash").await?;
        seed_user(&pool, 1, "ADMIN", "admin", 1).await?;
        seed_user(&pool, 2, "STANDARD", "brad", 2).await?;
        let factory = factory(&pool);

        // Act
        let response = factory
            .get_user()
            .execute(GetUserCommand::new(1, 2))
            .await?;

        // Assert
        assert_eq!(response.id(), 2);
        assert_eq!(response.username(), "brad");
        assert_eq!(response.role(), Role::Standard);
        Ok(())
    }

    #[tokio::test]
    async fn update_user_seeded_admin_caller_persists_updated_profile() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let pool = migrated_pool().await?;
        seed_credential(&pool, 1, "admin-hash").await?;
        seed_credential(&pool, 2, "brad-hash").await?;
        seed_user(&pool, 1, "ADMIN", "admin", 1).await?;
        seed_user(&pool, 2, "STANDARD", "brad", 2).await?;
        let factory = factory(&pool);

        // Act
        let response = factory
            .update_user()
            .execute(UpdateUserCommand::new(
                1,
                2,
                Some("robert".to_owned()),
                None,
                None,
            ))
            .await?;

        // Assert
        assert_eq!(response.username(), "robert");
        let persisted: String = sqlx::query_scalar("SELECT username FROM user WHERE user_id = ?1")
            .bind(2i64)
            .fetch_one(&pool)
            .await?;
        assert_eq!(persisted, "robert");
        Ok(())
    }

    #[tokio::test]
    async fn runtime_user_use_case_factory_repeated_accessor_calls_return_distinct_instances()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_lazy("sqlite::memory:")?;
        let factory = factory(&pool);

        // Act
        let first_current = factory.get_current_user();
        let second_current = factory.get_current_user();
        let first_user = factory.get_user();
        let second_user = factory.get_user();
        let first_update = factory.update_user();
        let second_update = factory.update_user();

        // Assert
        assert!(!Arc::ptr_eq(&first_current, &second_current));
        assert!(!Arc::ptr_eq(&first_user, &second_user));
        assert!(!Arc::ptr_eq(&first_update, &second_update));
        Ok(())
    }
}
