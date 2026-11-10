use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse as _, Response};

use crate::application::port::GetAudioCommand;
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::asset::{asset_file_href, parse_asset_id};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody};

/// Redirect the typed audio to its backing file.
#[utoipa::path(
    get,
    operation_id = "get_audio_file",
    path = "/audio/{audio_id}/file",
    tag = "asset",
    params(
        ("audio_id" = NumericID, Path, description = "Identifier of the audio whose file is addressed"),
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
            description = "Invalid audio identifier"
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
            description = "Unknown or unreadable audio"
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unexpected error"
        ),
    ),
    security(("bearer_auth" = [])),
    summary = "Redirect an audio to its stored file"
)]
pub(crate) async fn get_audio_file(
    Path(audio_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let audio_id = parse_asset_id(&audio_id_value, "audio_id", "invalid audio identifier")?;
    let response = state
        .asset_use_case_factory()
        .get_audio()
        .execute(GetAudioCommand::new(caller.user_id(), audio_id))
        .await?;
    Ok((
        StatusCode::SEE_OTHER,
        [(header::LOCATION, asset_file_href(response.file_id()))],
    )
        .into_response())
}
