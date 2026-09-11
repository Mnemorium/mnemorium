use std::fmt::Write as _;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::load_configuration::LoadConfigurationError;
use crate::application::port::load_configuration::LoadConfigurationResponse;
use crate::application::port::load_configuration::LoadConfigurationUseCase;
use crate::domain::model::configuration::Configuration;
use crate::domain::model::jwt::Jwt;
use crate::domain::model::persistence::Persistence;
use crate::domain::model::security::Security;
use crate::domain::model::sqlite3::Sqlite3;
use crate::domain::port::configuration_repository::ConfigurationRepository;
use crate::domain::port::configuration_source::ConfigurationSource;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::secret_generator::SecretGenerator;

/// Lifetime of a JWT token, in seconds.
const DEFAULT_JWT_TTL: u64 = 3600;
/// Maximum number of connections to the database.
const DEFAULT_SQLITE3_MAX_CONN: u32 = 1;
/// Path to the `SQLite3` database file.
const DEFAULT_SQLITE3_PATH: &str = "mnemorium.db";
/// Length of each generated secret, in bytes.
const SECRET_LENGTH: u32 = 32;

/// Use case implementation for loading the configuration.
pub struct LoadConfiguration<R, S, F> {
    /// Repository persisting the configuration singleton.
    configuration_repository: Arc<R>,
    /// Source loading the layered configuration.
    configuration_source: Arc<F>,
    /// Generator producing random secrets.
    secret_generator: Arc<S>,
}

impl<R, S, F> LoadConfiguration<R, S, F>
where
    R: ConfigurationRepository,
    S: SecretGenerator,
    F: ConfigurationSource,
{
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
        let security = Security::try_new(jwt, hex_encode(&pepper))
            .map_err(|error| LoadConfigurationError::InvalidConfiguration(error.into()))?;
        let sqlite3 =
            Sqlite3::try_new(DEFAULT_SQLITE3_PATH.to_owned(), DEFAULT_SQLITE3_MAX_CONN)
                .map_err(|error| LoadConfigurationError::InvalidConfiguration(error.into()))?;
        let persistence = Persistence::try_new(sqlite3);

        Ok(Configuration::try_new(persistence, security))
    }

    /// Ensure the configuration singleton row exists.
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
    async fn ensure_row(&self) -> Result<(), LoadConfigurationError> {
        let existing = self
            .configuration_repository
            .search()
            .await
            .map_err(|error| LoadConfigurationError::Unknown(error.into()))?;
        if existing.is_some() {
            return Ok(());
        }

        let configuration = self.default_configuration().await?;
        match self.configuration_repository.create(configuration).await {
            Ok(_) => Ok(()),
            Err(RepositoryError::AlreadyExist) => {
                // Another boot created the singleton concurrently; the row
                // exists, which is all this boot needs.
                Ok(())
            }
            Err(error) => Err(LoadConfigurationError::Unknown(error.into())),
        }
    }

    /// Load the configuration singleton through the configuration source.
    ///
    /// # Errors
    ///
    /// Returns [`LoadConfigurationError::Unknown`] when the repository or the
    /// configuration source fails.
    async fn load(&self) -> Result<Configuration, LoadConfigurationError> {
        self.ensure_row().await?;

        self.configuration_source
            .load()
            .await
            .map_err(|error| LoadConfigurationError::Unknown(error.into()))
    }

    /// Create a new use case.
    #[must_use]
    pub fn new(
        configuration_repository: Arc<R>,
        configuration_source: Arc<F>,
        secret_generator: Arc<S>,
    ) -> Self {
        Self {
            configuration_repository,
            configuration_source,
            secret_generator,
        }
    }
}

impl<R, S, F> LoadConfigurationUseCase for LoadConfiguration<R, S, F>
where
    R: ConfigurationRepository,
    S: SecretGenerator,
    F: ConfigurationSource,
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
            let configuration = self.load().await?;

            Ok(LoadConfigurationResponse::new(configuration))
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

    use crate::application::port::load_configuration::LoadConfigurationError;
    use crate::application::port::load_configuration::LoadConfigurationUseCase as _;
    use crate::domain::model::configuration::Configuration;
    use crate::domain::model::jwt::Jwt;
    use crate::domain::model::persistence::Persistence;
    use crate::domain::model::security::Security;
    use crate::domain::model::sqlite3::Sqlite3;
    use crate::domain::port::configuration_repository::MockConfigurationRepository;
    use crate::domain::port::configuration_source::MockConfigurationSource;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::error::SecretGeneratorError;
    use crate::domain::port::secret_generator::MockSecretGenerator;

    use super::LoadConfiguration;

    type UseCase = LoadConfiguration<
        MockConfigurationRepository,
        MockSecretGenerator,
        MockConfigurationSource,
    >;

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
    ) -> Result<UseCase, Box<dyn Error>> {
        let mut configuration_repository = MockConfigurationRepository::new();
        let mut secret_generator = MockSecretGenerator::new();
        let mut configuration_source = MockConfigurationSource::new();

        setup(
            &mut configuration_repository,
            &mut secret_generator,
            &mut configuration_source,
        )?;

        Ok(LoadConfiguration::new(
            Arc::new(configuration_repository),
            Arc::new(configuration_source),
            Arc::new(secret_generator),
        ))
    }

    fn configuration() -> Result<Configuration, Box<dyn Error>> {
        let jwt = Jwt::try_new(hex64('a'), 3600)?;
        let security = Security::try_new(jwt, hex64('b'))?;
        let sqlite3 = Sqlite3::try_new("mnemorium.db".to_owned(), 1)?;
        let persistence = Persistence::try_new(sqlite3);
        Ok(Configuration::try_new(persistence, security))
    }

    /// Expect the row to exist and the source to return the given layer.
    #[tokio::test]
    async fn row_exists_returns_loaded_configuration() -> Result<(), Box<dyn Error>> {
        // Arrange
        let expected = configuration()?;
        let use_case = use_case_with(|repository, secret_generator, configuration_source| {
            let row = configuration()?;
            repository.expect_search().times(1).returning(move || {
                let stored = row.clone();
                Box::pin(async move { Ok(Some(stored)) })
            });
            configuration_source.expect_load().times(1).returning({
                let stored = expected.clone();
                move || {
                    let layer = stored.clone();
                    Box::pin(async move { Ok(layer) })
                }
            });
            secret_generator.expect_generate().times(0);
            repository.expect_create().times(0);
            Ok(())
        })?;

        // Act
        let response = use_case.execute().await?;

        // Assert
        assert_eq!(response.configuration(), &expected);
        Ok(())
    }

    #[tokio::test]
    async fn row_missing_creates_row_then_loads() -> Result<(), Box<dyn Error>> {
        // Arrange
        let expected = configuration()?;
        let use_case = use_case_with(|repository, secret_generator, configuration_source| {
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
                move || {
                    let layer = stored.clone();
                    Box::pin(async move { Ok(layer) })
                }
            });
            Ok(())
        })?;

        // Act
        let response = use_case.execute().await?;

        // Assert
        assert_eq!(response.configuration(), &expected);
        Ok(())
    }

    #[tokio::test]
    async fn row_missing_create_race_still_loads() -> Result<(), Box<dyn Error>> {
        // Arrange
        let expected = configuration()?;
        let use_case = use_case_with(|repository, secret_generator, configuration_source| {
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
            configuration_source.expect_load().times(1).returning({
                let stored = expected.clone();
                move || {
                    let layer = stored.clone();
                    Box::pin(async move { Ok(layer) })
                }
            });
            Ok(())
        })?;

        // Act
        let response = use_case.execute().await?;

        // Assert
        assert_eq!(response.configuration(), &expected);
        Ok(())
    }

    #[tokio::test]
    async fn ensure_row_search_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|repository, secret_generator, _configuration_source| {
            repository
                .expect_search()
                .times(1)
                .returning(|| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            secret_generator.expect_generate().times(0);
            Ok(())
        })?;

        // Act
        let result = use_case.execute().await;

        // Assert
        assert!(matches!(result, Err(LoadConfigurationError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    async fn source_load_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|repository, secret_generator, configuration_source| {
            let row = configuration()?;
            repository.expect_search().times(1).returning(move || {
                let stored = row.clone();
                Box::pin(async move { Ok(Some(stored)) })
            });
            configuration_source
                .expect_load()
                .times(1)
                .returning(|| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            secret_generator.expect_generate().times(0);
            Ok(())
        })?;

        // Act
        let result = use_case.execute().await;

        // Assert
        assert!(matches!(result, Err(LoadConfigurationError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    async fn generation_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|repository, secret_generator, _configuration_source| {
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
        let result = use_case.execute().await;

        // Assert
        assert!(matches!(result, Err(LoadConfigurationError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    async fn create_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|repository, secret_generator, _configuration_source| {
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
        let result = use_case.execute().await;

        // Assert
        assert!(matches!(result, Err(LoadConfigurationError::Unknown(_))));
        Ok(())
    }
}
