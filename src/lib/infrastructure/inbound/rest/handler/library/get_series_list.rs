use axum::extract::Query;
use axum::extract::State;
use axum::extract::rejection::QueryRejection;
use axum::response::Response;
use serde::Deserialize;
use tracing::warn;
use utoipa::IntoParams;

use super::DEFAULT_LIMIT;
use super::MAX_LIMIT;
use crate::application::port::list_series::ListSeriesCommand;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::SeriesListLinks;
use crate::infrastructure::inbound::rest::handler::library::SeriesListResponse;
use crate::infrastructure::inbound::rest::handler::library::SeriesSummaryResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Filters and pagination restricting the returned shows.
///
/// Every filter is optional; an all-`None` query returns the first page of the
/// shows the caller may see.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[non_exhaustive]
pub struct ListSeriesQuery {
    /// Maximum number of shows to return, between 1 and 100.
    #[param(default = 20, minimum = 1, maximum = 100)]
    pub limit: Option<usize>,
    /// Only return shows whose name matches this value.
    pub name: Option<String>,
    /// Number of matching shows to skip before returning the page.
    #[param(default = 0)]
    pub offset: Option<usize>,
}

/// List the shows the caller may see.
///
/// Any authenticated caller may list shows: a show is not owned and carries no
/// visibility flag, so the collection is the same for every caller. It is
/// paginated and filterable, and each item carries only the show's identifier
/// and name as a navigation index.
#[utoipa::path(
    get,
    operation_id = "list_series",
    path = "/library/series",
    tag = "library",
    params(ListSeriesQuery),
    responses(
        (
            status = OK,
            body = SeriesListResponse,
            content_type = "application/hal+json",
            description = "Shows matching the filters"
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
    summary = "List shows"
)]
pub async fn get_series_list(
    raw_query: Result<Query<ListSeriesQuery>, QueryRejection>,
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
        .list_series()
        .execute(ListSeriesCommand::new(
            caller.user_id(),
            query.name.clone(),
            Some(limit),
            Some(offset),
        ))
        .await?;

    let returned = response.series().len();
    let total = response.total();
    let body = SeriesListResponse {
        items: response
            .series()
            .iter()
            .map(SeriesSummaryResponse::from)
            .collect(),
        links: SeriesListLinks::for_page(&query, limit, offset, returned, total),
    };
    Ok(hal_json(body))
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::response::Response;
    use axum::routing::get;
    use rstest::rstest;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::get_series_list;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::list_series::ListSeriesCommand;
    use crate::application::port::list_series::ListSeriesError;
    use crate::application::port::list_series::ListSeriesItem;
    use crate::application::port::list_series::ListSeriesResponse;
    use crate::application::port::list_series::ListSeriesUseCase;
    use crate::application::port::list_series::MockListSeriesUseCase;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_parts;

    /// Send a GET to `/api/v1/library/series{query}` on behalf of `caller_id`,
    /// injecting the caller identifier the way the auth middleware does.
    async fn send(
        use_case: MockListSeriesUseCase,
        caller_id: NumericID,
        query: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_list_series()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn ListSeriesUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/library/series{query}"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/library/series", get(get_series_list))
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn get_series_list_no_filter_returns_collection() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListSeriesUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| *command == ListSeriesCommand::new(1, None, Some(20), Some(0)))
            .return_once(|_| {
                Box::pin(async {
                    Ok(ListSeriesResponse::new(
                        vec![ListSeriesItem::new(7, "Twin Peaks".to_owned())],
                        1,
                    ))
                })
            });

        // Act
        let (status, content_type, payload) = into_parts(send(use_case, 1, "").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK, "the listing must succeed");
        assert_eq!(
            content_type.as_deref(),
            Some("application/hal+json"),
            "the listing must be served as HAL"
        );
        assert_eq!(
            payload,
            json!({
                "_links": {
                    "self": { "href": "/api/v1/library/series?limit=20&offset=0" },
                    "search": {
                        "href": "/api/v1/library/series{?name,limit,offset}",
                        "templated": true
                    }
                },
                "items": [{
                    "_links": {
                        "self": { "href": "/api/v1/library/series/7" }
                    },
                    "id": 7i64,
                    "name": "Twin Peaks"
                }]
            }),
            "each item must carry only the identifier, name and self link"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_series_list_filters_are_forwarded() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListSeriesUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                *command == ListSeriesCommand::new(1, Some("Twin".to_owned()), Some(20), Some(0))
            })
            .return_once(|_| Box::pin(async { Ok(ListSeriesResponse::new(Vec::new(), 0)) }));

        // Act
        let (status, _, payload) = into_parts(send(use_case, 1, "?name=Twin").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK, "the listing must succeed");
        assert_eq!(
            payload.get("_links").and_then(|links| links.get("self")),
            Some(&json!({
                "href": "/api/v1/library/series?name=Twin&limit=20&offset=0"
            })),
            "the self link must preserve the active filter"
        );
        Ok(())
    }

    #[rstest]
    #[case::zero("?limit=0")]
    #[case::too_large("?limit=101")]
    #[tokio::test]
    async fn get_series_list_limit_out_of_range_returns_unprocessable_entity(
        #[case] query: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockListSeriesUseCase::new();

        // Act
        let response = send(use_case, 1, query).await?;

        // Assert
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "an out-of-range limit must be rejected"
        );
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("limit must be between 1 and 100"),
            "the rejection message must be stable"
        );
        Ok(())
    }

    #[rstest]
    #[case::limit("?limit=abc")]
    #[case::offset("?offset=abc")]
    #[tokio::test]
    async fn get_series_list_malformed_query_returns_stable_bad_request(
        #[case] query: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockListSeriesUseCase::new();

        // Act
        let response = send(use_case, 1, query).await?;

        // Assert
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "a malformed query must be rejected"
        );
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the request query string is invalid"),
            "the rejection message must be stable"
        );
        Ok(())
    }

    #[rstest]
    #[case::unknown_caller(
        999,
        ListSeriesError::NoSuchCaller,
        StatusCode::UNAUTHORIZED,
        "the authenticated user does not exist"
    )]
    #[case::dependency_failure(
        1,
        ListSeriesError::Unknown(anyhow::anyhow!("boom")),
        StatusCode::INTERNAL_SERVER_ERROR,
        "an unexpected error occurred"
    )]
    #[tokio::test]
    async fn get_series_list_error_maps_to_status(
        #[case] caller_id: NumericID,
        #[case] error: ListSeriesError,
        #[case] status: StatusCode,
        #[case] message: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListSeriesUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));

        // Act
        let response = send(use_case, caller_id, "").await?;

        // Assert
        assert_eq!(
            response.status(),
            status,
            "the error must map to its status"
        );
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some(message),
            "the error message must be stable"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_series_list_missing_credentials_returns_unauthorized() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the real router, wrapped in its `authenticate` layer, with no
        // authorization header.
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("GET")
            .uri("/series")
            .body(Body::empty())?;

        // Act
        let response: Response = library_routes(&state).oneshot(request).await?;

        // Assert
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "a request without credentials must be rejected"
        );
        Ok(())
    }
}
