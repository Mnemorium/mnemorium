use axum::extract::{Path, State};
use axum::response::Response;

use crate::application::port::ApplySeriesEpisodeCommand;
use crate::domain::NumericID;
use crate::domain::model::{ExternalRef, ProviderId};
use crate::infrastructure::inbound::rest::handler::JsonObject;
use crate::infrastructure::inbound::rest::handler::library::{
    EpisodeResponse, VideoApplyRequest, parse_episode_index, parse_season_index, parse_series_id,
};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, hal_json};

/// Apply external metadata to one episode.
///
/// Only a caller whose `library.manage` scope covers the show may write it. The
/// provider is asked once for the addressed position; a provider that reports
/// another position is rejected. The name, plot and air date are written, an
/// air date the provider does not report is cleared, and the video behind the
/// episode is untouched.
#[utoipa::path(
    post,
    operation_id = "apply_series_episode_metadata",
    path = "/series/{series_id}/season/{season_index}/episode/{episode_index}/apply",
    tag = "library",
    params(
        ("series_id" = NumericID, Path, description = "Identifier of the show holding the episode"),
        ("season_index" = u64, Path, description = "Series-scoped index of the season", maximum = 4_294_967_295u64),
        ("episode_index" = u64, Path, description = "Season-scoped index of the episode", maximum = 4_294_967_295u64),
    ),
    request_body = VideoApplyRequest,
    responses(
        (
            status = OK,
            body = EpisodeResponse,
            content_type = "application/hal+json",
            description = "Metadata applied"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid identifier or index, or malformed body"
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
            description = "The caller may not manage the library"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown or invisible show, season or episode, or the provider does not know the reference"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The applied metadata conflicts with the stored catalogue"
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
            description = "The provider is not a video provider, the reference resolves to another kind, or the provider reports another position"
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unexpected error, the provider failed, or no video metadata provider is configured"
        ),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "Apply external metadata to an episode"
)]
pub(crate) async fn post_series_episode_apply(
    Path((series_id_value, season_index_value, episode_index_value)): Path<(
        String,
        String,
        String,
    )>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: JsonObject<VideoApplyRequest>,
) -> Result<Response, ApiError> {
    let series_id = parse_series_id(&series_id_value)?;
    let season_index = parse_season_index(&season_index_value)?;
    let episode_index = parse_episode_index(&episode_index_value)?;
    let JsonObject(request) = payload;
    let provider = ProviderId::parse(&request.reference.provider)
        .filter(|provider| provider.serves_video())
        .ok_or_else(|| ApiError::reject_enum("provider"))?;
    let reference = ExternalRef::new(provider, request.reference.id);
    let response = state
        .library_use_case_factory()
        .apply_series_episode()
        .execute(ApplySeriesEpisodeCommand::new(
            caller.user_id(),
            series_id,
            season_index,
            episode_index,
            reference,
        ))
        .await?;
    Ok(hal_json(EpisodeResponse::from_episode(
        response.series_id(),
        response.season_index(),
        response.episode(),
    )))
}
