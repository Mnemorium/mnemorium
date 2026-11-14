use axum::extract::{Path, State};
use axum::response::Response;
use chrono::NaiveDate;
use serde::Deserialize;
use tracing::warn;
use utoipa::ToSchema;

use crate::application::FieldUpdate;
use crate::application::port::UpdateSeriesCommand;
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::JsonObject;
use crate::infrastructure::inbound::rest::handler::library::{
    DATE_FORMAT, SeriesResponse, deserialize_field_update, parse_series_id,
};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, hal_json};

/// Payload to update a show partially.
///
/// An absent field keeps its value; a `null` clears an optional value. The
/// show's seasons and episodes are never part of the payload.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PatchSeriesRequest {
    /// Country of origin as an ISO 3166-1 alpha-2 code; `null` clears it.
    #[serde(default, deserialize_with = "deserialize_field_update")]
    #[schema(value_type = Option<String>, nullable = true, pattern = "^[A-Za-z]{2}$")]
    pub country_of_origin: FieldUpdate<String>,
    /// Identifier of the image holding the show's artwork; `null` clears it.
    #[serde(default, deserialize_with = "deserialize_field_update")]
    #[schema(value_type = Option<NumericID>, nullable = true)]
    pub cover_image_id: FieldUpdate<NumericID>,
    /// Date of the first airing in `YYYY-MM-DD` form; `null` clears it.
    #[serde(default, deserialize_with = "deserialize_field_update")]
    #[schema(value_type = Option<String>, nullable = true, format = "date")]
    pub first_air_date: FieldUpdate<String>,
    /// Genres of the show; omit to keep them.
    #[schema(min_length = 1)]
    pub genres: Option<Vec<String>>,
    /// Name of the show.
    #[schema(min_length = 1, max_length = 100)]
    pub name: Option<String>,
    /// Plot of the show; omit to keep it.
    pub plot: Option<String>,
}

/// Update a show partially.
///
/// Updating a show requires `View` and then `EditMetadata` on it. An absent
/// field keeps its value and a `null` clears an optional value.
#[utoipa::path(
    patch,
    operation_id = "patch_series",
    path = "/series/{series_id}",
    tag = "library",
    params(
        ("series_id" = NumericID, Path, description = "Identifier of the show to update"),
    ),
    request_body = PatchSeriesRequest,
    responses(
        (
            status = OK,
            body = SeriesResponse,
            content_type = "application/hal+json",
            description = "Show updated"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid show identifier or malformed body"
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
            description = "Unknown or invisible show"
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
            description = "The name, country, genre or first-air date is invalid"
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
    summary = "Update a show"
)]
pub(crate) async fn patch_series(
    Path(series_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: JsonObject<PatchSeriesRequest>,
) -> Result<Response, ApiError> {
    let series_id = parse_series_id(&series_id_value)?;
    let JsonObject(request) = payload;
    let first_air_date = request.first_air_date.try_map(|value| {
        NaiveDate::parse_from_str(&value, DATE_FORMAT).map_err(|_| {
            warn!(
                target: "security",
                event = "input_validation_failed",
                field = "first_air_date",
                reason = "invalid_date",
                "rejected a malformed first-air date"
            );
            ApiError::UnprocessableEntity("the first air date must use YYYY-MM-DD".to_owned())
        })
    })?;
    let response = state
        .library_use_case_factory()
        .update_series()
        .execute(UpdateSeriesCommand::new(
            caller.user_id(),
            series_id,
            request.name,
            request.plot,
            request.country_of_origin,
            first_air_date,
            request.genres,
            request.cover_image_id,
        ))
        .await?;
    Ok(hal_json(SeriesResponse::from(response.series())))
}
