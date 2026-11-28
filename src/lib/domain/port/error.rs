//! Error types raised by the outbound port adapters.

use std::io;

/// Error returned when a repository operation fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum RepositoryError {
    /// The entity already exists and cannot be created again. Typically caused
    /// by duplicate business keys or unique constraints.
    #[error("the entity already exists and cannot be created again")]
    AlreadyExist,
    /// The operation failed due to a concurrent modification of the same
    /// entity (for example, optimistic locking failure).
    #[error("the operation failed due to a concurrent modification of the same entity")]
    ConcurrencyConflict,
    /// The operation cannot be completed because the current state of the data
    /// conflicts with the requested action.
    #[error(
        "the operation cannot be completed because the current state of the data conflicts with the requested action"
    )]
    Conflict,
    /// The operation would violate a data integrity rule or constraint.
    #[error("the operation would violate a data integrity rule or constraint")]
    DataIntegrityViolation,
    /// The repository could not complete the requested operation for a
    /// non-specific reason.
    #[error("the repository could not complete the requested operation for a non-specific reason")]
    OperationFailed,
    /// The operation exceeded the allowed execution time.
    #[error("the operation exceeded the allowed execution time")]
    Timeout,
    /// The repository or underlying datastore is currently unavailable.
    #[error("the repository or underlying datastore is currently unavailable")]
    Unavailable,
    /// An unexpected or unmapped error occurred.
    #[error("an unexpected or unmapped error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Error returned when an external service operation fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum ExternalServiceError {
    /// A network, protocol, or transport-level error occurred while
    /// communicating with the service.
    #[error(
        "a network, protocol, or transport-level error occurred while communicating with the service"
    )]
    CommunicationFailure,
    /// The service failed due to a problem with one of its own dependencies.
    #[error("the service failed due to a problem with one of its own dependencies")]
    DependencyFailure,
    /// The service response could not be parsed or converted into the expected
    /// format.
    #[error("the service response could not be parsed or converted into the expected format")]
    DeserializationFailure,
    /// The caller is authenticated but does not have permission to perform the
    /// requested operation.
    #[error(
        "the caller is authenticated but does not have permission to perform the requested operation"
    )]
    Forbidden,
    /// The request was rejected because it contains invalid or missing
    /// information.
    #[error("the request was rejected because it contains invalid or missing information")]
    InvalidRequest,
    /// The external service rejected the request because a usage limit was
    /// exceeded.
    #[error("the external service rejected the request because a usage limit was exceeded")]
    RateLimited,
    /// The external service did not respond within the expected time.
    #[error("the external service did not respond within the expected time")]
    Timeout,
    /// Authentication is required or the provided credentials are invalid.
    #[error("authentication is required or the provided credentials are invalid")]
    Unauthorized,
    /// The external service is temporarily unavailable or unreachable.
    #[error("the external service is temporarily unavailable or unreachable")]
    Unavailable,
    /// An unexpected or unmapped error occurred.
    #[error("an unexpected or unmapped error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

impl ExternalServiceError {
    /// Return the stable classification of the failure, for a log field.
    ///
    /// The value is a code, never the error's `Display`: a transport error's
    /// `Display` can embed the request address, and therefore a query key
    /// (`OBS-003`, `OBS-004`).
    #[must_use]
    pub(crate) fn classification(&self) -> &'static str {
        match *self {
            Self::CommunicationFailure => "communication_failure",
            Self::DependencyFailure => "dependency_failure",
            Self::DeserializationFailure => "deserialization_failure",
            Self::Forbidden => "forbidden",
            Self::InvalidRequest => "invalid_request",
            Self::RateLimited => "rate_limited",
            Self::Timeout => "timeout",
            Self::Unauthorized => "unauthorized",
            Self::Unavailable => "unavailable",
            Self::Unknown(_) => "unknown",
        }
    }
}

/// Error returned when a content hashing operation fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum ContentHasherError {
    /// An unexpected or unmapped error occurred.
    #[error("an unexpected or unmapped error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Error returned when a pepper generation operation fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum SecretGeneratorError {}

/// Error returned when a password hashing operation fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum PasswordHasherError {
    /// The password hashing or verification could not complete for a
    /// non-specific reason.
    #[error("the password hashing operation could not complete for a non-specific reason")]
    OperationFailed,
    /// An unexpected or unmapped error occurred.
    #[error("an unexpected or unmapped error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Error returned when a password generation operation fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum PasswordGeneratorError {
    /// The password generation could not complete for a non-specific reason.
    #[error("the password generation operation could not complete for a non-specific reason")]
    OperationFailed,
}

/// Error returned when a token provider operation fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum TokenProviderError {
    /// The claims could not be embedded into a token.
    #[error("the given claims are invalid and cannot be embedded into a token")]
    InvalidClaims,
    /// The token is malformed, unreadable, or otherwise invalid.
    #[error("the token is invalid")]
    InvalidToken,
    /// The token provider could not complete the requested operation for a
    /// non-specific reason.
    #[error(
        "the token provider could not complete the requested operation for a non-specific reason"
    )]
    OperationFailed,
    /// The token has expired.
    #[error("the token has expired")]
    TokenExpired,
}

/// Error returned when a unit of work operation fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum UnitOfWorkError {
    /// The unit of work could not complete for a non-specific reason.
    #[error("the unit of work could not complete for a non-specific reason")]
    OperationFailed,
    /// The datastore is currently unavailable.
    #[error("the datastore is currently unavailable")]
    Unavailable,
    /// An unexpected or unmapped error occurred.
    #[error("an unexpected or unmapped error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Error returned when loading the layered configuration fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum ConfigurationSourceError {
    /// The configuration file was absent and could not be written.
    #[error("the configuration file could not be written: {0}")]
    FileWriteFailed(#[source] anyhow::Error),
    /// The layered settings do not form a valid configuration.
    #[error("the configuration is invalid: {0}")]
    InvalidConfiguration(#[source] anyhow::Error),
    /// An unexpected or unmapped error occurred.
    #[error("an unexpected or unmapped error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Error returned when the instance identity cannot be read or persisted.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum InstanceStateStoreError {
    /// A concurrent creator persisted an identity first. The caller must not
    /// replace it: the persisted value is authoritative.
    #[error("another process created the instance identity first")]
    Conflict,
    /// The persisted identity exists but cannot be decoded or fails validation.
    #[error("the persisted instance identity is corrupt: {0}")]
    Corrupt(#[source] anyhow::Error),
    /// The persisted identity is readable or writable by another account, or is
    /// not a regular file. It is never read.
    #[error(
        "the persisted instance identity in state.yaml is not private to the server account; run chmod 600 state.yaml or chown it to the account that owns the working directory"
    )]
    Insecure,
    /// The identity file could not be read or written.
    #[error("the instance identity store failed: {0}")]
    OperationFailed(#[source] anyhow::Error),
    /// The persisted identity declares a format version this build does not
    /// support.
    #[error("the persisted instance identity declares unsupported format version {0}")]
    UnsupportedVersion(u32),
}

/// Error returned when the creation seeds cannot be read from the environment.
///
/// The message names the environment and never a value (`OBS-004`).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum StateSeedSourceError {
    /// The environment cannot be mapped onto the seed fields.
    #[error("the instance state seeds could not be read from the environment: {0}")]
    Unreadable(#[source] envy::Error),
}

/// Error returned when the presence of the datastore cannot be decided.
///
/// A missing datastore is a value, never this error (`STY-RUST-015`); the error
/// is reserved for a path that cannot be inspected.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum DatastorePresenceError {
    /// The datastore path could not be inspected.
    #[error("the datastore path could not be inspected: {0}")]
    Unreadable(#[source] io::Error),
}

/// Error returned when probing a media file fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum MediaProbeError {
    /// The probe could not complete for a non-specific reason.
    #[error("the media probe could not complete for a non-specific reason")]
    OperationFailed,
    /// The probe exceeded the allowed execution time.
    #[error("the probe exceeded the allowed execution time")]
    Timeout,
    /// An unexpected or unmapped error occurred.
    #[error("an unexpected or unmapped error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
    /// The media file format is not supported.
    #[error("the media file format is not supported")]
    UnsupportedMedia,
}

/// Error returned when sniffing a file's content type fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum ContentTypeSnifferError {
    /// An unexpected or unmapped error occurred.
    #[error("an unexpected or unmapped error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Error returned when discovering files under the library root fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum MediaSourceError {
    /// An unexpected or unmapped error occurred.
    #[error("an unexpected or unmapped error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Error returned when a file storage operation fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum StorageError {
    /// The storage backend could not complete the requested operation for a
    /// non-specific reason.
    #[error(
        "the file storage could not complete the requested operation for a non-specific reason"
    )]
    OperationFailed,
    /// The storage backend is temporarily unavailable or unreachable.
    #[error("the file storage is temporarily unavailable or unreachable")]
    Unavailable,
    /// An unexpected or unmapped error occurred.
    #[error("an unexpected or unmapped error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}
