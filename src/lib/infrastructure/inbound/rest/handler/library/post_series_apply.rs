use axum::extract::{Path, State};
use axum::response::Response;

use crate::application::port::apply_series::ApplySeriesCommand;
use crate::domain::alias::NumericID;
use crate::domain::model::configuration::ProviderId;
use crate::domain::model::external_ref::ExternalRef;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::JsonObject;
use crate::infrastructure::inbound::rest::handler::library::{
    SeriesResponse, VideoApplyRequest, parse_series_id,
};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Apply external metadata to a show.
///
/// Only a caller whose `library.manage` scope covers the show may write it. The
/// provider reference is resolved once and its name, plot, first-air date,
/// country of origin and genres replace the stored values; a value the provider
/// does not report is cleared. The cover, seasons and episodes are untouched,
/// and the apply never creates a season or an episode.
#[utoipa::path(
    post,
    operation_id = "apply_series_metadata",
    path = "/series/{series_id}/apply",
    tag = "library",
    params(
        ("series_id" = NumericID, Path, description = "Identifier of the show"),
    ),
    request_body = VideoApplyRequest,
    responses(
        (
            status = OK,
            body = SeriesResponse,
            content_type = "application/hal+json",
            description = "Metadata applied"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid show identifier or malformed body"
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
            description = "Unknown or invisible show, or the provider does not know the reference"
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
    summary = "Apply external metadata to a show"
)]
pub(crate) async fn post_series_apply(
    Path(series_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: JsonObject<VideoApplyRequest>,
) -> Result<Response, ApiError> {
    let series_id = parse_series_id(&series_id_value)?;
    let JsonObject(request) = payload;
    let provider = ProviderId::parse(&request.reference.provider)
        .filter(|provider| provider.serves_video())
        .ok_or_else(|| ApiError::reject_enum("provider"))?;
    let reference = ExternalRef::new(provider, request.reference.id);
    let response = state
        .library_use_case_factory()
        .apply_series()
        .execute(ApplySeriesCommand::new(
            caller.user_id(),
            series_id,
            reference,
        ))
        .await?;
    Ok(hal_json(SeriesResponse::from(response.series())))
}
