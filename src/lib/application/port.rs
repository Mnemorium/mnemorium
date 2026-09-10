pub mod get_current_user;
pub mod get_user;
pub mod initialize_root_admin;
pub mod login_user;
pub mod patch_credential;
pub mod register_user;
pub mod update_user;

use std::sync::Arc;

use crate::application::port::get_current_user::GetCurrentUserUseCase;
use crate::application::port::get_user::GetUserUseCase;
use crate::application::port::login_user::LoginUserUseCase;
use crate::application::port::patch_credential::PatchCredentialUseCase;
use crate::application::port::register_user::RegisterUserUseCase;
use crate::application::port::update_user::UpdateUserUseCase;

/// All use cases exposed to the inbound layer.
///
/// Passed as a whole to the route setup, where each endpoint picks the
/// single use case (or the few) it serves.
#[derive(Clone)]
pub struct UseCaseCatalog {
    /// Use case for fetching the current user.
    pub get_current_user: Arc<dyn GetCurrentUserUseCase>,
    /// Use case for fetching a user by identifier.
    pub get_user: Arc<dyn GetUserUseCase>,
    /// Use case for authenticating users.
    pub login_user: Arc<dyn LoginUserUseCase>,
    /// Use case for changing the password behind a credential.
    pub patch_credential: Arc<dyn PatchCredentialUseCase>,
    /// Use case for registering users.
    pub register_user: Arc<dyn RegisterUserUseCase>,
    /// Use case for updating the profile of a user.
    pub update_user: Arc<dyn UpdateUserUseCase>,
}

impl UseCaseCatalog {
    /// Create a new use case catalog.
    #[must_use]
    pub fn new(
        get_current_user: Arc<dyn GetCurrentUserUseCase>,
        get_user: Arc<dyn GetUserUseCase>,
        login_user: Arc<dyn LoginUserUseCase>,
        patch_credential: Arc<dyn PatchCredentialUseCase>,
        register_user: Arc<dyn RegisterUserUseCase>,
        update_user: Arc<dyn UpdateUserUseCase>,
    ) -> Self {
        Self {
            get_current_user,
            get_user,
            login_user,
            patch_credential,
            register_user,
            update_user,
        }
    }
}
