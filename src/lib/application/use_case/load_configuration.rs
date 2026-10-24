use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::load_configuration::LoadConfigurationError;
use crate::application::port::load_configuration::LoadConfigurationResponse;
use crate::application::port::load_configuration::LoadConfigurationUseCase;
use crate::domain::port::configuration_source::ConfigurationSource;
use crate::domain::port::error::ConfigurationSourceError;

/// Use case implementation for loading the configuration.
pub struct LoadConfiguration<C> {
    /// Source resolving the layered configuration.
    configuration_source: Arc<C>,
}

impl<C: ConfigurationSource> LoadConfiguration<C> {
    /// Create a new use case.
    #[must_use]
    pub fn new(configuration_source: Arc<C>) -> Self {
        Self {
            configuration_source,
        }
    }
}

impl<C: ConfigurationSource> LoadConfigurationUseCase for LoadConfiguration<C> {
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
            let loaded = self
                .configuration_source
                .load()
                .await
                .map_err(|error| match error {
                    ConfigurationSourceError::InvalidConfiguration(source) => {
                        LoadConfigurationError::InvalidConfiguration(source)
                    }
                    other @ (ConfigurationSourceError::FileWriteFailed(_)
                    | ConfigurationSourceError::OperationFailed
                    | ConfigurationSourceError::Unknown(_)) => {
                        LoadConfigurationError::Unknown(anyhow::Error::new(other))
                    }
                })?;
            let (configuration, file_created) = loaded.into_parts();
            Ok(LoadConfigurationResponse::new(configuration, file_created))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use crate::application::port::load_configuration::LoadConfigurationError;
    use crate::application::port::load_configuration::LoadConfigurationUseCase as _;
    use crate::domain::model::configuration::Configuration;
    use crate::domain::port::configuration_source::LoadedConfiguration;
    use crate::domain::port::configuration_source::MockConfigurationSource;
    use crate::domain::port::error::ConfigurationSourceError;

    use super::LoadConfiguration;

    #[tokio::test]
    async fn load_configuration_source_success_returns_the_configuration()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let expected = Configuration::default();
        let mut source = MockConfigurationSource::new();
        let source_configuration = expected.clone();
        source.expect_load().times(1).returning(move || {
            let configuration = source_configuration.clone();
            Box::pin(async move { Ok(LoadedConfiguration::new(configuration, true)) })
        });
        let use_case = LoadConfiguration::new(Arc::new(source));

        // Act
        let response = use_case.execute().await?;

        // Assert
        assert_eq!(response.configuration(), &expected);
        assert!(
            response.file_created(),
            "the generated-file flag must travel to the caller"
        );
        Ok(())
    }

    #[tokio::test]
    async fn load_configuration_source_failure_is_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut source = MockConfigurationSource::new();
        source
            .expect_load()
            .times(1)
            .returning(|| Box::pin(async { Err(ConfigurationSourceError::OperationFailed) }));
        let use_case = LoadConfiguration::new(Arc::new(source));

        // Act
        let result = use_case.execute().await;

        // Assert
        assert!(matches!(result, Err(LoadConfigurationError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    async fn load_configuration_template_failure_is_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut source = MockConfigurationSource::new();
        source.expect_load().times(1).returning(|| {
            Box::pin(async {
                Err(ConfigurationSourceError::FileWriteFailed(anyhow::anyhow!(
                    "cannot write config.yaml: permission denied"
                )))
            })
        });
        let use_case = LoadConfiguration::new(Arc::new(source));

        // Act
        let result = use_case.execute().await;

        // Assert
        let message = result
            .map_err(|error| error.to_string())
            .err()
            .unwrap_or_default();
        assert!(
            message.contains("config.yaml"),
            "the template failure must name the path, got: {message:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn load_configuration_invalid_configuration_is_reported() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut source = MockConfigurationSource::new();
        source.expect_load().times(1).returning(|| {
            Box::pin(async {
                Err(ConfigurationSourceError::InvalidConfiguration(
                    anyhow::anyhow!("logging.level must not be empty"),
                ))
            })
        });
        let use_case = LoadConfiguration::new(Arc::new(source));

        // Act
        let result = use_case.execute().await;

        // Assert
        assert!(matches!(
            result,
            Err(LoadConfigurationError::InvalidConfiguration(_))
        ));
        Ok(())
    }
}
