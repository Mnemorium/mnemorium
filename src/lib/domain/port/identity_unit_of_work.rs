use crate::domain::port::credential_repository::CredentialRepository;
use crate::domain::port::unit_of_work::UnitOfWork;

/// Identity bounded-context view over a [`UnitOfWork`].
pub trait IdentityUnitOfWork: UnitOfWork {
    /// Repository persisting credentials.
    fn credentials(&mut self) -> impl CredentialRepository + '_;
}
