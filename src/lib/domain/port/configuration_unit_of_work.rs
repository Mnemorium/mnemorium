use crate::domain::port::configuration_repository::ConfigurationRepository;
use crate::domain::port::unit_of_work::UnitOfWork;

/// Configuration bounded-context view over a [`UnitOfWork`].
pub trait ConfigurationUnitOfWork: UnitOfWork {
    /// Repository persisting the configuration singleton.
    fn configuration(&mut self) -> impl ConfigurationRepository + '_;
}
