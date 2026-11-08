use axum::extract::{Path, State};
use axum::response::Response;

use crate::application::port::list_series_episodes::ListSeriesEpisodesCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::{SelfLinks, hal_json};
use crate::infrastructure::inbound::rest::handler::library::{
    EpisodeListResponse, EpisodeResponse, parse_season_index, parse_series_id,
};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// List the episodes of one season of a show.
///
/// Any authenticated caller may read a season's episodes; the show is not owned
/// and carries no visibility flag. The collection is returned in ascending
/// episode index order.
#[utoipa::path(
    get,
    operation_id = "list_series_episodes",
    path = "/series/{series_id}/season/{season_index}/episode",
    tag = "library",
    params(
        ("series_id" = NumericID, Path, description = "Identifier of the show holding the season"),
        ("season_index" = u64, Path, description = "Series-scoped index of the season whose episodes are listed", maximum = 4_294_967_295u64),
    ),
    responses(
        (
            status = OK,
            body = EpisodeListResponse,
            content_type = "application/hal+json",
            description = "Episodes of the season"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid show identifier or season index"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The season index is outside the 32-bit range"
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
    summary = "List the episodes of a show season"
)]
pub(crate) async fn get_series_episode_list(
    Path((series_id_value, season_index_value)): Path<(String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let series_id = parse_series_id(&series_id_value)?;
    let season_index = parse_season_index(&season_index_value)?;
    let response = state
        .library_use_case_factory()
        .list_series_episodes()
        .execute(ListSeriesEpisodesCommand::new(
            caller.user_id(),
            series_id,
            season_index,
        ))
        .await?;
    let body = EpisodeListResponse {
        items: response
            .episodes()
            .iter()
            .map(EpisodeResponse::from)
            .collect(),
        links: SelfLinks::new(&format!(
            "/api/v1/library/series/{series_id}/season/{season_index}/episode"
        )),
    };
    Ok(hal_json(body))
}
