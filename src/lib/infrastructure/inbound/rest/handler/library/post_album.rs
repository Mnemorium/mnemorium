use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use chrono::NaiveDate;
use serde::Deserialize;
use tracing::warn;
use utoipa::ToSchema;

use crate::application::port::create_music_album::CreateMusicAlbumCommand;
use crate::domain::alias::NumericID;
use crate::domain::model::music_album::ProductionType;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::JsonObject;
use crate::infrastructure::inbound::rest::handler::library::{
    AlbumResponse, DATE_FORMAT, ProductionTypeRequest,
};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to create a music album.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PostAlbumRequest {
    /// Identifier of the image holding the album's artwork, when set.
    #[schema(example = 1)]
    pub cover_image_id: Option<NumericID>,
    /// Genres of the album; omit to store the `UNKNOWN` genre.
    #[schema(min_length = 1)]
    pub genres: Option<Vec<String>>,
    /// Identifier of the music group the album belongs to.
    #[schema(example = 1)]
    pub music_group_id: NumericID,
    /// Name of the album.
    #[schema(min_length = 1, max_length = 100)]
    pub name: String,
    /// Production type of the album (`STUDIO`, `LIVE`, …), when set.
    pub production_type: Option<ProductionTypeRequest>,
    /// Date of the release, in `YYYY-MM-DD` form, when known.
    #[schema(format = "date", example = "1959-08-17")]
    pub release_date: Option<String>,
}

/// Create a music album.
///
/// Only a caller whose `library.manage` scope covers the whole catalogue may
/// create an album. The write is an upsert by `(name, group)`: an album whose
/// identity already exists is returned unchanged with `200 OK`.
#[utoipa::path(
    post,
    operation_id = "post_album",
    path = "/music/album",
    tag = "library",
    request_body = PostAlbumRequest,
    responses(
        (
            status = CREATED,
            body = AlbumResponse,
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "URI of the newly created album"),
            ),
            description = "Album created",
        ),
        (
            status = OK,
            body = AlbumResponse,
            content_type = "application/hal+json",
            description = "Album already existed"
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
            status = FORBIDDEN,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The caller may not manage the library"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The cover is not a library image"
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
            description = "The name, group, genre, production type or release date is invalid"
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
    summary = "Create a music album"
)]
pub(crate) async fn post_album(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: JsonObject<PostAlbumRequest>,
) -> Result<Response, ApiError> {
    let JsonObject(request) = payload;
    let production_type = request.production_type.map(ProductionType::from);
    let release_date = match request.release_date.as_deref() {
        Some(value) => Some(NaiveDate::parse_from_str(value, DATE_FORMAT).map_err(|_| {
            warn!(
                target: "security",
                event = "input_validation_failed",
                field = "release_date",
                reason = "invalid_date",
                "rejected a malformed release date"
            );
            ApiError::UnprocessableEntity("the release date must use YYYY-MM-DD".to_owned())
        })?),
        None => None,
    };
    let response = state
        .library_use_case_factory()
        .create_music_album()
        .execute(CreateMusicAlbumCommand::new(
            caller.user_id(),
            request.name,
            request.music_group_id,
            release_date,
            production_type,
            request.genres.unwrap_or_default(),
            request.cover_image_id,
        ))
        .await?;
    let body = AlbumResponse::from(response.album());
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
