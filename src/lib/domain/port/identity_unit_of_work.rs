use crate::domain::port::{CredentialRepository, PermissionRepository, UnitOfWork};

/// Identity bounded-context view over a [`UnitOfWork`].
pub(crate) trait IdentityUnitOfWork: UnitOfWork {
    /// Repository persisting credentials.
    fn credentials(&mut self) -> impl CredentialRepository + '_;

    /// Repository persisting per-user permission overrides.
    fn permissions(&mut self) -> impl PermissionRepository + '_;
}
