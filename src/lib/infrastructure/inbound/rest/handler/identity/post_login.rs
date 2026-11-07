use std::fmt::{Debug, Formatter, Result as FmtResult};

use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::application::port::login_user::{LoginUserCommand, LoginUserError, LoginUserResponse};
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;

/// Payload to authenticate a user.
#[derive(Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct LoginRequest {
    /// Password used to authenticate.
    #[schema(write_only)]
    pub password: String,
    /// Username of the user to authenticate.
    #[schema(write_only)]
    pub username: String,
}

impl Debug for LoginRequest {
    /// Redact the plaintext password (`OBS-004`): it never reaches a log sink,
    /// while the submitted username stays readable for correlation.
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("LoginRequest")
            .field("password", &"**")
            .field("username", &self.username)
            .finish_non_exhaustive()
    }
}

/// Token issued by a successful authentication.
#[derive(Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct LoginResponse {
    /// Token to present in the `Authorization` header of subsequent requests.
    pub access_token: String,
    /// Lifetime of the token, in seconds.
    pub expires_in: u64,
    /// Token type, always `bearer`.
    pub token_type: String,
}

impl Debug for LoginResponse {
    /// Redact the issued access token (`OBS-004`): it never reaches a log sink,
    /// while the token metadata stays readable.
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("LoginResponse")
            .field("access_token", &"**")
            .field("expires_in", &self.expires_in)
            .field("token_type", &self.token_type)
            .finish_non_exhaustive()
    }
}

/// Map the login-user response onto its HTTP representation.
impl From<LoginUserResponse> for LoginResponse {
    fn from(response: LoginUserResponse) -> Self {
        Self {
            access_token: response.access_token().to_owned(),
            expires_in: response.expires_in(),
            token_type: "bearer".to_owned(),
        }
    }
}

/// Map a login-user error to its API error.
impl From<LoginUserError> for ApiError {
    fn from(err: LoginUserError) -> Self {
        match err {
            LoginUserError::InvalidPassword | LoginUserError::InvalidUsername => {
                Self::Unauthorized("invalid credentials".to_owned())
            }
            LoginUserError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Authenticate a user and issue a token.
///
/// The caller presents a username and a password. On success the endpoint
/// returns an access token to present in the `Authorization` header of
/// subsequent requests, together with the number of seconds it stays valid.
#[utoipa::path(
    post,
    operation_id = "post_login",
    path = "/login",
    tag = "identity",
    request_body = LoginRequest,
    responses(
        (
            status = OK,
            body = LoginResponse,
            content_type = "application/json",
            description = "User authenticated",
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Malformed request body"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid credentials"
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
            description = "The request body does not match the expected schema"
        ),
        (
            status = PAYLOAD_TOO_LARGE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body exceeds the maximum allowed size"
        ),
        (
            status = TOO_MANY_REQUESTS,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Too many login attempts",
            headers(
                ("Retry-After" = u64, description = "Seconds before the next login attempt is permitted")
            )
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unexpected error"
        ),
    ),
    security(()),
    summary = "Authenticate a user"
)]
pub(crate) async fn post_login(
    State(state): State<AppState>,
    payload: Result<Json<LoginRequest>, JsonRejection>,
) -> Result<Json<LoginResponse>, ApiError> {
    let Json(request) = payload.map_err(ApiError::from)?;
    let response = state
        .identity_use_case_factory()
        .login_user()
        .execute(LoginUserCommand::new(request.username, request.password))
        .await?;
    Ok(Json(LoginResponse::from(response)))
}
