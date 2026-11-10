use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::response::Response;
use serde::Deserialize;
use tracing::warn;
use utoipa::IntoParams;

use super::{DEFAULT_LIMIT, MAX_LIMIT};
use crate::application::port::ListPlaylistsCommand;
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::library::{
    PlaylistListLinks, PlaylistListResponse, PlaylistResponse,
};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, hal_json};

/// Filters and pagination restricting the returned playlists.
///
/// Every filter is optional; an all-`None` query returns the first page of the
/// playlists the caller may see.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[non_exhaustive]
pub(crate) struct ListPlaylistsQuery {
    /// Only return playlists whose public flag matches this value.
    pub is_public: Option<bool>,
    /// Maximum number of playlists to return, between 1 and 100.
    #[param(default = 20, minimum = 1, maximum = 100)]
    pub limit: Option<usize>,
    /// Only return playlists whose name matches this value.
    pub name: Option<String>,
    /// Number of matching playlists to skip before returning the page.
    #[param(default = 0, maximum = 18_446_744_073_709_551_615usize)]
    pub offset: Option<usize>,
    /// Only return playlists owned by this user.
    pub owner_id: Option<NumericID>,
}

/// List the playlists the caller may see.
///
/// Any authenticated caller may list playlists: public playlists are visible to
/// all, private playlists only to their owner and the Root Admin. The collection
/// is paginated and filterable, and returned as a HAL collection with
/// navigation and search links.
#[utoipa::path(
    get,
    operation_id = "list_playlists",
    path = "/music/playlist",
    tag = "library",
    params(ListPlaylistsQuery),
    responses(
        (
            status = OK,
            body = PlaylistListResponse,
            content_type = "application/hal+json",
            description = "Playlists matching the filters"
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
    summary = "List music playlists"
)]
pub(crate) async fn get_playlist_list(
    raw_query: Result<Query<ListPlaylistsQuery>, QueryRejection>,
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
        .list_playlists()
        .execute(ListPlaylistsCommand::new(
            caller.user_id(),
            query.name.clone(),
            query.is_public,
            query.owner_id,
            Some(limit),
            Some(offset),
        ))
        .await?;

    let returned = response.playlists().len();
    let total = response.total();
    let body = PlaylistListResponse {
        items: response
            .playlists()
            .iter()
            .map(PlaylistResponse::from_listed)
            .collect(),
        links: PlaylistListLinks::for_page(&query, limit, offset, returned, total),
    };
    Ok(hal_json(body))
}
