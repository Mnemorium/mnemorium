use std::sync::Arc;

use axum::extract::FromRef;

use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;

/// Non-use-case application dependencies injected into routers and
/// middleware. Use cases travel through the
/// [`UseCaseCatalog`](crate::application::port::UseCaseCatalog) instead.
///
/// The `FromRef` impl lets the auth middleware pull the token provider
/// directly from the router state.
#[derive(Clone)]
pub struct AppState {
    /// Provider issuing and validating bearer tokens.
    token_provider: Arc<JwtTokenProvider>,
}

impl AppState {
    /// Create a new application state.
    #[must_use]
    pub fn new(token_provider: Arc<JwtTokenProvider>) -> Self {
        Self { token_provider }
    }
}

impl FromRef<AppState> for Arc<JwtTokenProvider> {
    fn from_ref(input: &AppState) -> Self {
        Arc::clone(&input.token_provider)
    }
}
