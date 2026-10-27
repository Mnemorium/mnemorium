use axum::extract::Query;
use axum::extract::State;
use axum::extract::rejection::QueryRejection;
use axum::response::Response;
use serde::Deserialize;
use tracing::warn;
use utoipa::IntoParams;

use super::DEFAULT_LIMIT;
use super::MAX_LIMIT;
use crate::application::port::list_playlists::ListPlaylistsCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::PlaylistListLinks;
use crate::infrastructure::inbound::rest::handler::library::PlaylistListResponse;
use crate::infrastructure::inbound::rest::handler::library::PlaylistResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Filters and pagination restricting the returned playlists.
///
/// Every filter is optional; an all-`None` query returns the first page of the
/// playlists the caller may see.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[non_exhaustive]
pub struct ListPlaylistsQuery {
    /// Only return playlists whose public flag matches this value.
    pub is_public: Option<bool>,
    /// Maximum number of playlists to return, between 1 and 100.
    #[param(default = 20, minimum = 1, maximum = 100)]
    pub limit: Option<usize>,
    /// Only return playlists whose name matches this value.
    pub name: Option<String>,
    /// Number of matching playlists to skip before returning the page.
    #[param(default = 0)]
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
    path = "/library/music/playlist",
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
pub async fn get_playlist_list(
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

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::response::Response;
    use axum::routing::get;
    use chrono::NaiveDate;
    use chrono::NaiveDateTime;
    use mockall::predicate::eq;
    use rstest::rstest;
    use serde_json::Value;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::get_playlist_list;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::list_playlists::ListPlaylistsCommand;
    use crate::application::port::list_playlists::ListPlaylistsError;
    use crate::application::port::list_playlists::ListPlaylistsItem;
    use crate::application::port::list_playlists::ListPlaylistsResponse;
    use crate::application::port::list_playlists::ListPlaylistsUseCase;
    use crate::application::port::list_playlists::MockListPlaylistsUseCase;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_parts;

    fn ts() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 6)
            .unwrap_or_default()
            .and_hms_opt(12, 0, 0)
            .unwrap_or_default()
    }

    async fn send(
        use_case: MockListPlaylistsUseCase,
        caller_id: NumericID,
        query: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_list_playlists()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn ListPlaylistsUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/library/music/playlist{query}"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/library/music/playlist", get(get_playlist_list))
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    fn playlist_item() -> ListPlaylistsItem {
        ListPlaylistsItem::new(2, 1, "Mix".to_owned(), true, ts(), ts(), 3)
    }

    #[tokio::test]
    async fn get_playlist_list_no_filter_returns_collection() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListPlaylistsUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(ListPlaylistsCommand::new(
                1,
                None,
                None,
                None,
                Some(20),
                Some(0),
            )))
            .return_once(|_| {
                Box::pin(async { Ok(ListPlaylistsResponse::new(vec![playlist_item()], 1)) })
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
                    "self": { "href": "/api/v1/library/music/playlist?limit=20&offset=0" },
                    "search": {
                        "href": "/api/v1/library/music/playlist{?name,is_public,owner_id,limit,offset}",
                        "templated": true
                    }
                },
                "items": [{
                    "_links": {
                        "self": { "href": "/api/v1/library/music/playlist/2" },
                        "tracks": { "href": "/api/v1/library/music/playlist/2/track" }
                    },
                    "id": 2i64,
                    "name": "Mix",
                    "owner_id": 1i64,
                    "is_public": true,
                    "created_at": "2026-10-06T12:00:00",
                    "last_modified_at": "2026-10-06T12:00:00",
                    "track_count": 3i64,
                    "tracks": []
                }]
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_playlist_list_filters_are_forwarded() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListPlaylistsUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(ListPlaylistsCommand::new(
                1,
                Some("Trip".to_owned()),
                Some(true),
                Some(4),
                Some(20),
                Some(0),
            )))
            .return_once(|_| Box::pin(async { Ok(ListPlaylistsResponse::new(Vec::new(), 0)) }));

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
                        "href": "/api/v1/library/music/playlist?name=Trip&is_public=true&owner_id=4&limit=20&offset=0"
                    },
                    "search": {
                        "href": "/api/v1/library/music/playlist{?name,is_public,owner_id,limit,offset}",
                        "templated": true
                    }
                },
                "items": []
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_playlist_list_full_page_exposes_a_next_link() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListPlaylistsUseCase::new();
        use_case.expect_execute().times(1).return_once(|_| {
            Box::pin(async { Ok(ListPlaylistsResponse::new(vec![playlist_item()], 2)) })
        });

        // Act
        let (status, _, payload) = into_parts(send(use_case, 1, "?limit=1").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            payload.get("_links").and_then(|links| links.get("next")),
            Some(&json!({ "href": "/api/v1/library/music/playlist?limit=1&offset=1" }))
        );
        Ok(())
    }

    #[rstest]
    #[case::zero("?limit=0")]
    #[case::too_large("?limit=101")]
    #[tokio::test]
    async fn get_playlist_list_limit_out_of_range_returns_unprocessable_entity(
        #[case] query: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockListPlaylistsUseCase::new();

        // Act
        let response = send(use_case, 1, query).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[rstest]
    #[case::unknown_caller(999, ListPlaylistsError::NoSuchCaller, StatusCode::UNAUTHORIZED)]
    #[case::dependency_failure(
        1,
        ListPlaylistsError::Unknown(anyhow::anyhow!("boom")),
        StatusCode::INTERNAL_SERVER_ERROR
    )]
    #[tokio::test]
    async fn get_playlist_list_error_maps_to_status(
        #[case] caller_id: NumericID,
        #[case] error: ListPlaylistsError,
        #[case] status: StatusCode,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListPlaylistsUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));

        // Act
        let response = send(use_case, caller_id, "").await?;

        // Assert
        assert_eq!(response.status(), status);
        assert!(error_message_of(&response).is_some());
        Ok(())
    }

    #[tokio::test]
    async fn get_playlist_list_malformed_query_returns_stable_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockListPlaylistsUseCase::new();

        // Act
        let response = send(use_case, 1, "?limit=abc").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the request query string is invalid")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_playlist_list_filtered_page_preserves_filters_in_links()
    -> Result<(), Box<dyn Error>> {
        // Arrange: a filtered page's next link keeps the active filter.
        let mut use_case = MockListPlaylistsUseCase::new();
        use_case.expect_execute().times(1).return_once(|_| {
            Box::pin(async { Ok(ListPlaylistsResponse::new(vec![playlist_item()], 3)) })
        });

        // Act
        let (status, _, payload) =
            into_parts(send(use_case, 1, "?name=Trip&limit=1").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        let links = payload.get("_links").cloned().unwrap_or(Value::Null);
        assert_eq!(
            links.get("next"),
            Some(&json!({ "href": "/api/v1/library/music/playlist?name=Trip&limit=1&offset=1" }))
        );
        Ok(())
    }
}
