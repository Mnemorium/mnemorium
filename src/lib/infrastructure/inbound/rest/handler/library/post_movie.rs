use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use chrono::NaiveDate;
use serde::Deserialize;
use tracing::warn;
use utoipa::ToSchema;

use crate::application::port::create_movie::CreateMovieCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::{DATE_FORMAT, MovieResponse};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to create a film from a video.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PostMovieRequest {
    /// Country of origin as an ISO 3166-1 alpha-2 code, when known.
    #[schema(example = "US")]
    pub country_of_origin: Option<String>,
    /// Identifier of the image holding the film's artwork, when set.
    #[schema(example = 1)]
    pub cover_image_id: Option<NumericID>,
    /// Genres of the film.
    pub genres: Option<Vec<String>>,
    /// Name of the film.
    #[schema(min_length = 1, max_length = 100)]
    pub name: String,
    /// Plot of the film, when known.
    pub plot: Option<String>,
    /// Date of the release, in `YYYY-MM-DD` form, when known.
    #[schema(example = "1959-08-17")]
    pub release_date: Option<String>,
    /// Identifier of the video backing the film.
    #[schema(example = 1)]
    pub video_id: NumericID,
}

/// Create a film from a video.
///
/// Only a caller whose `library.manage` scope covers the whole catalogue may
/// create a film. The write is an upsert by `video_id`: a film already backed
/// by the video is returned unchanged with `200 OK`.
#[utoipa::path(
    post,
    operation_id = "post_movie",
    path = "/movie",
    tag = "library",
    request_body = PostMovieRequest,
    responses(
        (
            status = CREATED,
            body = MovieResponse,
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "URI of the newly created film"),
            ),
            description = "Film created",
        ),
        (
            status = OK,
            body = MovieResponse,
            content_type = "application/hal+json",
            description = "Film already existed"
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
            description = "The video is already attached, or the cover is not a library image"
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
            description = "The name, country, genre or release date is invalid, or the video does not exist"
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
    summary = "Create a film from a video"
)]
pub(crate) async fn post_movie(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PostMovieRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(request) = payload.map_err(ApiError::from)?;
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
        .create_movie()
        .execute(CreateMovieCommand::new(
            caller.user_id(),
            request.video_id,
            request.name,
            request.plot,
            request.country_of_origin,
            release_date,
            request.genres.unwrap_or_default(),
            request.cover_image_id,
        ))
        .await?;
    let body = MovieResponse::from(response.movie());
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
