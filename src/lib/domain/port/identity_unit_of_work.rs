use crate::domain::port::{CredentialRepository, GrantRepository, ResourceRepository, UnitOfWork};

/// Identity bounded-context view over a [`UnitOfWork`].
pub(crate) trait IdentityUnitOfWork: UnitOfWork {
    /// Repository persisting credentials.
    fn credentials(&mut self) -> impl CredentialRepository + '_;

    /// Repository reading and writing grants.
    fn grants(&mut self) -> impl GrantRepository + '_;

    /// Repository reading the resource graph and writing its resources.
    fn resources(&mut self) -> impl ResourceRepository + '_;
}
