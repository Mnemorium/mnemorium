use crate::domain::alias::NumericID;
use crate::domain::model::user::User;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository;

/// Repository persisting users, backed by `SQLite`.
pub struct SqlxUserRepository;

impl SqlxUserRepository {
    /// Create a new repository.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for SqlxUserRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl UserRepository for SqlxUserRepository {
    async fn delete(&self, _id: NumericID) -> Result<bool, RepositoryError> {
        unimplemented!()
    }

    async fn save(&self, _user: User) -> Result<User, RepositoryError> {
        unimplemented!()
    }

    async fn search(&self, _filter: &UserFilter) -> Result<Vec<User>, RepositoryError> {
        unimplemented!()
    }
}
