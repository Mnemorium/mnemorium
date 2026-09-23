use sqlx::QueryBuilder;
use sqlx::Sqlite;
use sqlx::Transaction;

use crate::domain::alias::NumericID;
use crate::domain::model::user::Role;
use crate::domain::model::user::User;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository;

use super::model::user::Role as SqlxRole;
use super::model::user::User as SqlxUser;

/// Repository persisting users, backed by a `SQLite` transaction.
pub struct SqlxUserRepository<'transaction> {
    /// Transaction the repository reads from and writes to.
    transaction: &'transaction mut Transaction<'static, Sqlite>,
}

impl<'transaction> SqlxUserRepository<'transaction> {
    /// Create a new repository bound to `transaction`.
    #[must_use]
    pub fn new(transaction: &'transaction mut Transaction<'static, Sqlite>) -> Self {
        Self { transaction }
    }
}

impl UserRepository for SqlxUserRepository<'_> {
    async fn create(&mut self, user: User) -> Result<User, RepositoryError> {
        let row = sqlx::query_as::<_, SqlxUser>(
            "INSERT INTO user (credential_id, email, role, username)
            VALUES (?1, ?2, ?3, ?4)
            RETURNING
                user_id,
                credential_id,
                email,
                role,
                username",
        )
        .bind(user.credential_id())
        .bind(user.email())
        .bind(sqlx_role(user.role()))
        .bind(user.username())
        .fetch_one(&mut **self.transaction)
        .await?;

        domain_user(row)
    }

    async fn delete(&mut self, id: NumericID) -> Result<bool, RepositoryError> {
        let result = sqlx::query("DELETE FROM user WHERE user_id = ?")
            .bind(id)
            .execute(&mut **self.transaction)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn save(&mut self, user: User) -> Result<User, RepositoryError> {
        let row = sqlx::query_as::<_, SqlxUser>(
            "INSERT INTO user (user_id, credential_id, email, role, username)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT (user_id) DO UPDATE SET
                credential_id = excluded.credential_id,
                email = excluded.email,
                role = excluded.role,
                username = excluded.username
            RETURNING
                user_id,
                credential_id,
                email,
                role,
                username",
        )
        .bind(user.id())
        .bind(user.credential_id())
        .bind(user.email())
        .bind(sqlx_role(user.role()))
        .bind(user.username())
        .fetch_one(&mut **self.transaction)
        .await?;

        domain_user(row)
    }

    async fn search(&mut self, filter: &UserFilter) -> Result<Vec<User>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT user_id, credential_id, email, role, username FROM user",
        );
        let mut first = true;

        if let Some(id) = filter.id {
            push_filter(&mut first, &mut builder, "user_id = ", id);
        }
        if let Some(role) = filter.role {
            push_filter(&mut first, &mut builder, "role = ", sqlx_role(role));
        }
        if let Some(username) = filter.username.as_deref() {
            push_filter(&mut first, &mut builder, "username = ", username);
        }
        if let Some(email) = filter.email.as_deref() {
            push_filter(&mut first, &mut builder, "email = ", email);
        }

        let rows = builder
            .build_query_as::<SqlxUser>()
            .fetch_all(&mut **self.transaction)
            .await?;

        rows.into_iter().map(domain_user).collect()
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

/// Map the domain role to its persisted representation.
fn sqlx_role(role: Role) -> SqlxRole {
    match role {
        Role::Admin => SqlxRole::Admin,
        Role::Standard => SqlxRole::Standard,
    }
}

/// Map a persisted user row back to the domain model.
fn domain_user(row: SqlxUser) -> Result<User, RepositoryError> {
    let role = match row.role {
        SqlxRole::Admin => Role::Admin,
        SqlxRole::Standard => Role::Standard,
    };

    User::try_new(
        row.user_id,
        row.username,
        row.email,
        row.credential_id,
        role,
    )
    .map_err(|_| RepositoryError::DataIntegrityViolation)
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use sqlx::Sqlite;
    use sqlx::Transaction;
    use sqlx::sqlite::SqlitePoolOptions;

    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::model::user::UserError;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::user_repository::UserFilter;
    use crate::domain::port::user_repository::UserRepository as _;

    use super::SqlxUserRepository;

    async fn begin_transaction() -> Result<Transaction<'static, Sqlite>, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        pool.begin().await
    }

    fn user(
        id: i64,
        username: &str,
        email: Option<&str>,
        credential_id: i64,
        role: Role,
    ) -> Result<User, UserError> {
        User::try_new(
            id,
            username.to_owned(),
            email.map(str::to_owned),
            credential_id,
            role,
        )
    }

    async fn seed_credential(
        transaction: &mut Transaction<'static, Sqlite>,
        id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO credential (credential_id, password_hash) VALUES (?1, ?2)")
            .bind(id)
            .bind(format!("hash-{id}"))
            .execute(&mut **transaction)
            .await?;
        Ok(())
    }

    #[tokio::test]
    async fn save_then_search_round_trips_user() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_credential(&mut transaction, 1).await?;
        let expected = user(10, "alice", Some("alice@example.com"), 1, Role::Standard)?;

        // Act
        let mut repository = SqlxUserRepository::new(&mut transaction);
        let persisted = repository.save(expected.clone()).await?;
        let found = repository
            .search(&UserFilter {
                id: Some(10),
                ..UserFilter::default()
            })
            .await?;

        // Assert
        assert_eq!(persisted.id(), 10);
        assert_eq!(persisted.username(), "alice");
        assert_eq!(persisted.email(), Some("alice@example.com"));
        assert_eq!(persisted.credential_id(), 1);
        assert_eq!(persisted.role(), Role::Standard);
        assert_eq!(found.len(), 1);
        assert_eq!(found.first(), Some(&expected));
        Ok(())
    }

    #[tokio::test]
    async fn create_assigns_final_identifier_and_round_trips() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_credential(&mut transaction, 1).await?;
        let pending = user(0, "alice", Some("alice@example.com"), 1, Role::Standard)?;

        // Act
        let mut repository = SqlxUserRepository::new(&mut transaction);
        let persisted = repository.create(pending).await?;
        let found = repository
            .search(&UserFilter {
                id: Some(persisted.id()),
                ..UserFilter::default()
            })
            .await?;

        // Assert
        assert_ne!(
            persisted.id(),
            0,
            "a new user must receive a real identifier"
        );
        assert_eq!(persisted.username(), "alice");
        assert_eq!(persisted.email(), Some("alice@example.com"));
        assert_eq!(persisted.credential_id(), 1);
        assert_eq!(persisted.role(), Role::Standard);
        assert_eq!(found.first(), Some(&persisted));
        Ok(())
    }

    #[tokio::test]
    async fn create_duplicate_username_returns_already_exist() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_credential(&mut transaction, 1).await?;
        let mut repository = SqlxUserRepository::new(&mut transaction);
        repository
            .create(user(0, "bobby", None, 1, Role::Standard)?)
            .await?;

        // Act
        let result = repository
            .create(user(0, "bobby", None, 1, Role::Standard)?)
            .await;

        // Assert
        assert!(matches!(result, Err(RepositoryError::AlreadyExist)));
        Ok(())
    }

    #[tokio::test]
    async fn save_duplicate_username_returns_already_exist() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_credential(&mut transaction, 1).await?;
        let mut repository = SqlxUserRepository::new(&mut transaction);
        repository
            .save(user(1, "bobby", None, 1, Role::Standard)?)
            .await?;

        // Act
        let result = repository
            .save(user(2, "bobby", None, 1, Role::Standard)?)
            .await;

        // Assert
        assert!(matches!(result, Err(RepositoryError::AlreadyExist)));
        Ok(())
    }

    #[tokio::test]
    async fn save_duplicate_email_returns_already_exist() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_credential(&mut transaction, 1).await?;
        let mut repository = SqlxUserRepository::new(&mut transaction);
        repository
            .save(user(
                1,
                "carol",
                Some("shared@example.com"),
                1,
                Role::Standard,
            )?)
            .await?;

        // Act
        let result = repository
            .save(user(
                2,
                "david",
                Some("shared@example.com"),
                1,
                Role::Standard,
            )?)
            .await;

        // Assert
        assert!(matches!(result, Err(RepositoryError::AlreadyExist)));
        Ok(())
    }

    #[tokio::test]
    async fn save_invalid_role_returns_data_integrity_violation() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_credential(&mut transaction, 1).await?;

        // Act
        let result = sqlx::query(
            "INSERT INTO user (user_id, credential_id, email, role, username)
             VALUES (1, 1, NULL, 'NOT_A_ROLE', 'erin')",
        )
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

    #[tokio::test]
    async fn save_username_too_short_returns_data_integrity_violation() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_credential(&mut transaction, 1).await?;

        // Act
        let result = sqlx::query(
            "INSERT INTO user (user_id, credential_id, email, role, username)
             VALUES (1, 1, NULL, 'STANDARD', 'ab')",
        )
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

    #[tokio::test]
    async fn save_missing_credential_returns_data_integrity_violation() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut transaction = begin_transaction().await?;

        // Act
        let mut repository = SqlxUserRepository::new(&mut transaction);
        let result = repository
            .save(user(1, "frank", None, 999, Role::Standard)?)
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn delete_missing_user_returns_false() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;

        // Act
        let mut repository = SqlxUserRepository::new(&mut transaction);
        let deleted = repository.delete(404).await?;

        // Assert
        assert!(!deleted);
        Ok(())
    }

    #[tokio::test]
    async fn delete_existing_user_returns_true() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_credential(&mut transaction, 1).await?;
        let mut repository = SqlxUserRepository::new(&mut transaction);
        repository
            .save(user(7, "grace", None, 1, Role::Standard)?)
            .await?;

        // Act
        let deleted = repository.delete(7).await?;

        // Assert
        assert!(deleted);
        Ok(())
    }

    #[tokio::test]
    async fn trigger_blocks_deleting_root_admin() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_credential(&mut transaction, 1).await?;
        let mut repository = SqlxUserRepository::new(&mut transaction);
        repository
            .save(user(0, "rootadmin", None, 1, Role::Admin)?)
            .await?;

        // Act
        let result = repository.delete(0).await;

        // Assert
        assert!(matches!(result, Err(RepositoryError::OperationFailed)));
        Ok(())
    }

    #[tokio::test]
    async fn trigger_blocks_updating_root_admin() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_credential(&mut transaction, 1).await?;
        let mut repository = SqlxUserRepository::new(&mut transaction);
        repository
            .save(user(0, "rootadmin", None, 1, Role::Admin)?)
            .await?;

        // Act
        let result = repository
            .save(user(
                0,
                "rootadmin",
                Some("root@example.com"),
                1,
                Role::Admin,
            )?)
            .await;

        // Assert
        assert!(matches!(result, Err(RepositoryError::OperationFailed)));
        Ok(())
    }

    #[tokio::test]
    async fn search_filters_by_role() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_credential(&mut transaction, 1).await?;
        seed_credential(&mut transaction, 2).await?;
        let mut repository = SqlxUserRepository::new(&mut transaction);
        repository
            .save(user(1, "henry", None, 1, Role::Admin)?)
            .await?;
        repository
            .save(user(2, "isabel", None, 2, Role::Standard)?)
            .await?;

        // Act
        let admins = repository
            .search(&UserFilter {
                role: Some(Role::Admin),
                ..UserFilter::default()
            })
            .await?;

        // Assert
        assert_eq!(admins.len(), 1);
        assert_eq!(admins.first().map(User::username), Some("henry"));
        Ok(())
    }

    #[tokio::test]
    async fn search_returns_all_when_filter_is_empty() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_credential(&mut transaction, 1).await?;
        seed_credential(&mut transaction, 2).await?;
        let mut repository = SqlxUserRepository::new(&mut transaction);
        repository
            .save(user(1, "judie", None, 1, Role::Standard)?)
            .await?;
        repository
            .save(user(2, "kevin", None, 2, Role::Standard)?)
            .await?;

        // Act
        let all = repository.search(&UserFilter::default()).await?;

        // Assert
        assert_eq!(all.len(), 2);
        Ok(())
    }
}
