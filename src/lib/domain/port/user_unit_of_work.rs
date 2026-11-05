use crate::domain::port::unit_of_work::UnitOfWork;
use crate::domain::port::user_repository::UserRepository;

/// User bounded-context view over a [`UnitOfWork`].
pub(crate) trait UserUnitOfWork: UnitOfWork {
    /// Repository persisting users.
    fn users(&mut self) -> impl UserRepository + '_;
}
