use std::sync::Arc;

use crate::application::port::get_current_user::GetCurrentUserUseCase;
use crate::application::port::get_user::GetUserUseCase;
use crate::application::port::update_user::UpdateUserUseCase;

/// Builds the use cases of the User bounded context on demand.
///
/// The inbound layer depends on this port instead of a pre-built use case, so
/// that every request gets a use case bound to the current dependencies.
#[cfg_attr(test, mockall::automock)]
pub trait UserUseCaseFactory: Send + Sync {
    /// Build the use case fetching the current user.
    fn get_current_user(&self) -> Arc<dyn GetCurrentUserUseCase>;

    /// Build the use case fetching a user by identifier.
    fn get_user_by_id(&self) -> Arc<dyn GetUserUseCase>;

    /// Build the use case updating the profile of a user.
    fn update_user(&self) -> Arc<dyn UpdateUserUseCase>;
}
