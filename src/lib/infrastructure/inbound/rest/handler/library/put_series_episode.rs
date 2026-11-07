use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use chrono::NaiveDate;
use serde::Deserialize;
use tracing::warn;
use utoipa::ToSchema;

use crate::application::port::put_series_episode::PutSeriesEpisodeCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::{
    DATE_FORMAT, EpisodeResponse, parse_episode_index, parse_season_index, parse_series_id,
};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to create or replace a season episode.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PutSeriesEpisodeRequest {
    /// Date of the first airing, in `YYYY-MM-DD` form, when known.
    #[schema(example = "1991-04-08")]
    pub air_date: Option<String>,
    /// Name of the episode.
    #[schema(min_length = 1, max_length = 100)]
    pub name: String,
    /// Plot of the episode, when known.
    pub plot: Option<String>,
    /// Identifier of the video backing the episode.
    #[schema(example = 1)]
    pub video_id: NumericID,
}

/// Create or replace a season episode.
///
/// Only a caller whose `library.manage` scope covers the whole catalogue may
/// write it. The write is idempotent by index: a free index creates the episode
/// and an existing index replaces it, replacing its video when a different one
/// is named.
#[utoipa::path(
    put,
    operation_id = "put_series_episode",
    path = "/series/{series_id}/season/{season_index}/episode/{episode_index}",
    tag = "library",
    params(
        ("series_id" = NumericID, Path, description = "Identifier of the show holding the episode"),
        ("season_index" = u32, Path, description = "Series-scoped index of the season"),
        ("episode_index" = u32, Path, description = "Season-scoped index of the episode"),
    ),
    request_body = PutSeriesEpisodeRequest,
    responses(
        (
            status = CREATED,
            body = EpisodeResponse,
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "URI of the newly created episode"),
            ),
            description = "Episode created",
        ),
        (
            status = OK,
            body = EpisodeResponse,
            content_type = "application/hal+json",
            description = "Episode replaced"
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
            description = "Unknown or invisible show, or a season the show does not hold"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The video is already attached"
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
            description = "The name is invalid, the air date is malformed, or the video does not exist"
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
    summary = "Create or replace a season episode"
)]
pub(crate) async fn put_series_episode(
    Path((series_id_value, season_index_value, episode_index_value)): Path<(
        String,
        String,
        String,
    )>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PutSeriesEpisodeRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let series_id = parse_series_id(&series_id_value)?;
    let season_index = parse_season_index(&season_index_value)?;
    let episode_index = parse_episode_index(&episode_index_value)?;
    let Json(request) = payload.map_err(ApiError::from)?;
    let air_date = match request.air_date.as_deref() {
        Some(value) => Some(NaiveDate::parse_from_str(value, DATE_FORMAT).map_err(|_| {
            warn!(
                target: "security",
                event = "input_validation_failed",
                field = "air_date",
                reason = "invalid_date",
                "rejected a malformed air date"
            );
            ApiError::UnprocessableEntity("the air date must use YYYY-MM-DD".to_owned())
        })?),
        None => None,
    };
    let response = state
        .library_use_case_factory()
        .put_series_episode()
        .execute(PutSeriesEpisodeCommand::new(
            caller.user_id(),
            series_id,
            season_index,
            episode_index,
            request.video_id,
            request.name,
            request.plot,
            air_date,
        ))
        .await?;
    let body = EpisodeResponse::from_episode(series_id, season_index, response.episode());
    let location = body.links.self_link.href.clone();
    let mut http_response = hal_json(body);
    if response.created() {
        *http_response.status_mut() = StatusCode::CREATED;
        http_response.headers_mut().insert(
            header::LOCATION,
            HeaderValue::from_str(&location).map_err(|_| ApiError::InternalServerError)?,
        );
    }
    Ok(http_response)
}
