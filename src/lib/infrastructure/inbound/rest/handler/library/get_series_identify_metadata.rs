use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use axum::response::Response;
use serde::Deserialize;
use tracing::warn;
use utoipa::IntoParams;

use crate::application::port::{ResolveVideoMetadataCommand, ResolveVideoMetadataError};
use crate::domain::model::{ExternalRef, ProviderId, Target};
use crate::infrastructure::inbound::rest::handler::library::VideoMetadataDocumentResponse;
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, hal_json};

/// The optional episode position a show reference may address.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[non_exhaustive]
pub(crate) struct IdentifySeriesMetadataQuery {
    /// Index of the episode within its season; required with `season`.
    episode: Option<String>,
    /// Index of the season within the show; required with `episode`.
    season: Option<String>,
}

/// Resolve one provider reference into a show or episode metadata document.
///
/// Without a position the reference resolves to the show; with both `season` and
/// `episode` it resolves to that episode through the show's reference. A partial
/// or out-of-range position is rejected. A provider the system does not serve
/// answers `404 Not Found`, and no configured provider answers an
/// `available: false` document.
#[utoipa::path(
    get,
    operation_id = "identify_video_series_metadata",
    path = "/series/identify/{provider}/{reference}",
    tag = "library",
    params(
        ("provider" = String, Path, description = "Provider that issued the reference"),
        ("reference" = String, Path, description = "Identifier the provider issued"),
        IdentifySeriesMetadataQuery,
    ),
    responses(
        (
            status = OK,
            body = VideoMetadataDocumentResponse,
            content_type = "application/hal+json",
            description = "The provider-independent show or episode metadata document"
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
            description = "The provider is not served, the caller may not read the library, or the provider does not know the reference"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Only one of season and episode was supplied, a position is not a 32-bit unsigned integer, or the provider reported another kind or position"
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
    summary = "Resolve a video provider reference into a show or episode's metadata"
)]
pub(crate) async fn get_series_identify_metadata(
    Path((provider_value, reference_value)): Path<(String, String)>,
    raw_query: Result<Query<IdentifySeriesMetadataQuery>, QueryRejection>,
    caller: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Response, ApiError> {
    let provider = ProviderId::parse(&provider_value)
        .filter(|provider| provider.serves_video())
        .ok_or_else(|| {
            warn!(
                target: "security",
                event = "input_validation_failed",
                field = "provider",
                reason = "unknown_provider",
                "rejected an unknown video metadata provider"
            );
            ApiError::NotFound("the video metadata provider is not configured".to_owned())
        })?;
    let Query(query) = raw_query.map_err(ApiError::from)?;
    let position = match (query.season, query.episode) {
        (None, None) => None,
        (Some(season_value), Some(episode_value)) => {
            let season = season_value.parse::<u32>().map_err(|_| {
                warn!(
                    target: "security",
                    event = "input_validation_failed",
                    field = "episode_position",
                    reason = "invalid_value",
                    "rejected an invalid episode position"
                );
                ApiError::UnprocessableEntity(
                    "the season and episode must be supplied together as 32-bit unsigned integers"
                        .to_owned(),
                )
            })?;
            let episode = episode_value.parse::<u32>().map_err(|_| {
                warn!(
                    target: "security",
                    event = "input_validation_failed",
                    field = "episode_position",
                    reason = "invalid_value",
                    "rejected an invalid episode position"
                );
                ApiError::UnprocessableEntity(
                    "the season and episode must be supplied together as 32-bit unsigned integers"
                        .to_owned(),
                )
            })?;
            Some((season, episode))
        }
        _ => {
            warn!(
                target: "security",
                event = "input_validation_failed",
                field = "episode_position",
                reason = "invalid_value",
                "rejected an invalid episode position"
            );
            return Err(ApiError::UnprocessableEntity(
                "the season and episode must be supplied together as 32-bit unsigned integers"
                    .to_owned(),
            ));
        }
    };
    let href = match position {
        Some((season, episode)) => format!(
            "/api/v1/library/series/identify/{}/{reference_value}?season={season}&episode={episode}",
            provider.as_str()
        ),
        None => format!(
            "/api/v1/library/series/identify/{}/{reference_value}",
            provider.as_str()
        ),
    };
    let reference = ExternalRef::new(provider, reference_value);
    let target = match position {
        Some((season, episode)) => Target::Episode {
            episode,
            reference,
            season,
        },
        None => Target::Series(reference),
    };
    let result = state
        .library_use_case_factory()
        .resolve_video_metadata()
        .execute(ResolveVideoMetadataCommand::new(caller.user_id(), target))
        .await;
    match result {
        Ok(Some(response)) => Ok(hal_json(VideoMetadataDocumentResponse::new(
            &href,
            response.metadata(),
        ))),
        Ok(None) => Err(ApiError::NotFound(
            "the video metadata provider does not know the reference".to_owned(),
        )),
        Err(ResolveVideoMetadataError::Unavailable) => {
            Ok(hal_json(VideoMetadataDocumentResponse::unavailable(&href)))
        }
        Err(error) => Err(ApiError::from(error)),
    }
}
