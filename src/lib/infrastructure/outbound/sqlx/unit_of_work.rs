use sqlx::Sqlite;
use sqlx::SqlitePool;
use sqlx::Transaction;

use crate::domain::port::configuration_repository::ConfigurationRepository;
use crate::domain::port::configuration_unit_of_work::ConfigurationUnitOfWork;
use crate::domain::port::credential_repository::CredentialRepository;
use crate::domain::port::error::UnitOfWorkError;
use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
use crate::domain::port::unit_of_work::UnitOfWork;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserRepository;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::infrastructure::outbound::sqlx::configuration_repository::SqlxConfigurationRepository;
use crate::infrastructure::outbound::sqlx::credential_repository::SqlxCredentialRepository;
use crate::infrastructure::outbound::sqlx::user_repository::SqlxUserRepository;

/// Unit of work backed by a single `SQLite` transaction.
pub struct SqlxUnitOfWork {
    /// Transaction shared by every repository the unit of work exposes.
    transaction: Transaction<'static, Sqlite>,
}

impl SqlxUnitOfWork {
    /// Borrow the transaction backing this unit of work.
    fn transaction(&mut self) -> &mut Transaction<'static, Sqlite> {
        &mut self.transaction
    }
}

impl ConfigurationUnitOfWork for SqlxUnitOfWork {
    fn configuration(&mut self) -> impl ConfigurationRepository + '_ {
        SqlxConfigurationRepository::new(self.transaction())
    }
}

impl IdentityUnitOfWork for SqlxUnitOfWork {
    fn credentials(&mut self) -> impl CredentialRepository + '_ {
        SqlxCredentialRepository::new(self.transaction())
    }
}

impl UserUnitOfWork for SqlxUnitOfWork {
    fn users(&mut self) -> impl UserRepository + '_ {
        SqlxUserRepository::new(self.transaction())
    }
}

impl UnitOfWork for SqlxUnitOfWork {
    async fn commit(self) -> Result<(), UnitOfWorkError> {
        let Self { transaction } = self;
        transaction.commit().await.map_err(UnitOfWorkError::from)
    }

    async fn rollback(self) -> Result<(), UnitOfWorkError> {
        let Self { transaction } = self;
        transaction.rollback().await.map_err(UnitOfWorkError::from)
    }
}

/// Factory opening [`SqlxUnitOfWork`] instances from a `SQLite` pool.
pub struct SqlxUnitOfWorkFactory {
    /// Connection pool the transactions are acquired from.
    pool: SqlitePool,
}

impl SqlxUnitOfWorkFactory {
    /// Create a new factory bound to `pool`.
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl UnitOfWorkFactory for SqlxUnitOfWorkFactory {
    type Uow = SqlxUnitOfWork;

    async fn begin(&self) -> Result<Self::Uow, UnitOfWorkError> {
        let transaction = self.pool.begin().await.map_err(UnitOfWorkError::from)?;
        Ok(SqlxUnitOfWork { transaction })
    }
}
