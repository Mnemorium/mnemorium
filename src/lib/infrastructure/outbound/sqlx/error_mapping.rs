//! Error mapping from `sqlx::Error` to the repository port error.

use sqlx::Error;
use sqlx::error::ErrorKind;
use tracing::error;
use tracing::warn;

use crate::domain::port::error::RepositoryError;
use crate::domain::port::error::UnitOfWorkError;

/// `SQLite` extended result code for a trigger `RAISE(ABORT)`
/// (`SQLITE_CONSTRAINT_TRIGGER`), which `sqlx` collapses into `ErrorKind::Other`.
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

/// Emit the catalogued event for a classified `sqlx` failure (`OBS-006`).
///
/// An expected constraint is a constraint outcome a use case handles as a
/// business result (`AlreadyExist`, `Conflict`), so it is a reviewable event
/// logged at `warn` as `constraint_rejected`. Every other failure is an
/// operator-actionable dependency fault logged at `error` as `port_fault`
/// (`OBS-005`). Only the stable `kind` and `operation` are logged, never the
/// `sqlx` error `Debug` or a driver code (`OBS-003`).
fn emit_catalogued_failure(kind: &str, operation: &str, expected_constraint: bool) {
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
    use std::io as stdio;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::PoisonError;

    use sqlx::SqlitePool;
    use sqlx::sqlite::SqlitePoolOptions;
    use tracing::subscriber::DefaultGuard;
    use tracing::subscriber::set_default;
    use tracing_subscriber::fmt;
    use tracing_subscriber::layer::SubscriberExt as _;

    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::error::UnitOfWorkError;

    /// Append-only sink that lets a test read back the lines [`fmt`] emits.
    #[derive(Clone)]
    struct CaptureWriter {
        /// Buffer shared with the test that asserts on the captured output.
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "only the raw `write` and `flush` are meaningful for an in-memory capture buffer"
    )]
    impl stdio::Write for CaptureWriter {
        fn flush(&mut self) -> stdio::Result<()> {
            Ok(())
        }

        fn write(&mut self, buf: &[u8]) -> stdio::Result<usize> {
            let mut buffer = self.buffer.lock().unwrap_or_else(PoisonError::into_inner);
            buffer.extend_from_slice(buf);
            Ok(buf.len())
        }
    }

    /// Hand every formatted event to a fresh clone of the shared buffer.
    #[expect(
        clippy::missing_trait_methods,
        reason = "the default `make_writer_for` already routes through `make_writer`"
    )]
    impl<'writer> fmt::MakeWriter<'writer> for CaptureWriter {
        type Writer = Self;

        fn make_writer(&'writer self) -> Self::Writer {
            self.clone()
        }
    }

    /// Install a capturing subscriber on the current thread and return the
    /// shared buffer plus the guard that keeps it active.
    ///
    /// The guard must stay alive for the whole probe: `#[tokio::test]` runs on
    /// a current-thread runtime, so the awaited query is polled on this thread
    /// and sees the scoped default dispatcher. The subscriber is scoped to this
    /// thread, so no process-global subscriber is installed and tests stay
    /// isolated.
    fn capture_logs() -> (Arc<Mutex<Vec<u8>>>, DefaultGuard) {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::registry().with(
            fmt::layer().with_ansi(false).with_writer(CaptureWriter {
                buffer: Arc::clone(&buffer),
            }),
        );
        let guard = set_default(subscriber);
        (buffer, guard)
    }

    /// Read the captured bytes back as a lossy UTF-8 string.
    fn captured_logs(buffer: &Mutex<Vec<u8>>) -> String {
        let bytes = buffer.lock().unwrap_or_else(PoisonError::into_inner);
        String::from_utf8_lossy(bytes.as_slice()).into_owned()
    }

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
    fn assert_constraint_rejected(logs: &str, kind: &str) {
        assert_eq!(
            logs.matches("WARN").count(),
            1,
            "an expected constraint must emit exactly one warn event: {logs}"
        );
        assert!(
            logs.contains("event=\"constraint_rejected\""),
            "the event must be constraint_rejected: {logs}"
        );
        assert!(
            logs.contains(&format!("kind=\"{kind}\"")),
            "the event must carry the stable kind `{kind}`: {logs}"
        );
        assert!(
            logs.contains("operation=\"repository\""),
            "the event must name the repository operation: {logs}"
        );
        assert!(
            logs.contains("security"),
            "the event must target the reserved security target: {logs}"
        );
    }

    /// Assert the captured output is the single `error` `port_fault` event for
    /// `kind` and `operation` on the reserved `security` target.
    fn assert_port_fault(logs: &str, kind: &str, operation: &str) {
        assert_eq!(
            logs.matches("ERROR").count(),
            1,
            "a dependency fault must emit exactly one error event: {logs}"
        );
        assert!(
            logs.contains("event=\"port_fault\""),
            "the event must be port_fault: {logs}"
        );
        assert!(
            logs.contains(&format!("kind=\"{kind}\"")),
            "the event must carry the stable kind `{kind}`: {logs}"
        );
        assert!(
            logs.contains(&format!("operation=\"{operation}\"")),
            "the event must name the owning operation: {logs}"
        );
        assert!(
            logs.contains("security"),
            "the event must target the reserved security target: {logs}"
        );
    }

    #[tokio::test]
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
        let (buffer, _capture) = capture_logs();

        // Act
        let mapped = sqlx::query("INSERT INTO probe (value) VALUES (0)")
            .execute(&pool)
            .await
            .map_err(RepositoryError::from);

        // Assert: a handled conflict is a `warn` `constraint_rejected` event.
        assert!(matches!(mapped, Err(RepositoryError::Conflict)));
        assert_constraint_rejected(&captured_logs(&buffer), "conflict");
        Ok(())
    }

    #[tokio::test]
    async fn check_violation_logs_port_fault() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = probe_pool("CREATE TABLE probe (value INTEGER CHECK (value > 0));").await?;
        let (buffer, _capture) = capture_logs();

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
        assert_port_fault(&captured_logs(&buffer), "check_violation", "repository");
        Ok(())
    }

    #[tokio::test]
    async fn unique_violation_logs_constraint_rejected() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = probe_pool("CREATE TABLE probe (value INTEGER UNIQUE);").await?;
        sqlx::query("INSERT INTO probe (value) VALUES (1)")
            .execute(&pool)
            .await?;
        let (buffer, _capture) = capture_logs();

        // Act
        let mapped = sqlx::query("INSERT INTO probe (value) VALUES (1)")
            .execute(&pool)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(mapped, Err(RepositoryError::AlreadyExist)));
        assert_constraint_rejected(&captured_logs(&buffer), "unique_violation");
        Ok(())
    }

    #[tokio::test]
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
        let (buffer, _capture) = capture_logs();

        // Act
        let mapped = sqlx::query("INSERT INTO probe (value) VALUES (0)")
            .execute(&pool)
            .await
            .map_err(RepositoryError::from);

        // Assert: the mapped event carries only the stable classification.
        assert!(matches!(mapped, Err(RepositoryError::Conflict)));
        let logs = captured_logs(&buffer);
        assert!(
            !logs.contains("LEAK_SENTINEL"),
            "the event must not carry the raw sqlx Debug: {logs}"
        );
        assert!(
            !logs.contains("SqliteError"),
            "the event must not carry the raw sqlx error type: {logs}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn unit_of_work_failure_logs_port_fault() -> Result<(), Box<dyn Error>> {
        // Arrange: a database failure surfaced through the unit of work.
        let pool = probe_pool("CREATE TABLE probe (value INTEGER);").await?;
        let (buffer, _capture) = capture_logs();

        // Act
        let mapped = sqlx::query("SELECT missing FROM probe")
            .execute(&pool)
            .await
            .map_err(UnitOfWorkError::from);

        // Assert: no expected constraint can arise here, so it stays a fault.
        assert!(matches!(mapped, Err(UnitOfWorkError::OperationFailed)));
        assert_port_fault(&captured_logs(&buffer), "operation_failed", "unit_of_work");
        Ok(())
    }
}
