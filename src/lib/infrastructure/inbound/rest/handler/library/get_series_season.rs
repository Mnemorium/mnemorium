use axum::extract::{Path, State};
use axum::response::Response;

use crate::application::port::get_series_season::GetSeriesSeasonCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::{
    SeasonResponse, parse_season_index, parse_series_id,
};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Fetch one season of a show.
///
/// Any authenticated caller may read a season; the show is not owned and
/// carries no visibility flag. The season is scoped to the show named in the
/// path, so two shows may each hold a season 1.
#[utoipa::path(
    get,
    operation_id = "get_series_season",
    path = "/series/{series_id}/season/{season_index}",
    tag = "library",
    params(
        ("series_id" = NumericID, Path, description = "Identifier of the show holding the season"),
        ("season_index" = u32, Path, description = "Series-scoped index of the season"),
    ),
    responses(
        (
            status = OK,
            body = SeasonResponse,
            content_type = "application/hal+json",
            description = "Season found"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid show identifier or season index"
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
            description = "Unknown or inaccessible show, or a season the show does not hold"
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
    summary = "Fetch a show season"
)]
pub(crate) async fn get_series_season(
    Path((series_id_value, season_index_value)): Path<(String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let series_id = parse_series_id(&series_id_value)?;
    let season_index = parse_season_index(&season_index_value)?;
    let response = state
        .library_use_case_factory()
        .get_series_season()
        .execute(GetSeriesSeasonCommand::new(
            caller.user_id(),
            series_id,
            season_index,
        ))
        .await?;
    Ok(hal_json(SeasonResponse::from(&response)))
}
