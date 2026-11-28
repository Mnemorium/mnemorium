use std::future::Future;

use crate::domain::port::UnitOfWorkError;

/// Transaction boundary spanning the repositories a use case needs.
///
/// The application service opens a unit of work, runs business logic across
/// the repositories it exposes, then commits on success or rolls back on
/// failure. Only the application service manages the transaction lifecycle.
pub(crate) trait UnitOfWork: Send + Sync {
    /// Commit the unit of work, persisting every change made through it.
    fn commit(self) -> impl Future<Output = Result<(), UnitOfWorkError>> + Send;

    /// Roll back the unit of work, discarding every change made through it.
    fn rollback(self) -> impl Future<Output = Result<(), UnitOfWorkError>> + Send;
}

/// Opens a new [`UnitOfWork`] bound to a single database transaction.
pub(crate) trait UnitOfWorkFactory: Send + Sync {
    /// Concrete unit of work produced by this factory.
    type Uow: UnitOfWork;

    /// Open a read-only unit of work.
    ///
    /// The adapter takes no write lock until the unit of work writes.
    fn begin(&self) -> impl Future<Output = Result<Self::Uow, UnitOfWorkError>> + Send;

    /// Open a unit of work that will write.
    ///
    /// A use case that writes must open through this method: a deferred
    /// transaction that reads before it writes pins a snapshot, and upgrading
    /// over a snapshot another writer committed meanwhile fails immediately
    /// instead of waiting on the busy timeout.
    fn begin_write(&self) -> impl Future<Output = Result<Self::Uow, UnitOfWorkError>> + Send;
}
