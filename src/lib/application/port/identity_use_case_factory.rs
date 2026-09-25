use std::sync::Arc;

use crate::application::port::login_user::LoginUserUseCase;
use crate::application::port::patch_credential::PatchCredentialUseCase;
use crate::application::port::register_user::RegisterUserUseCase;

/// Builds the use cases of the Identity bounded context on demand.
///
/// The inbound layer depends on this port instead of a pre-built use case, so
/// that every request gets a use case bound to the current configuration.
#[cfg_attr(test, mockall::automock)]
pub trait IdentityUseCaseFactory: Send + Sync {
    /// Build the use case authenticating a user.
    fn login_user(&self) -> Arc<dyn LoginUserUseCase>;

    /// Build the use case changing the password behind a credential.
    fn patch_credential(&self) -> Arc<dyn PatchCredentialUseCase>;

    /// Build the use case registering a new user.
    fn register_user(&self) -> Arc<dyn RegisterUserUseCase>;
}
