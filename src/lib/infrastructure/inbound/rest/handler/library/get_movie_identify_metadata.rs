use axum::extract::{Path, State};
use axum::response::Response;
use tracing::warn;

use crate::application::port::{ResolveVideoMetadataCommand, ResolveVideoMetadataError};
use crate::domain::model::{ExternalRef, ProviderId, Target};
use crate::infrastructure::inbound::rest::handler::library::VideoMetadataDocumentResponse;
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, hal_json};

/// Resolve one provider reference into a film metadata document.
///
/// The reference names the provider that issued it; a provider the system does
/// not serve answers `404 Not Found`. The route reads the provider rather than
/// the catalogue, and is guarded by `ViewMetadata` on the catalogue. With no provider configured
/// it answers an `available: false` document.
#[utoipa::path(
    get,
    operation_id = "identify_video_movie_metadata",
    path = "/movie/identify/{provider}/{reference}",
    tag = "library",
    params(
        ("provider" = String, Path, description = "Provider that issued the reference"),
        ("reference" = String, Path, description = "Identifier the provider issued"),
    ),
    responses(
        (
            status = OK,
            body = VideoMetadataDocumentResponse,
            content_type = "application/hal+json",
            description = "The provider-independent film metadata document"
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
            description = "The reference does not resolve to a film"
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
    summary = "Resolve a video provider reference into a film's metadata"
)]
pub(crate) async fn get_movie_identify_metadata(
    Path((provider_value, reference_value)): Path<(String, String)>,
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
    let href = format!(
        "/api/v1/library/movie/identify/{}/{reference_value}",
        provider.as_str()
    );
    let target = Target::Movie(ExternalRef::new(provider, reference_value));
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
