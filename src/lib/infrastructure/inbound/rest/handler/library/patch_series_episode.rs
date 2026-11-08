use axum::extract::{Path, State};
use axum::response::Response;
use chrono::NaiveDate;
use serde::Deserialize;
use tracing::warn;
use utoipa::ToSchema;

use crate::application::field_update::FieldUpdate;
use crate::application::port::update_series_episode::UpdateSeriesEpisodeCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::JsonObject;
use crate::infrastructure::inbound::rest::handler::library::{
    DATE_FORMAT, EpisodeResponse, deserialize_field_update, parse_episode_index,
    parse_season_index, parse_series_id,
};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to update a season episode partially.
///
/// An absent field keeps its value; a `null` clears the air date. The backing
/// video is never part of the payload.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PatchSeriesEpisodeRequest {
    /// Date of the first airing in `YYYY-MM-DD` form; `null` clears it.
    #[serde(default, deserialize_with = "deserialize_field_update")]
    #[schema(value_type = Option<String>, nullable = true, format = "date")]
    pub air_date: FieldUpdate<String>,
    /// Name of the episode.
    #[schema(min_length = 1, max_length = 100)]
    pub name: Option<String>,
    /// Plot of the episode; omit to keep it.
    pub plot: Option<String>,
}

/// Update a season episode partially.
///
/// Only a caller whose `library.manage` scope covers the show may update it.
#[utoipa::path(
    patch,
    operation_id = "patch_series_episode",
    path = "/series/{series_id}/season/{season_index}/episode/{episode_index}",
    tag = "library",
    params(
        ("series_id" = NumericID, Path, description = "Identifier of the show holding the episode"),
        ("season_index" = u64, Path, description = "Series-scoped index of the season", maximum = 4_294_967_295u64),
        ("episode_index" = u64, Path, description = "Season-scoped index of the episode", maximum = 4_294_967_295u64),
    ),
    request_body = PatchSeriesEpisodeRequest,
    responses(
        (
            status = OK,
            body = EpisodeResponse,
            content_type = "application/hal+json",
            description = "Episode updated"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid show, season or episode identifier"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
        ),
        (
            status = FORBIDDEN,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The caller may not manage the show"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown or invisible show, season or episode"
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
            description = "The name is invalid or the air date is malformed"
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
    summary = "Update a season episode"
)]
pub(crate) async fn patch_series_episode(
    Path((series_id_value, season_index_value, episode_index_value)): Path<(
        String,
        String,
        String,
    )>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: JsonObject<PatchSeriesEpisodeRequest>,
) -> Result<Response, ApiError> {
    let series_id = parse_series_id(&series_id_value)?;
    let season_index = parse_season_index(&season_index_value)?;
    let episode_index = parse_episode_index(&episode_index_value)?;
    let JsonObject(request) = payload;
    let air_date = request.air_date.try_map(|value| {
        NaiveDate::parse_from_str(&value, DATE_FORMAT).map_err(|_| {
            warn!(
                target: "security",
                event = "input_validation_failed",
                field = "air_date",
                reason = "invalid_date",
                "rejected a malformed air date"
            );
            ApiError::UnprocessableEntity("the air date must use YYYY-MM-DD".to_owned())
        })
    })?;
    let response = state
        .library_use_case_factory()
        .update_series_episode()
        .execute(UpdateSeriesEpisodeCommand::new(
            caller.user_id(),
            series_id,
            season_index,
            episode_index,
            request.name,
            request.plot,
            air_date,
        ))
        .await?;
    Ok(hal_json(EpisodeResponse::from_episode(
        series_id,
        season_index,
        response.episode(),
    )))
}
