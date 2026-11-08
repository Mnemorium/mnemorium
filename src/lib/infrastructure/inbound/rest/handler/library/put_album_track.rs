use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::port::put_album_track::PutAlbumTrackCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::JsonObject;
use crate::infrastructure::inbound::rest::handler::library::{
    TrackResponse, parse_album_id, parse_write_index,
};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to place a recording at a medium track address.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PutAlbumTrackRequest {
    /// Identifier of the recording to place.
    #[schema(example = 5)]
    pub recording_id: NumericID,
}

/// Place or move a recording at a medium track address.
///
/// Only a caller whose `library.manage` scope covers the album may write it.
/// The server removes the recording's previous medium placement and inserts the
/// new one in one transaction; an occupied destination is a conflict.
#[utoipa::path(
    put,
    operation_id = "put_album_track",
    path = "/music/album/{album_id}/medium/{medium_index}/track/{track_index}",
    tag = "library",
    params(
        ("album_id" = NumericID, Path, description = "Identifier of the album"),
        ("medium_index" = u8, Path, description = "Album-scoped index of the medium", maximum = 255u8),
        ("track_index" = u8, Path, description = "Index of the track within the medium", maximum = 255u8),
    ),
    request_body = PutAlbumTrackRequest,
    responses(
        (
            status = CREATED,
            body = TrackResponse,
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "URI of the new track placement"),
            ),
            description = "Recording placed"
        ),
        (
            status = OK,
            body = TrackResponse,
            content_type = "application/hal+json",
            description = "Recording moved or already placed"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid album identifier or track address"
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
            description = "Unknown or invisible album, or unknown medium"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The destination track holds another recording"
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
            description = "The recording is unknown, an index is out of range, or the album is the default one"
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
    summary = "Place or move a recording at a medium track address"
)]
pub(crate) async fn put_album_track(
    Path((album_id_value, medium_index_value, track_index_value)): Path<(String, String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: JsonObject<PutAlbumTrackRequest>,
) -> Result<Response, ApiError> {
    let album_id = parse_album_id(&album_id_value)?;
    let medium_index = parse_write_index(
        &medium_index_value,
        "medium_index",
        "invalid music medium index",
    )?;
    let track_index = parse_write_index(
        &track_index_value,
        "track_index",
        "invalid music track index",
    )?;
    let JsonObject(request) = payload;
    let response = state
        .library_use_case_factory()
        .put_album_track()
        .execute(PutAlbumTrackCommand::new(
            caller.user_id(),
            album_id,
            medium_index,
            track_index,
            request.recording_id,
        ))
        .await?;
    let body = TrackResponse::from_album(
        response.album(),
        response.medium_index(),
        response.track_index(),
    )
    .ok_or(ApiError::InternalServerError)?;
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
