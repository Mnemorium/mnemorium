use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::port::PutSeriesSeasonCommand;
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::JsonObject;
use crate::infrastructure::inbound::rest::handler::library::{
    SeasonResponse, parse_season_index, parse_series_id,
};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, hal_json};

/// Payload to create or replace a show season.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PutSeriesSeasonRequest {
    /// Identifier of the image holding the season's artwork, when set.
    #[schema(example = 1)]
    pub cover_image_id: Option<NumericID>,
    /// Name of the season, when set.
    pub name: Option<String>,
}

/// Create or replace a show season.
///
/// Only a caller whose `library.manage` scope covers the whole catalogue may
/// write it. The write is idempotent by index: a free index creates the season
/// and an existing index replaces its metadata, keeping its episodes.
#[utoipa::path(
    put,
    operation_id = "put_series_season",
    path = "/series/{series_id}/season/{season_index}",
    tag = "library",
    params(
        ("series_id" = NumericID, Path, description = "Identifier of the show holding the season"),
        ("season_index" = u64, Path, description = "Series-scoped index of the season", maximum = 4_294_967_295u64),
    ),
    request_body = PutSeriesSeasonRequest,
    responses(
        (
            status = CREATED,
            body = SeasonResponse,
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "URI of the newly created season"),
            ),
            description = "Season created",
        ),
        (
            status = OK,
            body = SeasonResponse,
            content_type = "application/hal+json",
            description = "Season replaced"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid show identifier or season index"
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
            description = "The cover is not a library image"
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
    summary = "Create or replace a show season"
)]
pub(crate) async fn put_series_season(
    Path((series_id_value, season_index_value)): Path<(String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: JsonObject<PutSeriesSeasonRequest>,
) -> Result<Response, ApiError> {
    let series_id = parse_series_id(&series_id_value)?;
    let season_index = parse_season_index(&season_index_value)?;
    let JsonObject(request) = payload;
    let response = state
        .library_use_case_factory()
        .put_series_season()
        .execute(PutSeriesSeasonCommand::new(
            caller.user_id(),
            series_id,
            season_index,
            request.name,
            request.cover_image_id,
        ))
        .await?;
    let season = response.season();
    let body = SeasonResponse::new(
        series_id,
        season.index().value(),
        season.name().map(str::to_owned),
        season.episodes().len(),
        season.cover_image_id(),
    );
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
