use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::load_configuration::LoadConfigurationError;
use crate::application::port::load_configuration::LoadConfigurationResponse;
use crate::application::port::load_configuration::LoadConfigurationUseCase;
use crate::domain::model::configuration::Asset;
use crate::domain::model::configuration::AssetUpload;
use crate::domain::model::configuration::ClientIpHeader;
use crate::domain::model::configuration::Configuration;
use crate::domain::model::configuration::Jwt;
use crate::domain::model::configuration::Logging;
use crate::domain::model::configuration::Persistence;
use crate::domain::model::configuration::RateLimit;
use crate::domain::model::configuration::RateLimitError;
use crate::domain::model::configuration::Security;
use crate::domain::model::configuration::Sqlite3;
use crate::domain::model::configuration::parse_trusted_proxies;
use crate::domain::model::partial_configuration::PartialConfiguration;
use crate::domain::port::configuration_source::ConfigurationSource;
use crate::domain::port::configuration_source::EnvironmentConfiguration;
use crate::domain::port::error::ConfigurationSourceError;

/// Use case implementation for loading the configuration.
///
/// The use case owns the precedence: the built-in defaults are the base, the
/// file layer is merged over them, and the environment layer is merged over the
/// result. Each adapter only loads the settings its layer names; the merge
/// itself lives here.
pub struct LoadConfiguration<C, E> {
    /// Source loading the configuration file layer.
    configuration_source: Arc<C>,
    /// Source loading the environment layer.
    environment_configuration: Arc<E>,
}

impl<C: ConfigurationSource, E: EnvironmentConfiguration> LoadConfiguration<C, E> {
    /// Create a new use case.
    #[must_use]
    pub fn new(configuration_source: Arc<C>, environment_configuration: Arc<E>) -> Self {
        Self {
            configuration_source,
            environment_configuration,
        }
    }
}

impl<C: ConfigurationSource, E: EnvironmentConfiguration> LoadConfigurationUseCase
    for LoadConfiguration<C, E>
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
            let base = Configuration::default();
            let loaded = self
                .configuration_source
                .load()
                .await
                .map_err(map_source_error)?;
            let (file_partial, file_created) = loaded.into_parts();
            let (configuration, _file_applied) = merge(file_partial, &base)?;
            let environment_partial = self
                .environment_configuration
                .load()
                .await
                .map_err(map_source_error)?;
            let (resolved, applied) = merge(environment_partial, &configuration)?;
            Ok(LoadConfigurationResponse::new(
                resolved,
                file_created,
                applied,
            ))
        })
    }
}

/// Map a loader failure onto the use-case error.
fn map_source_error(error: ConfigurationSourceError) -> LoadConfigurationError {
    match error {
        ConfigurationSourceError::InvalidConfiguration(source) => {
            LoadConfigurationError::InvalidConfiguration(source)
        }
        other @ (ConfigurationSourceError::FileWriteFailed(_)
        | ConfigurationSourceError::OperationFailed
        | ConfigurationSourceError::Unknown(_)) => {
            LoadConfigurationError::Unknown(anyhow::Error::new(other))
        }
    }
}

/// Merge one layer over `base` and re-validate every touched section.
///
/// A setting the layer omits keeps the value `base` already holds; a setting it
/// names — even with the value `base` holds — is applied, and its setting path
/// is reported. The returned list is sorted and de-duplicated, and carries no
/// value (`OBS-004`).
///
/// # Errors
///
/// Returns [`LoadConfigurationError::InvalidConfiguration`] naming the setting
/// when a present value fails the domain validation.
fn merge(
    partial: PartialConfiguration,
    base: &Configuration,
) -> Result<(Configuration, Vec<&'static str>), LoadConfigurationError> {
    let mut applied = Vec::new();

    let upload = AssetUpload::try_new(
        resolve(
            partial.asset.upload.chunk_size_bytes,
            || base.asset().upload().chunk_size_bytes(),
            "asset.upload.chunk_size_bytes",
            &mut applied,
        ),
        resolve(
            partial.asset.upload.expiry_seconds,
            || base.asset().upload().expiry_seconds(),
            "asset.upload.expiry_seconds",
            &mut applied,
        ),
        resolve(
            partial.asset.upload.max_file_size_bytes,
            || base.asset().upload().max_file_size_bytes(),
            "asset.upload.max_file_size_bytes",
            &mut applied,
        ),
    )
    .map_err(|error| invalid("asset.upload", error))?;
    let asset = Asset::new(upload);

    let logging = Logging::try_new(
        resolve(
            partial.logging.ansi,
            || base.logging().ansi(),
            "logging.ansi",
            &mut applied,
        ),
        resolve(
            partial.logging.level,
            || base.logging().level().to_owned(),
            "logging.level",
            &mut applied,
        ),
        resolve(
            partial.logging.max_files,
            || base.logging().max_files(),
            "logging.max_files",
            &mut applied,
        ),
        resolve(
            partial.logging.rotation,
            || base.logging().rotation(),
            "logging.rotation",
            &mut applied,
        ),
    )
    .map_err(|error| invalid("logging.level", error))?;

    let persistence = Persistence::new(
        Sqlite3::try_new(resolve(
            partial.persistence.sqlite3.max_connections,
            || base.persistence().sqlite3().max_connections(),
            "persistence.sqlite3.max_connections",
            &mut applied,
        ))
        .map_err(|error| invalid("persistence.sqlite3.max_connections", error))?,
    );

    let jwt = Jwt::try_new(resolve(
        partial.security.jwt.ttl,
        || base.security().jwt().ttl(),
        "security.jwt.ttl",
        &mut applied,
    ))
    .map_err(|error| invalid("security.jwt.ttl", error))?;

    let client_ip_header = match partial.security.rate_limit.client_ip_header {
        Some(value) => {
            applied.push("security.rate_limit.client_ip_header");
            ClientIpHeader::parse(&value)
                .map_err(|error| invalid("security.rate_limit.client_ip_header", error))?
        }
        None => base.security().rate_limit().client_ip_header(),
    };
    let trusted_proxies = match partial.security.rate_limit.trusted_proxies {
        Some(value) => {
            applied.push("security.rate_limit.trusted_proxies");
            parse_trusted_proxies(&value)
                .map_err(|error| invalid("security.rate_limit.trusted_proxies", error))?
        }
        None => base.security().rate_limit().trusted_proxies().to_vec(),
    };
    let rate_limit = RateLimit::try_new(
        resolve(
            partial.security.rate_limit.burst_size,
            || base.security().rate_limit().burst_size(),
            "security.rate_limit.burst_size",
            &mut applied,
        ),
        client_ip_header,
        resolve(
            partial.security.rate_limit.period_seconds,
            || base.security().rate_limit().period_seconds(),
            "security.rate_limit.period_seconds",
            &mut applied,
        ),
        trusted_proxies,
    )
    .map_err(|error| {
        let setting = match error {
            RateLimitError::InvalidBurstSize => "security.rate_limit.burst_size",
            RateLimitError::InvalidClientIpHeader => "security.rate_limit.client_ip_header",
            RateLimitError::InvalidPeriodSeconds => "security.rate_limit.period_seconds",
            RateLimitError::InvalidTrustedProxy => "security.rate_limit.trusted_proxies",
        };
        invalid(setting, error)
    })?;

    let security = Security::try_new(
        jwt,
        resolve(
            partial.security.log_root_admin_password,
            || base.security().log_root_admin_password(),
            "security.log_root_admin_password",
            &mut applied,
        ),
        rate_limit,
    )
    .map_err(|error| invalid("security", error))?;

    applied.sort_unstable();
    applied.dedup();
    Ok((
        Configuration::new(persistence, security, logging, asset),
        applied,
    ))
}

/// Build the rejection of `setting` from the domain failure `source`.
fn invalid(setting: &'static str, source: impl Into<anyhow::Error>) -> LoadConfigurationError {
    LoadConfigurationError::InvalidConfiguration(source.into().context(setting))
}

/// Resolve one setting: the layer's value when present, `base` otherwise.
///
/// The setting path is recorded only when the layer named it, so the applied
/// list reports the settings that were effective, not the ones that merely
/// repeated `base`.
fn resolve<T>(
    layer: Option<T>,
    base: impl FnOnce() -> T,
    setting: &'static str,
    applied: &mut Vec<&'static str>,
) -> T {
    layer.map_or_else(base, |value| {
        applied.push(setting);
        value
    })
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use crate::application::port::load_configuration::LoadConfigurationError;
    use crate::application::port::load_configuration::LoadConfigurationUseCase as _;
    use crate::domain::model::configuration::ClientIpHeader;
    use crate::domain::model::configuration::Configuration;
    use crate::domain::model::configuration::Rotation;
    use crate::domain::model::partial_configuration::PartialAsset;
    use crate::domain::model::partial_configuration::PartialAssetUpload;
    use crate::domain::model::partial_configuration::PartialConfiguration;
    use crate::domain::model::partial_configuration::PartialJwt;
    use crate::domain::model::partial_configuration::PartialLogging;
    use crate::domain::model::partial_configuration::PartialRateLimit;
    use crate::domain::model::partial_configuration::PartialSecurity;
    use crate::domain::port::configuration_source::LoadedFileConfiguration;
    use crate::domain::port::configuration_source::MockConfigurationSource;
    use crate::domain::port::configuration_source::MockEnvironmentConfiguration;
    use crate::domain::port::error::ConfigurationSourceError;

    use super::LoadConfiguration;

    type UseCase = LoadConfiguration<MockConfigurationSource, MockEnvironmentConfiguration>;

    /// Build a file layer returning `partial` and the given file-created flag.
    fn file_layer(partial: PartialConfiguration, file_created: bool) -> MockConfigurationSource {
        let mut source = MockConfigurationSource::new();
        source.expect_load().times(1).returning(move || {
            let settings = partial.clone();
            Box::pin(async move { Ok(LoadedFileConfiguration::new(settings, file_created)) })
        });
        source
    }

    /// Build an environment layer returning `partial`.
    fn environment_layer(partial: PartialConfiguration) -> MockEnvironmentConfiguration {
        let mut source = MockEnvironmentConfiguration::new();
        source.expect_load().times(1).returning(move || {
            let settings = partial.clone();
            Box::pin(async move { Ok(settings) })
        });
        source
    }

    /// A use case composed of the given layers.
    fn use_case(
        file: MockConfigurationSource,
        environment: MockEnvironmentConfiguration,
    ) -> UseCase {
        LoadConfiguration::new(Arc::new(file), Arc::new(environment))
    }

    /// A partial naming only the logging level.
    fn logging_level(level: &str) -> PartialConfiguration {
        PartialConfiguration {
            logging: PartialLogging {
                level: Some(level.to_owned()),
                ..PartialLogging::default()
            },
            ..PartialConfiguration::default()
        }
    }

    /// A partial naming only the upload setting `apply` sets.
    fn upload_settings(apply: impl FnOnce(&mut PartialAssetUpload)) -> PartialConfiguration {
        let mut upload = PartialAssetUpload::default();
        apply(&mut upload);
        PartialConfiguration {
            asset: PartialAsset {
                upload,
                ..PartialAsset::default()
            },
            ..PartialConfiguration::default()
        }
    }

    /// A partial naming only the rate-limit setting `apply` sets.
    fn rate_limit_settings(apply: impl FnOnce(&mut PartialRateLimit)) -> PartialConfiguration {
        let mut rate_limit = PartialRateLimit::default();
        apply(&mut rate_limit);
        PartialConfiguration {
            security: PartialSecurity {
                rate_limit,
                ..PartialSecurity::default()
            },
            ..PartialConfiguration::default()
        }
    }

    /// Assert the file layer naming `partial` is rejected and the message names `setting`.
    ///
    /// The environment layer must not be consulted: a file value that fails validation stops the
    /// merge before the higher layer runs.
    async fn assert_rejected_naming(partial: PartialConfiguration, setting: &str) {
        // Arrange
        let mut environment = MockEnvironmentConfiguration::new();
        environment.expect_load().times(0);
        let use_case = use_case(file_layer(partial, false), environment);

        // Act
        let result = use_case.execute().await;

        // Assert
        assert!(
            matches!(
                result.as_ref(),
                Err(LoadConfigurationError::InvalidConfiguration(_))
            ),
            "an invalid merged value must be reported as such, got: {result:?}"
        );
        let message = result
            .map_err(|error| error.to_string())
            .err()
            .unwrap_or_default();
        assert!(
            message.contains(setting),
            "the rejection must name {setting}, got: {message:?}"
        );
    }

    #[tokio::test]
    async fn load_configuration_empty_layers_yield_the_defaults() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case(
            file_layer(PartialConfiguration::default(), true),
            environment_layer(PartialConfiguration::default()),
        );

        // Act
        let response = use_case.execute().await?;

        // Assert
        assert_eq!(response.configuration(), &Configuration::default());
        assert!(
            response.file_created(),
            "the generated-file flag must travel to the caller"
        );
        assert!(
            response.applied().is_empty(),
            "a layer that named nothing must report nothing"
        );
        Ok(())
    }

    #[tokio::test]
    async fn load_configuration_file_layer_overrides_the_defaults() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case(
            file_layer(logging_level("info"), false),
            environment_layer(PartialConfiguration::default()),
        );

        // Act
        let response = use_case.execute().await?;

        // Assert
        assert_eq!(response.configuration().logging().level(), "info");
        assert_eq!(
            response.configuration().security(),
            Configuration::default().security(),
            "an untouched section keeps its default"
        );
        Ok(())
    }

    #[tokio::test]
    async fn load_configuration_environment_layer_overrides_the_file() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = use_case(
            file_layer(logging_level("info"), false),
            environment_layer(logging_level("warn")),
        );

        // Act
        let response = use_case.execute().await?;

        // Assert
        assert_eq!(
            response.configuration().logging().level(),
            "warn",
            "the environment is the highest layer"
        );
        assert_eq!(
            response.applied(),
            &["logging.level"],
            "the environment layer's applied paths reach the caller"
        );
        Ok(())
    }

    #[tokio::test]
    async fn load_configuration_value_equal_to_the_default_is_still_applied()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the file raises the level, the environment brings it back to
        // the built-in default.
        let use_case = use_case(
            file_layer(logging_level("info"), false),
            environment_layer(logging_level(Configuration::default().logging().level())),
        );

        // Act
        let response = use_case.execute().await?;

        // Assert
        assert_eq!(
            response.configuration().logging().level(),
            Configuration::default().logging().level(),
            "a layer may re-impose a value equal to the built-in default"
        );
        assert_eq!(response.applied(), &["logging.level"]);
        Ok(())
    }

    #[tokio::test]
    async fn load_configuration_rate_limit_strings_are_parsed() -> Result<(), Box<dyn Error>> {
        // Arrange
        let partial = PartialConfiguration {
            security: PartialSecurity {
                rate_limit: PartialRateLimit {
                    client_ip_header: Some("x-real-ip".to_owned()),
                    trusted_proxies: Some("127.0.0.1, 10.0.0.1".to_owned()),
                    ..PartialRateLimit::default()
                },
                ..PartialSecurity::default()
            },
            ..PartialConfiguration::default()
        };
        let use_case = use_case(
            file_layer(PartialConfiguration::default(), false),
            environment_layer(partial),
        );

        // Act
        let response = use_case.execute().await?;

        // Assert
        assert_eq!(
            response
                .configuration()
                .security()
                .rate_limit()
                .client_ip_header(),
            ClientIpHeader::XRealIp
        );
        assert_eq!(
            response
                .configuration()
                .security()
                .rate_limit()
                .trusted_proxies()
                .len(),
            2
        );
        Ok(())
    }

    #[tokio::test]
    async fn load_configuration_invalid_value_names_the_setting() -> Result<(), Box<dyn Error>> {
        // Arrange: the level is present but empty, which the domain rejects
        // before the environment layer is consulted.
        let mut environment = MockEnvironmentConfiguration::new();
        environment.expect_load().times(0);
        let use_case = use_case(file_layer(logging_level(""), false), environment);

        // Act
        let result = use_case.execute().await;

        // Assert
        assert!(
            matches!(
                result.as_ref(),
                Err(LoadConfigurationError::InvalidConfiguration(_))
            ),
            "an invalid merged value must be reported as such, got: {result:?}"
        );
        let message = result
            .map_err(|error| error.to_string())
            .err()
            .unwrap_or_default();
        assert!(
            message.contains("logging.level"),
            "the rejection must name the setting, got: {message:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn load_configuration_file_failure_is_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut file = MockConfigurationSource::new();
        file.expect_load()
            .times(1)
            .returning(|| Box::pin(async { Err(ConfigurationSourceError::OperationFailed) }));
        let mut environment = MockEnvironmentConfiguration::new();
        environment.expect_load().times(0);
        let use_case = use_case(file, environment);

        // Act
        let result = use_case.execute().await;

        // Assert
        assert!(matches!(result, Err(LoadConfigurationError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    async fn load_configuration_template_failure_is_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut file = MockConfigurationSource::new();
        file.expect_load().times(1).returning(|| {
            Box::pin(async {
                Err(ConfigurationSourceError::FileWriteFailed(anyhow::anyhow!(
                    "cannot write config.yaml: permission denied"
                )))
            })
        });
        let mut environment = MockEnvironmentConfiguration::new();
        environment.expect_load().times(0);
        let use_case = use_case(file, environment);

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
    async fn load_configuration_invalid_layer_is_reported() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut file = MockConfigurationSource::new();
        file.expect_load().times(1).returning(|| {
            Box::pin(async {
                Err(ConfigurationSourceError::InvalidConfiguration(
                    anyhow::anyhow!("logging.level must not be empty"),
                ))
            })
        });
        let mut environment = MockEnvironmentConfiguration::new();
        environment.expect_load().times(0);
        let use_case = use_case(file, environment);

        // Act
        let result = use_case.execute().await;

        // Assert
        assert!(matches!(
            result,
            Err(LoadConfigurationError::InvalidConfiguration(_))
        ));
        Ok(())
    }

    #[tokio::test]
    async fn load_configuration_environment_failure_is_reported() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut environment = MockEnvironmentConfiguration::new();
        environment.expect_load().times(1).returning(|| {
            Box::pin(async {
                Err(ConfigurationSourceError::InvalidConfiguration(
                    anyhow::anyhow!("the environment variable MNEMORIUM_LOGGING_LEVEL is invalid"),
                ))
            })
        });
        let use_case = use_case(
            file_layer(PartialConfiguration::default(), false),
            environment,
        );

        // Act
        let result = use_case.execute().await;

        // Assert
        assert!(matches!(
            result,
            Err(LoadConfigurationError::InvalidConfiguration(_))
        ));
        Ok(())
    }

    #[tokio::test]
    async fn load_configuration_file_runs_before_the_environment() -> Result<(), Box<dyn Error>> {
        // Arrange: both layers name a different setting, so the resolved value
        // proves each one is applied over the other in order.
        let file = PartialConfiguration {
            logging: PartialLogging {
                max_files: Some(3),
                ..PartialLogging::default()
            },
            ..PartialConfiguration::default()
        };
        let use_case = use_case(
            file_layer(file, false),
            environment_layer(logging_level("warn")),
        );

        // Act
        let response = use_case.execute().await?;

        // Assert
        assert_eq!(response.configuration().logging().max_files(), 3);
        assert_eq!(response.configuration().logging().level(), "warn");
        Ok(())
    }

    #[tokio::test]
    async fn load_configuration_applied_paths_are_sorted() -> Result<(), Box<dyn Error>> {
        // Arrange
        let partial = PartialConfiguration {
            asset: PartialAsset {
                upload: PartialAssetUpload {
                    chunk_size_bytes: Some(1_024),
                    ..PartialAssetUpload::default()
                },
                ..PartialAsset::default()
            },
            logging: PartialLogging {
                max_files: Some(3),
                rotation: Some(Rotation::Hourly),
                ..PartialLogging::default()
            },
            ..PartialConfiguration::default()
        };
        let use_case = use_case(
            file_layer(PartialConfiguration::default(), false),
            environment_layer(partial),
        );

        // Act
        let response = use_case.execute().await?;

        // Assert
        assert_eq!(
            response.applied(),
            &[
                "asset.upload.chunk_size_bytes",
                "logging.max_files",
                "logging.rotation"
            ],
            "the applied paths must be sorted"
        );
        assert_eq!(
            response.configuration().asset().upload().chunk_size_bytes(),
            1_024
        );
        assert_eq!(
            response.configuration().logging().rotation(),
            Rotation::Hourly
        );
        Ok(())
    }
    #[tokio::test]
    async fn load_configuration_zero_chunk_size_is_rejected() {
        // Arrange: the upload section names a chunk size the domain refuses.
        let partial = upload_settings(|upload| upload.chunk_size_bytes = Some(0));

        // Act & Assert
        assert_rejected_naming(partial, "asset.upload").await;
    }

    #[tokio::test]
    async fn load_configuration_zero_expiry_is_rejected() {
        // Arrange
        let partial = upload_settings(|upload| upload.expiry_seconds = Some(0));

        // Act & Assert
        assert_rejected_naming(partial, "asset.upload").await;
    }

    #[tokio::test]
    async fn load_configuration_zero_max_file_size_is_rejected() {
        // Arrange
        let partial = upload_settings(|upload| upload.max_file_size_bytes = Some(0));

        // Act & Assert
        assert_rejected_naming(partial, "asset.upload").await;
    }

    #[tokio::test]
    async fn load_configuration_oversized_max_file_size_is_rejected() {
        // Arrange: one byte above `i64::MAX`, which the domain refuses.
        let partial = upload_settings(|upload| upload.max_file_size_bytes = Some(u64::MAX));

        // Act & Assert
        assert_rejected_naming(partial, "asset.upload").await;
    }

    #[tokio::test]
    async fn load_configuration_zero_jwt_ttl_names_the_setting() {
        // Arrange
        let partial = PartialConfiguration {
            security: PartialSecurity {
                jwt: PartialJwt {
                    ttl: Some(0),
                    ..PartialJwt::default()
                },
                ..PartialSecurity::default()
            },
            ..PartialConfiguration::default()
        };

        // Act & Assert
        assert_rejected_naming(partial, "security.jwt.ttl").await;
    }

    #[tokio::test]
    async fn load_configuration_zero_burst_size_names_the_setting() {
        // Arrange
        let partial = rate_limit_settings(|rate_limit| rate_limit.burst_size = Some(0));

        // Act & Assert
        assert_rejected_naming(partial, "security.rate_limit.burst_size").await;
    }

    #[tokio::test]
    async fn load_configuration_zero_period_seconds_names_the_setting() {
        // Arrange
        let partial = rate_limit_settings(|rate_limit| rate_limit.period_seconds = Some(0));

        // Act & Assert
        assert_rejected_naming(partial, "security.rate_limit.period_seconds").await;
    }

    #[tokio::test]
    async fn load_configuration_invalid_client_ip_header_names_the_setting() {
        // Arrange: the header is not one of the three the extractor reads.
        let partial = rate_limit_settings(|rate_limit| {
            rate_limit.client_ip_header = Some("x-forwarded-host".to_owned());
        });

        // Act & Assert
        assert_rejected_naming(partial, "security.rate_limit.client_ip_header").await;
    }

    #[tokio::test]
    async fn load_configuration_invalid_trusted_proxies_names_the_setting() {
        // Arrange
        let partial = rate_limit_settings(|rate_limit| {
            rate_limit.trusted_proxies = Some("not-an-address".to_owned());
        });

        // Act & Assert
        assert_rejected_naming(partial, "security.rate_limit.trusted_proxies").await;
    }

    #[tokio::test]
    async fn load_configuration_level_off_names_the_setting() {
        // Arrange: a bare `off` would silence every non-security event (`OBS-007`).
        let partial = logging_level("off");

        // Act & Assert
        assert_rejected_naming(partial, "logging.level").await;
    }

    #[tokio::test]
    async fn load_configuration_level_none_names_the_setting() {
        // Arrange
        let partial = logging_level("none");

        // Act & Assert
        assert_rejected_naming(partial, "logging.level").await;
    }

    #[tokio::test]
    async fn load_configuration_level_targeting_security_names_the_setting() {
        // Arrange: a directive for the reserved target would suppress a catalog event.
        let partial = logging_level("warn,security=info");

        // Act & Assert
        assert_rejected_naming(partial, "logging.level").await;
    }
}
