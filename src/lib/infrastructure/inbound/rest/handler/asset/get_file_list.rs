use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use tracing::warn;
use utoipa::{IntoParams, ToSchema};

use crate::application::port::{ListFilesCommand, ListFilesError};
use crate::infrastructure::inbound::rest::handler::asset::FileResponse;
use crate::infrastructure::inbound::rest::handler::{DEFAULT_LIMIT, MAX_LIMIT};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, Link, hal_json};

/// Filters and pagination restricting the returned stored files.
///
/// Every filter is optional; an all-`None` query returns the first page of the
/// files the caller may discover.
#[derive(Debug, Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
#[into_params(parameter_in = Query)]
#[non_exhaustive]
pub(crate) struct ListFilesQuery {
    /// Maximum number of files to return, between 1 and 100.
    #[param(default = 20, minimum = 1, maximum = 100)]
    pub limit: Option<usize>,
    /// Number of matching files to skip before returning the page.
    #[param(default = 0, maximum = 18_446_744_073_709_551_615usize)]
    pub offset: Option<usize>,
    /// Only return the file whose root-relative path matches this value
    /// exactly.
    pub path: Option<String>,
}

/// HAL links exposed by a stored-file collection representation.
///
/// The `self` link addresses the collection view the representation is about,
/// `next` and `prev` navigate that same view when another page exists, and the
/// templated `search` link exposes the collection filters (`REQ-API-006`). The
/// active filter and the page size are carried into every link so a filtered or
/// paginated listing stays navigable.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
struct FileListLinks {
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

impl FileListLinks {
    /// Build the links of a stored-file collection page.
    ///
    /// `query` carries the active filter, `limit` and `offset` are the effective
    /// page, `returned` is the number of files in the page and `total` the
    /// number of discoverable files before pagination. `next` is exposed when
    /// the page ends before `total`; `prev` is exposed when the page does not
    /// start at the first file.
    #[must_use]
    fn for_page(
        query: &ListFilesQuery,
        limit: usize,
        offset: usize,
        returned: usize,
        total: usize,
    ) -> Self {
        let page_href = |page_offset: usize| {
            let mut href = String::from("/api/v1/asset/file?");
            if let Some(path) = query.path.as_deref() {
                href.push_str("path=");
                // Percent-encode the filter value with the unreserved character
                // set, so a path cannot break out of the query string.
                for byte in path.bytes() {
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
            href.push_str("limit=");
            href.push_str(&limit.to_string());
            href.push_str("&offset=");
            href.push_str(&page_offset.to_string());
            href
        };
        let next = (offset.saturating_add(returned) < total)
            .then(|| Link::new(&page_href(offset.saturating_add(limit))));
        let prev = (offset > 0).then(|| Link::new(&page_href(offset.saturating_sub(limit))));
        Self {
            next,
            prev,
            search: Link::templated("/api/v1/asset/file{?path,limit,offset}"),
            self_link: Link::new(&page_href(offset)),
        }
    }
}

/// Stored files matching the requested filters.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
struct FileListResponse {
    /// The files of the returned page.
    pub items: Vec<FileResponse>,
    /// Navigation links of the collection.
    #[serde(rename = "_links")]
    pub links: FileListLinks,
}

/// Map a list-files error to its API error.
impl From<ListFilesError> for ApiError {
    fn from(err: ListFilesError) -> Self {
        match err {
            ListFilesError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            ListFilesError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// List the stored files the caller may discover.
///
/// Any authenticated caller may list files: a file is listed when its `FILE`
/// resource grants `Discover`, so a library asset a caller may read appears and
/// another caller's private upload does not. The collection is paginated and
/// filterable by one exact root-relative path, and returned as a HAL collection
/// with navigation and search links.
#[utoipa::path(
    get,
    operation_id = "list_files",
    path = "/file",
    tag = "asset",
    params(ListFilesQuery),
    responses(
        (
            status = OK,
            body = FileListResponse,
            content_type = "application/hal+json",
            description = "Stored files matching the filters"
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
    summary = "List the stored files"
)]
pub(crate) async fn get_file_list(
    raw_query: Result<Query<ListFilesQuery>, QueryRejection>,
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
        .asset_use_case_factory()
        .list_files()
        .execute(ListFilesCommand::new(
            caller.user_id(),
            query.path.clone(),
            Some(limit),
            Some(offset),
        ))
        .await?;

    let returned = response.files().len();
    let total = response.total();
    let body = FileListResponse {
        items: response
            .files()
            .iter()
            .map(FileResponse::from_response)
            .collect(),
        links: FileListLinks::for_page(&query, limit, offset, returned, total),
    };
    Ok(hal_json(body))
}
