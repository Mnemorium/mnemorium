use axum::extract::{Path, State};
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::port::{AlbumPlacement, ApplyMusicAlbumCommand};
use crate::domain::NumericID;
use crate::domain::model::{ExternalRef, ProviderId};
use crate::infrastructure::inbound::rest::handler::JsonObject;
use crate::infrastructure::inbound::rest::handler::library::{AlbumResponse, parse_album_id};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, hal_json};

/// Reference to the provider work an apply resolves.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct ApplyReferenceRequest {
    /// Identifier the provider issued.
    #[schema(example = "75b3b57b-5f6f-3c1f-9b7e-9b2f0a2b4b6e")]
    pub id: String,
    /// Provider that issued the identifier (`musicbrainz`).
    #[schema(example = "musicbrainz")]
    pub provider: String,
}

/// One recording the apply places at a destination address.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct ApplyPlacementRequest {
    /// Album-scoped index of the destination medium.
    #[schema(example = 1)]
    pub medium_index: u8,
    /// Identifier of the recording to move.
    #[schema(example = 5)]
    pub recording_id: NumericID,
    /// Index of the destination track within the medium.
    #[schema(example = 1)]
    pub track_index: u8,
}

/// Payload to apply external metadata to a music album.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct ApplyAlbumRequest {
    /// Recordings to move into the applied structure; omit to leave every
    /// placement unchanged.
    pub placements: Option<Vec<ApplyPlacementRequest>>,
    /// Reference to the provider work.
    pub reference: ApplyReferenceRequest,
}

/// Apply external metadata to a music album.
///
/// Only a caller whose `library.manage` scope covers the album may write it.
/// The provider reference is resolved once; the album and its medium structure
/// are written, and only the supplied placements move a recording.
#[utoipa::path(
    post,
    operation_id = "apply_album_metadata",
    path = "/music/album/{album_id}/apply",
    tag = "library",
    params(
        ("album_id" = NumericID, Path, description = "Identifier of the album"),
    ),
    request_body = ApplyAlbumRequest,
    responses(
        (
            status = OK,
            body = AlbumResponse,
            content_type = "application/hal+json",
            description = "Metadata applied"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid album identifier or malformed body"
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
            description = "Unknown or invisible album, or the provider does not know the reference"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The applied metadata duplicates another album, or a placement collides"
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
            description = "The reference, provider, placement or default album cannot be applied"
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unexpected error, or no music metadata provider is configured"
        ),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "Apply external metadata to a music album"
)]
pub(crate) async fn post_album_apply(
    Path(album_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: JsonObject<ApplyAlbumRequest>,
) -> Result<Response, ApiError> {
    let album_id = parse_album_id(&album_id_value)?;
    let JsonObject(request) = payload;
    let provider = ProviderId::parse(&request.reference.provider)
        .filter(|provider| provider.serves_music())
        .ok_or_else(|| ApiError::reject_enum("provider"))?;
    let reference = ExternalRef::new(provider, request.reference.id);
    let placements = request
        .placements
        .unwrap_or_default()
        .into_iter()
        .map(|placement| {
            AlbumPlacement::new(
                placement.medium_index,
                placement.track_index,
                placement.recording_id,
            )
        })
        .collect();
    let response = state
        .library_use_case_factory()
        .apply_music_album()
        .execute(ApplyMusicAlbumCommand::new(
            caller.user_id(),
            album_id,
            reference,
            placements,
        ))
        .await?;
    Ok(hal_json(AlbumResponse::from(response.album())))
}
