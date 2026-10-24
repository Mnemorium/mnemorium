use std::sync::Arc;

use axum::extract::FromRef;

use crate::application::port::asset_use_case_factory::AssetUseCaseFactory;
use crate::application::port::identity_use_case_factory::IdentityUseCaseFactory;
use crate::application::port::library_use_case_factory::LibraryUseCaseFactory;
use crate::application::port::user_use_case_factory::UserUseCaseFactory;
use crate::domain::model::configuration::Configuration;
use crate::domain::model::instance_state::InstanceState;
use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;

/// Application state injected into routers and middleware.
///
/// Holds the immutable configuration, the immutable instance identity and the
/// per-context use-case factories the inbound layer builds its use cases from.
/// It deliberately does not cache a token provider: the `FromRef` impl builds
/// one from the identity and the configured lifetime, so the auth middleware
/// validates with the same secret and TTL the Identity use cases issue with.
#[derive(Clone)]
pub struct AppState {
    /// Factory building the Asset use cases.
    asset_use_case_factory: Arc<dyn AssetUseCaseFactory>,
    /// Immutable application configuration.
    configuration: Arc<Configuration>,
    /// Factory building the Identity use cases.
    identity_use_case_factory: Arc<dyn IdentityUseCaseFactory>,
    /// Immutable instance identity carrying the secrets.
    instance_state: Arc<InstanceState>,
    /// Factory building the Library use cases.
    library_use_case_factory: Arc<dyn LibraryUseCaseFactory>,
    /// Factory building the User use cases.
    user_use_case_factory: Arc<dyn UserUseCaseFactory>,
}

impl AppState {
    /// Return the factory building the Asset use cases.
    #[must_use]
    pub fn asset_use_case_factory(&self) -> Arc<dyn AssetUseCaseFactory> {
        Arc::clone(&self.asset_use_case_factory)
    }

    /// Return the immutable application configuration.
    #[must_use]
    pub fn configuration(&self) -> Arc<Configuration> {
        Arc::clone(&self.configuration)
    }

    /// Return the factory building the Identity use cases.
    #[must_use]
    pub fn identity_use_case_factory(&self) -> Arc<dyn IdentityUseCaseFactory> {
        Arc::clone(&self.identity_use_case_factory)
    }

    /// Return the immutable instance identity.
    #[must_use]
    pub fn instance_state(&self) -> Arc<InstanceState> {
        Arc::clone(&self.instance_state)
    }

    /// Return the factory building the Library use cases.
    #[must_use]
    pub fn library_use_case_factory(&self) -> Arc<dyn LibraryUseCaseFactory> {
        Arc::clone(&self.library_use_case_factory)
    }

    /// Create a new application state.
    #[must_use]
    pub fn new(
        asset_use_case_factory: Arc<dyn AssetUseCaseFactory>,
        configuration: Arc<Configuration>,
        identity_use_case_factory: Arc<dyn IdentityUseCaseFactory>,
        instance_state: Arc<InstanceState>,
        library_use_case_factory: Arc<dyn LibraryUseCaseFactory>,
        user_use_case_factory: Arc<dyn UserUseCaseFactory>,
    ) -> Self {
        Self {
            asset_use_case_factory,
            configuration,
            identity_use_case_factory,
            instance_state,
            library_use_case_factory,
            user_use_case_factory,
        }
    }

    /// Return the factory building the User use cases.
    #[must_use]
    pub fn user_use_case_factory(&self) -> Arc<dyn UserUseCaseFactory> {
        Arc::clone(&self.user_use_case_factory)
    }
}

impl FromRef<AppState> for Arc<JwtTokenProvider> {
    fn from_ref(input: &AppState) -> Self {
        Arc::new(JwtTokenProvider::new(
            input.instance_state.jwt_secret().to_owned(),
            input.configuration.security().jwt().ttl(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use axum::extract::FromRef as _;

    use crate::domain::port::token_provider::TokenProvider as _;
    use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;
    use crate::test_helpers::TEST_JWT_SECRET;
    use crate::test_helpers::app_state;

    #[tokio::test]
    async fn from_ref_builds_the_provider_from_the_identity() -> Result<(), Box<dyn Error>> {
        // Arrange
        let state = app_state()?;
        let provider = Arc::<JwtTokenProvider>::from_ref(&state);
        let fixture = JwtTokenProvider::new(TEST_JWT_SECRET.to_owned(), 3600);

        // Act: a token minted with the identity secret.
        let token = fixture.issue(11).await?.value().to_owned();

        // Assert: the extracted provider is built from the immutable identity,
        // not from a startup-cached provider.
        assert_eq!(provider.validate(&token).await?, 11);
        Ok(())
    }

    #[tokio::test]
    async fn instance_state_is_shared_immutably() -> Result<(), Box<dyn Error>> {
        // Arrange
        let state = app_state()?;

        // Act
        let first = state.instance_state();
        let second = state.instance_state();

        // Assert
        assert!(
            Arc::ptr_eq(&first, &second),
            "the identity must be a single shared immutable value"
        );
        Ok(())
    }
}
