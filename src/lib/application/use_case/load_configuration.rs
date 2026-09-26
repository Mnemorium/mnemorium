use std::fmt::Write as _;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tracing::error;

use crate::application::port::load_configuration::LoadConfigurationError;
use crate::application::port::load_configuration::LoadConfigurationResponse;
use crate::application::port::load_configuration::LoadConfigurationUseCase;
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
/// Maximum number of connections to the database.
pub(crate) const DEFAULT_SQLITE3_MAX_CONN: u32 = 1;
/// Path to the `SQLite3` database file.
pub(crate) const DEFAULT_SQLITE3_PATH: &str = "mnemorium.db";
/// Length of each generated secret, in bytes.
const SECRET_LENGTH: u32 = 32;

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
    /// Ensure the configuration singleton row exists, returning its base layer.
    ///
    /// On first boot the row is created with defaults and freshly generated
    /// secrets, so the configuration source always finds a base layer.
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
    ) -> Result<Configuration, LoadConfigurationError> {
        if let Some(row) = configuration
            .search()
            .await
            .map_err(|error| LoadConfigurationError::Unknown(error.into()))?
        {
            return Ok(row);
        }

        let configuration_row = self.default_configuration().await?;
        match configuration.create(configuration_row.clone()).await {
            Ok(_) => Ok(configuration_row),
            Err(RepositoryError::AlreadyExist) => {
                // Another boot created the singleton concurrently; the row
                // exists, which is all this boot needs.
                configuration
                    .search()
                    .await
                    .map_err(|error| LoadConfigurationError::Unknown(error.into()))?
                    .ok_or_else(|| {
                        LoadConfigurationError::Unknown(anyhow::anyhow!(
                            "the configuration singleton row does not exist"
                        ))
                    })
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
        let sqlite3 =
            Sqlite3::try_new(DEFAULT_SQLITE3_PATH.to_owned(), DEFAULT_SQLITE3_MAX_CONN)
                .map_err(|error| LoadConfigurationError::InvalidConfiguration(error.into()))?;
        let persistence = Persistence::try_new(sqlite3);
        let logging = Logging::try_new(
            DEFAULT_LOG_ANSI,
            DEFAULT_LOG_LEVEL.to_owned(),
            DEFAULT_LOG_MAX_FILES,
            DEFAULT_LOG_ROTATION,
        )
        .map_err(|error| LoadConfigurationError::InvalidConfiguration(error.into()))?;

        Ok(Configuration::try_new(persistence, security, logging))
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

            let result =
                async {
                    let base = self.base_layer(&mut unit_of_work.configuration()).await?;
                    let configuration = self.configuration_source.load(base).await.map_err(
                        |error| match error {
                            ConfigurationSourceError::InvalidConfiguration(source) => {
                                LoadConfigurationError::InvalidConfiguration(source)
                            }
                            other @ (ConfigurationSourceError::OperationFailed
                            | ConfigurationSourceError::Unknown(_)) => {
                                LoadConfigurationError::Unknown(anyhow::Error::new(other))
                            }
                        },
                    )?;

                    Ok(LoadConfigurationResponse::new(configuration))
                }
                .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| LoadConfigurationError::Unknown(error.into()))?;
                    Ok(value)
                }
                Err(error) => {
                    if let Err(rollback_error) = unit_of_work.rollback().await {
                        error!(
                            error = ?rollback_error,
                            "failed to roll back the load configuration unit of work"
                        );
                    }
                    Err(error)
                }
            }
        })
    }
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
    use std::sync::Mutex;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;

    use crate::application::port::load_configuration::LoadConfigurationError;
    use crate::application::port::load_configuration::LoadConfigurationUseCase as _;
    use crate::domain::model::configuration::Configuration;
    use crate::domain::model::jwt::Jwt;
    use crate::domain::model::logging::Logging;
    use crate::domain::model::logging::Rotation;
    use crate::domain::model::persistence::Persistence;
    use crate::domain::model::security::Security;
    use crate::domain::model::sqlite3::Sqlite3;
    use crate::domain::port::configuration_repository::MockConfigurationRepository;
    use crate::domain::port::configuration_source::MockConfigurationSource;
    use crate::domain::port::credential_repository::MockCredentialRepository;
    use crate::domain::port::error::ConfigurationSourceError;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::error::SecretGeneratorError;
    use crate::domain::port::secret_generator::MockSecretGenerator;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::TestUnitOfWork;
    use crate::test_helpers::TestUnitOfWorkFactory;

    use super::LoadConfiguration;

    type UseCase = LoadConfiguration<
        TestUnitOfWorkFactory<
            MockUserRepository,
            MockCredentialRepository,
            MockConfigurationRepository,
        >,
        MockSecretGenerator,
        MockConfigurationSource,
    >;

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
        let factory = TestUnitOfWorkFactory {
            unit_of_work: Mutex::new(Some(TestUnitOfWork {
                committed: Arc::clone(&committed),
                configuration: configuration_repository,
                credentials: MockCredentialRepository::new(),
                rolled_back: Arc::clone(&rolled_back),
                users: MockUserRepository::new(),
            })),
        };

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
        let persistence = Persistence::try_new(sqlite3);
        let logging = Logging::try_new(false, "debug,sqlx=warn".to_owned(), 7, Rotation::Daily)?;
        Ok(Configuration::try_new(persistence, security, logging))
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
}
