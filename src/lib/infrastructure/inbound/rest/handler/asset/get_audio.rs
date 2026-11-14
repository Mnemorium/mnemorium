use axum::extract::{Path, State};
use axum::http::{HeaderValue, header};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::application::port::{
    GetAudioCommand, GetAudioError, GetAudioResponse as GetAudioResponseData,
};
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::asset::{
    asset_audio_href, asset_file_href, content_href, parse_asset_id,
};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, Link, hal_json};

/// HAL links exposed by a typed-audio representation.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
struct AssetAudioLinks {
    /// Link to the audio's playable content.
    pub content: Link,
    /// Link to the stored file backing the audio.
    pub file: Link,
    /// Link to the audio resource itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

/// Representation of a typed audio.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
struct AssetAudioResponse {
    /// Bit depth, absent when the content does not report one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bit_depth: Option<i64>,
    /// Block size, absent when the content does not report one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_size: Option<i64>,
    /// Codec of the audio.
    pub codec: String,
    /// Duration, in milliseconds, absent when the content does not report one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<f64>,
    /// Identifier of the backing file.
    pub file_id: NumericID,
    /// Unique identifier of the audio.
    pub id: NumericID,
    /// Links to the audio, its backing file and its content.
    #[serde(rename = "_links")]
    pub links: AssetAudioLinks,
    /// Name of the audio.
    pub name: String,
    /// Sample rate, in hertz, absent when the content does not report one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_rate_hz: Option<f64>,
}

impl AssetAudioResponse {
    /// Map a fetched audio onto its HTTP representation.
    #[must_use]
    fn from_response(response: &GetAudioResponseData) -> Self {
        Self {
            bit_depth: response.bit_depth(),
            block_size: response.block_size(),
            codec: response.codec().to_owned(),
            duration_ms: response.duration_ms(),
            file_id: response.file_id(),
            id: response.id(),
            links: AssetAudioLinks {
                content: content_href("audio", response.id()),
                file: Link::new(&asset_file_href(response.file_id())),
                self_link: Link::new(&asset_audio_href(response.id())),
            },
            name: response.name().to_owned(),
            sample_rate_hz: response.sample_rate_hz(),
        }
    }
}

/// Map a fetch-audio error to its API error.
impl From<GetAudioError> for ApiError {
    fn from(err: GetAudioError) -> Self {
        match err {
            GetAudioError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetAudioError::NoSuchAudio => Self::NotFound(err.to_string()),
            GetAudioError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Fetch one typed audio by its identifier.
#[utoipa::path(
    get,
    operation_id = "get_audio",
    path = "/audio/{audio_id}",
    tag = "asset",
    params(
        ("audio_id" = NumericID, Path, description = "Identifier of the audio to fetch"),
    ),
    responses(
        (
            status = OK,
            body = AssetAudioResponse,
            content_type = "application/hal+json",
            description = "Audio found",
            headers(("Cache-Control" = String, description = "Always `private, max-age=60`"))
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
    summary = "Fetch a typed audio"
)]
pub(crate) async fn get_audio(
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
    let mut representation = hal_json(AssetAudioResponse::from_response(&response));
    representation.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=60"),
    );
    Ok(representation)
}
