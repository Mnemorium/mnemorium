use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use chrono::NaiveDate;
use serde::Deserialize;
use tracing::warn;
use utoipa::ToSchema;

use crate::application::port::create_series::CreateSeriesCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::{DATE_FORMAT, SeriesResponse};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to create a show.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PostSeriesRequest {
    /// Country of origin as an ISO 3166-1 alpha-2 code, when known.
    #[schema(example = "US")]
    pub country_of_origin: Option<String>,
    /// Identifier of the image holding the show's artwork, when set.
    #[schema(example = 1)]
    pub cover_image_id: Option<NumericID>,
    /// Date of the first airing, in `YYYY-MM-DD` form, when known.
    #[schema(example = "1990-04-08")]
    pub first_air_date: Option<String>,
    /// Genres of the show.
    pub genres: Option<Vec<String>>,
    /// Name of the show.
    #[schema(min_length = 1, max_length = 100)]
    pub name: String,
    /// Plot of the show, when known.
    pub plot: Option<String>,
}

/// Create a show.
///
/// Only a caller whose `library.manage` scope covers the whole catalogue may
/// create a show. Creation is insert-only: the schema holds no unique show
/// name, so a repeated name creates another show.
#[utoipa::path(
    post,
    operation_id = "post_series",
    path = "/series",
    tag = "library",
    request_body = PostSeriesRequest,
    responses(
        (
            status = CREATED,
            body = SeriesResponse,
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "URI of the newly created show"),
            ),
            description = "Show created",
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
    summary = "Create a show"
)]
pub(crate) async fn post_series(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PostSeriesRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(request) = payload.map_err(ApiError::from)?;
    let first_air_date = match request.first_air_date.as_deref() {
        Some(value) => Some(NaiveDate::parse_from_str(value, DATE_FORMAT).map_err(|_| {
            warn!(
                target: "security",
                event = "input_validation_failed",
                field = "first_air_date",
                reason = "invalid_date",
                "rejected a malformed first-air date"
            );
            ApiError::UnprocessableEntity("the first air date must use YYYY-MM-DD".to_owned())
        })?),
        None => None,
    };
    let response = state
        .library_use_case_factory()
        .create_series()
        .execute(CreateSeriesCommand::new(
            caller.user_id(),
            request.name,
            request.plot,
            request.country_of_origin,
            first_air_date,
            request.genres.unwrap_or_default(),
            request.cover_image_id,
        ))
        .await?;
    let body = SeriesResponse::from(response.series());
    let location = body.links.self_link.href.clone();
    let mut http_response = hal_json(body);
    *http_response.status_mut() = StatusCode::CREATED;
    http_response.headers_mut().insert(
        header::LOCATION,
        HeaderValue::from_str(&location).map_err(|_| ApiError::InternalServerError)?,
    );
    Ok(http_response)
}
