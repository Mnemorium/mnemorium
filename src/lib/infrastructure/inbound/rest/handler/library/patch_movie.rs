use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::response::Response;
use chrono::NaiveDate;
use serde::Deserialize;
use tracing::warn;
use utoipa::ToSchema;

use crate::application::field_update::FieldUpdate;
use crate::application::port::update_movie::UpdateMovieCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::{
    DATE_FORMAT, MovieResponse, deserialize_field_update,
};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to update a film partially.
///
/// An absent field keeps its value; a `null` clears an optional value. The
/// backing video is never part of the payload.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PatchMovieRequest {
    /// Country of origin as an ISO 3166-1 alpha-2 code; `null` clears it.
    #[serde(default, deserialize_with = "deserialize_field_update")]
    #[schema(value_type = Option<String>, nullable = true)]
    pub country_of_origin: FieldUpdate<String>,
    /// Identifier of the image holding the film's artwork; `null` clears it.
    #[serde(default, deserialize_with = "deserialize_field_update")]
    #[schema(value_type = Option<NumericID>, nullable = true)]
    pub cover_image_id: FieldUpdate<NumericID>,
    /// Genres of the film; omit to keep them.
    pub genres: Option<Vec<String>>,
    /// Name of the film.
    #[schema(min_length = 1, max_length = 100)]
    pub name: Option<String>,
    /// Plot of the film; omit to keep it.
    pub plot: Option<String>,
    /// Date of the release in `YYYY-MM-DD` form; `null` clears it.
    #[serde(default, deserialize_with = "deserialize_field_update")]
    #[schema(value_type = Option<String>, nullable = true)]
    pub release_date: FieldUpdate<String>,
}

/// Update a film partially.
///
/// Only a caller whose `library.manage` scope covers the film may update it. An
/// absent field keeps its value and a `null` clears an optional value.
#[utoipa::path(
    patch,
    operation_id = "patch_movie",
    path = "/movie/{movie_id}",
    tag = "library",
    params(
        ("movie_id" = NumericID, Path, description = "Identifier of the film to update"),
    ),
    request_body = PatchMovieRequest,
    responses(
        (
            status = OK,
            body = MovieResponse,
            content_type = "application/hal+json",
            description = "Film updated"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid film identifier or malformed body"
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
            description = "The caller may not manage the film"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown or invisible film"
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
            description = "The name, country, genre or release date is invalid"
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
    summary = "Update a film"
)]
pub(crate) async fn patch_movie(
    Path(movie_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PatchMovieRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let movie_id = movie_id_value.parse::<NumericID>().map_err(|_| {
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "movie_id",
            reason = "invalid_identifier",
            "rejected a malformed film identifier"
        );
        ApiError::BadRequest("invalid film identifier".to_owned())
    })?;
    let Json(request) = payload.map_err(ApiError::from)?;
    let release_date = request.release_date.try_map(|value| {
        NaiveDate::parse_from_str(&value, DATE_FORMAT).map_err(|_| {
            warn!(
                target: "security",
                event = "input_validation_failed",
                field = "release_date",
                reason = "invalid_date",
                "rejected a malformed release date"
            );
            ApiError::UnprocessableEntity("the release date must use YYYY-MM-DD".to_owned())
        })
    })?;
    let response = state
        .library_use_case_factory()
        .update_movie()
        .execute(UpdateMovieCommand::new(
            caller.user_id(),
            movie_id,
            request.name,
            request.plot,
            request.country_of_origin,
            release_date,
            request.genres,
            request.cover_image_id,
        ))
        .await?;
    Ok(hal_json(MovieResponse::from(response.movie())))
}
