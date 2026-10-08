use axum::extract::Query;
use axum::extract::State;
use axum::extract::rejection::QueryRejection;
use axum::response::Response;
use serde::Deserialize;
use serde::Serialize;
use tracing::warn;
use utoipa::IntoParams;
use utoipa::ToSchema;

use crate::application::port::list_galleries::ListGalleriesCommand;
use crate::application::port::list_galleries::ListGalleriesError;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::links::GalleryListLinks;
use crate::infrastructure::inbound::rest::handler::library::representation::GalleryResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Default number of galleries returned by a listing when `limit` is omitted.
const DEFAULT_LIMIT: usize = 20;

/// Largest number of galleries a listing may return.
const MAX_LIMIT: usize = 100;

/// Filters and pagination restricting the returned galleries.
///
/// Every filter is optional; an all-`None` query returns the first page of the
/// galleries the caller may see.
#[derive(Debug, Deserialize, Serialize, IntoParams)]
#[non_exhaustive]
pub struct ListGalleriesQuery {
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

/// Galleries matching the requested filters.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct GalleryListResponse {
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

/// List the galleries the caller may see.
///
/// Any authenticated caller may list galleries: public galleries are visible to
/// all, private galleries only to their owner and administrators. The
/// collection is paginated and filterable, and returned as a HAL collection
/// with navigation and search links.
#[utoipa::path(
    get,
    operation_id = "list_galleries",
    path = "/library/gallery",
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
            description = "Invalid filter values"
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
pub async fn get_gallery_list(
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

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::body::to_bytes;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::response::Response;
    use axum::routing::get;
    use chrono::NaiveDate;
    use chrono::NaiveDateTime;
    use mockall::predicate::eq;
    use serde_json::Value;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::get_gallery_list;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::list_galleries::ListGalleriesCommand;
    use crate::application::port::list_galleries::ListGalleriesError;
    use crate::application::port::list_galleries::ListGalleriesItem;
    use crate::application::port::list_galleries::ListGalleriesResponse;
    use crate::application::port::list_galleries::ListGalleriesUseCase;
    use crate::application::port::list_galleries::MockListGalleriesUseCase;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;

    /// A fixed instant the fixtures timestamp with.
    fn ts() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 6)
            .unwrap_or_default()
            .and_hms_opt(12, 0, 0)
            .unwrap_or_default()
    }

    /// Send a GET to `/api/v1/library/gallery{query}` on behalf of `caller_id`,
    /// injecting the caller identifier the way the auth middleware does.
    async fn send(
        use_case: MockListGalleriesUseCase,
        caller_id: NumericID,
        query: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_list_galleries()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn ListGalleriesUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/library/gallery{query}"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/library/gallery", get(get_gallery_list))
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    /// Split `response` into its status, `Content-Type` header and decoded
    /// JSON body.
    async fn into_parts(
        response: Response,
    ) -> Result<(StatusCode, Option<String>, Value), Box<dyn Error>> {
        let status = response.status();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let body = serde_json::from_slice(&bytes)?;
        Ok((status, content_type, body))
    }

    /// Make the mocked use case fail with `error`.
    fn expect_error(use_case: &mut MockListGalleriesUseCase, error: ListGalleriesError) {
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));
    }

    /// The gallery representation of a public gallery owned by user `1`.
    fn gallery_item() -> ListGalleriesItem {
        ListGalleriesItem::new(2, Some(1), "Holidays".to_owned(), true, ts(), ts(), 3)
    }

    #[tokio::test]
    async fn get_gallery_list_no_filter_returns_collection() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListGalleriesUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(ListGalleriesCommand::new(
                1,
                None,
                None,
                None,
                Some(20),
                Some(0),
            )))
            .return_once(|_| {
                Box::pin(async { Ok(ListGalleriesResponse::new(vec![gallery_item()], 1)) })
            });

        // Act
        let (status, content_type, payload) = into_parts(send(use_case, 1, "").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(
            payload,
            json!({
                "_links": {
                    "self": { "href": "/api/v1/library/gallery?limit=20&offset=0" },
                    "search": {
                        "href": "/api/v1/library/gallery{?name,is_public,owner_id,limit,offset}",
                        "templated": true
                    }
                },
                "items": [{
                    "_links": {
                        "self": { "href": "/api/v1/library/gallery/2" },
                        "items": { "href": "/api/v1/library/gallery/2/item" }
                    },
                    "id": 2i64,
                    "name": "Holidays",
                    "owner_id": 1i64,
                    "is_public": true,
                    "created_at": "2026-10-06T12:00:00",
                    "last_modified_at": "2026-10-06T12:00:00",
                    "item_count": 3i64,
                    "items": []
                }]
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_list_filters_are_forwarded() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListGalleriesUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(ListGalleriesCommand::new(
                1,
                Some("Trip".to_owned()),
                Some(true),
                Some(4),
                Some(20),
                Some(0),
            )))
            .return_once(|_| Box::pin(async { Ok(ListGalleriesResponse::new(Vec::new(), 0)) }));

        // Act
        let (status, _, payload) =
            into_parts(send(use_case, 1, "?name=Trip&is_public=true&owner_id=4").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            payload,
            json!({
                "_links": {
                    "self": {
                        "href": "/api/v1/library/gallery?name=Trip&is_public=true&owner_id=4&limit=20&offset=0"
                    },
                    "search": {
                        "href": "/api/v1/library/gallery{?name,is_public,owner_id,limit,offset}",
                        "templated": true
                    }
                },
                "items": []
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_list_full_page_exposes_a_next_link() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListGalleriesUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(ListGalleriesCommand::new(
                1,
                None,
                None,
                None,
                Some(1),
                Some(0),
            )))
            .return_once(|_| {
                Box::pin(async { Ok(ListGalleriesResponse::new(vec![gallery_item()], 2)) })
            });

        // Act
        let (status, _, payload) = into_parts(send(use_case, 1, "?limit=1").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            payload.get("_links").and_then(|links| links.get("next")),
            Some(&json!({ "href": "/api/v1/library/gallery?limit=1&offset=1" }))
        );
        assert!(
            payload
                .get("_links")
                .and_then(|links| links.get("prev"))
                .is_none(),
            "the first page must not advertise a previous page"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_list_second_page_exposes_prev_and_next() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListGalleriesUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(ListGalleriesCommand::new(
                1,
                None,
                None,
                None,
                Some(2),
                Some(2),
            )))
            .return_once(|_| {
                Box::pin(async { Ok(ListGalleriesResponse::new(vec![gallery_item()], 5)) })
            });

        // Act
        let (status, _, payload) =
            into_parts(send(use_case, 1, "?limit=2&offset=2").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            payload.get("_links").and_then(|links| links.get("prev")),
            Some(&json!({ "href": "/api/v1/library/gallery?limit=2&offset=0" }))
        );
        assert_eq!(
            payload.get("_links").and_then(|links| links.get("next")),
            Some(&json!({ "href": "/api/v1/library/gallery?limit=2&offset=4" }))
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_list_limit_zero_returns_unprocessable_entity() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = MockListGalleriesUseCase::new();

        // Act
        let response = send(use_case, 1, "?limit=0").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("limit must be between 1 and 100")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_list_limit_too_large_returns_unprocessable_entity()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockListGalleriesUseCase::new();

        // Act
        let response = send(use_case, 1, "?limit=101").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_list_unknown_caller_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListGalleriesUseCase::new();
        expect_error(&mut use_case, ListGalleriesError::NoSuchCaller);

        // Act
        let response = send(use_case, 999, "").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the authenticated user does not exist")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_list_dependency_failure_returns_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListGalleriesUseCase::new();
        expect_error(
            &mut use_case,
            ListGalleriesError::Unknown(anyhow::anyhow!("boom")),
        );

        // Act
        let response = send(use_case, 1, "").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("an unexpected error occurred")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_list_filtered_page_preserves_filters_in_links()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListGalleriesUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(ListGalleriesCommand::new(
                1,
                Some("Trip".to_owned()),
                None,
                None,
                Some(1),
                Some(0),
            )))
            .return_once(|_| {
                Box::pin(async { Ok(ListGalleriesResponse::new(vec![gallery_item()], 3)) })
            });

        // Act
        let (status, _, payload) =
            into_parts(send(use_case, 1, "?name=Trip&limit=1").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        let links = payload.get("_links").cloned().unwrap_or(Value::Null);
        assert_eq!(
            links.get("self"),
            Some(&json!({ "href": "/api/v1/library/gallery?name=Trip&limit=1&offset=0" })),
            "self must name the filtered view the representation is about (`API-042`)"
        );
        assert_eq!(
            links.get("next"),
            Some(&json!({ "href": "/api/v1/library/gallery?name=Trip&limit=1&offset=1" })),
            "next must keep the active filter, or the client lands on an unfiltered page (`API-037`)"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_list_percent_encodes_filter_values() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListGalleriesUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command: &ListGalleriesCommand| command.name() == Some("a &é"))
            .return_once(|_| {
                Box::pin(async { Ok(ListGalleriesResponse::new(vec![gallery_item()], 2)) })
            });

        // Act
        let (status, _, payload) =
            into_parts(send(use_case, 1, "?name=a%20%26%C3%A9&limit=1").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            payload
                .get("_links")
                .and_then(|links| links.get("self"))
                .and_then(|link| link.get("href")),
            Some(&Value::String(
                "/api/v1/library/gallery?name=a%20%26%C3%A9&limit=1&offset=0".to_owned()
            )),
            "a filter value must be percent-encoded, or it breaks out of the link (`API-037`)"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_list_false_filter_is_preserved() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListGalleriesUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(ListGalleriesCommand::new(
                1,
                None,
                Some(false),
                None,
                Some(20),
                Some(0),
            )))
            .return_once(|_| {
                Box::pin(async { Ok(ListGalleriesResponse::new(vec![gallery_item()], 3)) })
            });

        // Act
        let (status, _, payload) = into_parts(send(use_case, 1, "?is_public=false").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            payload
                .get("_links")
                .and_then(|links| links.get("self"))
                .and_then(|link| link.get("href")),
            Some(&Value::String(
                "/api/v1/library/gallery?is_public=false&limit=20&offset=0".to_owned()
            ))
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_list_invalid_filter_returns_stable_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockListGalleriesUseCase::new();

        // Act
        let response = send(use_case, 1, "?is_public=maybe").await?;

        // Assert: the handler-owned message is stable, never the framework's
        // rejection text (`API-040`).
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the request query string is invalid")
        );
        Ok(())
    }
}
