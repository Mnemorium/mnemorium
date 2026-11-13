use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::response::Response;
use serde::Deserialize;
use tracing::warn;
use utoipa::IntoParams;

use super::{DEFAULT_LIMIT, MAX_LIMIT};
use crate::application::port::ListMusicGroupsCommand;
use crate::infrastructure::inbound::rest::handler::library::{
    MusicGroupEmbedded, MusicGroupListLinks, MusicGroupListResponse, MusicGroupResponse,
};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, hal_json};

/// Filters and pagination restricting the returned music groups.
///
/// Every filter is optional; an all-`None` query returns the first page of the
/// groups the caller may see.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[non_exhaustive]
pub(crate) struct ListMusicGroupsQuery {
    /// Maximum number of groups to return, between 1 and 100.
    #[param(default = 20, minimum = 1, maximum = 100)]
    pub limit: Option<usize>,
    /// Only return groups whose name matches this value.
    pub name: Option<String>,
    /// Number of matching groups to skip before returning the page.
    #[param(default = 0, maximum = 18_446_744_073_709_551_615usize)]
    pub offset: Option<usize>,
}

/// List the music groups the caller may see.
///
/// Any authenticated caller with library access may list groups: a group is not
/// owned and carries no visibility flag. It is paginated and filterable, and
/// returned as a HAL collection with navigation and search links.
#[utoipa::path(
    get,
    operation_id = "list_music_groups",
    path = "/music/group",
    tag = "library",
    params(ListMusicGroupsQuery),
    responses(
        (
            status = OK,
            body = MusicGroupListResponse,
            content_type = "application/hal+json",
            description = "Music groups matching the filters"
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
    summary = "List music groups"
)]
pub(crate) async fn get_music_group_list(
    raw_query: Result<Query<ListMusicGroupsQuery>, QueryRejection>,
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
        .list_music_groups()
        .execute(ListMusicGroupsCommand::new(
            caller.user_id(),
            query.name.clone(),
            Some(limit),
            Some(offset),
        ))
        .await?;

    let returned = response.groups().len();
    let total = response.total();
    let body = MusicGroupListResponse {
        embedded: MusicGroupEmbedded {
            items: response
                .groups()
                .iter()
                .map(MusicGroupResponse::from)
                .collect(),
        },
        links: MusicGroupListLinks::for_page(&query, limit, offset, returned, total),
    };
    Ok(hal_json(body))
}
