use axum::extract::{Path, State};
use axum::response::Response;
use tracing::warn;

use crate::application::port::GetMovieCommand;
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::library::MovieResponse;
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, hal_json};

/// Fetch one film by its identifier.
///
/// Any authenticated caller may read a film; the film is not owned and carries
/// no visibility flag. The representation carries the film's genres and the
/// video that backs it.
#[utoipa::path(
    get,
    operation_id = "get_movie",
    path = "/movie/{movie_id}",
    tag = "library",
    params(
        ("movie_id" = NumericID, Path, description = "Identifier of the film to fetch"),
    ),
    responses(
        (
            status = OK,
            body = MovieResponse,
            content_type = "application/hal+json",
            description = "Film found"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid film identifier"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown or inaccessible film"
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
    summary = "Fetch a film"
)]
pub(crate) async fn get_movie(
    Path(movie_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let movie_id = movie_id_value.parse::<NumericID>().map_err(|_| {
        // The malformed value and the parser detail never reach the log
        // (`OBS-003`); only the classification and its declared fields do
        // (`OBS-006`).
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "movie_id",
            reason = "invalid_identifier",
            "rejected a malformed film identifier"
        );
        ApiError::BadRequest("invalid film identifier".to_owned())
    })?;
    let response = state
        .library_use_case_factory()
        .get_movie()
        .execute(GetMovieCommand::new(caller.user_id(), movie_id))
        .await?;
    Ok(hal_json(MovieResponse::from(&response)))
}
