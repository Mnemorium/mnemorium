use std::future::Future;

use crate::domain::port::error::UnitOfWorkError;

/// Transaction boundary spanning the repositories a use case needs.
///
/// The application service opens a unit of work, runs business logic across
/// the repositories it exposes, then commits on success or rolls back on
/// failure. Only the application service manages the transaction lifecycle.
pub trait UnitOfWork: Send + Sync {
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

/// Compile-time proof that [`UnitOfWork`] requires `Send + Sync`.
///
/// Never called: the body is type-checked against the trait's declared
/// supertraits, so removing `Sync` from [`UnitOfWork`] breaks the build.
#[cfg(test)]
#[expect(dead_code, reason = "compile-time bound assertion; never called")]
fn unit_of_work_is_send_and_sync<T: UnitOfWork>() {
    /// Assert `T` implements `Send + Sync`.
    #[expect(
        clippy::single_call_fn,
        reason = "a named bound assertion reads better than an inline bound"
    )]
    fn assert_send_sync<T: Send + Sync + ?Sized>() {}

    assert_send_sync::<T>();
}
