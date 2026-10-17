use axum::extract::Query;
use axum::extract::State;
use axum::extract::rejection::QueryRejection;
use axum::response::Response;
use serde::Deserialize;
use tracing::warn;
use utoipa::IntoParams;

use crate::application::port::list_albums::ListAlbumsCommand;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::AlbumListLinks;
use crate::infrastructure::inbound::rest::handler::library::AlbumListResponse;
use crate::infrastructure::inbound::rest::handler::library::AlbumResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Default number of albums returned by a listing when `limit` is omitted.
const DEFAULT_LIMIT: usize = 20;

/// Largest number of albums a listing may return.
const MAX_LIMIT: usize = 100;

/// Filters and pagination restricting the returned music albums.
///
/// Every filter is optional; an all-`None` query returns the first page of the
/// albums the caller may see.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[non_exhaustive]
pub struct ListAlbumsQuery {
    /// Maximum number of albums to return, between 1 and 100.
    #[param(default = 20, minimum = 1, maximum = 100)]
    pub limit: Option<usize>,
    /// Only return albums whose name matches this value.
    pub name: Option<String>,
    /// Number of matching albums to skip before returning the page.
    #[param(default = 0)]
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
    path = "/library/music/album",
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
    summary = "List music albums"
)]
pub async fn get_album_list(
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

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::response::Response;
    use axum::routing::get;
    use mockall::predicate::eq;
    use rstest::rstest;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::get_album_list;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::list_albums::ListAlbumsCommand;
    use crate::application::port::list_albums::ListAlbumsError;
    use crate::application::port::list_albums::ListAlbumsItem;
    use crate::application::port::list_albums::ListAlbumsResponse;
    use crate::application::port::list_albums::ListAlbumsUseCase;
    use crate::application::port::list_albums::MockListAlbumsUseCase;
    use crate::domain::alias::NumericID;
    use crate::domain::model::music_album::ProductionType;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::album_release_date;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_parts;

    /// Send a GET to `/api/v1/library/music/album{query}` on behalf of
    /// `caller_id`, injecting the caller identifier the way the auth
    /// middleware does.
    async fn send(
        use_case: MockListAlbumsUseCase,
        caller_id: NumericID,
        query: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_list_albums()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn ListAlbumsUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/library/music/album{query}"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/library/music/album", get(get_album_list))
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    /// The album representation of a studio album holding one jazz track.
    fn album_item() -> ListAlbumsItem {
        ListAlbumsItem::new(
            7,
            "Kind of Blue".to_owned(),
            album_release_date(),
            Some(ProductionType::Studio),
            vec!["JAZZ".to_owned()],
            5,
        )
    }

    #[tokio::test]
    async fn get_album_list_no_filter_returns_collection() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListAlbumsUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(ListAlbumsCommand::new(1, None, Some(20), Some(0))))
            .return_once(|_| {
                let item = album_item();
                Box::pin(async move { Ok(ListAlbumsResponse::new(vec![item], 1)) })
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
                    "self": { "href": "/api/v1/library/music/album?limit=20&offset=0" },
                    "search": {
                        "href": "/api/v1/library/music/album{?name,limit,offset}",
                        "templated": true
                    }
                },
                "items": [{
                    "_links": {
                        "self": { "href": "/api/v1/library/music/album/7" },
                        "tracks": { "href": "/api/v1/library/music/album/7/track" }
                    },
                    "id": 7i64,
                    "name": "Kind of Blue",
                    "release_date": "1959-08-17",
                    "production_type": "STUDIO",
                    "genres": ["JAZZ"],
                    "track_count": 5i64
                }]
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_album_list_filters_are_forwarded() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListAlbumsUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(ListAlbumsCommand::new(
                1,
                Some("Kind".to_owned()),
                Some(20),
                Some(0),
            )))
            .return_once(|_| Box::pin(async { Ok(ListAlbumsResponse::new(Vec::new(), 0)) }));

        // Act
        let (status, _, payload) = into_parts(send(use_case, 1, "?name=Kind").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            payload,
            json!({
                "_links": {
                    "self": {
                        "href": "/api/v1/library/music/album?name=Kind&limit=20&offset=0"
                    },
                    "search": {
                        "href": "/api/v1/library/music/album{?name,limit,offset}",
                        "templated": true
                    }
                },
                "items": []
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_album_list_full_page_exposes_a_next_link() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListAlbumsUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(ListAlbumsCommand::new(1, None, Some(1), Some(0))))
            .return_once(|_| {
                let item = album_item();
                Box::pin(async move { Ok(ListAlbumsResponse::new(vec![item], 2)) })
            });

        // Act
        let (status, _, payload) = into_parts(send(use_case, 1, "?limit=1").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            payload.get("_links").and_then(|links| links.get("next")),
            Some(&json!({ "href": "/api/v1/library/music/album?limit=1&offset=1" }))
        );
        Ok(())
    }

    #[rstest]
    #[case::zero("?limit=0")]
    #[case::too_large("?limit=101")]
    #[tokio::test]
    async fn get_album_list_limit_out_of_range_returns_unprocessable_entity(
        #[case] query: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockListAlbumsUseCase::new();

        // Act
        let response = send(use_case, 1, query).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("limit must be between 1 and 100")
        );
        Ok(())
    }

    #[rstest]
    #[case::limit("?limit=abc")]
    #[case::offset("?offset=abc")]
    #[tokio::test]
    async fn get_album_list_invalid_filter_returns_stable_bad_request(
        #[case] query: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockListAlbumsUseCase::new();

        // Act
        let response = send(use_case, 1, query).await?;

        // Assert: the handler-owned message is stable, never the framework's
        // rejection text (`API-040`).
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the request query string is invalid")
        );
        Ok(())
    }

    #[rstest]
    #[case::unknown_caller(
        999,
        ListAlbumsError::NoSuchCaller,
        StatusCode::UNAUTHORIZED,
        "the authenticated user does not exist"
    )]
    #[case::dependency_failure(
        1,
        ListAlbumsError::Unknown(anyhow::anyhow!("boom")),
        StatusCode::INTERNAL_SERVER_ERROR,
        "an unexpected error occurred"
    )]
    #[tokio::test]
    async fn get_album_list_error_maps_to_status(
        #[case] caller_id: NumericID,
        #[case] error: ListAlbumsError,
        #[case] status: StatusCode,
        #[case] message: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListAlbumsUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));

        // Act
        let response = send(use_case, caller_id, "").await?;

        // Assert
        assert_eq!(response.status(), status);
        assert_eq!(error_message_of(&response).as_deref(), Some(message));
        Ok(())
    }

    #[tokio::test]
    async fn get_album_list_missing_credentials_returns_unauthorized() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the real router, wrapped in its `authenticate` layer, with no
        // authorization header.
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("GET")
            .uri("/music/album")
            .body(Body::empty())?;

        // Act
        let response: Response = library_routes(&state).oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
