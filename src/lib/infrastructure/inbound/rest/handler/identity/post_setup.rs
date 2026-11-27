use std::fmt::{Debug, Formatter, Result as FmtResult};

use axum::extract::{Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse as _, Response};
use serde::Deserialize;
use tracing::warn;
use utoipa::ToSchema;

use crate::application::port::{SetupRootAdminCommand, SetupRootAdminError};
use crate::infrastructure::inbound::rest::handler::{JsonObject, NoParametersQuery};
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody};

/// Payload exchanging the Root Admin setup token for a chosen credential.
#[derive(Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct SetupRequest {
    /// Optional email address to store on the Root Admin.
    #[schema(write_only)]
    pub email: Option<String>,
    /// Password the Root Admin chooses; satisfies the credential policy.
    #[schema(write_only)]
    pub password: String,
    /// One-time setup token the first start revealed.
    #[schema(write_only)]
    pub token: String,
}

impl Debug for SetupRequest {
    /// Redact the token, the password and the email (`OBS-004`): none reaches a
    /// log sink.
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("SetupRequest")
            .field("email", &"**")
            .field("password", &"**")
            .field("token", &"**")
            .finish_non_exhaustive()
    }
}

/// Map a setup-root-admin error to its API error.
impl From<SetupRootAdminError> for ApiError {
    fn from(error: SetupRootAdminError) -> Self {
        match error {
            SetupRootAdminError::EmailTaken => Self::Conflict(error.to_string()),
            SetupRootAdminError::InvalidEmail | SetupRootAdminError::WeakPassword => {
                Self::UnprocessableEntity(error.to_string())
            }
            SetupRootAdminError::InvalidToken => {
                Self::Unauthorized("invalid setup token".to_owned())
            }
            SetupRootAdminError::RateLimited { wait } => Self::TooManyRequests(format!(
                "too many setup attempts; retry in {} seconds",
                wait.as_secs().max(1)
            )),
            SetupRootAdminError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Exchange a live Root Admin setup token for the chosen credential.
///
/// The operation is public: the token is the only authority, it is single-use,
/// and it never authenticates a request.
#[utoipa::path(
    post,
    operation_id = "post_setup",
    path = "/setup",
    tag = "identity",
    request_body = SetupRequest,
    responses(
        (
            status = NO_CONTENT,
            description = "The Root Admin credential was replaced",
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The setup token is unknown, expired or already used"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Another account already holds the submitted email address"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The password or the email fails its validation"
        ),
        (
            status = TOO_MANY_REQUESTS,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Too many setup attempts",
            headers(
                ("Retry-After" = u64, description = "Seconds before another setup attempt is permitted")
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
    summary = "Complete the Root Admin setup"
)]
pub(crate) async fn post_setup(
    _query: Query<NoParametersQuery>,
    State(state): State<AppState>,
    payload: JsonObject<SetupRequest>,
) -> Result<Response, ApiError> {
    let JsonObject(request) = payload;
    match state
        .identity_use_case_factory()
        .setup_root_admin()
        .execute(SetupRootAdminCommand::new(
            request.email,
            request.password,
            request.token,
        ))
        .await
    {
        Ok(()) => Ok(StatusCode::NO_CONTENT.into_response()),
        Err(SetupRootAdminError::RateLimited { wait }) => {
            warn!(
                target: "security",
                event = "rate_limit_exceeded",
                route = "identity_setup",
                reason = "quota_exceeded",
                "rejected a rate-limited setup attempt"
            );
            let mut response =
                ApiError::TooManyRequests("too many requests".to_owned()).into_response();
            if let Ok(value) = HeaderValue::from_str(&wait.as_secs().max(1).to_string()) {
                response.headers_mut().insert(header::RETRY_AFTER, value);
            }
            Ok(response)
        }
        Err(error) => Err(error.into()),
    }
}
