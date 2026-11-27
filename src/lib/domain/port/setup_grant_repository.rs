use std::future::Future;

use chrono::NaiveDateTime;

use crate::domain::model::SetupGrant;
use crate::domain::port::RepositoryError;

/// Port for persisting and reading the single Root Admin setup grant.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait SetupGrantRepository: Send + Sync {
    /// Consume the live grant at `now`, returning whether a row was consumed.
    ///
    /// The update is conditional on the grant being unconsumed and unexpired,
    /// so two concurrent submissions can never both succeed.
    fn consume(
        &mut self,
        now: NaiveDateTime,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Insert the singleton grant, returning the stored row.
    fn create(
        &mut self,
        grant: SetupGrant,
    ) -> impl Future<Output = Result<SetupGrant, RepositoryError>> + Send;

    /// Read the singleton grant, or `None` when no row exists.
    fn find(&mut self) -> impl Future<Output = Result<Option<SetupGrant>, RepositoryError>> + Send;

    /// Replace the singleton grant's nonce and expiry and clear its consumption.
    fn save(
        &mut self,
        grant: SetupGrant,
    ) -> impl Future<Output = Result<SetupGrant, RepositoryError>> + Send;
}

#[cfg(test)]
impl<T: SetupGrantRepository + ?Sized> SetupGrantRepository for &mut T {
    fn consume(
        &mut self,
        now: NaiveDateTime,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).consume(now)
    }

    fn create(
        &mut self,
        grant: SetupGrant,
    ) -> impl Future<Output = Result<SetupGrant, RepositoryError>> + Send {
        (**self).create(grant)
    }

    fn find(&mut self) -> impl Future<Output = Result<Option<SetupGrant>, RepositoryError>> + Send {
        (**self).find()
    }

    fn save(
        &mut self,
        grant: SetupGrant,
    ) -> impl Future<Output = Result<SetupGrant, RepositoryError>> + Send {
        (**self).save(grant)
    }
}
