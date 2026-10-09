use axum::extract::Query;
use axum::extract::State;
use axum::response::Response;
use serde::{Deserialize, Serialize};
use utoipa::IntoParams;
use utoipa::ToSchema;

use crate::application::port::list_users::ListUsersCommand;
use crate::application::port::list_users::ListUsersError;
use crate::application::port::list_users::ListUsersResponse as ListUsersResponseData;
use crate::domain::model::user::Role;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::Link;
use crate::infrastructure::inbound::rest::hal::SelfLinks;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::user::GetUserResponse;
use crate::infrastructure::inbound::rest::handler::user::user_self_href;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Filters restricting the returned users.
///
/// Every filter is optional; an all-`None` query returns every user.
#[derive(Debug, Deserialize, Serialize, IntoParams)]
#[non_exhaustive]
pub struct ListUsersQuery {
    /// Only return users whose email matches this value.
    #[param(format = "email")]
    pub email: Option<String>,
    /// Only return users holding this role.
    pub role: Option<Role>,
    /// Only return users whose username matches this value.
    #[param(min_length = 4, max_length = 100)]
    pub username: Option<String>,
}

/// HAL links of a user collection representation.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct UserListLinks {
    /// Templated link exposing the collection's search filters.
    pub search: Link,
    /// Link to the collection itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

/// Users matching the requested filters.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct GetUserListResponse {
    /// The users matching the filters.
    pub items: Vec<GetUserResponse>,
    /// Navigation links of the collection.
    #[serde(rename = "_links")]
    pub links: UserListLinks,
}

/// Map the listed-user response onto its HTTP representation.
///
/// Items reuse the single-resource [`GetUserResponse`] schema, so the
/// representation behind an item's `_links.self` is exactly the one the
/// collection advertises.
impl From<ListUsersResponseData> for GetUserResponse {
    fn from(response: ListUsersResponseData) -> Self {
        Self {
            email: response.email().map(str::to_owned),
            id: response.id(),
            links: SelfLinks::new(&user_self_href(response.id())),
            role: response.role(),
            username: response.username().to_owned(),
        }
    }
}

/// Map a list-users error to its API error.
impl From<ListUsersError> for ApiError {
    fn from(err: ListUsersError) -> Self {
        match err {
            ListUsersError::NotAdmin => Self::Forbidden(err.to_string()),
            ListUsersError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            ListUsersError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// List the users of the instance.
///
/// Requires an `Admin` caller. Returns the profiles of every user matching
/// the optional filters as a HAL collection. An empty list is a valid outcome.
#[utoipa::path(
    get,
    operation_id = "list_users",
    path = "/user",
    tag = "user",
    params(ListUsersQuery),
    responses(
        (
            status = OK,
            body = GetUserListResponse,
            content_type = "application/hal+json",
            description = "Users matching the filters"
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
            status = FORBIDDEN,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Caller is not an administrator"
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
    summary = "List users"
)]
pub async fn get_user_list(
    Query(query): Query<ListUsersQuery>,
    caller: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Response, ApiError> {
    let users = state
        .user_use_case_factory()
        .list_users()
        .execute(ListUsersCommand::new(
            caller.user_id(),
            query.email,
            query.role,
            query.username,
        ))
        .await?;

    let body = GetUserListResponse {
        links: UserListLinks {
            search: Link::templated("/api/v1/user{?email,role,username}"),
            self_link: Link::new("/api/v1/user"),
        },
        items: users.into_iter().map(GetUserResponse::from).collect(),
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
    use mockall::predicate::eq;
    use serde_json::Value;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::get_user_list;
    use crate::application::port::list_users::ListUsersCommand;
    use crate::application::port::list_users::ListUsersError;
    use crate::application::port::list_users::ListUsersResponse;
    use crate::application::port::list_users::ListUsersUseCase;
    use crate::application::port::list_users::MockListUsersUseCase;
    use crate::application::port::user_use_case_factory::MockUserUseCaseFactory;
    use crate::domain::alias::NumericID;
    use crate::domain::model::user::Role;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_user;
    use crate::test_helpers::error_message_of;

    /// Send a GET to `/api/v1/user{query}` on behalf of `caller_id`, injecting
    /// the caller identifier the way the auth middleware does.
    async fn send(
        use_case: MockListUsersUseCase,
        caller_id: NumericID,
        query: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockUserUseCaseFactory::new();
        factory
            .expect_list_users()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn ListUsersUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/user{query}"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/user", get(get_user_list))
            .with_state(app_state_with_user(Arc::new(factory))?);
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
    fn expect_error(use_case: &mut MockListUsersUseCase, error: ListUsersError) {
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));
    }

    #[tokio::test]
    async fn get_user_list_no_filter_returns_users() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListUsersUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(ListUsersCommand::new(1, None, None, None)))
            .return_once(|_| {
                Box::pin(async {
                    Ok(vec![
                        ListUsersResponse::new(
                            2,
                            "alice".to_owned(),
                            Some("alice@example.com".to_owned()),
                            Role::Standard,
                        ),
                        ListUsersResponse::new(3, "brad".to_owned(), None, Role::Admin),
                    ])
                })
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
                    "self": { "href": "/api/v1/user" },
                    "search": {
                        "href": "/api/v1/user{?email,role,username}",
                        "templated": true
                    }
                },
                "items": [
                    {
                        "_links": { "self": { "href": "/api/v1/user/2" } },
                        "id": 2i64,
                        "username": "alice",
                        "email": "alice@example.com",
                        "role": "STANDARD"
                    },
                    {
                        "_links": { "self": { "href": "/api/v1/user/3" } },
                        "id": 3i64,
                        "username": "brad",
                        "email": null,
                        "role": "ADMIN"
                    }
                ]
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_user_list_email_filter_is_forwarded() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListUsersUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(ListUsersCommand::new(
                1,
                Some("alice@example.com".to_owned()),
                None,
                None,
            )))
            .return_once(|_| Box::pin(async { Ok(Vec::new()) }));

        // Act
        let (status, _, payload) =
            into_parts(send(use_case, 1, "?email=alice@example.com").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            payload,
            json!({
                "_links": {
                    "self": { "href": "/api/v1/user" },
                    "search": {
                        "href": "/api/v1/user{?email,role,username}",
                        "templated": true
                    }
                },
                "items": []
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_user_list_role_filter_is_forwarded() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListUsersUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(ListUsersCommand::new(1, None, Some(Role::Admin), None)))
            .return_once(|_| Box::pin(async { Ok(Vec::new()) }));

        // Act
        let (status, _, _) = into_parts(send(use_case, 1, "?role=ADMIN").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn get_user_list_username_filter_is_forwarded() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListUsersUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(ListUsersCommand::new(
                1,
                None,
                None,
                Some("alice".to_owned()),
            )))
            .return_once(|_| Box::pin(async { Ok(Vec::new()) }));

        // Act
        let (status, _, _) = into_parts(send(use_case, 1, "?username=alice").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn get_user_list_no_match_returns_empty_items() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListUsersUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Ok(Vec::new()) }));

        // Act
        let (status, _, payload) = into_parts(send(use_case, 1, "").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            payload,
            json!({
                "_links": {
                    "self": { "href": "/api/v1/user" },
                    "search": {
                        "href": "/api/v1/user{?email,role,username}",
                        "templated": true
                    }
                },
                "items": []
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_user_list_non_admin_returns_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListUsersUseCase::new();
        expect_error(&mut use_case, ListUsersError::NotAdmin);

        // Act
        let response = send(use_case, 3, "").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("only administrators may list users")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_user_list_unknown_caller_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListUsersUseCase::new();
        expect_error(&mut use_case, ListUsersError::NoSuchCaller);

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
    async fn get_user_list_dependency_failure_returns_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListUsersUseCase::new();
        expect_error(
            &mut use_case,
            ListUsersError::Unknown(anyhow::anyhow!("boom")),
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
    async fn get_user_list_invalid_role_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockListUsersUseCase::new();

        // Act
        let response = send(use_case, 1, "?role=WIZARD").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }
}
