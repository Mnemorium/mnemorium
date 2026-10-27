use axum::extract::Query;
use axum::extract::State;
use axum::extract::rejection::QueryRejection;
use axum::response::Response;
use serde::Deserialize;
use tracing::warn;
use utoipa::IntoParams;

use super::DEFAULT_LIMIT;
use super::MAX_LIMIT;
use crate::application::port::list_movies::ListMoviesCommand;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::MovieListLinks;
use crate::infrastructure::inbound::rest::handler::library::MovieListResponse;
use crate::infrastructure::inbound::rest::handler::library::MovieResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Filters and pagination restricting the returned films.
///
/// Every filter is optional; an all-`None` query returns the first page of the
/// films the caller may see.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[non_exhaustive]
pub struct ListMoviesQuery {
    /// Maximum number of films to return, between 1 and 100.
    #[param(default = 20, minimum = 1, maximum = 100)]
    pub limit: Option<usize>,
    /// Only return films whose name matches this value.
    pub name: Option<String>,
    /// Number of matching films to skip before returning the page.
    #[param(default = 0)]
    pub offset: Option<usize>,
}

/// List the films the caller may see.
///
/// Any authenticated caller may list films: a film is not owned and carries no
/// visibility flag, so the collection is the same for every caller. It is
/// paginated and filterable, and returned as a HAL collection with navigation
/// and search links.
#[utoipa::path(
    get,
    operation_id = "list_movies",
    path = "/library/movie",
    tag = "library",
    params(ListMoviesQuery),
    responses(
        (
            status = OK,
            body = MovieListResponse,
            content_type = "application/hal+json",
            description = "Films matching the filters"
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
    summary = "List films"
)]
pub async fn get_movie_list(
    raw_query: Result<Query<ListMoviesQuery>, QueryRejection>,
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
        .list_movies()
        .execute(ListMoviesCommand::new(
            caller.user_id(),
            query.name.clone(),
            Some(limit),
            Some(offset),
        ))
        .await?;

    let returned = response.movies().len();
    let total = response.total();
    let body = MovieListResponse {
        items: response.movies().iter().map(MovieResponse::from).collect(),
        links: MovieListLinks::for_page(&query, limit, offset, returned, total),
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

    use super::get_movie_list;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::list_movies::ListMoviesCommand;
    use crate::application::port::list_movies::ListMoviesError;
    use crate::application::port::list_movies::ListMoviesItem;
    use crate::application::port::list_movies::ListMoviesResponse;
    use crate::application::port::list_movies::ListMoviesUseCase;
    use crate::application::port::list_movies::MockListMoviesUseCase;
    use crate::application::port::video_summary::VideoSummary;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_parts;
    use crate::test_helpers::sample_movie;

    /// Send a GET to `/api/v1/library/movie{query}` on behalf of `caller_id`,
    /// injecting the caller identifier the way the auth middleware does.
    async fn send(
        use_case: MockListMoviesUseCase,
        caller_id: NumericID,
        query: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_list_movies()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn ListMoviesUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/library/movie{query}"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/library/movie", get(get_movie_list))
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    /// The film representation of a drama bearing one progressive video.
    fn movie_item() -> Result<ListMoviesItem, Box<dyn Error>> {
        let movie = sample_movie(7)?;
        Ok(ListMoviesItem::new(
            movie.id(),
            movie.name().to_owned(),
            movie.plot().to_owned(),
            movie.country_of_origin().to_owned(),
            movie.release_date(),
            movie.genre_ids().to_vec(),
            VideoSummary::from(movie.video()),
        ))
    }

    #[tokio::test]
    async fn get_movie_list_no_filter_returns_collection() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListMoviesUseCase::new();
        let item = movie_item()?;
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| *command == ListMoviesCommand::new(1, None, Some(20), Some(0)))
            .return_once(move |_| {
                let copy = item.clone();
                Box::pin(async move { Ok(ListMoviesResponse::new(vec![copy], 1)) })
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
                    "self": { "href": "/api/v1/library/movie?limit=20&offset=0" },
                    "search": {
                        "href": "/api/v1/library/movie{?name,limit,offset}",
                        "templated": true
                    }
                },
                "items": [{
                    "_links": {
                        "self": { "href": "/api/v1/library/movie/7" }
                    },
                    "id": 7i64,
                    "name": "Movie 7",
                    "plot": "A plot",
                    "country_of_origin": "US",
                    "release_date": "1990-04-08",
                    "genres": ["DRAMA"],
                    "video": {
                        "id": 7i64,
                        "file_id": 7i64,
                        "duration_ms": 1_000.0f64,
                        "codec": "H264",
                        "width": 1920i64,
                        "height": 1080i64,
                        "scan_type": "PROGRESSIVE"
                    }
                }]
            }),
            "the collection must carry the full film representation"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_movie_list_filters_are_forwarded() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListMoviesUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                *command == ListMoviesCommand::new(1, Some("Movie".to_owned()), Some(20), Some(0))
            })
            .return_once(|_| Box::pin(async { Ok(ListMoviesResponse::new(Vec::new(), 0)) }));

        // Act
        let (status, _, payload) = into_parts(send(use_case, 1, "?name=Movie").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK, "the listing must succeed");
        assert_eq!(
            payload.get("_links").and_then(|links| links.get("self")),
            Some(&json!({
                "href": "/api/v1/library/movie?name=Movie&limit=20&offset=0"
            })),
            "the self link must preserve the active filter"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_movie_list_full_page_exposes_a_next_link() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListMoviesUseCase::new();
        let item = movie_item()?;
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| *command == ListMoviesCommand::new(1, None, Some(1), Some(0)))
            .return_once(move |_| {
                let copy = item.clone();
                Box::pin(async move { Ok(ListMoviesResponse::new(vec![copy], 2)) })
            });

        // Act
        let (status, _, payload) = into_parts(send(use_case, 1, "?limit=1").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK, "the listing must succeed");
        assert_eq!(
            payload.get("_links").and_then(|links| links.get("next")),
            Some(&json!({ "href": "/api/v1/library/movie?limit=1&offset=1" })),
            "a full page must link to the next page"
        );
        Ok(())
    }

    #[rstest]
    #[case::zero("?limit=0")]
    #[case::too_large("?limit=101")]
    #[tokio::test]
    async fn get_movie_list_limit_out_of_range_returns_unprocessable_entity(
        #[case] query: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockListMoviesUseCase::new();

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
    async fn get_movie_list_malformed_query_returns_stable_bad_request(
        #[case] query: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockListMoviesUseCase::new();

        // Act
        let response = send(use_case, 1, query).await?;

        // Assert: the handler-owned message is stable, never the framework's
        // rejection text (`API-040`).
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
        ListMoviesError::NoSuchCaller,
        StatusCode::UNAUTHORIZED,
        "the authenticated user does not exist"
    )]
    #[case::dependency_failure(
        1,
        ListMoviesError::Unknown(anyhow::anyhow!("boom")),
        StatusCode::INTERNAL_SERVER_ERROR,
        "an unexpected error occurred"
    )]
    #[tokio::test]
    async fn get_movie_list_error_maps_to_status(
        #[case] caller_id: NumericID,
        #[case] error: ListMoviesError,
        #[case] status: StatusCode,
        #[case] message: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListMoviesUseCase::new();
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
    async fn get_movie_list_missing_credentials_returns_unauthorized() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the real router, wrapped in its `authenticate` layer, with no
        // authorization header.
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("GET")
            .uri("/movie")
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
