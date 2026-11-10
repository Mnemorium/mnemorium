use axum::extract::{Path, State};
use axum::response::Response;

use crate::application::port::GetSeriesCommand;
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::library::{SeriesResponse, parse_series_id};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, hal_json};

/// Fetch one show by its identifier.
///
/// Any authenticated caller may read a show; the show is not owned and carries
/// no visibility flag. The representation carries the show's genres and its
/// season summaries, so a client can navigate to each season.
#[utoipa::path(
    get,
    operation_id = "get_series",
    path = "/series/{series_id}",
    tag = "library",
    params(
        ("series_id" = NumericID, Path, description = "Identifier of the show to fetch"),
    ),
    responses(
        (
            status = OK,
            body = SeriesResponse,
            content_type = "application/hal+json",
            description = "Show found"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid show identifier"
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
            description = "Unknown or inaccessible show"
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
    summary = "Fetch a show"
)]
pub(crate) async fn get_series(
    Path(series_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let series_id = parse_series_id(&series_id_value)?;
    let response = state
        .library_use_case_factory()
        .get_series()
        .execute(GetSeriesCommand::new(caller.user_id(), series_id))
        .await?;
    Ok(hal_json(SeriesResponse::from(&response)))
}
