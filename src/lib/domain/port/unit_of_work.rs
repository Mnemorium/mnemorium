use std::future::Future;

use crate::domain::port::error::UnitOfWorkError;

/// Transaction boundary spanning the repositories a use case needs.
///
/// The application service opens a unit of work, runs business logic across
/// the repositories it exposes, then commits on success or rolls back on
/// failure. Only the application service manages the transaction lifecycle.
pub trait UnitOfWork: Send {
    /// Commit the unit of work, persisting every change made through it.
    fn commit(self) -> impl Future<Output = Result<(), UnitOfWorkError>> + Send;

    /// Roll back the unit of work, discarding every change made through it.
    fn rollback(self) -> impl Future<Output = Result<(), UnitOfWorkError>> + Send;
}

/// Opens a new [`UnitOfWork`] bound to a single database transaction.
pub trait UnitOfWorkFactory: Send + Sync {
    /// Concrete unit of work produced by this factory.
    type Uow: UnitOfWork;

    /// Open a new unit of work.
    fn begin(&self) -> impl Future<Output = Result<Self::Uow, UnitOfWorkError>> + Send;
}
