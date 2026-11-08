use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::application::port::register_user::{
    RegisterUserCommand, RegisterUserError, RegisterUserResponse,
};
use crate::domain::alias::NumericID;
use crate::domain::model::user::Role;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::{SelfLinks, hal_json};
use crate::infrastructure::inbound::rest::handler::JsonObject;
use crate::infrastructure::inbound::rest::handler::user::user_self_href;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to register a new user.
///
/// The requested `role` is granted only when the caller holds the matching
/// permission: `user.create_admin` for an `Admin` account and `user.create`
/// for a `Standard` one. The default presets grant `user.create` to Admin and
/// Root and `user.create_admin` to the Root Admin only, and a per-user
/// override can change either.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct RegisterRequest {
    /// Email address of the new user, when provided.
    #[schema(format = "email", min_length = 1)]
    pub email: Option<String>,
    /// Password of the new user; never returned by the API.
    ///
    /// Must be at least 8 characters long and contain at least one symbol
    /// (a non-alphanumeric character).
    #[schema(write_only, min_length = 8, pattern = r"[^A-Za-z0-9]")]
    pub password: String,
    /// Role to grant to the new user.
    // The raw wire value is validated by the handler so a value outside the
    // discrete list is classified as `enum_validation_failed` (`OBS-006`)
    // rather than a generic deserialization failure.
    #[schema(value_type = Role)]
    pub role: String,
    /// Username of the new user.
    ///
    /// Must be 4 to 100 characters long, contain no uppercase letter, and not begin or end with whitespace.
    #[schema(min_length = 4, max_length = 100)]
    pub username: String,
}

/// User created by a successful registration.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
struct RegisterResponse {
    /// Email address of the newly registered user, when one was provided.
    #[schema(format = "email")]
    pub email: Option<String>,
    /// Unique identifier of the newly registered user.
    pub id: NumericID,
    /// Link to the newly registered user resource.
    #[serde(rename = "_links")]
    pub links: SelfLinks,
    /// Role granted to the newly registered user.
    pub role: Role,
    /// Username of the newly registered user.
    pub username: String,
}

/// Map the register-user response onto its HTTP representation.
impl From<RegisterUserResponse> for RegisterResponse {
    fn from(response: RegisterUserResponse) -> Self {
        Self {
            email: response.email().map(str::to_owned),
            id: response.id(),
            links: SelfLinks::new(&user_self_href(response.id())),
            role: response.role(),
            username: response.username().to_owned(),
        }
    }
}

/// Map a register-user error to its API error.
impl From<RegisterUserError> for ApiError {
    fn from(err: RegisterUserError) -> Self {
        match err {
            RegisterUserError::Forbidden => Self::Forbidden(err.to_string()),
            RegisterUserError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            RegisterUserError::InvalidEmail
            | RegisterUserError::InvalidPassword
            | RegisterUserError::InvalidUsername => Self::UnprocessableEntity(err.to_string()),
            RegisterUserError::Unknown(_) => Self::InternalServerError,
            RegisterUserError::UsernameAlreadyExists
            | RegisterUserError::EmailAlreadyExists
            | RegisterUserError::UserAlreadyExists => Self::Conflict(err.to_string()),
        }
    }
}

/// Register a new user on behalf of an authenticated caller.
///
/// Creating a `Standard` account requires `user.create`; creating an `Admin`
/// account requires `user.create_admin`. A caller that does not hold the
/// matching permission receives `403` and creates nothing, and a token naming
/// a deleted account receives `401`.
#[utoipa::path(
    post,
    operation_id = "post_register",
    path = "/register",
    tag = "identity",
    request_body = RegisterRequest,
    responses(
        (
            status = CREATED,
            body = RegisterResponse,
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "URI of the newly registered user"),
            ),
            description = "User registered",
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Malformed request body"
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
            description = "The request body does not match the expected schema, or the username, email or password fails validation"
        ),
        (
            status = FORBIDDEN,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Insufficient privileges to register the requested role"
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
    summary = "Register a new user"
)]
pub(crate) async fn post_register(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: JsonObject<RegisterRequest>,
) -> Result<Response, ApiError> {
    let JsonObject(request) = payload;
    let role = Role::from_wire(&request.role).ok_or_else(|| ApiError::reject_enum("role"))?;
    let response = state
        .identity_use_case_factory()
        .register_user()
        .execute(RegisterUserCommand::new(
            caller.user_id(),
            request.username,
            request.email,
            request.password,
            role,
        ))
        .await?;
    let body = RegisterResponse::from(response);
    let location = body.links.self_link.href.clone();
    let mut http_response = hal_json(body);
    *http_response.status_mut() = StatusCode::CREATED;
    http_response.headers_mut().insert(
        header::LOCATION,
        HeaderValue::from_str(&location).map_err(|_| ApiError::InternalServerError)?,
    );
    Ok(http_response)
}
