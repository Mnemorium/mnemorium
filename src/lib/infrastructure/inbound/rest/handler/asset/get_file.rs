use axum::extract::{Path, State};
use axum::http::{HeaderValue, header};
use axum::response::Response;

use crate::application::port::{GetFileCommand, GetFileError};
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::asset::{FileResponse, parse_asset_id};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{
    ApiError, AppState, CacheControlHeader, ErrorBody, hal_json,
};

/// Map a fetch-file error to its API error.
impl From<GetFileError> for ApiError {
    fn from(err: GetFileError) -> Self {
        match err {
            GetFileError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetFileError::NoSuchFile => Self::NotFound(err.to_string()),
            GetFileError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Fetch one stored file by its identifier.
///
/// Any authenticated caller may read a file it is allowed to read: a catalogue
/// asset follows `ViewMetadata` on the catalogue, a gallery asset its gallery and a playlist
/// cover its playlist. An unknown file and an unreadable file answer the same
/// `404 Not Found`.
#[utoipa::path(
    get,
    operation_id = "get_file",
    path = "/file/{file_id}",
    tag = "asset",
    params(
        ("file_id" = NumericID, Path, description = "Identifier of the file to fetch"),
    ),
    responses(
        (
            status = OK,
            body = FileResponse,
            content_type = "application/hal+json",
            description = "File found",
            headers(("Cache-Control" = inline(CacheControlHeader), description = "Always `private, max-age=60`"))
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid file identifier"
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
            description = "Unknown or unreadable file"
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
    summary = "Fetch a stored file"
)]
pub(crate) async fn get_file(
    Path(file_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let file_id = parse_asset_id(&file_id_value, "file_id", "invalid file identifier")?;
    let response = state
        .asset_use_case_factory()
        .get_file()
        .execute(GetFileCommand::new(caller.user_id(), file_id))
        .await?;
    let mut representation = hal_json(FileResponse::from_response(&response));
    representation.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=60"),
    );
    Ok(representation)
}
