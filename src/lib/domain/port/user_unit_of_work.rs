use crate::domain::port::{UnitOfWork, UserRepository};

/// User bounded-context view over a [`UnitOfWork`].
pub(crate) trait UserUnitOfWork: UnitOfWork {
    /// Repository persisting users.
    fn users(&mut self) -> impl UserRepository + '_;
}
