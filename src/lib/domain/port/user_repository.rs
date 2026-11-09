use std::future::Future;

use crate::domain::alias::NumericID;
use crate::domain::model::user::{Role, User};
use crate::domain::port::error::RepositoryError;

/// Search filters for [`UserRepository::search`]. Every field is optional;
/// an all-`None` filter returns every user.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct UserFilter {
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
pub(crate) trait UserRepository: Send + Sync {
    /// Insert a new `user`, returning the persisted user with its final
    /// identifier.
    ///
    /// The identifier of `user` is ignored: the repository assigns a fresh
    /// identity.
    fn create(&mut self, user: User) -> impl Future<Output = Result<User, RepositoryError>> + Send;

    /// Delete every user matching `filter`.
    ///
    /// Returns `Ok(true)` when at least one user was deleted, and `Ok(false)`
    /// when none matched. An all-`None` filter deletes every user; the root
    /// administrator is never deleted and is reported as
    /// [`RepositoryError::Conflict`].
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "part of the type's declared contract; no caller exercises it yet"
        )
    )]
    fn delete(
        &mut self,
        filter: &UserFilter,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Insert or update an existing `user` targeted by its identifier,
    /// returning the persisted user.
    fn save(&mut self, user: User) -> impl Future<Output = Result<User, RepositoryError>> + Send;

    /// Search users matching `filter`, returned as `Vec<User>`.
    ///
    /// Returns an empty list when no user matches; a missing match is a valid
    /// outcome, not an error.
    fn search(
        &mut self,
        filter: &UserFilter,
    ) -> impl Future<Output = Result<Vec<User>, RepositoryError>> + Send;
}

#[cfg(test)]
impl<T: UserRepository + ?Sized> UserRepository for &mut T {
    fn create(&mut self, user: User) -> impl Future<Output = Result<User, RepositoryError>> + Send {
        (**self).create(user)
    }

    fn delete(
        &mut self,
        filter: &UserFilter,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).delete(filter)
    }

    fn save(&mut self, user: User) -> impl Future<Output = Result<User, RepositoryError>> + Send {
        (**self).save(user)
    }

    fn search(
        &mut self,
        filter: &UserFilter,
    ) -> impl Future<Output = Result<Vec<User>, RepositoryError>> + Send {
        (**self).search(filter)
    }
}
