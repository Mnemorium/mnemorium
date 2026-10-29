//! Error mapping from `sqlx::Error` to the repository port error.

use sqlx::Error;
use sqlx::error::ErrorKind;
use tracing::error;
use tracing::warn;

use crate::domain::port::error::RepositoryError;
use crate::domain::port::error::UnitOfWorkError;

/// `SQLite` extended result code for a trigger `RAISE(ABORT)`
/// (`SQLITE_CONSTRAINT_TRIGGER`), which `sqlx` collapses into `ErrorKind::Other`.
/// The code is `1811`, carried as a string because the consumer compares text.
const SQLITE_CONSTRAINT_TRIGGER: &str = "1811";

impl From<Error> for RepositoryError {
    fn from(err: Error) -> Self {
        let (kind, expected_constraint, mapped) = match err {
            Error::Database(db) => match db.kind() {
                ErrorKind::UniqueViolation => ("unique_violation", true, Self::AlreadyExist),
                ErrorKind::ForeignKeyViolation => {
                    ("foreign_key_violation", false, Self::DataIntegrityViolation)
                }
                ErrorKind::NotNullViolation => {
                    ("not_null_violation", false, Self::DataIntegrityViolation)
                }
                ErrorKind::CheckViolation => {
                    ("check_violation", false, Self::DataIntegrityViolation)
                }
                ErrorKind::ExclusionViolation => {
                    ("exclusion_violation", false, Self::DataIntegrityViolation)
                }
                // A trigger `RAISE(ABORT)` guards the current state: the
                // requested write conflicts with the persisted row. The
                // extended code classifies the failure; it is never logged
                // (`OBS-003`).
                ErrorKind::Other if db.code().as_deref() == Some(SQLITE_CONSTRAINT_TRIGGER) => {
                    ("conflict", true, Self::Conflict)
                }
                ErrorKind::Other | _ => ("database_error", false, Self::OperationFailed),
            },
            Error::PoolTimedOut => ("timeout", false, Self::Timeout),
            Error::PoolClosed
            | Error::WorkerCrashed
            | Error::Io(_)
            | Error::Configuration(_)
            | Error::Tls(_)
            | Error::ConfigFile(_) => ("unavailable", false, Self::Unavailable),
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
            | Error::InvalidSavePointStatement => {
                ("operation_failed", false, Self::OperationFailed)
            }
            Error::Migrate(migration_error) => (
                "migration_failed",
                false,
                Self::Unknown(anyhow::anyhow!(migration_error)),
            ),
            Error::BeginFailed => (
                "begin_failed",
                false,
                Self::Unknown(anyhow::anyhow!("beginning the transaction failed")),
            ),
            other => ("unknown", false, Self::Unknown(anyhow::anyhow!(other))),
        };
        emit_catalogued_failure(kind, "repository", expected_constraint);
        mapped
    }
}

impl From<Error> for UnitOfWorkError {
    fn from(err: Error) -> Self {
        // These arms are never expected constraints: the schema declares no
        // `DEFERRABLE` foreign key, so `begin`/`commit`/`rollback` cannot
        // surface one. Every arm is therefore a `port_fault` at `error`
        // (`OBS-005`, § 9 § 2).
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
        emit_catalogued_failure(kind, "unit_of_work", false);
        mapped
    }
}

/// Emit the catalogued `output_validation_failed` event for stored data that
/// fails to decode or validate, and return the mapped repository error.
///
/// The adapter owns the detail of a stored value that no longer matches the
/// domain (`OBS-002`). Only the stable `reason` classification reaches the
/// sink; the stored value and the decoder detail never do (`OBS-003`).
pub(crate) fn stored_data_invalid(reason: &'static str) -> RepositoryError {
    warn!(
        target: "security",
        event = "output_validation_failed",
        reason = reason,
        "rejected stored data that failed validation"
    );
    RepositoryError::DataIntegrityViolation
}

/// Emit the catalogued event for a classified `sqlx` failure (`OBS-006`).
///
/// An expected constraint is a constraint outcome a use case handles as a
/// business result (`AlreadyExist`, `Conflict`), so it is a reviewable event
/// logged at `warn` as `constraint_rejected`. Every other failure is an
/// operator-actionable dependency fault logged at `error` as `port_fault`
/// (`OBS-005`). Only the stable `kind` and `operation` are logged, never the
/// `sqlx` error `Debug` or a driver code (`OBS-003`).
///
/// Exposed to the other `SQLx` outbound adapters so a failure they classify
/// themselves is logged once, at the adapter that owns it (`OBS-002`).
pub(crate) fn emit_catalogued_failure(kind: &str, operation: &str, expected_constraint: bool) {
    if expected_constraint {
        warn!(
            target: "security",
            event = "constraint_rejected",
            kind = kind,
            operation = operation,
            "an expected constraint was handled as a business result"
        );
    } else {
        error!(
            target: "security",
            event = "port_fault",
            kind = kind,
            operation = operation,
            "an outbound dependency failed"
        );
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use sqlx::SqlitePool;
    use sqlx::sqlite::SqlitePoolOptions;

    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::error::UnitOfWorkError;
    use tracing_test::traced_test;

    /// Build an in-memory `SQLite` pool holding `schema`.
    async fn probe_pool(schema: &'static str) -> Result<SqlitePool, Box<dyn Error>> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        sqlx::raw_sql(schema).execute(&pool).await?;
        Ok(pool)
    }

    /// Assert the captured output is the single `warn` `constraint_rejected`
    /// event for `kind` on the reserved `security` target.
    fn assert_constraint_rejected(lines: &[&str], kind: &str) -> Result<(), String> {
        let ours: Vec<&str> = lines
            .iter()
            .copied()
            .filter(|line| line.contains("security:"))
            .collect();
        let warn_count = ours.iter().filter(|line| line.contains("WARN")).count();
        if warn_count != 1 {
            return Err(format!(
                "an expected constraint must emit exactly one warn event (found {warn_count})"
            ));
        }
        if !ours
            .iter()
            .any(|line| line.contains("event=\"constraint_rejected\""))
        {
            return Err("the event must be constraint_rejected".to_owned());
        }
        if !ours
            .iter()
            .any(|line| line.contains(&format!("kind=\"{kind}\"")))
        {
            return Err(format!("the event must carry the stable kind `{kind}`"));
        }
        if !ours
            .iter()
            .any(|line| line.contains("operation=\"repository\""))
        {
            return Err("the event must name the repository operation".to_owned());
        }
        if !ours.iter().any(|line| line.contains("security")) {
            return Err("the event must target the reserved security target".to_owned());
        }
        Ok(())
    }

    /// Assert the captured output is the single `error` `port_fault` event for
    /// `kind` and `operation` on the reserved `security` target.
    fn assert_port_fault(lines: &[&str], kind: &str, operation: &str) -> Result<(), String> {
        let ours: Vec<&str> = lines
            .iter()
            .copied()
            .filter(|line| line.contains("security:"))
            .collect();
        let error_count = ours.iter().filter(|line| line.contains("ERROR")).count();
        if error_count != 1 {
            return Err(format!(
                "a dependency fault must emit exactly one error event (found {error_count})"
            ));
        }
        if !ours
            .iter()
            .any(|line| line.contains("event=\"port_fault\""))
        {
            return Err("the event must be port_fault".to_owned());
        }
        if !ours
            .iter()
            .any(|line| line.contains(&format!("kind=\"{kind}\"")))
        {
            return Err(format!("the event must carry the stable kind `{kind}`"));
        }
        if !ours
            .iter()
            .any(|line| line.contains(&format!("operation=\"{operation}\"")))
        {
            return Err("the event must name the owning operation".to_owned());
        }
        if !ours.iter().any(|line| line.contains("security")) {
            return Err("the event must target the reserved security target".to_owned());
        }
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn constraint_trigger_abort_logs_constraint_rejected() -> Result<(), Box<dyn Error>> {
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

        // Assert: a handled conflict is a `warn` `constraint_rejected` event.
        assert!(matches!(mapped, Err(RepositoryError::Conflict)));
        logs_assert(|lines| assert_constraint_rejected(lines, "conflict"));
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn check_violation_logs_port_fault() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = probe_pool("CREATE TABLE probe (value INTEGER CHECK (value > 0));").await?;

        // Act
        let mapped = sqlx::query("INSERT INTO probe (value) VALUES (0)")
            .execute(&pool)
            .await
            .map_err(RepositoryError::from);

        // Assert: an integrity violation is a genuine fault, not a business result.
        assert!(matches!(
            mapped,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        logs_assert(|lines| assert_port_fault(lines, "check_violation", "repository"));
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn unique_violation_logs_constraint_rejected() -> Result<(), Box<dyn Error>> {
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
        logs_assert(|lines| assert_constraint_rejected(lines, "unique_violation"));
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn port_fault_logs_never_carry_the_sqlx_debug() -> Result<(), Box<dyn Error>> {
        // Arrange: a trigger whose `RAISE(ABORT)` message is a unique sentinel.
        let pool = probe_pool(
            "CREATE TABLE probe (value INTEGER);
             CREATE TRIGGER tg_probe_leak
             BEFORE INSERT ON probe
             FOR EACH ROW
             WHEN new.value = 0
             BEGIN
                 SELECT RAISE(ABORT, 'LEAK_SENTINEL');
             END;",
        )
        .await?;

        // Act
        let mapped = sqlx::query("INSERT INTO probe (value) VALUES (0)")
            .execute(&pool)
            .await
            .map_err(RepositoryError::from);

        // Assert: the mapped event carries only the stable classification.
        assert!(matches!(mapped, Err(RepositoryError::Conflict)));
        logs_assert(|lines| {
            let leaked = lines
                .iter()
                .filter(|line| line.contains("security:"))
                .any(|line| line.contains("LEAK_SENTINEL") || line.contains("SqliteError"));
            (!leaked)
                .then_some(())
                .ok_or_else(|| "the event must not carry the raw sqlx detail".to_owned())
        });
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn unit_of_work_failure_logs_port_fault() -> Result<(), Box<dyn Error>> {
        // Arrange: a database failure surfaced through the unit of work.
        let pool = probe_pool("CREATE TABLE probe (value INTEGER);").await?;

        // Act
        let mapped = sqlx::query("SELECT missing FROM probe")
            .execute(&pool)
            .await
            .map_err(UnitOfWorkError::from);

        // Assert: no expected constraint can arise here, so it stays a fault.
        assert!(matches!(mapped, Err(UnitOfWorkError::OperationFailed)));
        logs_assert(|lines| assert_port_fault(lines, "operation_failed", "unit_of_work"));
        Ok(())
    }
}
