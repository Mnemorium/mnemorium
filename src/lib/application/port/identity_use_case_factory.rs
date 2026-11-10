use std::sync::Arc;

use crate::application::port::{
    GetPermissionUseCase, LoginUserUseCase, PatchCredentialUseCase, RegisterUserUseCase,
    ReplacePermissionUseCase,
};

/// Builds the use cases of the Identity bounded context on demand.
///
/// The inbound layer depends on this port instead of a pre-built use case, so
/// that every request gets a use case bound to the current configuration.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait IdentityUseCaseFactory: Send + Sync {
    /// Build the use case reading a user's effective permissions.
    fn get_permission(&self) -> Arc<dyn GetPermissionUseCase>;

    /// Build the use case authenticating a user.
    fn login_user(&self) -> Arc<dyn LoginUserUseCase>;

    /// Build the use case changing the password behind a credential.
    fn patch_credential(&self) -> Arc<dyn PatchCredentialUseCase>;

    /// Build the use case registering a new user.
    fn register_user(&self) -> Arc<dyn RegisterUserUseCase>;

    /// Build the use case replacing a user's permission overrides.
    fn replace_permission(&self) -> Arc<dyn ReplacePermissionUseCase>;
}
