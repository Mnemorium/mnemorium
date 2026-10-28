use crate::domain::port::credential_repository::CredentialRepository;
use crate::domain::port::permission_repository::PermissionRepository;
use crate::domain::port::unit_of_work::UnitOfWork;

/// Identity bounded-context view over a [`UnitOfWork`].
pub trait IdentityUnitOfWork: UnitOfWork {
    /// Repository persisting credentials.
    fn credentials(&mut self) -> impl CredentialRepository + '_;

    /// Repository persisting per-user permission overrides.
    fn permissions(&mut self) -> impl PermissionRepository + '_;
}
