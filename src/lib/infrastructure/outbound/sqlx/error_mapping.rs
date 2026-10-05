//! Error mapping from `sqlx::Error` to the repository port error.

use sqlx::Error;
use sqlx::error::ErrorKind;
use tracing::error;

use crate::domain::port::error::RepositoryError;
use crate::domain::port::error::UnitOfWorkError;

/// `SQLite` extended result code for a trigger `RAISE(ABORT)`
/// (`SQLITE_CONSTRAINT_TRIGGER`), which `sqlx` collapses into `ErrorKind::Other`.
const SQLITE_CONSTRAINT_TRIGGER: &str = "1811";

impl From<Error> for RepositoryError {
    fn from(err: Error) -> Self {
        let (kind, mapped) = match err {
            Error::Database(db) => match db.kind() {
                ErrorKind::UniqueViolation => ("unique_violation", Self::AlreadyExist),
                ErrorKind::ForeignKeyViolation => {
                    ("foreign_key_violation", Self::DataIntegrityViolation)
                }
                ErrorKind::NotNullViolation => ("not_null_violation", Self::DataIntegrityViolation),
                ErrorKind::CheckViolation => ("check_violation", Self::DataIntegrityViolation),
                ErrorKind::ExclusionViolation => {
                    ("exclusion_violation", Self::DataIntegrityViolation)
                }
                ErrorKind::Other if db.code().as_deref() == Some(SQLITE_CONSTRAINT_TRIGGER) => {
                    ("conflict", Self::Conflict)
                }
                ErrorKind::Other | _ => ("database_error", Self::OperationFailed),
            },
            Error::PoolTimedOut => ("timeout", Self::Timeout),
            Error::PoolClosed
            | Error::WorkerCrashed
            | Error::Io(_)
            | Error::Configuration(_)
            | Error::Tls(_)
            | Error::ConfigFile(_) => ("unavailable", Self::Unavailable),
            Error::Protocol(_)
            | Error::InvalidArgument(_)
            | Error::RowNotFound
            | Error::TypeNotFound { .. }
            | Error::ColumnIndexOutOfBounds { .. }
            | Error::ColumnNotFound(_)
            | Error::ColumnDecode { .. }
            | Error::Encode(_)
            | Error::Decode(_)
            | Error::AnyDriverError(_)
            | Error::InvalidSavePointStatement => ("operation_failed", Self::OperationFailed),
            Error::Migrate(migration_error) => (
                "migration_failed",
                Self::Unknown(anyhow::anyhow!(migration_error)),
            ),
            Error::BeginFailed => (
                "begin_failed",
                Self::Unknown(anyhow::anyhow!("beginning the transaction failed")),
            ),
            other => ("unknown", Self::Unknown(anyhow::anyhow!(other))),
        };
        error!(
            target: "security",
            event = "port_fault",
            kind = %kind,
            operation = "repository",
            "an outbound dependency failed"
        );
        mapped
    }
}

impl From<Error> for UnitOfWorkError {
    fn from(err: Error) -> Self {
        let (kind, mapped) = match err {
            Error::PoolTimedOut => ("timeout", Self::Unavailable),
            Error::PoolClosed
            | Error::WorkerCrashed
            | Error::Io(_)
            | Error::Configuration(_)
            | Error::Tls(_)
            | Error::ConfigFile(_) => ("unavailable", Self::Unavailable),
            Error::Migrate(migration_error) => (
                "migration_failed",
                Self::Unknown(anyhow::anyhow!(migration_error)),
            ),
            Error::Database(_)
            | Error::Protocol(_)
            | Error::InvalidArgument(_)
            | Error::RowNotFound
            | Error::TypeNotFound { .. }
            | Error::ColumnIndexOutOfBounds { .. }
            | Error::ColumnNotFound(_)
            | Error::ColumnDecode { .. }
            | Error::Encode(_)
            | Error::Decode(_)
            | Error::AnyDriverError(_)
            | Error::InvalidSavePointStatement
            | Error::BeginFailed
            | _ => ("operation_failed", Self::OperationFailed),
        };
        error!(
            target: "security",
            event = "port_fault",
            kind = %kind,
            operation = "unit_of_work",
            "an outbound dependency failed"
        );
        mapped
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use sqlx::SqlitePool;
    use sqlx::sqlite::SqlitePoolOptions;

    use crate::domain::port::error::RepositoryError;

    /// Build an in-memory `SQLite` pool holding `schema`.
    async fn probe_pool(schema: &'static str) -> Result<SqlitePool, Box<dyn Error>> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        sqlx::raw_sql(schema).execute(&pool).await?;
        Ok(pool)
    }

    #[tokio::test]
    async fn constraint_trigger_abort_returns_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange: a guard trigger whose `RAISE(ABORT)` reports extended code 1811.
        let pool = probe_pool(
            "CREATE TABLE probe (value INTEGER);
             CREATE TRIGGER tg_probe_abort
             BEFORE INSERT ON probe
             FOR EACH ROW
             WHEN new.value = 0
             BEGIN
                 SELECT RAISE(ABORT, 'probe rejects zero');
             END;",
        )
        .await?;

        // Act
        let mapped = sqlx::query("INSERT INTO probe (value) VALUES (0)")
            .execute(&pool)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(mapped, Err(RepositoryError::Conflict)));
        Ok(())
    }

    #[tokio::test]
    async fn check_violation_returns_data_integrity_violation() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = probe_pool("CREATE TABLE probe (value INTEGER CHECK (value > 0));").await?;

        // Act
        let mapped = sqlx::query("INSERT INTO probe (value) VALUES (0)")
            .execute(&pool)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(
            mapped,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn unique_violation_returns_already_exist() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = probe_pool("CREATE TABLE probe (value INTEGER UNIQUE);").await?;
        sqlx::query("INSERT INTO probe (value) VALUES (1)")
            .execute(&pool)
            .await?;

        // Act
        let mapped = sqlx::query("INSERT INTO probe (value) VALUES (1)")
            .execute(&pool)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(mapped, Err(RepositoryError::AlreadyExist)));
        Ok(())
    }
}
