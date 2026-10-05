use sqlx::Sqlite;
use sqlx::Transaction;

use crate::domain::model::asset::Asset;
use crate::domain::model::asset::AssetStorage;
use crate::domain::model::asset::AssetUpload;
use crate::domain::model::configuration::Configuration;
use crate::domain::model::jwt::Jwt;
use crate::domain::model::logging::Logging;
use crate::domain::model::logging::Rotation;
use crate::domain::model::persistence::Persistence;
use crate::domain::model::rate_limit::ClientIpHeader;
use crate::domain::model::rate_limit::RateLimit;
use crate::domain::model::rate_limit::parse_trusted_proxies;
use crate::domain::model::security::Security;
use crate::domain::model::sqlite3::Sqlite3;
use crate::domain::port::configuration_repository::ConfigurationRepository;
use crate::domain::port::error::RepositoryError;
use crate::infrastructure::outbound::sqlx::model::configuration::Configuration as SqlxConfiguration;
use crate::infrastructure::outbound::sqlx::model::configuration::Rotation as SqlxRotation;

/// Repository persisting the configuration singleton, backed by a `SQLite`
/// transaction.
pub struct SqlxConfigurationRepository<'transaction> {
    /// Transaction the repository reads from and writes to.
    transaction: &'transaction mut Transaction<'static, Sqlite>,
}

impl<'transaction> SqlxConfigurationRepository<'transaction> {
    /// Create a new repository bound to `transaction`.
    #[must_use]
    pub fn new(transaction: &'transaction mut Transaction<'static, Sqlite>) -> Self {
        Self { transaction }
    }
}

impl ConfigurationRepository for SqlxConfigurationRepository<'_> {
    async fn create(
        &mut self,
        configuration: Configuration,
    ) -> Result<Configuration, RepositoryError> {
        let log_rotation = match configuration.logging().rotation() {
            Rotation::Daily => SqlxRotation::Daily,
            Rotation::Hourly => SqlxRotation::Hourly,
            Rotation::Minutely => SqlxRotation::Minutely,
            Rotation::Never => SqlxRotation::Never,
        };
        let row = sqlx::query_as::<_, SqlxConfiguration>(
            "INSERT INTO configuration (
                configuration_id,
                jwt_secret,
                jwt_ttl,
                pepper,
                is_root_admin_password_logged,
                sqlite3_path,
                sqlite3_max_connections,
                is_log_ansi,
                log_level,
                log_max_files,
                log_rotation,
                asset_storage_root,
                asset_upload_chunk_size_bytes,
                asset_upload_expiry_seconds,
                asset_upload_max_file_size_bytes,
                rate_limit_burst_size,
                rate_limit_client_ip_header,
                rate_limit_period_seconds,
                rate_limit_trusted_proxies
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
            RETURNING
                configuration_id,
                jwt_secret,
                jwt_ttl,
                pepper,
                is_root_admin_password_logged,
                sqlite3_path,
                sqlite3_max_connections,
                is_log_ansi,
                log_level,
                log_max_files,
                log_rotation,
                asset_storage_root,
                asset_upload_chunk_size_bytes,
                asset_upload_expiry_seconds,
                asset_upload_max_file_size_bytes,
                rate_limit_burst_size,
                rate_limit_client_ip_header,
                rate_limit_period_seconds,
                rate_limit_trusted_proxies",
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
        .bind(configuration.logging().ansi())
        .bind(configuration.logging().level())
        .bind(to_i64(
            u64::from(configuration.logging().max_files()),
            "configuration log_max_files does not fit in i64",
        )?)
        .bind(log_rotation)
        .bind(configuration.asset().storage().root())
        .bind(to_i64(
            configuration.asset().upload().chunk_size_bytes(),
            "configuration asset_upload_chunk_size_bytes does not fit in i64",
        )?)
        .bind(to_i64(
            configuration.asset().upload().expiry_seconds(),
            "configuration asset_upload_expiry_seconds does not fit in i64",
        )?)
        .bind(to_i64(
            configuration.asset().upload().max_file_size_bytes(),
            "configuration asset_upload_max_file_size_bytes does not fit in i64",
        )?)
        .bind(to_i64(
            u64::from(configuration.security().rate_limit().burst_size()),
            "configuration rate_limit_burst_size does not fit in i64",
        )?)
        .bind(
            configuration
                .security()
                .rate_limit()
                .client_ip_header()
                .as_str(),
        )
        .bind(to_i64(
            configuration.security().rate_limit().period_seconds(),
            "configuration rate_limit_period_seconds does not fit in i64",
        )?)
        .bind(
            configuration
                .security()
                .rate_limit()
                .trusted_proxies_value(),
        )
        .fetch_one(&mut **self.transaction)
        .await?;

        domain_configuration(row)
    }

    async fn save(
        &mut self,
        configuration: Configuration,
    ) -> Result<Configuration, RepositoryError> {
        let log_rotation = match configuration.logging().rotation() {
            Rotation::Daily => SqlxRotation::Daily,
            Rotation::Hourly => SqlxRotation::Hourly,
            Rotation::Minutely => SqlxRotation::Minutely,
            Rotation::Never => SqlxRotation::Never,
        };
        let row = sqlx::query_as::<_, SqlxConfiguration>(
            "INSERT INTO configuration (
                configuration_id,
                jwt_secret,
                jwt_ttl,
                pepper,
                is_root_admin_password_logged,
                sqlite3_path,
                sqlite3_max_connections,
                is_log_ansi,
                log_level,
                log_max_files,
                log_rotation,
                asset_storage_root,
                asset_upload_chunk_size_bytes,
                asset_upload_expiry_seconds,
                asset_upload_max_file_size_bytes,
                rate_limit_burst_size,
                rate_limit_client_ip_header,
                rate_limit_period_seconds,
                rate_limit_trusted_proxies
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
            ON CONFLICT (configuration_id) DO UPDATE SET
                jwt_secret = excluded.jwt_secret,
                jwt_ttl = excluded.jwt_ttl,
                pepper = excluded.pepper,
                is_root_admin_password_logged = excluded.is_root_admin_password_logged,
                sqlite3_path = excluded.sqlite3_path,
                sqlite3_max_connections = excluded.sqlite3_max_connections,
                is_log_ansi = excluded.is_log_ansi,
                log_level = excluded.log_level,
                log_max_files = excluded.log_max_files,
                log_rotation = excluded.log_rotation,
                asset_storage_root = excluded.asset_storage_root,
                asset_upload_chunk_size_bytes = excluded.asset_upload_chunk_size_bytes,
                asset_upload_expiry_seconds = excluded.asset_upload_expiry_seconds,
                asset_upload_max_file_size_bytes = excluded.asset_upload_max_file_size_bytes,
                rate_limit_burst_size = excluded.rate_limit_burst_size,
                rate_limit_client_ip_header = excluded.rate_limit_client_ip_header,
                rate_limit_period_seconds = excluded.rate_limit_period_seconds,
                rate_limit_trusted_proxies = excluded.rate_limit_trusted_proxies
            RETURNING
                configuration_id,
                jwt_secret,
                jwt_ttl,
                pepper,
                is_root_admin_password_logged,
                sqlite3_path,
                sqlite3_max_connections,
                is_log_ansi,
                log_level,
                log_max_files,
                log_rotation,
                asset_storage_root,
                asset_upload_chunk_size_bytes,
                asset_upload_expiry_seconds,
                asset_upload_max_file_size_bytes,
                rate_limit_burst_size,
                rate_limit_client_ip_header,
                rate_limit_period_seconds,
                rate_limit_trusted_proxies",
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
        .bind(configuration.logging().ansi())
        .bind(configuration.logging().level())
        .bind(to_i64(
            u64::from(configuration.logging().max_files()),
            "configuration log_max_files does not fit in i64",
        )?)
        .bind(log_rotation)
        .bind(configuration.asset().storage().root())
        .bind(to_i64(
            configuration.asset().upload().chunk_size_bytes(),
            "configuration asset_upload_chunk_size_bytes does not fit in i64",
        )?)
        .bind(to_i64(
            configuration.asset().upload().expiry_seconds(),
            "configuration asset_upload_expiry_seconds does not fit in i64",
        )?)
        .bind(to_i64(
            configuration.asset().upload().max_file_size_bytes(),
            "configuration asset_upload_max_file_size_bytes does not fit in i64",
        )?)
        .bind(to_i64(
            u64::from(configuration.security().rate_limit().burst_size()),
            "configuration rate_limit_burst_size does not fit in i64",
        )?)
        .bind(
            configuration
                .security()
                .rate_limit()
                .client_ip_header()
                .as_str(),
        )
        .bind(to_i64(
            configuration.security().rate_limit().period_seconds(),
            "configuration rate_limit_period_seconds does not fit in i64",
        )?)
        .bind(
            configuration
                .security()
                .rate_limit()
                .trusted_proxies_value(),
        )
        .fetch_one(&mut **self.transaction)
        .await?;

        domain_configuration(row)
    }

    async fn search(&mut self) -> Result<Option<Configuration>, RepositoryError> {
        let row = sqlx::query_as::<_, SqlxConfiguration>(
            "SELECT
                configuration_id,
                jwt_secret,
                jwt_ttl,
                pepper,
                is_root_admin_password_logged,
                sqlite3_path,
                sqlite3_max_connections,
                is_log_ansi,
                log_level,
                log_max_files,
                log_rotation,
                asset_storage_root,
                asset_upload_chunk_size_bytes,
                asset_upload_expiry_seconds,
                asset_upload_max_file_size_bytes,
                rate_limit_burst_size,
                rate_limit_client_ip_header,
                rate_limit_period_seconds,
                rate_limit_trusted_proxies
            FROM configuration
            WHERE configuration_id = 0",
        )
        .fetch_optional(&mut **self.transaction)
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
    let rate_limit_burst_size = u32::try_from(row.rate_limit_burst_size).map_err(|error| {
        RepositoryError::Unknown(
            anyhow::anyhow!(error)
                .context("configuration rate_limit_burst_size does not fit in u32"),
        )
    })?;
    let rate_limit_period_seconds =
        u64::try_from(row.rate_limit_period_seconds).map_err(|error| {
            RepositoryError::Unknown(
                anyhow::anyhow!(error)
                    .context("configuration rate_limit_period_seconds does not fit in u64"),
            )
        })?;
    let rate_limit = RateLimit::try_new(
        rate_limit_burst_size,
        ClientIpHeader::parse(&row.rate_limit_client_ip_header)
            .map_err(|_| RepositoryError::DataIntegrityViolation)?,
        rate_limit_period_seconds,
        parse_trusted_proxies(&row.rate_limit_trusted_proxies)
            .map_err(|_| RepositoryError::DataIntegrityViolation)?,
    )
    .map_err(|_| RepositoryError::DataIntegrityViolation)?;
    let mut security = Security::try_new(jwt, row.pepper, row.is_root_admin_password_logged)
        .map_err(|_| RepositoryError::DataIntegrityViolation)?;
    security.set_rate_limit(rate_limit);
    let persistence = Persistence::new(sqlite3);
    let log_max_files = u32::try_from(row.log_max_files).map_err(|error| {
        RepositoryError::Unknown(
            anyhow::anyhow!(error).context("configuration log_max_files does not fit in u32"),
        )
    })?;
    let rotation = match row.log_rotation {
        SqlxRotation::Daily => Rotation::Daily,
        SqlxRotation::Hourly => Rotation::Hourly,
        SqlxRotation::Minutely => Rotation::Minutely,
        SqlxRotation::Never => Rotation::Never,
    };
    let logging = Logging::try_new(row.is_log_ansi, row.log_level, log_max_files, rotation)
        .map_err(|_| RepositoryError::DataIntegrityViolation)?;

    let storage = AssetStorage::try_new(row.asset_storage_root)
        .map_err(|_| RepositoryError::DataIntegrityViolation)?;
    let chunk_size_bytes = u64::try_from(row.asset_upload_chunk_size_bytes).map_err(|error| {
        RepositoryError::Unknown(
            anyhow::anyhow!(error)
                .context("configuration asset_upload_chunk_size_bytes does not fit in u64"),
        )
    })?;
    let expiry_seconds = u64::try_from(row.asset_upload_expiry_seconds).map_err(|error| {
        RepositoryError::Unknown(
            anyhow::anyhow!(error)
                .context("configuration asset_upload_expiry_seconds does not fit in u64"),
        )
    })?;
    let max_file_size_bytes =
        u64::try_from(row.asset_upload_max_file_size_bytes).map_err(|error| {
            RepositoryError::Unknown(
                anyhow::anyhow!(error)
                    .context("configuration asset_upload_max_file_size_bytes does not fit in u64"),
            )
        })?;
    let asset_upload = AssetUpload::try_new(chunk_size_bytes, expiry_seconds, max_file_size_bytes)
        .map_err(|_| RepositoryError::DataIntegrityViolation)?;
    let asset = Asset::new(storage, asset_upload);
    Ok(Configuration::new(persistence, security, logging, asset))
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::iter::repeat_n;

    use rstest::rstest;
    use sqlx::Sqlite;
    use sqlx::Transaction;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::net::IpAddr;

    use crate::domain::model::asset::Asset;
    use crate::domain::model::asset::AssetStorage;
    use crate::domain::model::asset::AssetUpload;
    use crate::domain::model::configuration::Configuration;
    use crate::domain::model::jwt::Jwt;
    use crate::domain::model::logging::Logging;
    use crate::domain::model::logging::Rotation;
    use crate::domain::model::persistence::Persistence;
    use crate::domain::model::rate_limit::ClientIpHeader;
    use crate::domain::model::rate_limit::RateLimit;
    use crate::domain::model::security::Security;
    use crate::domain::model::sqlite3::Sqlite3;
    use crate::domain::port::configuration_repository::ConfigurationRepository as _;
    use crate::domain::port::error::RepositoryError;

    use super::SqlxConfigurationRepository;
    /// A 64-character hexadecimal string, valid for secrets and peppers.
    fn hex64(character: char) -> String {
        repeat_n(character, 64).collect()
    }

    async fn begin_transaction() -> Result<Transaction<'static, Sqlite>, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        pool.begin().await
    }

    fn configuration() -> Result<Configuration, Box<dyn Error>> {
        let jwt = Jwt::try_new(hex64('a'), 3600)?;
        let security = Security::try_new(jwt, hex64('b'), true)?;
        let sqlite3 = Sqlite3::try_new("mnemorium.db".to_owned(), 1)?;
        let persistence = Persistence::new(sqlite3);
        let logging = Logging::try_new(true, "info,sqlx=trace".to_owned(), 3, Rotation::Hourly)?;
        let asset = Asset::new(
            AssetStorage::try_new("media".to_owned())?,
            AssetUpload::try_new(2048, 120, 4_294_967_296)?,
        );
        Ok(Configuration::new(persistence, security, logging, asset))
    }

    #[tokio::test]
    async fn search_missing_configuration_returns_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxConfigurationRepository::new(&mut transaction);

        // Act
        let found = repository.search().await?;

        // Assert
        assert!(found.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn create_then_search_round_trips_configuration() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxConfigurationRepository::new(&mut transaction);
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
    async fn create_then_search_round_trips_asset_settings() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxConfigurationRepository::new(&mut transaction);
        let expected = configuration()?;

        // Act
        let persisted = repository.create(expected.clone()).await?;
        let found = repository.search().await?;

        // Assert
        assert_eq!(persisted.asset().storage().root(), "media");
        assert_eq!(persisted.asset().upload().chunk_size_bytes(), 2048);
        assert_eq!(persisted.asset().upload().expiry_seconds(), 120);
        assert_eq!(
            persisted.asset().upload().max_file_size_bytes(),
            4_294_967_296
        );
        assert_eq!(found, Some(expected));
        Ok(())
    }

    #[tokio::test]
    async fn save_round_trips_the_rate_limit() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxConfigurationRepository::new(&mut transaction);
        repository.create(configuration()?).await?;
        let mut security = Security::try_new(Jwt::try_new(hex64('e'), 120)?, hex64('f'), false)?;
        security.set_rate_limit(RateLimit::try_new(
            9,
            ClientIpHeader::XRealIp,
            30,
            vec!["10.0.0.1".parse()?, "2001:db8::1".parse()?],
        )?);
        let updated = Configuration::new(
            Persistence::new(Sqlite3::try_new("other.db".to_owned(), 3)?),
            security,
            Logging::try_new(false, "warn".to_owned(), 0, Rotation::Never)?,
            Asset::default(),
        );

        // Act
        let persisted = repository.save(updated).await?;

        // Assert
        let expected: Vec<IpAddr> = vec!["10.0.0.1".parse()?, "2001:db8::1".parse()?];
        assert_eq!(
            persisted.security().rate_limit().trusted_proxies(),
            expected
        );
        assert_eq!(persisted.security().rate_limit().burst_size(), 9);
        assert_eq!(
            persisted.security().rate_limit().client_ip_header(),
            ClientIpHeader::XRealIp
        );
        assert_eq!(persisted.security().rate_limit().period_seconds(), 30);
        Ok(())
    }

    #[tokio::test]
    async fn create_existing_configuration_returns_already_exist() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxConfigurationRepository::new(&mut transaction);
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
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxConfigurationRepository::new(&mut transaction);
        repository.create(configuration()?).await?;
        let updated = Configuration::new(
            Persistence::new(Sqlite3::try_new("other.db".to_owned(), 3)?),
            Security::try_new(Jwt::try_new(hex64('c'), 60)?, hex64('d'), false)?,
            Logging::try_new(false, "warn".to_owned(), 0, Rotation::Never)?,
            Asset::default(),
        );

        // Act
        let persisted = repository.save(updated.clone()).await?;
        let found = repository.search().await?;

        // Assert
        assert_eq!(persisted, updated);
        assert_eq!(found, Some(updated));
        Ok(())
    }

    #[rstest]
    #[case::daily(Rotation::Daily)]
    #[case::hourly(Rotation::Hourly)]
    #[case::minutely(Rotation::Minutely)]
    #[case::never(Rotation::Never)]
    #[tokio::test]
    async fn save_round_trips_every_rotation_period(
        #[case] rotation: Rotation,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxConfigurationRepository::new(&mut transaction);
        let base = configuration()?;
        let expected = Configuration::new(
            base.persistence().clone(),
            base.security().clone(),
            Logging::try_new(
                base.logging().ansi(),
                base.logging().level().to_owned(),
                base.logging().max_files(),
                rotation,
            )?,
            base.asset().clone(),
        );

        // Act
        let persisted = repository.save(expected.clone()).await?;
        let found = repository.search().await?;

        // Assert
        assert_eq!(persisted, expected);
        assert_eq!(found, Some(expected));
        Ok(())
    }

    #[tokio::test]
    async fn save_missing_configuration_inserts_it() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxConfigurationRepository::new(&mut transaction);
        let expected = configuration()?;

        // Act
        let persisted = repository.save(expected.clone()).await?;
        let found = repository.search().await?;

        // Assert
        assert_eq!(persisted, expected);
        assert_eq!(found, Some(expected));
        Ok(())
    }

    #[rstest]
    #[case::configuration_id_not_zero(
        "INSERT INTO configuration (
            configuration_id, jwt_secret, jwt_ttl, pepper, sqlite3_path, sqlite3_max_connections
         ) VALUES (
            1, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 3600,
            'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', 'mnemorium.db', 1
         )"
    )]
    #[case::jwt_secret_length(
        "INSERT INTO configuration (
            configuration_id, jwt_secret, jwt_ttl, pepper, sqlite3_path, sqlite3_max_connections
         ) VALUES (
            0, 'short', 3600,
            'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', 'mnemorium.db', 1
         )"
    )]
    #[case::jwt_ttl_not_positive(
        "INSERT INTO configuration (
            configuration_id, jwt_secret, jwt_ttl, pepper, sqlite3_path, sqlite3_max_connections
         ) VALUES (
            0, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 0,
            'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', 'mnemorium.db', 1
         )"
    )]
    #[case::pepper_length(
        "INSERT INTO configuration (
            configuration_id, jwt_secret, jwt_ttl, pepper, sqlite3_path, sqlite3_max_connections
         ) VALUES (
            0, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 3600,
            'short', 'mnemorium.db', 1
         )"
    )]
    #[case::is_root_admin_password_logged_out_of_range(
        "INSERT INTO configuration (
            configuration_id, jwt_secret, jwt_ttl, pepper, sqlite3_path,
            sqlite3_max_connections, is_root_admin_password_logged
         ) VALUES (
            0, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 3600,
            'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', 'mnemorium.db', 1, 2
         )"
    )]
    #[case::sqlite3_max_connections_not_positive(
        "INSERT INTO configuration (
            configuration_id, jwt_secret, jwt_ttl, pepper, sqlite3_path, sqlite3_max_connections
         ) VALUES (
            0, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 3600,
            'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', 'mnemorium.db', 0
         )"
    )]
    #[case::asset_storage_root_empty(
        "INSERT INTO configuration (
            configuration_id, jwt_secret, jwt_ttl, pepper, sqlite3_path,
            sqlite3_max_connections, asset_storage_root
         ) VALUES (
            0, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 3600,
            'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', 'mnemorium.db', 1, ''
         )"
    )]
    #[case::asset_upload_chunk_size_bytes_not_positive(
        "INSERT INTO configuration (
            configuration_id, jwt_secret, jwt_ttl, pepper, sqlite3_path,
            sqlite3_max_connections, asset_upload_chunk_size_bytes
         ) VALUES (
            0, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 3600,
            'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', 'mnemorium.db', 1, 0
         )"
    )]
    #[case::asset_upload_expiry_seconds_not_positive(
        "INSERT INTO configuration (
            configuration_id, jwt_secret, jwt_ttl, pepper, sqlite3_path,
            sqlite3_max_connections, asset_upload_expiry_seconds
         ) VALUES (
            0, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 3600,
            'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', 'mnemorium.db', 1, 0
         )"
    )]
    #[case::asset_upload_max_file_size_bytes_not_positive(
        "INSERT INTO configuration (
            configuration_id, jwt_secret, jwt_ttl, pepper, sqlite3_path,
            sqlite3_max_connections, asset_upload_max_file_size_bytes
         ) VALUES (
            0, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 3600,
            'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', 'mnemorium.db', 1, 0
         )"
    )]
    #[case::is_log_ansi_out_of_range(
        "INSERT INTO configuration (
            configuration_id, jwt_secret, jwt_ttl, pepper, sqlite3_path,
            sqlite3_max_connections, is_log_ansi
         ) VALUES (
            0, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 3600,
            'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', 'mnemorium.db', 1, 2
         )"
    )]
    #[case::log_max_files_negative(
        "INSERT INTO configuration (
            configuration_id, jwt_secret, jwt_ttl, pepper, sqlite3_path,
            sqlite3_max_connections, log_max_files
         ) VALUES (
            0, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 3600,
            'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', 'mnemorium.db', 1, -1
         )"
    )]
    #[case::log_rotation_unknown(
        "INSERT INTO configuration (
            configuration_id, jwt_secret, jwt_ttl, pepper, sqlite3_path,
            sqlite3_max_connections, log_rotation
         ) VALUES (
            0, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 3600,
            'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', 'mnemorium.db', 1, 'WEEKLY'
         )"
    )]
    #[tokio::test]
    async fn create_violating_a_check_returns_data_integrity_violation(
        #[case] statement: &'static str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;

        // Act & Assert
        let result = sqlx::query(statement).execute(&mut *transaction).await;
        let mapped = result.map_err(RepositoryError::from);
        assert!(
            matches!(mapped, Err(RepositoryError::DataIntegrityViolation)),
            "statement must violate a check constraint: {statement}"
        );
        Ok(())
    }

    #[rstest]
    #[case::jwt_secret(
        "INSERT INTO configuration (
            configuration_id, jwt_secret, jwt_ttl, pepper, sqlite3_path, sqlite3_max_connections
         ) VALUES (
            0, NULL, 3600,
            'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', 'mnemorium.db', 1
         )"
    )]
    #[case::jwt_ttl(
        "INSERT INTO configuration (
            configuration_id, jwt_secret, jwt_ttl, pepper, sqlite3_path, sqlite3_max_connections
         ) VALUES (
            0, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', NULL,
            'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', 'mnemorium.db', 1
         )"
    )]
    #[case::pepper(
        "INSERT INTO configuration (
            configuration_id, jwt_secret, jwt_ttl, pepper, sqlite3_path, sqlite3_max_connections
         ) VALUES (
            0, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 3600,
            NULL, 'mnemorium.db', 1
         )"
    )]
    #[case::sqlite3_path(
        "INSERT INTO configuration (
            configuration_id, jwt_secret, jwt_ttl, pepper, sqlite3_path, sqlite3_max_connections
         ) VALUES (
            0, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 3600,
            'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', NULL, 1
         )"
    )]
    #[case::sqlite3_max_connections(
        "INSERT INTO configuration (
            configuration_id, jwt_secret, jwt_ttl, pepper, sqlite3_path, sqlite3_max_connections
         ) VALUES (
            0, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 3600,
            'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', 'mnemorium.db', NULL
         )"
    )]
    #[tokio::test]
    async fn insert_null_required_column_returns_data_integrity_violation(
        #[case] statement: &'static str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;

        // Act & Assert
        let result = sqlx::query(statement).execute(&mut *transaction).await;
        let mapped = result.map_err(RepositoryError::from);
        assert!(
            matches!(mapped, Err(RepositoryError::DataIntegrityViolation)),
            "statement must violate a not-null constraint: {statement}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn delete_configuration_returns_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxConfigurationRepository::new(&mut transaction);
        repository.create(configuration()?).await?;

        // Act: the delete-guard trigger rejects removing the singleton row.
        let result = sqlx::query("DELETE FROM configuration WHERE configuration_id = 0")
            .execute(&mut *transaction)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(result, Err(RepositoryError::Conflict)));
        Ok(())
    }
}
