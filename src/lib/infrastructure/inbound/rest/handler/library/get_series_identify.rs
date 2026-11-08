use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::response::Response;
use serde::Deserialize;
use tracing::warn;
use utoipa::IntoParams;

use crate::application::port::identify_video::{IdentifyVideoCommand, IdentifyVideoError};
use crate::domain::model::media_query::VideoKind;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::VideoCandidateListResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// The name a caller knows about the show work it wants identified.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[non_exhaustive]
pub(crate) struct IdentifySeriesQuery {
    /// Media name to search for; never empty.
    #[param(min_length = 1)]
    name: String,
}

/// Search the configured video provider for show candidates.
///
/// The route reads a provider rather than the catalogue, so it is guarded by
/// `library.read` and returns only the candidates the show kind names. With no
/// provider configured it answers an `available: false` document.
#[utoipa::path(
    get,
    operation_id = "identify_video_series",
    path = "/series/identify",
    tag = "library",
    params(IdentifySeriesQuery),
    responses(
        (
            status = OK,
            body = VideoCandidateListResponse,
            content_type = "application/hal+json",
            description = "Show candidates the configured provider reported, in its order"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Malformed query string"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The caller may not read the library"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The name is empty"
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
    summary = "Search the configured video metadata provider for shows"
)]
pub(crate) async fn get_series_identify(
    raw_query: Result<Query<IdentifySeriesQuery>, QueryRejection>,
    caller: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Response, ApiError> {
    let Query(query) = raw_query.map_err(ApiError::from)?;
    if query.name.is_empty() {
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "name",
            reason = "empty",
            "rejected an empty video identification name"
        );
        return Err(ApiError::UnprocessableEntity(
            "the name must not be empty".to_owned(),
        ));
    }
    // The representation is the answer to the search view, so its `self` link
    // addresses the surface rather than repeating the caller's filter.
    let href = "/api/v1/library/series/identify";
    let result = state
        .library_use_case_factory()
        .identify_video()
        .execute(IdentifyVideoCommand::new(
            caller.user_id(),
            VideoKind::Series,
            query.name,
        ))
        .await;
    match result {
        Ok(response) => Ok(hal_json(VideoCandidateListResponse::new(
            href,
            response.candidates(),
        ))),
        Err(IdentifyVideoError::Unavailable) => {
            Ok(hal_json(VideoCandidateListResponse::unavailable(href)))
        }
        Err(error) => Err(ApiError::from(error)),
    }
}
