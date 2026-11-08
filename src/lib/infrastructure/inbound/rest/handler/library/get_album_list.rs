use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::response::Response;
use serde::Deserialize;
use tracing::warn;
use utoipa::IntoParams;

use super::{DEFAULT_LIMIT, MAX_LIMIT};
use crate::application::port::list_albums::ListAlbumsCommand;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::{
    AlbumListLinks, AlbumListResponse, AlbumResponse,
};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Filters and pagination restricting the returned music albums.
///
/// Every filter is optional; an all-`None` query returns the first page of the
/// albums the caller may see.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[non_exhaustive]
pub(crate) struct ListAlbumsQuery {
    /// Maximum number of albums to return, between 1 and 100.
    #[param(default = 20, minimum = 1, maximum = 100)]
    pub limit: Option<usize>,
    /// Only return albums whose name matches this value.
    pub name: Option<String>,
    /// Number of matching albums to skip before returning the page.
    #[param(default = 0, maximum = 18_446_744_073_709_551_615usize)]
    pub offset: Option<usize>,
}

/// List the music albums the caller may see.
///
/// Any authenticated caller may list albums: an album is not owned and carries
/// no visibility flag, so the collection is the same for every caller. It is
/// paginated and filterable, and returned as a HAL collection with navigation
/// and search links.
#[utoipa::path(
    get,
    operation_id = "list_albums",
    path = "/music/album",
    tag = "library",
    params(ListAlbumsQuery),
    responses(
        (
            status = OK,
            body = AlbumListResponse,
            content_type = "application/hal+json",
            description = "Music albums matching the filters"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Malformed query string"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The limit is out of range"
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
    summary = "List music albums"
)]
pub(crate) async fn get_album_list(
    raw_query: Result<Query<ListAlbumsQuery>, QueryRejection>,
    caller: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Response, ApiError> {
    let Query(query) = raw_query.map_err(ApiError::from)?;
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT);
    if limit == 0 || limit > MAX_LIMIT {
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "limit",
            reason = "out_of_range",
            "rejected an out-of-range page limit"
        );
        return Err(ApiError::UnprocessableEntity(format!(
            "limit must be between 1 and {MAX_LIMIT}"
        )));
    }
    let offset = query.offset.unwrap_or(0);
    let response = state
        .library_use_case_factory()
        .list_albums()
        .execute(ListAlbumsCommand::new(
            caller.user_id(),
            query.name.clone(),
            Some(limit),
            Some(offset),
        ))
        .await?;

    let returned = response.albums().len();
    let total = response.total();
    let body = AlbumListResponse {
        items: response.albums().iter().map(AlbumResponse::from).collect(),
        links: AlbumListLinks::for_page(&query, limit, offset, returned, total),
    };
    Ok(hal_json(body))
}
