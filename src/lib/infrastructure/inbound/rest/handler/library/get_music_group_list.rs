use axum::extract::Query;
use axum::extract::State;
use axum::extract::rejection::QueryRejection;
use axum::response::Response;
use serde::Deserialize;
use tracing::warn;
use utoipa::IntoParams;

use super::DEFAULT_LIMIT;
use super::MAX_LIMIT;
use crate::application::port::list_music_groups::ListMusicGroupsCommand;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::MusicGroupListLinks;
use crate::infrastructure::inbound::rest::handler::library::MusicGroupListResponse;
use crate::infrastructure::inbound::rest::handler::library::MusicGroupResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Filters and pagination restricting the returned music groups.
///
/// Every filter is optional; an all-`None` query returns the first page of the
/// groups the caller may see.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[non_exhaustive]
pub struct ListMusicGroupsQuery {
    /// Maximum number of groups to return, between 1 and 100.
    #[param(default = 20, minimum = 1, maximum = 100)]
    pub limit: Option<usize>,
    /// Only return groups whose name matches this value.
    pub name: Option<String>,
    /// Number of matching groups to skip before returning the page.
    #[param(default = 0)]
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
pub async fn get_music_group_list(
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
        items: response
            .groups()
            .iter()
            .map(MusicGroupResponse::from)
            .collect(),
        links: MusicGroupListLinks::for_page(&query, limit, offset, returned, total),
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

    use super::get_music_group_list;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::list_music_groups::ListMusicGroupsCommand;
    use crate::application::port::list_music_groups::ListMusicGroupsError;
    use crate::application::port::list_music_groups::ListMusicGroupsItem;
    use crate::application::port::list_music_groups::ListMusicGroupsResponse;
    use crate::application::port::list_music_groups::ListMusicGroupsUseCase;
    use crate::application::port::list_music_groups::MockListMusicGroupsUseCase;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_parts;

    /// Send a GET to `/api/v1/library/music/group{query}` on behalf of
    /// `caller_id`, injecting the caller identifier the way the auth middleware
    /// does.
    async fn send(
        use_case: MockListMusicGroupsUseCase,
        caller_id: NumericID,
        query: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_list_music_groups()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn ListMusicGroupsUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/library/music/group{query}"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/library/music/group", get(get_music_group_list))
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    /// The group representation of a covered group.
    fn group_item() -> ListMusicGroupsItem {
        ListMusicGroupsItem::new(7, "The Group".to_owned(), Some(19))
    }

    #[tokio::test]
    async fn get_music_group_list_no_filter_returns_collection() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListMusicGroupsUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(ListMusicGroupsCommand::new(1, None, Some(20), Some(0))))
            .return_once(|_| {
                let item = group_item();
                Box::pin(async move { Ok(ListMusicGroupsResponse::new(vec![item], 1)) })
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
                    "self": { "href": "/api/v1/library/music/group?limit=20&offset=0" },
                    "search": {
                        "href": "/api/v1/library/music/group{?name,limit,offset}",
                        "templated": true
                    }
                },
                "items": [{
                    "_links": {
                        "cover": { "href": "/api/v1/asset/image/19" },
                        "self": { "href": "/api/v1/library/music/group/7" }
                    },
                    "cover_image_id": 19i64,
                    "id": 7i64,
                    "name": "The Group"
                }]
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_music_group_list_filters_are_forwarded() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListMusicGroupsUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(ListMusicGroupsCommand::new(
                1,
                Some("The Group".to_owned()),
                Some(20),
                Some(0),
            )))
            .return_once(|_| Box::pin(async { Ok(ListMusicGroupsResponse::new(Vec::new(), 0)) }));

        // Act
        let (status, _, payload) =
            into_parts(send(use_case, 1, "?name=The%20Group").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            payload.get("_links").and_then(|links| links.get("self")),
            Some(&json!({
                "href": "/api/v1/library/music/group?name=The%20Group&limit=20&offset=0"
            }))
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_music_group_list_full_page_exposes_a_next_link() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListMusicGroupsUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(ListMusicGroupsCommand::new(1, None, Some(1), Some(0))))
            .return_once(|_| {
                let item = group_item();
                Box::pin(async move { Ok(ListMusicGroupsResponse::new(vec![item], 2)) })
            });

        // Act
        let (status, _, payload) = into_parts(send(use_case, 1, "?limit=1").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            payload.get("_links").and_then(|links| links.get("next")),
            Some(&json!({ "href": "/api/v1/library/music/group?limit=1&offset=1" }))
        );
        Ok(())
    }

    #[rstest]
    #[case::zero("?limit=0")]
    #[case::too_large("?limit=101")]
    #[tokio::test]
    async fn get_music_group_list_limit_out_of_range_returns_unprocessable_entity(
        #[case] query: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockListMusicGroupsUseCase::new();

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
    async fn get_music_group_list_malformed_query_returns_stable_bad_request(
        #[case] query: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockListMusicGroupsUseCase::new();

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
        ListMusicGroupsError::NoSuchCaller,
        StatusCode::UNAUTHORIZED,
        "the authenticated user does not exist"
    )]
    #[case::dependency_failure(
        1,
        ListMusicGroupsError::Unknown(anyhow::anyhow!("boom")),
        StatusCode::INTERNAL_SERVER_ERROR,
        "an unexpected error occurred"
    )]
    #[tokio::test]
    async fn get_music_group_list_error_maps_to_status(
        #[case] caller_id: NumericID,
        #[case] error: ListMusicGroupsError,
        #[case] status: StatusCode,
        #[case] message: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListMusicGroupsUseCase::new();
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
    async fn get_music_group_list_missing_credentials_returns_unauthorized()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the real router, wrapped in its `authenticate` layer, with no
        // authorization header.
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("GET")
            .uri("/music/group")
            .body(Body::empty())?;

        // Act
        let response: Response = axum::Router::from(library_routes(&state))
            .oneshot(request)
            .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
