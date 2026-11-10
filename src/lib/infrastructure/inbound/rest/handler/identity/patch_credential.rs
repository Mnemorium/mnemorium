use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::port::{PatchCredentialCommand, PatchCredentialError};
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::JsonObject;
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody};

/// Payload to change the password behind a credential.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PatchCredentialRequest {
    /// New password; never returned by the API.
    ///
    /// Must be at least 8 characters long and contain at least one symbol
    /// (a non-alphanumeric character).
    #[schema(write_only, min_length = 8, pattern = r"[^A-Za-z0-9]")]
    pub password: String,
}

/// Map a change-password error to its API error.
impl From<PatchCredentialError> for ApiError {
    fn from(err: PatchCredentialError) -> Self {
        match err {
            PatchCredentialError::Forbidden => Self::Forbidden(err.to_string()),
            PatchCredentialError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            PatchCredentialError::InvalidPassword => Self::UnprocessableEntity(err.to_string()),
            PatchCredentialError::Unknown(_) => Self::InternalServerError,
            PatchCredentialError::UnknownCredential => Self::NotFound(err.to_string()),
        }
    }
}

/// Change the password behind a credential.
///
/// Only the Root Admin (user identifier `0`, role `Admin`) may invoke this
/// endpoint; any other authenticated caller is rejected with `403`. The Root
/// Admin may change its own credential as well as the credential of any other
/// user. The new password must satisfy the password policy: at least 8
/// characters long and containing at least one symbol.
#[utoipa::path(
    patch,
    operation_id = "patch_credential",
    path = "/credential/{id}",
    tag = "identity",
    request_body = PatchCredentialRequest,
    params(
        ("id" = NumericID, Path, description = "Identifier of the credential whose password is changed"),
    ),
    responses(
        (
            status = NO_CONTENT,
            description = "Password changed",
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Malformed path identifier or request body"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
        ),
        (
            status = PAYLOAD_TOO_LARGE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body exceeds the maximum allowed size"
        ),
        (
            status = UNSUPPORTED_MEDIA_TYPE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body is not application/json"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body does not match the expected schema, or the new password violates the password policy"
        ),
        (
            status = FORBIDDEN,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Caller is not the Root Admin"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown credential"
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unexpected error"
        ),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "Change the password behind a credential"
)]
pub(crate) async fn patch_credential(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    credential_path: Result<Path<NumericID>, PathRejection>,
    payload: JsonObject<PatchCredentialRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let Path(credential_id) = credential_path.map_err(ApiError::from)?;
    let JsonObject(request) = payload;
    state
        .identity_use_case_factory()
        .patch_credential()
        .execute(PatchCredentialCommand::new(
            caller.user_id(),
            credential_id,
            request.password,
        ))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
