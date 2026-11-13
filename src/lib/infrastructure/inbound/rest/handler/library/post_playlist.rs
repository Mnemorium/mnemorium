use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::port::CreatePlaylistCommand;
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::JsonObject;
use crate::infrastructure::inbound::rest::handler::library::PlaylistResponse;
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, hal_json};

/// Payload to create a playlist owned by the caller.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PostPlaylistRequest {
    /// Identifier of the image holding the playlist's artwork, when set.
    #[schema(example = 1)]
    pub cover_image_id: Option<NumericID>,
    /// Whether any authenticated user may read and moderate the playlist.
    #[serde(default)]
    pub is_public: bool,
    /// Name of the playlist.
    #[schema(min_length = 1, max_length = 100)]
    pub name: String,
}

/// Create a playlist owned by the caller.
///
/// Creating a playlist requires `CreateChild` for a music playlist on the
/// **Playlist collection**. The name is required, trimmed and at most 100
/// characters long; `is_public` defaults to `false` and the optional cover must
/// be a `USER` image.
#[utoipa::path(
    post,
    operation_id = "post_playlist",
    path = "/music/playlist",
    tag = "library",
    request_body = PostPlaylistRequest,
    responses(
        (
            status = CREATED,
            body = PlaylistResponse,
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "URI of the newly created playlist"),
            ),
            description = "Playlist created",
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Malformed request body"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
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
            description = "The playlist name is empty or too long"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The caller already owns a playlist with this name"
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
    summary = "Create a music playlist"
)]
pub(crate) async fn post_playlist(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: JsonObject<PostPlaylistRequest>,
) -> Result<Response, ApiError> {
    let JsonObject(request) = payload;
    let response = state
        .library_use_case_factory()
        .create_playlist()
        .execute(CreatePlaylistCommand::new(
            caller.user_id(),
            request.name,
            request.is_public,
            request.cover_image_id,
        ))
        .await?;
    let body = PlaylistResponse::from_playlist(response.playlist());
    let location = body.links.self_link.href.clone();
    let mut http_response = hal_json(body);
    *http_response.status_mut() = StatusCode::CREATED;
    http_response.headers_mut().insert(
        header::LOCATION,
        HeaderValue::from_str(&location).map_err(|_| ApiError::InternalServerError)?,
    );
    Ok(http_response)
}
