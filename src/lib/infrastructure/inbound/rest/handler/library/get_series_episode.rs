use axum::extract::{Path, State};
use axum::response::Response;
use tracing::warn;

use crate::application::port::GetSeriesEpisodeCommand;
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::library::{
    EpisodeResponse, parse_season_index, parse_series_id,
};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, hal_json};

/// Fetch one episode of a season of a show.
///
/// A caller that may `View` an episode reads it; the show is not owned and
/// carries no visibility flag. The episode is scoped to the show and season
/// named in the path.
#[utoipa::path(
    get,
    operation_id = "get_series_episode",
    path = "/series/{series_id}/season/{season_index}/episode/{episode_index}",
    tag = "library",
    params(
        ("series_id" = NumericID, Path, description = "Identifier of the show holding the episode"),
        ("season_index" = u64, Path, description = "Series-scoped index of the season holding the episode", maximum = 4_294_967_295u64),
        ("episode_index" = u64, Path, description = "Season-scoped index of the episode", maximum = 4_294_967_295u64),
    ),
    responses(
        (
            status = OK,
            body = EpisodeResponse,
            content_type = "application/hal+json",
            description = "Episode found"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid show identifier or season or episode index"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "An index is outside the 32-bit range"
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
            description = "Unknown or inaccessible show, or a season or episode the show does not hold"
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
    summary = "Fetch a show season episode"
)]
pub(crate) async fn get_series_episode(
    Path((series_id_value, season_index_value, episode_index_value)): Path<(
        String,
        String,
        String,
    )>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let series_id = parse_series_id(&series_id_value)?;
    let season_index = parse_season_index(&season_index_value)?;
    let episode_index = episode_index_value.parse::<u32>().map_err(|_| {
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "episode_index",
            reason = "invalid_index",
            "rejected a malformed episode index"
        );
        ApiError::BadRequest("invalid episode index".to_owned())
    })?;
    let response = state
        .library_use_case_factory()
        .get_series_episode()
        .execute(GetSeriesEpisodeCommand::new(
            caller.user_id(),
            series_id,
            season_index,
            episode_index,
        ))
        .await?;
    Ok(hal_json(EpisodeResponse::from(&response)))
}
