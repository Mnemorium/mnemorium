//! Error types raised by the outbound port adapters.

/// Error returned when a repository operation fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RepositoryError {
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
    /// The provided data does not satisfy validation rules required by the
    /// repository.
    #[error("the provided data does not satisfy validation rules required by the repository")]
    ValidationFailed,
}

/// Error returned when an external service operation fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ExternalServiceError {
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
    /// A transient error occurred and the operation may succeed if retried.
    #[error("a transient error occurred and the operation may succeed if retried")]
    RetryableFailure,
    /// The request could not be properly serialized before being sent to the
    /// service.
    #[error("the request could not be properly serialized before being sent to the service")]
    SerializationFailure,
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

/// Error returned when a password hashing operation fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PasswordHasherError {
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
pub enum PasswordGeneratorError {
    /// The password generation could not complete for a non-specific reason.
    #[error("the password generation operation could not complete for a non-specific reason")]
    OperationFailed,
    /// An unexpected or unmapped error occurred.
    #[error("an unexpected or unmapped error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Error returned when a token provider operation fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TokenProviderError {
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
    /// An unexpected or unmapped error occurred.
    #[error("an unexpected or unmapped error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}
