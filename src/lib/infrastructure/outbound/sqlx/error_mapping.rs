//! Error mapping from `sqlx::Error` to the repository port error.

use sqlx::Error;
use sqlx::error::ErrorKind;
use tracing::error;

use crate::domain::port::error::RepositoryError;

impl From<Error> for RepositoryError {
    fn from(err: Error) -> Self {
        error!(
            error = ?err,
            "an error occurred while accessing the repository"
        );
        match err {
            Error::Database(db) => match db.kind() {
                ErrorKind::UniqueViolation => Self::AlreadyExist,
                ErrorKind::ForeignKeyViolation
                | ErrorKind::NotNullViolation
                | ErrorKind::CheckViolation
                | ErrorKind::ExclusionViolation => Self::DataIntegrityViolation,
                ErrorKind::Other | _ => Self::OperationFailed,
            },
            Error::PoolTimedOut => Self::Timeout,
            Error::PoolClosed
            | Error::WorkerCrashed
            | Error::Io(_)
            | Error::Configuration(_)
            | Error::Tls(_)
            | Error::ConfigFile(_) => Self::Unavailable,
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
            | Error::InvalidSavePointStatement => Self::OperationFailed,
            Error::Migrate(migration_error) => Self::Unknown(anyhow::anyhow!(migration_error)),
            Error::BeginFailed => {
                Self::Unknown(anyhow::anyhow!("beginning the transaction failed"))
            }
            other => Self::Unknown(anyhow::anyhow!(other)),
        }
    }
}
