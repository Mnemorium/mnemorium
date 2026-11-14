use std::fmt::{Debug, Formatter, Result as FmtResult};

use axum::extract::State;
use axum::response::Response;
use serde::Serialize;
use utoipa::ToSchema;

use crate::application::port::{RegenerateTokenCommand, RegenerateTokenError};
use crate::domain::port::IssuedToken;
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, SelfLinks, hal_json};

/// Token issued by a successful regeneration.
#[derive(Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct TokenResponse {
    /// Token to present in the `Authorization` header of subsequent requests.
    pub access_token: String,
    /// Lifetime of the token, in seconds.
    pub expires_in: u64,
    /// Links to the regeneration operation.
    #[serde(rename = "_links")]
    pub links: SelfLinks,
    /// Token type, always `bearer`.
    pub token_type: String,
}

impl Debug for TokenResponse {
    /// Redact the issued access token (`OBS-004`): it never reaches a log sink,
    /// while the token metadata stays readable.
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("TokenResponse")
            .field("access_token", &"**")
            .field("expires_in", &self.expires_in)
            .field("token_type", &self.token_type)
            .finish_non_exhaustive()
    }
}

/// Map the issued token onto its HTTP representation.
impl From<IssuedToken> for TokenResponse {
    fn from(response: IssuedToken) -> Self {
        Self {
            access_token: response.value().to_owned(),
            expires_in: response.expires_in(),
            links: SelfLinks::new("/api/v1/identity/token"),
            token_type: "bearer".to_owned(),
        }
    }
}

/// Map a token-regeneration error to its API error.
impl From<RegenerateTokenError> for ApiError {
    fn from(err: RegenerateTokenError) -> Self {
        match err {
            RegenerateTokenError::NoSuchUser => {
                Self::Unauthorized("the authenticated caller no longer exists".to_owned())
            }
            RegenerateTokenError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Issue a fresh access token for the authenticated caller.
///
/// The caller presents its current bearer token and receives the same
/// representation as login: a new access token, its lifetime and the `bearer`
/// token type. The previous token stays valid until its own expiry.
#[utoipa::path(
    post,
    operation_id = "post_token",
    path = "/token",
    tag = "identity",
    responses(
        (
            status = OK,
            body = TokenResponse,
            content_type = "application/hal+json",
            description = "Token regenerated",
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials, or the caller no longer exists"
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
    summary = "Regenerate the caller's access token"
)]
pub(crate) async fn post_token(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let response = state
        .identity_use_case_factory()
        .regenerate_token()
        .execute(RegenerateTokenCommand::new(caller.user_id()))
        .await?;
    Ok(hal_json(TokenResponse::from(response)))
}
