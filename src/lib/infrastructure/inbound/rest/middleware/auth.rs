use std::future::Future;
use std::future::ready;
use std::sync::Arc;

use axum::Json;
use axum::extract::FromRequestParts;
use axum::extract::{Request, State};
use axum::http::request::Parts;
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse as _, Response};
use serde_json::json;

use crate::domain::alias::NumericID;
use crate::domain::port::token_provider::TokenProvider;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::app_state::AppState;

/// Authenticated user identifier attached to a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthenticatedUser(NumericID);

impl AuthenticatedUser {
    /// Return the authenticated user identifier.
    #[must_use]
    pub fn user_id(&self) -> NumericID {
        self.0
    }
}

impl From<NumericID> for AuthenticatedUser {
    fn from(user_id: NumericID) -> Self {
        Self(user_id)
    }
}

impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = ApiError;

    fn from_request_parts(
        parts: &mut Parts,
        _state: &AppState,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        ready(
            parts
                .extensions
                .get::<AuthenticatedUser>()
                .copied()
                .ok_or(ApiError::InternalServerError),
        )
    }
}

/// Require a valid `Bearer` token and attach the authenticated user to the
/// request.
pub async fn authenticate<P>(
    State(token_provider): State<Arc<P>>,
    mut request: Request,
    next: Next,
) -> Response
where
    P: TokenProvider,
{
    let Some(token) = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split_once(' '))
        .and_then(|(scheme, token)| scheme.eq_ignore_ascii_case("bearer").then_some(token))
    else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "missing or malformed authorization header" })),
        )
            .into_response();
    };
    let Some(user_id) = token_provider.validate(token).await.ok() else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "invalid or expired token" })),
        )
            .into_response();
    };
    request
        .extensions_mut()
        .insert(AuthenticatedUser::from(user_id));
    next.run(request).await
}
