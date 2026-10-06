use std::fmt::Write as _;
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::Arc;
use std::thread::available_parallelism;

use crate::application::port::load_configuration::LoadConfigurationError;
use crate::application::port::load_configuration::LoadConfigurationResponse;
use crate::application::port::load_configuration::LoadConfigurationUseCase;
use crate::application::security_event;
use crate::domain::model::asset::Asset;
use crate::domain::model::configuration::Configuration;
use crate::domain::model::jwt::Jwt;
use crate::domain::model::logging::Logging;
use crate::domain::model::logging::Rotation;
use crate::domain::model::persistence::Persistence;
use crate::domain::model::security::Security;
use crate::domain::model::sqlite3::Sqlite3;
use crate::domain::port::configuration_repository::ConfigurationRepository;
use crate::domain::port::configuration_source::ConfigurationSource;
use crate::domain::port::configuration_unit_of_work::ConfigurationUnitOfWork;
use crate::domain::port::error::ConfigurationSourceError;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::secret_generator::SecretGenerator;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;

/// Lifetime of a JWT token, in seconds.
const DEFAULT_JWT_TTL: u64 = 3600;
/// Whether to colour the standard-output log sink with ANSI escape codes.
pub(crate) const DEFAULT_LOG_ANSI: bool = false;
/// Verbosity filter applied to the `tracing` instrumentation.
pub(crate) const DEFAULT_LOG_LEVEL: &str = "debug,sqlx=warn";
/// Maximum number of rotated log files to keep; `0` keeps every file.
pub(crate) const DEFAULT_LOG_MAX_FILES: u32 = 7;
/// Rotation period of the log file sink.
pub(crate) const DEFAULT_LOG_ROTATION: Rotation = Rotation::Daily;
/// Path to the `SQLite3` database file.
pub(crate) const DEFAULT_SQLITE3_PATH: &str = "mnemorium.db";
/// Length of each generated secret, in bytes.
const SECRET_LENGTH: u32 = 32;
/// Upper bound of the default database connection count.
///
/// The host's available parallelism over-provisions connections in a container
/// without a CPU quota (it reports the physical host's cores), and `SQLite`
/// serializes writers regardless, so the default is capped.
const MAX_DEFAULT_SQLITE3_MAX_CONN: u32 = 8;

/// Use case implementation for loading the configuration.
pub struct LoadConfiguration<F, S, C> {
    /// Source loading the layered configuration.
    configuration_source: Arc<C>,
    /// Generator producing random secrets.
    secret_generator: Arc<S>,
    /// Factory opening the unit of work wrapping the load.
    unit_of_work_factory: Arc<F>,
}

impl<F, S, C> LoadConfiguration<F, S, C>
where
    F: UnitOfWorkFactory,
    S: SecretGenerator,
    C: ConfigurationSource,
{
    /// Ensure the configuration singleton row exists, returning its base layer
    /// and whether this call created it.
    ///
    /// On first boot the row is created with defaults and freshly generated
    /// secrets, so the configuration source always finds a base layer. The
    /// `created` flag lets the caller emit `system_object` only after the unit
    /// of work commits (`OBS-002`, `OBS-006`).
    ///
    /// # Errors
    ///
    /// Returns [`LoadConfigurationError::Unknown`] when the repository or the
    /// secret generator fails, and
    /// [`LoadConfigurationError::InvalidConfiguration`] when the default
    /// settings fail validation.
    async fn base_layer(
        &self,
        configuration: &mut impl ConfigurationRepository,
    ) -> Result<(Configuration, bool), LoadConfigurationError> {
        if let Some(row) = configuration
            .search()
            .await
            .map_err(|error| LoadConfigurationError::Unknown(error.into()))?
        {
            return Ok((row, false));
        }

        let configuration_row = self.default_configuration().await?;
        match configuration.create(configuration_row.clone()).await {
            Ok(_) => Ok((configuration_row, true)),
            Err(RepositoryError::AlreadyExist) => {
                // Another boot created the singleton concurrently; the row
                // exists, which is all this boot needs.
                let row = configuration
                    .search()
                    .await
                    .map_err(|error| LoadConfigurationError::Unknown(error.into()))?
                    .ok_or_else(|| {
                        security_event::application_error("load_configuration");
                        LoadConfigurationError::Unknown(anyhow::anyhow!(
                            "the configuration singleton row does not exist"
                        ))
                    })?;
                Ok((row, false))
            }
            Err(error) => Err(LoadConfigurationError::Unknown(error.into())),
        }
    }

    /// Build the default configuration with freshly generated secrets.
    ///
    /// # Errors
    ///
    /// Returns [`LoadConfigurationError::Unknown`] when a secret cannot be
    /// generated, and [`LoadConfigurationError::InvalidConfiguration`] when
    /// the default settings fail validation.
    async fn default_configuration(&self) -> Result<Configuration, LoadConfigurationError> {
        let pepper = self
            .secret_generator
            .generate(SECRET_LENGTH)
            .await
            .map_err(|error| LoadConfigurationError::Unknown(error.into()))?;
        let secret = self
            .secret_generator
            .generate(SECRET_LENGTH)
            .await
            .map_err(|error| LoadConfigurationError::Unknown(error.into()))?;

        let jwt = Jwt::try_new(hex_encode(&secret), DEFAULT_JWT_TTL)
            .map_err(|error| LoadConfigurationError::InvalidConfiguration(error.into()))?;
        let security = Security::try_new(jwt, hex_encode(&pepper), true)
            .map_err(|error| LoadConfigurationError::InvalidConfiguration(error.into()))?;
        let sqlite3 = Sqlite3::try_new(DEFAULT_SQLITE3_PATH.to_owned(), default_sqlite3_max_conn())
            .map_err(|error| LoadConfigurationError::InvalidConfiguration(error.into()))?;
        let persistence = Persistence::new(sqlite3);
        let logging = Logging::try_new(
            DEFAULT_LOG_ANSI,
            DEFAULT_LOG_LEVEL.to_owned(),
            DEFAULT_LOG_MAX_FILES,
            DEFAULT_LOG_ROTATION,
        )
        .map_err(|error| LoadConfigurationError::InvalidConfiguration(error.into()))?;

        Ok(Configuration::new(
            persistence,
            security,
            logging,
            Asset::default(),
        ))
    }

    /// Create a new use case.
    #[must_use]
    pub fn new(
        unit_of_work_factory: Arc<F>,
        configuration_source: Arc<C>,
        secret_generator: Arc<S>,
    ) -> Self {
        Self {
            configuration_source,
            secret_generator,
            unit_of_work_factory,
        }
    }
}

impl<F, S, C> LoadConfigurationUseCase for LoadConfiguration<F, S, C>
where
    F: UnitOfWorkFactory,
    F::Uow: ConfigurationUnitOfWork,
    S: SecretGenerator,
    C: ConfigurationSource,
{
    fn execute<'future>(
        &'future self,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<LoadConfigurationResponse, LoadConfigurationError>>
                + Send
                + 'future,
        >,
    > {
        Box::pin(async move {
            let mut unit_of_work = self
                .unit_of_work_factory
                .begin()
                .await
                .map_err(|error| LoadConfigurationError::Unknown(error.into()))?;

            let result = async {
                let (base, created) = self.base_layer(&mut unit_of_work.configuration()).await?;
                let configuration =
                    self.configuration_source
                        .load(base)
                        .await
                        .map_err(|error| match error {
                            ConfigurationSourceError::InvalidConfiguration(source) => {
                                LoadConfigurationError::InvalidConfiguration(source)
                            }
                            other @ (ConfigurationSourceError::OperationFailed
                            | ConfigurationSourceError::Unknown(_)) => {
                                LoadConfigurationError::Unknown(anyhow::Error::new(other))
                            }
                        })?;

                Ok((LoadConfigurationResponse::new(configuration), created))
            }
            .await;

            match result {
                Ok((value, created)) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| LoadConfigurationError::Unknown(error.into()))?;
                    if created {
                        security_event::system_object("configuration", "create");
                    }
                    Ok(value)
                }
                Err(error) => {
                    if unit_of_work.rollback().await.is_err() {
                        // The unit-of-work adapter owns the rollback-failure log (OBS-002).
                    }
                    Err(error)
                }
            }
        })
    }
}

/// Default maximum number of connections to the database.
///
/// At least two, so a request holding a connection while another long
/// operation is in flight cannot starve every other database-backed request,
/// and at most [`MAX_DEFAULT_SQLITE3_MAX_CONN`]. Between those bounds it
/// follows the host's available parallelism.
pub(crate) fn default_sqlite3_max_conn() -> u32 {
    let parallelism = available_parallelism().map_or(2, NonZeroUsize::get);
    u32::try_from(parallelism).map_or(2, |connections| {
        connections.clamp(2, MAX_DEFAULT_SQLITE3_MAX_CONN)
    })
}

/// Hexadecimal-encode `bytes`.
fn hex_encode(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        let _result = write!(hex, "{byte:02x}");
    }
    hex
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::iter::repeat_n;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;

    use crate::application::port::load_configuration::LoadConfigurationError;
    use crate::application::port::load_configuration::LoadConfigurationUseCase as _;
    use crate::domain::model::asset::Asset;
    use crate::domain::model::asset::AssetStorage;
    use crate::domain::model::asset::AssetUpload;
    use crate::domain::model::configuration::Configuration;
    use crate::domain::model::jwt::Jwt;
    use crate::domain::model::logging::Logging;
    use crate::domain::model::logging::LoggingError;
    use crate::domain::model::logging::Rotation;
    use crate::domain::model::persistence::Persistence;
    use crate::domain::model::rate_limit::ClientIpHeader;
    use crate::domain::model::rate_limit::RateLimit;
    use crate::domain::model::security::Security;
    use crate::domain::model::sqlite3::Sqlite3;
    use crate::domain::model::sqlite3::Sqlite3Error;
    use crate::domain::port::configuration_repository::MockConfigurationRepository;
    use crate::domain::port::configuration_source::MockConfigurationSource;
    use crate::domain::port::credential_repository::MockCredentialRepository;
    use crate::domain::port::error::ConfigurationSourceError;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::error::SecretGeneratorError;
    use crate::domain::port::secret_generator::MockSecretGenerator;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::unit_of_work_factory;

    use super::LoadConfiguration;

    type UseCase = LoadConfiguration<TestFactory, MockSecretGenerator, MockConfigurationSource>;

    /// A use case under test together with its transaction-lifecycle flags.
    struct Harness {
        /// Set when the unit of work is committed.
        committed: Arc<AtomicBool>,
        /// Set when the unit of work is rolled back.
        rolled_back: Arc<AtomicBool>,
        /// The use case under test.
        use_case: UseCase,
    }

    /// A 64-character hexadecimal string, valid for secrets and peppers.
    fn hex64(character: char) -> String {
        repeat_n(character, 64).collect()
    }

    /// Build a use case whose outbound dependencies are mocked; `setup` defines
    /// the mock expectations before the mocks are handed over.
    fn use_case_with(
        setup: impl FnOnce(
            &mut MockConfigurationRepository,
            &mut MockSecretGenerator,
            &mut MockConfigurationSource,
        ) -> Result<(), Box<dyn Error>>,
    ) -> Result<Harness, Box<dyn Error>> {
        let mut configuration_repository = MockConfigurationRepository::new();
        let mut secret_generator = MockSecretGenerator::new();
        let mut configuration_source = MockConfigurationSource::new();

        setup(
            &mut configuration_repository,
            &mut secret_generator,
            &mut configuration_source,
        )?;

        let committed = Arc::new(AtomicBool::new(false));
        let rolled_back = Arc::new(AtomicBool::new(false));
        let factory = unit_of_work_factory(
            MockUserRepository::new(),
            MockCredentialRepository::new(),
            configuration_repository,
            Arc::clone(&committed),
            Arc::clone(&rolled_back),
        );

        Ok(Harness {
            use_case: LoadConfiguration::new(
                Arc::new(factory),
                Arc::new(configuration_source),
                Arc::new(secret_generator),
            ),
            committed,
            rolled_back,
        })
    }

    fn configuration() -> Result<Configuration, Box<dyn Error>> {
        let jwt = Jwt::try_new(hex64('a'), 3600)?;
        let security = Security::try_new(jwt, hex64('b'), true)?;
        let sqlite3 = Sqlite3::try_new("mnemorium.db".to_owned(), 1)?;
        let persistence = Persistence::new(sqlite3);
        let logging = Logging::try_new(false, "debug,sqlx=warn".to_owned(), 7, Rotation::Daily)?;
        Ok(Configuration::new(
            persistence,
            security,
            logging,
            Asset::default(),
        ))
    }

    #[tokio::test]
    async fn row_exists_returns_loaded_configuration() -> Result<(), Box<dyn Error>> {
        // Arrange
        let expected = configuration()?;
        let harness = use_case_with(|repository, secret_generator, configuration_source| {
            let row = configuration()?;
            repository.expect_search().times(1).returning(move || {
                let stored = row.clone();
                Box::pin(async move { Ok(Some(stored)) })
            });
            configuration_source.expect_load().times(1).returning({
                let stored = expected.clone();
                move |_base| {
                    let layer = stored.clone();
                    Box::pin(async move { Ok(layer) })
                }
            });
            secret_generator.expect_generate().times(0);
            repository.expect_create().times(0);
            Ok(())
        })?;

        // Act
        let response = harness.use_case.execute().await?;

        // Assert
        assert_eq!(response.configuration(), &expected);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn row_missing_creates_row_then_loads() -> Result<(), Box<dyn Error>> {
        // Arrange
        let expected = configuration()?;
        let harness = use_case_with(|repository, secret_generator, configuration_source| {
            repository
                .expect_search()
                .times(1)
                .returning(|| Box::pin(async { Ok(None) }));
            secret_generator
                .expect_generate()
                .times(2)
                .returning(|_| Box::pin(async { Ok(vec![0xAA; 32]) }));
            repository
                .expect_create()
                .times(1)
                .returning(|configuration| Box::pin(async move { Ok(configuration) }));
            configuration_source.expect_load().times(1).returning({
                let stored = expected.clone();
                move |_base| {
                    let layer = stored.clone();
                    Box::pin(async move { Ok(layer) })
                }
            });
            Ok(())
        })?;

        // Act
        let response = harness.use_case.execute().await?;

        // Assert
        assert_eq!(response.configuration(), &expected);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn row_missing_create_race_still_loads() -> Result<(), Box<dyn Error>> {
        // Arrange
        let expected = configuration()?;
        let harness = use_case_with(|repository, secret_generator, configuration_source| {
            repository
                .expect_search()
                .times(1)
                .returning(|| Box::pin(async { Ok(None) }));
            secret_generator
                .expect_generate()
                .times(2)
                .returning(|_| Box::pin(async { Ok(vec![0xAA; 32]) }));
            repository
                .expect_create()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::AlreadyExist) }));
            let row = configuration()?;
            repository.expect_search().times(1).returning(move || {
                let stored = row.clone();
                Box::pin(async move { Ok(Some(stored)) })
            });
            configuration_source.expect_load().times(1).returning({
                let stored = expected.clone();
                move |_base| {
                    let layer = stored.clone();
                    Box::pin(async move { Ok(layer) })
                }
            });
            Ok(())
        })?;

        // Act
        let response = harness.use_case.execute().await?;

        // Assert
        assert_eq!(response.configuration(), &expected);
        Ok(())
    }

    #[tokio::test]
    async fn ensure_row_search_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|repository, secret_generator, _configuration_source| {
            repository
                .expect_search()
                .times(1)
                .returning(|| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            secret_generator.expect_generate().times(0);
            Ok(())
        })?;

        // Act
        let result = harness.use_case.execute().await;

        // Assert
        assert!(matches!(result, Err(LoadConfigurationError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn source_load_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|repository, secret_generator, configuration_source| {
            let row = configuration()?;
            repository.expect_search().times(1).returning(move || {
                let stored = row.clone();
                Box::pin(async move { Ok(Some(stored)) })
            });
            configuration_source
                .expect_load()
                .times(1)
                .returning(|_base| {
                    Box::pin(async { Err(ConfigurationSourceError::OperationFailed) })
                });
            secret_generator.expect_generate().times(0);
            Ok(())
        })?;

        // Act
        let result = harness.use_case.execute().await;

        // Assert
        assert!(matches!(result, Err(LoadConfigurationError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn generation_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|repository, secret_generator, _configuration_source| {
            repository
                .expect_search()
                .times(1)
                .returning(|| Box::pin(async { Ok(None) }));
            secret_generator
                .expect_generate()
                .times(1)
                .returning(|_| Box::pin(async { Err(SecretGeneratorError::OperationFailed) }));
            Ok(())
        })?;

        // Act
        let result = harness.use_case.execute().await;

        // Assert
        assert!(matches!(result, Err(LoadConfigurationError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|repository, secret_generator, _configuration_source| {
            repository
                .expect_search()
                .times(1)
                .returning(|| Box::pin(async { Ok(None) }));
            secret_generator
                .expect_generate()
                .times(2)
                .returning(|_| Box::pin(async { Ok(vec![0xAA; 32]) }));
            repository
                .expect_create()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;

        // Act
        let result = harness.use_case.execute().await;

        // Assert
        assert!(matches!(result, Err(LoadConfigurationError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn logging_rejects_empty_level() {
        // Act
        let result = Logging::try_new(false, String::new(), 7, Rotation::Daily);

        // Assert
        assert!(matches!(result, Err(LoggingError::LevelEmpty)));
    }

    #[tokio::test]
    async fn sqlite3_rejects_zero_max_connections() {
        // Act
        let result = Sqlite3::try_new("mnemorium.db".to_owned(), 0);

        // Assert
        assert!(matches!(result, Err(Sqlite3Error::InvalidMaxConnections)));
    }

    #[tokio::test]
    async fn sqlite3_rejects_empty_path() {
        // Act
        let result = Sqlite3::try_new(String::new(), 1);

        // Assert
        assert!(matches!(result, Err(Sqlite3Error::PathEmpty)));
    }

    #[tokio::test]
    async fn logging_exposes_validated_settings() -> Result<(), Box<dyn Error>> {
        // Act
        let logging = Logging::try_new(true, "info,sqlx=trace".to_owned(), 3, Rotation::Hourly)?;

        // Assert
        assert!(logging.ansi());
        assert_eq!(logging.level(), "info,sqlx=trace");
        assert_eq!(logging.max_files(), 3);
        assert_eq!(logging.rotation(), Rotation::Hourly);
        Ok(())
    }

    #[tokio::test]
    async fn logging_rotation_deserializes_its_persisted_representation()
    -> Result<(), Box<dyn Error>> {
        // Act & Assert
        assert_eq!(
            serde_json::from_str::<Rotation>("\"DAILY\"")?,
            Rotation::Daily
        );
        assert_eq!(
            serde_json::from_str::<Rotation>("\"NEVER\"")?,
            Rotation::Never
        );
        Ok(())
    }

    #[tokio::test]
    async fn asset_upload_deserialization_rejects_zero_chunk_size() {
        // Arrange
        let payload = r#"{"chunk_size_bytes":0,"expiry_seconds":3600,"max_file_size_bytes":1024}"#;

        // Act
        let message = serde_json::from_str::<AssetUpload>(payload)
            .map_err(|error| error.to_string())
            .err()
            .unwrap_or_default();

        // Assert
        assert!(
            message.contains("chunk_size_bytes must be greater than zero"),
            "deserialization must reject a zero chunk size, got: {message:?}"
        );
    }

    #[tokio::test]
    async fn asset_upload_valid_payload_round_trips_all_fields() -> Result<(), Box<dyn Error>> {
        // Arrange
        let payload =
            r#"{"chunk_size_bytes":2048,"expiry_seconds":120,"max_file_size_bytes":4294967296}"#;

        // Act
        let upload = serde_json::from_str::<AssetUpload>(payload)?;

        // Assert
        assert_eq!(upload.chunk_size_bytes(), 2048);
        assert_eq!(upload.expiry_seconds(), 120);
        assert_eq!(upload.max_file_size_bytes(), 4_294_967_296);
        Ok(())
    }

    #[tokio::test]
    async fn logging_deserialization_rejects_empty_level() {
        // Arrange
        let payload = r#"{"ansi":false,"level":"","max_files":7,"rotation":"DAILY"}"#;

        // Act
        let message = serde_json::from_str::<Logging>(payload)
            .map_err(|error| error.to_string())
            .err()
            .unwrap_or_default();

        // Assert
        assert!(
            message.contains("logging level must not be empty"),
            "deserialization must reject an empty level, got: {message:?}"
        );
    }

    #[tokio::test]
    async fn logging_valid_payload_round_trips_all_fields() -> Result<(), Box<dyn Error>> {
        // Arrange
        let payload =
            r#"{"ansi":true,"level":"info,sqlx=trace","max_files":3,"rotation":"HOURLY"}"#;

        // Act
        let logging = serde_json::from_str::<Logging>(payload)?;

        // Assert
        assert!(logging.ansi());
        assert_eq!(logging.level(), "info,sqlx=trace");
        assert_eq!(logging.max_files(), 3);
        assert_eq!(logging.rotation(), Rotation::Hourly);
        Ok(())
    }

    #[tokio::test]
    async fn security_deserialization_rejects_invalid_pepper() {
        // Arrange
        let payload = format!(
            r#"{{"jwt":{{"secret":"{}","ttl":3600}},"pepper":"not-a-pepper"}}"#,
            hex64('a')
        );

        // Act
        let message = serde_json::from_str::<Security>(&payload)
            .map_err(|error| error.to_string())
            .err()
            .unwrap_or_default();

        // Assert
        assert!(
            message.contains("pepper must be a"),
            "deserialization must reject an invalid pepper, got: {message:?}"
        );
    }

    #[tokio::test]
    async fn security_valid_payload_round_trips_all_fields() -> Result<(), Box<dyn Error>> {
        // Arrange
        let payload = format!(
            r#"{{"jwt":{{"secret":"{}","ttl":3600}},"pepper":"{}"}}"#,
            hex64('a'),
            hex64('b')
        );

        // Act
        let security = serde_json::from_str::<Security>(&payload)?;

        // Assert
        assert_eq!(security.jwt().secret(), hex64('a'));
        assert_eq!(security.jwt().ttl(), 3600);
        assert_eq!(security.pepper(), hex64('b'));
        assert!(
            security.log_root_admin_password(),
            "log_root_admin_password must default to true when omitted"
        );
        Ok(())
    }

    #[tokio::test]
    async fn jwt_deserialization_rejects_invalid_secret() {
        // Arrange
        let payload = r#"{"secret":"not-a-secret","ttl":3600}"#;

        // Act
        let message = serde_json::from_str::<Jwt>(payload)
            .map_err(|error| error.to_string())
            .err()
            .unwrap_or_default();

        // Assert
        assert!(
            message.contains("jwt secret must be a"),
            "deserialization must reject an invalid secret, got: {message:?}"
        );
    }

    #[tokio::test]
    async fn jwt_deserialization_rejects_zero_ttl() {
        // Arrange
        let payload = format!(r#"{{"secret":"{}","ttl":0}}"#, hex64('a'));

        // Act
        let message = serde_json::from_str::<Jwt>(&payload)
            .map_err(|error| error.to_string())
            .err()
            .unwrap_or_default();

        // Assert
        assert!(
            message.contains("jwt ttl must be greater than zero"),
            "deserialization must reject a zero ttl, got: {message:?}"
        );
    }

    #[tokio::test]
    async fn jwt_valid_payload_round_trips_all_fields() -> Result<(), Box<dyn Error>> {
        // Arrange
        let payload = format!(r#"{{"secret":"{}","ttl":3600}}"#, hex64('a'));

        // Act
        let jwt = serde_json::from_str::<Jwt>(&payload)?;

        // Assert
        assert_eq!(jwt.secret(), hex64('a'));
        assert_eq!(jwt.ttl(), 3600);
        Ok(())
    }

    #[tokio::test]
    async fn sqlite3_deserialization_rejects_empty_path() {
        // Arrange
        let payload = r#"{"max_connections":4,"path":""}"#;

        // Act
        let message = serde_json::from_str::<Sqlite3>(payload)
            .map_err(|error| error.to_string())
            .err()
            .unwrap_or_default();

        // Assert
        assert!(
            message.contains("sqlite3 path must not be empty"),
            "deserialization must reject an empty path, got: {message:?}"
        );
    }

    #[tokio::test]
    async fn sqlite3_deserialization_rejects_zero_max_connections() {
        // Arrange
        let payload = r#"{"max_connections":0,"path":"mnemorium.db"}"#;

        // Act
        let message = serde_json::from_str::<Sqlite3>(payload)
            .map_err(|error| error.to_string())
            .err()
            .unwrap_or_default();

        // Assert
        assert!(
            message.contains("sqlite3 max_connections must be greater than zero"),
            "deserialization must reject zero max connections, got: {message:?}"
        );
    }

    #[tokio::test]
    async fn sqlite3_valid_payload_round_trips_all_fields() -> Result<(), Box<dyn Error>> {
        // Arrange
        let payload = r#"{"max_connections":4,"path":"mnemorium.db"}"#;

        // Act
        let sqlite3 = serde_json::from_str::<Sqlite3>(payload)?;

        // Assert
        assert_eq!(sqlite3.max_connections(), 4);
        assert_eq!(sqlite3.path(), "mnemorium.db");
        Ok(())
    }

    #[tokio::test]
    async fn asset_storage_deserialization_rejects_empty_root() {
        // Arrange
        let payload = r#"{"root":""}"#;

        // Act
        let message = serde_json::from_str::<AssetStorage>(payload)
            .map_err(|error| error.to_string())
            .err()
            .unwrap_or_default();

        // Assert
        assert!(
            message.contains("asset storage root must not be empty"),
            "deserialization must reject an empty root, got: {message:?}"
        );
    }

    #[tokio::test]
    async fn asset_storage_valid_payload_round_trips_root() -> Result<(), Box<dyn Error>> {
        // Arrange
        let payload = r#"{"root":"media"}"#;

        // Act
        let storage = serde_json::from_str::<AssetStorage>(payload)?;

        // Assert
        assert_eq!(storage.root(), "media");
        Ok(())
    }

    #[test]
    fn default_sqlite3_max_conn_always_allows_a_concurrent_request() {
        // Act
        let connections = super::default_sqlite3_max_conn();

        // Assert
        assert!(
            (2..=super::MAX_DEFAULT_SQLITE3_MAX_CONN).contains(&connections),
            "the default pool must be bounded between two and the configured cap"
        );
    }

    #[test]
    fn configuration_round_trips_trusted_proxies_through_json() -> Result<(), Box<dyn Error>> {
        // Arrange: a non-empty trusted-proxy list is the case that must survive
        // the `DbSqlite3Source` JSON round-trip (`security.rate_limit`).
        let mut security = Security::try_new(Jwt::try_new(hex64('a'), 3600)?, hex64('b'), true)?;
        security.set_rate_limit(RateLimit::try_new(
            9,
            ClientIpHeader::XRealIp,
            30,
            vec!["10.0.0.1".parse()?, "2001:db8::1".parse()?],
        )?);
        let configuration = Configuration::new(
            Persistence::new(Sqlite3::try_new("mnemorium.db".to_owned(), 1)?),
            security,
            Logging::try_new(false, "debug,sqlx=warn".to_owned(), 7, Rotation::Daily)?,
            Asset::default(),
        );

        // Act
        let json = serde_json::to_string(&configuration)?;
        let decoded: Configuration = serde_json::from_str(&json)?;

        // Assert
        assert_eq!(
            decoded.security().rate_limit(),
            configuration.security().rate_limit()
        );
        Ok(())
    }
}
