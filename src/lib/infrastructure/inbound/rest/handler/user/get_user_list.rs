use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use tracing::warn;
use utoipa::{IntoParams, ToSchema};

use crate::application::port::{
    ListUsersCommand, ListUsersError, ListUsersResponse as ListUsersResponseData,
};
use crate::domain::model::Role;
use crate::domain::service::{MAX_USERNAME_LENGTH, MIN_USERNAME_LENGTH};
use crate::infrastructure::inbound::rest::handler::user::{GetUserResponse, user_self_href};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{
    ApiError, AppState, ErrorBody, Link, SelfLinks, hal_json,
};

/// Filters restricting the returned users.
///
/// Every filter is optional; an all-`None` query returns every user.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[non_exhaustive]
pub(crate) struct ListUsersQuery {
    /// Only return users whose email matches this value.
    #[param(format = "email")]
    pub email: Option<String>,
    /// Only return users holding this role.
    // The raw wire value is validated by the handler so a value outside the
    // discrete list is classified as `enum_validation_failed` (`OBS-006`)
    // rather than a generic query rejection.
    #[param(value_type = Option<Role>)]
    pub role: Option<String>,
    /// Only return users whose username matches this value.
    #[param(min_length = 4, max_length = 100)]
    pub username: Option<String>,
}

/// HAL links of a user collection representation.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
struct UserListLinks {
    /// Templated link exposing the collection's search filters.
    pub search: Link,
    /// Link to the collection itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

/// Users matching the requested filters.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
struct GetUserListResponse {
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
    path = "",
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
            description = "Malformed query string"
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
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid filter values"
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
pub(crate) async fn get_user_list(
    raw_query: Result<Query<ListUsersQuery>, QueryRejection>,
    caller: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Response, ApiError> {
    let Query(query) = raw_query.map_err(ApiError::from)?;
    if let Some(username) = query.username.as_deref() {
        let length = username.chars().count();
        if length < MIN_USERNAME_LENGTH {
            warn!(
                target: "security",
                event = "input_validation_failed",
                field = "username",
                reason = "too_short",
                "rejected a username filter"
            );
            return Err(ApiError::UnprocessableEntity(format!(
                "username must be at least {MIN_USERNAME_LENGTH} characters long"
            )));
        }
        if length > MAX_USERNAME_LENGTH {
            warn!(
                target: "security",
                event = "input_validation_failed",
                field = "username",
                reason = "too_long",
                "rejected a username filter"
            );
            return Err(ApiError::UnprocessableEntity(format!(
                "username must be at most {MAX_USERNAME_LENGTH} characters long"
            )));
        }
    }
    // The email-format rule is the domain's (`User::validate_email`); a filter
    // value is held to the same constraint the query declares (`API-017`,
    // `API-045`).
    if let Some(email) = query.email.as_deref()
        && !email_address::EmailAddress::is_valid(email)
    {
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "email",
            reason = "invalid_format",
            "rejected an email filter"
        );
        return Err(ApiError::UnprocessableEntity(
            "email must be a valid email address".to_owned(),
        ));
    }
    let role = query
        .role
        .as_deref()
        .map(|value| Role::from_wire(value).ok_or_else(|| ApiError::reject_enum("role")))
        .transpose()?;
    let users = state
        .user_use_case_factory()
        .list_users()
        .execute(ListUsersCommand::new(
            caller.user_id(),
            query.email,
            role,
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
