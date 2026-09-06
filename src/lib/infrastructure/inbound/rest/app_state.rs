use std::sync::Arc;

use axum::extract::FromRef;

use crate::application::port::login_user::LoginUserUseCase;
use crate::application::port::register_user::RegisterUserUseCase;
use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;

/// Shared application dependencies injected into every handler.
#[derive(Clone)]
pub struct AppState {
    /// Use case for authenticating users.
    login_user: Arc<dyn LoginUserUseCase>,
    /// Use case for registering users.
    register_user: Arc<dyn RegisterUserUseCase>,
    /// Provider issuing and validating bearer tokens.
    token_provider: Arc<JwtTokenProvider>,
}

impl AppState {
    /// Return the login-user use case.
    #[must_use]
    pub fn login_user(&self) -> &dyn LoginUserUseCase {
        self.login_user.as_ref()
    }

    /// Create a new application state.
    #[must_use]
    pub fn new(
        register_user: Arc<dyn RegisterUserUseCase>,
        login_user: Arc<dyn LoginUserUseCase>,
        token_provider: Arc<JwtTokenProvider>,
    ) -> Self {
        Self {
            login_user,
            register_user,
            token_provider,
        }
    }

    /// Return the register-user use case.
    #[must_use]
    pub fn register_user(&self) -> &dyn RegisterUserUseCase {
        self.register_user.as_ref()
    }
}

impl FromRef<AppState> for Arc<JwtTokenProvider> {
    fn from_ref(input: &AppState) -> Arc<JwtTokenProvider> {
        Arc::clone(&input.token_provider)
    }
}
