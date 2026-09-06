use std::future::Future;

use crate::domain::alias::NumericID;
use crate::domain::model::user::{Role, User};
use crate::domain::port::error::RepositoryError;

/// Search filters for [`UserRepository::search`]. Every field is optional;
/// an all-`None` filter returns every user.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct UserFilter {
    /// Filter on the email address.
    pub email: Option<String>,
    /// Filter on the user identifier.
    pub id: Option<NumericID>,
    /// Filter on the role.
    pub role: Option<Role>,
    /// Filter on the username.
    pub username: Option<String>,
}

/// Port for persisting and querying `User`.
#[cfg_attr(test, mockall::automock)]
pub trait UserRepository: Send + Sync {
    /// Delete the user identified by `id`.
    ///
    /// Returns `Ok(true)` when a user matched `id` and was deleted, and
    /// `Ok(false)` when no user matched. A missing user is a valid outcome,
    /// not an error.
    fn delete(&self, id: NumericID) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Insert or update `user`.
    fn save(&self, user: User) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// Search users matching `filter`, returned as `Vec<User>`.
    ///
    /// Returns an empty list when no user matches; a missing match is a valid
    /// outcome, not an error.
    fn search(
        &self,
        filter: &UserFilter,
    ) -> impl Future<Output = Result<Vec<User>, RepositoryError>> + Send;
}
