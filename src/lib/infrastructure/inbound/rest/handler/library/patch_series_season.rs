use axum::extract::{Path, State};
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::field_update::FieldUpdate;
use crate::application::port::update_series_season::UpdateSeriesSeasonCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::JsonObject;
use crate::infrastructure::inbound::rest::handler::library::{
    SeasonResponse, deserialize_field_update, parse_season_index, parse_series_id,
};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to update a show season partially.
///
/// An absent field keeps its value; a `null` clears the name or cover. The
/// season's episodes are never part of the payload.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PatchSeriesSeasonRequest {
    /// Identifier of the image holding the season's artwork; `null` clears it.
    #[serde(default, deserialize_with = "deserialize_field_update")]
    #[schema(value_type = Option<NumericID>, nullable = true)]
    pub cover_image_id: FieldUpdate<NumericID>,
    /// Name of the season; `null` clears it.
    #[serde(default, deserialize_with = "deserialize_field_update")]
    #[schema(value_type = Option<String>, nullable = true)]
    pub name: FieldUpdate<String>,
}

/// Update a show season partially.
///
/// Only a caller whose `library.manage` scope covers the show may update it.
#[utoipa::path(
    patch,
    operation_id = "patch_series_season",
    path = "/series/{series_id}/season/{season_index}",
    tag = "library",
    params(
        ("series_id" = NumericID, Path, description = "Identifier of the show holding the season"),
        ("season_index" = u64, Path, description = "Series-scoped index of the season", maximum = 4_294_967_295u64),
    ),
    request_body = PatchSeriesSeasonRequest,
    responses(
        (
            status = OK,
            body = SeasonResponse,
            content_type = "application/hal+json",
            description = "Season updated"
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
            description = "Unknown or invisible show, or a season the show does not hold"
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
    summary = "Update a show season"
)]
pub(crate) async fn patch_series_season(
    Path((series_id_value, season_index_value)): Path<(String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: JsonObject<PatchSeriesSeasonRequest>,
) -> Result<Response, ApiError> {
    let series_id = parse_series_id(&series_id_value)?;
    let season_index = parse_season_index(&season_index_value)?;
    let JsonObject(request) = payload;
    let response = state
        .library_use_case_factory()
        .update_series_season()
        .execute(UpdateSeriesSeasonCommand::new(
            caller.user_id(),
            series_id,
            season_index,
            request.name,
            request.cover_image_id,
        ))
        .await?;
    let season = response.season();
    Ok(hal_json(SeasonResponse::new(
        series_id,
        season.index().value(),
        season.name().map(str::to_owned),
        season.episodes().len(),
        season.cover_image_id(),
    )))
}
