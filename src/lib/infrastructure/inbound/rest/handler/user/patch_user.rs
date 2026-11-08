use axum::extract::{Path, State};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::application::port::update_user::{
    UpdateUserCommand, UpdateUserError, UpdateUserResponse as UpdateUserResponseData,
};
use crate::domain::alias::NumericID;
use crate::domain::model::user::Role;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::{SelfLinks, hal_json};
use crate::infrastructure::inbound::rest::handler::JsonObject;
use crate::infrastructure::inbound::rest::handler::user::{parse_user_id, user_self_href};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to update the profile of a user.
///
/// Every attribute is optional; an omitted attribute leaves the stored value
/// unchanged.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PatchUserRequest {
    /// New email address.
    #[schema(format = "email", min_length = 1)]
    pub email: Option<String>,
    /// New role. Only the Root Admin may change the role of a user.
    // The raw wire value is validated by the handler so a value outside the
    // discrete list is classified as `enum_validation_failed` (`OBS-006`)
    // rather than a generic deserialization failure.
    #[schema(value_type = Option<Role>)]
    pub role: Option<String>,
    /// New username; must be 4 to 100 characters long, contain no uppercase letter, and not begin or end with
    /// whitespace.
    #[schema(min_length = 4, max_length = 100)]
    pub username: Option<String>,
}

/// Updated user returned by a successful update.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
struct PatchUserResponse {
    /// Email address of the user, when one was provided.
    #[schema(format = "email")]
    pub email: Option<String>,
    /// Unique identifier of the user.
    pub id: NumericID,
    /// Link to the updated user resource.
    #[serde(rename = "_links")]
    pub links: SelfLinks,
    /// Role of the user.
    pub role: Role,
    /// Username of the user.
    pub username: String,
}

/// Map the updated-user response onto its HTTP representation.
impl From<UpdateUserResponseData> for PatchUserResponse {
    fn from(response: UpdateUserResponseData) -> Self {
        Self {
            email: response.email().map(str::to_owned),
            id: response.id(),
            links: SelfLinks::new(&user_self_href(response.id())),
            role: response.role(),
            username: response.username().to_owned(),
        }
    }
}

/// Map an update-user error to its API error.
impl From<UpdateUserError> for ApiError {
    fn from(err: UpdateUserError) -> Self {
        match err {
            UpdateUserError::InvalidEmail | UpdateUserError::InvalidUsername => {
                Self::UnprocessableEntity(err.to_string())
            }
            UpdateUserError::NotAdmin
            | UpdateUserError::TargetNotModifiable
            | UpdateUserError::RoleChangeForbidden => Self::Forbidden(err.to_string()),
            UpdateUserError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            UpdateUserError::NoSuchUser => Self::NotFound(err.to_string()),
            UpdateUserError::UsernameAlreadyExists
            | UpdateUserError::EmailAlreadyExists
            | UpdateUserError::UserAlreadyExists => Self::Conflict(err.to_string()),
            UpdateUserError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Update the profile of a user by its identifier.
///
/// Requires an `Admin` caller. The root admin (identifier `0`) cannot be
/// updated by anyone. A non-root `Admin` may update its own profile (username
/// and email) but may not modify another admin; it may update standard users
/// of the instance. Only the Root Admin may change the role of a user.
#[utoipa::path(
    patch,
    operation_id = "update_user",
    path = "/{id}",
    tag = "user",
    request_body = PatchUserRequest,
    params(
        ("id" = NumericID, Path, description = "Identifier of the user to update"),
    ),
    responses(
        (status = OK, body = PatchUserResponse, content_type = "application/hal+json", description = "User updated"),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid user identifier or malformed request body"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
        ),
        (
            status = PAYLOAD_TOO_LARGE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body exceeds the maximum allowed size"
        ),
        (
            status = UNSUPPORTED_MEDIA_TYPE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body is not application/json"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body does not match the expected schema, or the username or email fails validation"
        ),
        (
            status = FORBIDDEN,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Caller is not an administrator, the target cannot be modified, or the role change is not allowed"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown user"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Username or email is already taken"
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
    summary = "Update a user profile"
)]
pub(crate) async fn patch_user(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    Path(id): Path<String>,
    payload: JsonObject<PatchUserRequest>,
) -> Result<Response, ApiError> {
    let user_id = parse_user_id(&id)?;
    let JsonObject(request) = payload;
    let role = request
        .role
        .as_deref()
        .map(|value| Role::from_wire(value).ok_or_else(|| ApiError::reject_enum("role")))
        .transpose()?;
    let response = state
        .user_use_case_factory()
        .update_user()
        .execute(UpdateUserCommand::new(
            caller.user_id(),
            user_id,
            request.username,
            request.email,
            role,
        ))
        .await?;
    Ok(hal_json(PatchUserResponse::from(response)))
}
