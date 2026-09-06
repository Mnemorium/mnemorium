use crate::domain::alias::NumericID;
use crate::domain::model::credential::Credential;
use crate::domain::port::credential_repository::CredentialRepository;
use crate::domain::port::error::RepositoryError;

/// Repository persisting credentials, backed by `SQLite`.
pub struct SqlxCredentialRepository;

impl SqlxCredentialRepository {
    /// Create a new repository.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for SqlxCredentialRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialRepository for SqlxCredentialRepository {
    async fn delete(&self, _id: NumericID) -> Result<bool, RepositoryError> {
        unimplemented!()
    }

    async fn find(&self, _id: NumericID) -> Result<Option<Credential>, RepositoryError> {
        unimplemented!()
    }

    async fn save(&self, _credential: Credential) -> Result<Credential, RepositoryError> {
        unimplemented!()
    }
}
