use axum::extract::{Path, State};
use axum::response::Response;
use tracing::warn;

use crate::application::port::ApplyMovieCommand;
use crate::domain::NumericID;
use crate::domain::model::{ExternalRef, ProviderId};
use crate::infrastructure::inbound::rest::handler::JsonObject;
use crate::infrastructure::inbound::rest::handler::library::{MovieResponse, VideoApplyRequest};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, hal_json};

/// Apply external metadata to a film.
///
/// Only a caller whose `library.manage` scope covers the film may write it. The
/// provider reference is resolved once and its name, plot, release date,
/// country of origin and genres replace the stored values; a value the provider
/// does not report is cleared. The cover and the backing video are untouched.
#[utoipa::path(
    post,
    operation_id = "apply_movie_metadata",
    path = "/movie/{movie_id}/apply",
    tag = "library",
    params(
        ("movie_id" = NumericID, Path, description = "Identifier of the film"),
    ),
    request_body = VideoApplyRequest,
    responses(
        (
            status = OK,
            body = MovieResponse,
            content_type = "application/hal+json",
            description = "Metadata applied"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid film identifier or malformed body"
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
            description = "The caller may not manage the library"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown or invisible film, or the provider does not know the reference"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The applied metadata conflicts with the stored catalogue"
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
            description = "The provider is not a video provider, or the reference resolves to another kind"
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unexpected error, the provider failed, or no video metadata provider is configured"
        ),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "Apply external metadata to a film"
)]
pub(crate) async fn post_movie_apply(
    Path(movie_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: JsonObject<VideoApplyRequest>,
) -> Result<Response, ApiError> {
    let movie_id = movie_id_value.parse::<NumericID>().map_err(|_| {
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "movie_id",
            reason = "invalid_identifier",
            "rejected a malformed film identifier"
        );
        ApiError::BadRequest("invalid film identifier".to_owned())
    })?;
    let JsonObject(request) = payload;
    let provider = ProviderId::parse(&request.reference.provider)
        .filter(|provider| provider.serves_video())
        .ok_or_else(|| ApiError::reject_enum("provider"))?;
    let reference = ExternalRef::new(provider, request.reference.id);
    let response = state
        .library_use_case_factory()
        .apply_movie()
        .execute(ApplyMovieCommand::new(
            caller.user_id(),
            movie_id,
            reference,
        ))
        .await?;
    Ok(hal_json(MovieResponse::from(response.movie())))
}
