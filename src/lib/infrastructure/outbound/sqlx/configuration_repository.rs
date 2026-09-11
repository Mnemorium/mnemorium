use sqlx::SqlitePool;

use crate::domain::model::configuration::Configuration;
use crate::domain::model::jwt::Jwt;
use crate::domain::model::persistence::Persistence;
use crate::domain::model::security::Security;
use crate::domain::model::sqlite3::Sqlite3;
use crate::domain::port::configuration_repository::ConfigurationRepository;
use crate::domain::port::error::RepositoryError;
use crate::infrastructure::outbound::sqlx::model::configuration::Configuration as SqlxConfiguration;

/// Repository persisting the configuration singleton, backed by `SQLite`.
pub struct SqlxConfigurationRepository {
    /// Connection pool to the `SQLite` database.
    pool: SqlitePool,
}

impl SqlxConfigurationRepository {
    /// Create a new repository bound to `pool`.
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl ConfigurationRepository for SqlxConfigurationRepository {
    async fn create(&self, configuration: Configuration) -> Result<Configuration, RepositoryError> {
        let row = sqlx::query_as::<_, SqlxConfiguration>(
            "INSERT INTO configuration (
                configuration_id,
                jwt_secret,
                jwt_ttl,
                pepper,
                log_root_admin_password,
                sqlite3_path,
                sqlite3_max_connections
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            RETURNING
                configuration_id,
                jwt_secret,
                jwt_ttl,
                pepper,
                log_root_admin_password,
                sqlite3_path,
                sqlite3_max_connections",
        )
        .bind(0i64)
        .bind(configuration.security().jwt().secret())
        .bind(to_i64(
            configuration.security().jwt().ttl(),
            "configuration jwt_ttl does not fit in i64",
        )?)
        .bind(configuration.security().pepper())
        .bind(configuration.security().log_root_admin_password())
        .bind(configuration.persistence().sqlite3().path())
        .bind(to_i64(
            u64::from(configuration.persistence().sqlite3().max_connections()),
            "configuration sqlite3_max_connections does not fit in i64",
        )?)
        .fetch_one(&self.pool)
        .await?;

        domain_configuration(row)
    }

    async fn save(&self, configuration: Configuration) -> Result<Configuration, RepositoryError> {
        let row = sqlx::query_as::<_, SqlxConfiguration>(
            "INSERT INTO configuration (
                configuration_id,
                jwt_secret,
                jwt_ttl,
                pepper,
                log_root_admin_password,
                sqlite3_path,
                sqlite3_max_connections
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT (configuration_id) DO UPDATE SET
                jwt_secret = excluded.jwt_secret,
                jwt_ttl = excluded.jwt_ttl,
                pepper = excluded.pepper,
                log_root_admin_password = excluded.log_root_admin_password,
                sqlite3_path = excluded.sqlite3_path,
                sqlite3_max_connections = excluded.sqlite3_max_connections
            RETURNING
                configuration_id,
                jwt_secret,
                jwt_ttl,
                pepper,
                log_root_admin_password,
                sqlite3_path,
                sqlite3_max_connections",
        )
        .bind(0i64)
        .bind(configuration.security().jwt().secret())
        .bind(to_i64(
            configuration.security().jwt().ttl(),
            "configuration jwt_ttl does not fit in i64",
        )?)
        .bind(configuration.security().pepper())
        .bind(configuration.security().log_root_admin_password())
        .bind(configuration.persistence().sqlite3().path())
        .bind(to_i64(
            u64::from(configuration.persistence().sqlite3().max_connections()),
            "configuration sqlite3_max_connections does not fit in i64",
        )?)
        .fetch_one(&self.pool)
        .await?;

        domain_configuration(row)
    }

    async fn search(&self) -> Result<Option<Configuration>, RepositoryError> {
        let row = sqlx::query_as::<_, SqlxConfiguration>(
            "SELECT
                configuration_id,
                jwt_secret,
                jwt_ttl,
                pepper,
                log_root_admin_password,
                sqlite3_path,
                sqlite3_max_connections
            FROM configuration
            WHERE configuration_id = 0",
        )
        .fetch_optional(&self.pool)
        .await?;

        row.map(domain_configuration).transpose()
    }
}

/// Convert an unsigned value to its persisted `i64` representation.
fn to_i64(value: u64, message: &'static str) -> Result<i64, RepositoryError> {
    i64::try_from(value)
        .map_err(|error| RepositoryError::Unknown(anyhow::anyhow!(error).context(message)))
}

/// Map a persisted configuration row back to the domain model.
fn domain_configuration(row: SqlxConfiguration) -> Result<Configuration, RepositoryError> {
    let max_connections = u32::try_from(row.sqlite3_max_connections).map_err(|error| {
        RepositoryError::Unknown(
            anyhow::anyhow!(error)
                .context("configuration sqlite3_max_connections does not fit in u32"),
        )
    })?;
    let ttl = u64::try_from(row.jwt_ttl).map_err(|error| {
        RepositoryError::Unknown(
            anyhow::anyhow!(error).context("configuration jwt_ttl does not fit in u64"),
        )
    })?;
    let sqlite3 = Sqlite3::try_new(row.sqlite3_path, max_connections)
        .map_err(|_| RepositoryError::DataIntegrityViolation)?;
    let jwt =
        Jwt::try_new(row.jwt_secret, ttl).map_err(|_| RepositoryError::DataIntegrityViolation)?;
    let security = Security::try_new(jwt, row.pepper, row.log_root_admin_password)
        .map_err(|_| RepositoryError::DataIntegrityViolation)?;
    let persistence = Persistence::try_new(sqlite3);

    Ok(Configuration::try_new(persistence, security))
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::iter::repeat_n;

    use sqlx::sqlite::SqlitePoolOptions;

    use crate::domain::model::configuration::Configuration;
    use crate::domain::model::jwt::Jwt;
    use crate::domain::model::persistence::Persistence;
    use crate::domain::model::security::Security;
    use crate::domain::model::sqlite3::Sqlite3;
    use crate::domain::port::configuration_repository::ConfigurationRepository as _;
    use crate::domain::port::error::RepositoryError;

    use super::SqlxConfigurationRepository;

    /// A 64-character hexadecimal string, valid for secrets and peppers.
    fn hex64(character: char) -> String {
        repeat_n(character, 64).collect()
    }

    async fn repo() -> Result<SqlxConfigurationRepository, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(SqlxConfigurationRepository::new(pool))
    }

    fn configuration() -> Result<Configuration, Box<dyn Error>> {
        let jwt = Jwt::try_new(hex64('a'), 3600)?;
        let security = Security::try_new(jwt, hex64('b'), true)?;
        let sqlite3 = Sqlite3::try_new("mnemorium.db".to_owned(), 1)?;
        let persistence = Persistence::try_new(sqlite3);
        Ok(Configuration::try_new(persistence, security))
    }

    #[tokio::test]
    async fn search_missing_configuration_returns_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let repository = repo().await?;

        // Act
        let found = repository.search().await?;

        // Assert
        assert!(found.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn create_then_search_round_trips_configuration() -> Result<(), Box<dyn Error>> {
        // Arrange
        let repository = repo().await?;
        let expected = configuration()?;

        // Act
        let persisted = repository.create(expected.clone()).await?;
        let found = repository.search().await?;

        // Assert
        assert_eq!(persisted, expected);
        assert_eq!(found, Some(expected));
        Ok(())
    }

    #[tokio::test]
    async fn create_existing_configuration_returns_already_exist() -> Result<(), Box<dyn Error>> {
        // Arrange
        let repository = repo().await?;
        repository.create(configuration()?).await?;

        // Act
        let result = repository.create(configuration()?).await;

        // Assert
        assert!(matches!(result, Err(RepositoryError::AlreadyExist)));
        Ok(())
    }

    #[tokio::test]
    async fn save_updates_existing_configuration() -> Result<(), Box<dyn Error>> {
        // Arrange
        let repository = repo().await?;
        repository.create(configuration()?).await?;
        let updated = Configuration::try_new(
            Persistence::try_new(Sqlite3::try_new("other.db".to_owned(), 3)?),
            Security::try_new(Jwt::try_new(hex64('c'), 60)?, hex64('d'), false)?,
        );

        // Act
        let persisted = repository.save(updated.clone()).await?;
        let found = repository.search().await?;

        // Assert
        assert_eq!(persisted, updated);
        assert_eq!(found, Some(updated));
        Ok(())
    }

    #[tokio::test]
    async fn save_missing_configuration_inserts_it() -> Result<(), Box<dyn Error>> {
        // Arrange
        let repository = repo().await?;
        let expected = configuration()?;

        // Act
        let persisted = repository.save(expected.clone()).await?;
        let found = repository.search().await?;

        // Assert
        assert_eq!(persisted, expected);
        assert_eq!(found, Some(expected));
        Ok(())
    }
}
