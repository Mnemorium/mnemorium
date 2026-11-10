use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse as _, Response};

use crate::application::port::GetVideoCommand;
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::asset::{asset_file_href, parse_asset_id};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody};

/// Redirect the typed video to its backing file.
#[utoipa::path(
    get,
    operation_id = "get_video_file",
    path = "/video/{video_id}/file",
    tag = "asset",
    params(
        ("video_id" = NumericID, Path, description = "Identifier of the video whose file is addressed"),
    ),
    responses(
        (
            status = SEE_OTHER,
            description = "Redirect to the stored file resource",
            headers(("Location" = String, description = "Canonical URI of the stored file"))
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid video identifier"
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
            description = "Unknown or unreadable video"
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unexpected error"
        ),
    ),
    security(("bearer_auth" = [])),
    summary = "Redirect a video to its stored file"
)]
pub(crate) async fn get_video_file(
    Path(video_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let video_id = parse_asset_id(&video_id_value, "video_id", "invalid video identifier")?;
    let response = state
        .asset_use_case_factory()
        .get_video()
        .execute(GetVideoCommand::new(caller.user_id(), video_id))
        .await?;
    Ok((
        StatusCode::SEE_OTHER,
        [(header::LOCATION, asset_file_href(response.file_id()))],
    )
        .into_response())
}
