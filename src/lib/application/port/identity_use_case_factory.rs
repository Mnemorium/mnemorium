use std::sync::Arc;

use crate::application::port::{
    GetPermissionSummaryUseCase, GetResourcePermissionUseCase, LoginUserUseCase,
    PatchCredentialUseCase, PutResourcePermissionUseCase, RegenerateTokenUseCase,
    RegisterUserUseCase, RevokeShareUseCase, ShareResourceUseCase,
};

/// Builds the use cases of the Identity bounded context on demand.
///
/// The inbound layer depends on this port instead of a pre-built use case, so
/// that every request gets a use case bound to the current configuration.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait IdentityUseCaseFactory: Send + Sync {
    /// Build the use case returning one user's effective permissions.
    fn get_permission_summary(&self) -> Arc<dyn GetPermissionSummaryUseCase>;

    /// Build the use case returning one resource's permission.
    fn get_resource_permission(&self) -> Arc<dyn GetResourcePermissionUseCase>;

    /// Build the use case authenticating a user.
    fn login_user(&self) -> Arc<dyn LoginUserUseCase>;

    /// Build the use case changing the password behind a credential.
    fn patch_credential(&self) -> Arc<dyn PatchCredentialUseCase>;

    /// Build the use case replacing one resource's permission.
    fn put_resource_permission(&self) -> Arc<dyn PutResourcePermissionUseCase>;

    /// Build the use case regenerating the caller's access token.
    fn regenerate_token(&self) -> Arc<dyn RegenerateTokenUseCase>;

    /// Build the use case registering a new user.
    fn register_user(&self) -> Arc<dyn RegisterUserUseCase>;

    /// Build the use case revoking one grant.
    fn revoke_share(&self) -> Arc<dyn RevokeShareUseCase>;

    /// Build the use case sharing one resource.
    fn share_resource(&self) -> Arc<dyn ShareResourceUseCase>;
}
