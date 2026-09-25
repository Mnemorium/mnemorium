use std::sync::Arc;

use arc_swap::ArcSwap;
use axum::extract::FromRef;

use crate::application::port::identity_use_case_factory::IdentityUseCaseFactory;
use crate::application::port::user_use_case_factory::UserUseCaseFactory;
use crate::domain::model::configuration::Configuration;
use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;

/// Application state injected into routers and middleware.
///
/// Holds the live configuration, the per-context use-case factories the inbound
/// layer builds its use cases from, and the token provider the auth middleware
/// validates with.
///
/// The `FromRef` impl lets the auth middleware pull the token provider
/// directly from the router state.
#[derive(Clone)]
pub struct AppState {
    /// Live application configuration, swappable at runtime.
    configuration: Arc<ArcSwap<Configuration>>,
    /// Factory building the Identity use cases.
    identity_use_case_factory: Arc<dyn IdentityUseCaseFactory>,
    /// Provider issuing and validating bearer tokens.
    token_provider: Arc<JwtTokenProvider>,
    /// Factory building the User use cases.
    user_use_case_factory: Arc<dyn UserUseCaseFactory>,
}

impl AppState {
    /// Return the live application configuration.
    #[must_use]
    pub fn configuration(&self) -> Arc<ArcSwap<Configuration>> {
        Arc::clone(&self.configuration)
    }

    /// Return the factory building the Identity use cases.
    #[must_use]
    pub fn identity_use_case_factory(&self) -> Arc<dyn IdentityUseCaseFactory> {
        Arc::clone(&self.identity_use_case_factory)
    }

    /// Create a new application state.
    #[must_use]
    pub fn new(
        configuration: Arc<ArcSwap<Configuration>>,
        identity_use_case_factory: Arc<dyn IdentityUseCaseFactory>,
        token_provider: Arc<JwtTokenProvider>,
        user_use_case_factory: Arc<dyn UserUseCaseFactory>,
    ) -> Self {
        Self {
            configuration,
            identity_use_case_factory,
            token_provider,
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
        Arc::clone(&input.token_provider)
    }
}
