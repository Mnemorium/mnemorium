pub mod get_current_user;
pub mod initialize_root_admin;
pub mod login_user;
pub mod register_user;

use std::sync::Arc;

use crate::application::port::get_current_user::GetCurrentUserUseCase;
use crate::application::port::login_user::LoginUserUseCase;
use crate::application::port::register_user::RegisterUserUseCase;

/// All use cases exposed to the inbound layer.
///
/// Passed as a whole to the route setup, where each endpoint picks the
/// single use case (or the few) it serves.
#[derive(Clone)]
pub struct UseCaseCatalog {
    /// Use case for fetching the current user.
    pub get_current_user: Arc<dyn GetCurrentUserUseCase>,
    /// Use case for authenticating users.
    pub login_user: Arc<dyn LoginUserUseCase>,
    /// Use case for registering users.
    pub register_user: Arc<dyn RegisterUserUseCase>,
}

impl UseCaseCatalog {
    /// Create a new use case catalog.
    #[must_use]
    pub fn new(
        get_current_user: Arc<dyn GetCurrentUserUseCase>,
        login_user: Arc<dyn LoginUserUseCase>,
        register_user: Arc<dyn RegisterUserUseCase>,
    ) -> Self {
        Self {
            get_current_user,
            login_user,
            register_user,
        }
    }
}
