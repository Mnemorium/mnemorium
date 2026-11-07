use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use tracing::warn;
use utoipa::{IntoParams, ToSchema};

use super::{DEFAULT_LIMIT, MAX_LIMIT};
use crate::application::port::list_galleries::{ListGalleriesCommand, ListGalleriesError};
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::{Link, hal_json};
use crate::infrastructure::inbound::rest::handler::library::GalleryResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Filters and pagination restricting the returned galleries.
///
/// Every filter is optional; an all-`None` query returns the first page of the
/// galleries the caller may see.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[non_exhaustive]
pub(crate) struct ListGalleriesQuery {
    /// Only return galleries whose public flag matches this value.
    pub is_public: Option<bool>,
    /// Maximum number of galleries to return, between 1 and 100.
    #[param(default = 20, minimum = 1, maximum = 100)]
    pub limit: Option<usize>,
    /// Only return galleries whose name matches this value.
    pub name: Option<String>,
    /// Number of matching galleries to skip before returning the page.
    #[param(default = 0)]
    pub offset: Option<usize>,
    /// Only return galleries owned by this user.
    pub owner_id: Option<NumericID>,
}

/// HAL links exposed by a gallery collection representation.
///
/// The `self` link addresses the collection view the representation is about,
/// `next` and `prev` navigate that same view when another page exists, and the
/// templated `search` link exposes the collection filters. The active filters
/// and the page size are carried into every link so a filtered or paginated
/// listing stays navigable.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
struct GalleryListLinks {
    /// Link to the next page, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<Link>,
    /// Link to the previous page, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev: Option<Link>,
    /// Templated link exposing the collection's search filters.
    pub search: Link,
    /// Link to the collection itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

impl GalleryListLinks {
    /// Build the links of a gallery collection page.
    ///
    /// `query` carries the active filters, `limit` and `offset` are the
    /// effective page, `returned` is the number of galleries in the page and
    /// `total` the number of matching galleries before pagination. `next` is
    /// exposed when the page ends before `total`; `prev` is exposed when the
    /// page does not start at the first gallery.
    #[must_use]
    pub(crate) fn for_page(
        query: &ListGalleriesQuery,
        limit: usize,
        offset: usize,
        returned: usize,
        total: usize,
    ) -> Self {
        let next = (offset.saturating_add(returned) < total)
            .then(|| Link::new(&page_href(query, limit, offset.saturating_add(limit))));
        let prev =
            (offset > 0).then(|| Link::new(&page_href(query, limit, offset.saturating_sub(limit))));
        Self {
            next,
            prev,
            search: Link::templated(
                "/api/v1/library/gallery{?name,is_public,owner_id,limit,offset}",
            ),
            self_link: Link::new(&page_href(query, limit, offset)),
        }
    }
}

/// Galleries matching the requested filters.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
struct GalleryListResponse {
    /// The galleries of the returned page.
    pub items: Vec<GalleryResponse>,
    /// Navigation links of the collection.
    #[serde(rename = "_links")]
    pub links: GalleryListLinks,
}

/// Map a list-galleries error to its API error.
impl From<ListGalleriesError> for ApiError {
    fn from(err: ListGalleriesError) -> Self {
        match err {
            ListGalleriesError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            ListGalleriesError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Build the href of the gallery collection view at `limit`/`offset`,
/// preserving the active filters so a filtered page keeps its navigation
/// (`API-037`).
fn page_href(query: &ListGalleriesQuery, limit: usize, offset: usize) -> String {
    let mut href = String::from("/api/v1/library/gallery?");
    if let Some(name) = query.name.as_deref() {
        href.push_str("name=");
        // Percent-encode the filter value with the unreserved character set, so
        // it cannot break out of the query string.
        for byte in name.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                    href.push(char::from(byte));
                }
                _ => {
                    let high = char::from_digit(u32::from(byte >> 4u8), 16)
                        .unwrap_or('0')
                        .to_ascii_uppercase();
                    let low = char::from_digit(u32::from(byte & 0x0fu8), 16)
                        .unwrap_or('0')
                        .to_ascii_uppercase();
                    href.push('%');
                    href.push(high);
                    href.push(low);
                }
            }
        }
        href.push('&');
    }
    if let Some(is_public) = query.is_public {
        href.push_str("is_public=");
        href.push_str(if is_public { "true" } else { "false" });
        href.push('&');
    }
    if let Some(owner_id) = query.owner_id {
        href.push_str("owner_id=");
        href.push_str(&owner_id.to_string());
        href.push('&');
    }
    href.push_str("limit=");
    href.push_str(&limit.to_string());
    href.push_str("&offset=");
    href.push_str(&offset.to_string());
    href
}

/// List the galleries the caller may see.
///
/// Any authenticated caller may list galleries: public galleries are visible to
/// all, private galleries only to their owner and the Root Admin. The
/// collection is paginated and filterable, and returned as a HAL collection
/// with navigation and search links.
#[utoipa::path(
    get,
    operation_id = "list_galleries",
    path = "/gallery",
    tag = "library",
    params(ListGalleriesQuery),
    responses(
        (
            status = OK,
            body = GalleryListResponse,
            content_type = "application/hal+json",
            description = "Galleries matching the filters"
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
    summary = "List galleries"
)]
pub(crate) async fn get_gallery_list(
    raw_query: Result<Query<ListGalleriesQuery>, QueryRejection>,
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
        .list_galleries()
        .execute(ListGalleriesCommand::new(
            caller.user_id(),
            query.name.clone(),
            query.is_public,
            query.owner_id,
            Some(limit),
            Some(offset),
        ))
        .await?;

    let returned = response.galleries().len();
    let total = response.total();
    let body = GalleryListResponse {
        items: response
            .galleries()
            .iter()
            .cloned()
            .map(GalleryResponse::from)
            .collect(),
        links: GalleryListLinks::for_page(&query, limit, offset, returned, total),
    };
    Ok(hal_json(body))
}
