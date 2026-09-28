use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::application::port::begin_upload::BeginUploadCommand;
use crate::application::port::begin_upload::BeginUploadError;
use crate::application::port::begin_upload::BeginUploadResponse as BeginUploadResponseData;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload initializing an upload session.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct PostUploadRequest {
    /// Declared media type of the file content.
    #[schema(example = json!("video/mp4"))]
    pub content_type: String,
    /// Name of the file being uploaded.
    #[schema(min_length = 1, example = json!("clip.mp4"))]
    pub file_name: String,
    /// Total size of the file being uploaded, in bytes.
    #[schema(minimum = 1, example = json!(1_048_576))]
    pub file_size: u64,
    /// MD5 digest of the complete file, as a 32-character lowercase
    /// hexadecimal string.
    #[schema(min_length = 32, max_length = 32, example = json!("d41d8cd98f00b204e9800998ecf8427e"))]
    pub md5: String,
}

/// Upload session created by a successful initialization.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct PostUploadResponse {
    /// Fixed size of every chunk except the last, in bytes.
    pub chunk_size: u64,
    /// Date and time at which the upload session expires, as
    /// `YYYY-MM-DDTHH:MM:SS`.
    #[schema(example = json!("2026-01-01T12:00:00"))]
    pub expires_at: String,
    /// Unique identifier of the upload session.
    pub upload_id: NumericID,
}

/// Map the begin-upload response onto its HTTP representation.
impl From<BeginUploadResponseData> for PostUploadResponse {
    fn from(response: BeginUploadResponseData) -> Self {
        Self {
            chunk_size: response.chunk_size(),
            expires_at: response
                .expires_at()
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string(),
            upload_id: response.upload_id(),
        }
    }
}

/// Map a begin-upload error to its API error.
impl From<BeginUploadError> for ApiError {
    fn from(err: BeginUploadError) -> Self {
        match err {
            BeginUploadError::InvalidFileName
            | BeginUploadError::InvalidFileSize
            | BeginUploadError::InvalidMd5 => Self::BadRequest(err.to_string()),
            BeginUploadError::FileTooLarge => Self::PayloadTooLarge(err.to_string()),
            BeginUploadError::Unknown(_) => Self::InternalServerError,
            BeginUploadError::UnsupportedMediaType => Self::UnsupportedMediaType(err.to_string()),
        }
    }
}

/// Initialize an upload session.
///
/// Validates the declared content type, file name, size and MD5 digest, creates
/// the upload session and its staging file, and returns the session identifier,
/// the fixed chunk size and the expiry instant.
#[utoipa::path(
    post,
    operation_id = "post_upload",
    path = "/asset/upload",
    tag = "asset",
    request_body(
        content_type = "application/json",
        content = PostUploadRequest,
    ),
    responses(
        (
            status = CREATED,
            body = PostUploadResponse,
            description = "Upload session created"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            description = "Invalid payload"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            description = "Missing or invalid credentials"
        ),
        (
            status = PAYLOAD_TOO_LARGE,
            body = ErrorBody,
            description = "The declared file size exceeds the maximum allowed size"
        ),
        (
            status = UNSUPPORTED_MEDIA_TYPE,
            body = ErrorBody,
            description = "The declared content type is not a supported media type"
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            body = ErrorBody,
            description = "Unexpected error"
        ),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "Initialize an upload session"
)]
pub async fn post_upload(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PostUploadRequest>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Json(request) = payload.map_err(ApiError::from)?;
    let response = state
        .asset_use_case_factory()
        .begin_upload()
        .execute(BeginUploadCommand::new(
            request.file_name,
            request.file_size,
            request.content_type,
            request.md5,
            caller.user_id(),
        ))
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(PostUploadResponse::from(response)),
    ))
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::body::to_bytes;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::response::Response;
    use axum::routing::post;
    use chrono::NaiveDate;
    use mockall::predicate::eq;
    use serde_json::Value;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::post_upload;
    use crate::application::port::asset_use_case_factory::MockAssetUseCaseFactory;
    use crate::application::port::begin_upload::BeginUploadCommand;
    use crate::application::port::begin_upload::BeginUploadError;
    use crate::application::port::begin_upload::BeginUploadResponse;
    use crate::application::port::begin_upload::BeginUploadUseCase;
    use crate::application::port::begin_upload::MockBeginUploadUseCase;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_asset;

    const DIGEST: &str = "d41d8cd98f00b204e9800998ecf8427e";

    /// Send `body` through the endpoint router on behalf of `caller_id`,
    /// injecting the caller identifier the way the auth middleware does.
    async fn send(
        use_case: MockBeginUploadUseCase,
        caller_id: NumericID,
        body: Body,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockAssetUseCaseFactory::new();
        factory
            .expect_begin_upload()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn BeginUploadUseCase>);

        let mut request = Request::builder()
            .method("POST")
            .uri("/api/v1/asset/upload")
            .header(header::CONTENT_TYPE, "application/json")
            .body(body)?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/asset/upload", post(post_upload))
            .with_state(app_state_with_asset(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    /// Split `response` into its status and decoded JSON body.
    async fn into_parts(response: Response) -> Result<(StatusCode, Value), Box<dyn Error>> {
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let body = serde_json::from_slice(&bytes)?;
        Ok((status, body))
    }

    /// Make the mocked use case fail with `error`.
    fn expect_error(use_case: &mut MockBeginUploadUseCase, error: BeginUploadError) {
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));
    }

    fn request_body() -> Result<Body, Box<dyn Error>> {
        Ok(Body::from(serde_json::to_vec(&json!({
            "file_name": "clip.mp4",
            "file_size": 1_048_576,
            "content_type": "video/mp4",
            "md5": DIGEST,
        }))?))
    }

    #[tokio::test]
    async fn post_upload_valid_request_creates_session() -> Result<(), Box<dyn Error>> {
        // Arrange
        let expires_at = NaiveDate::from_ymd_opt(2026, 1, 1)
            .unwrap_or_default()
            .and_hms_opt(12, 0, 0)
            .unwrap_or_default();
        let expected_command = BeginUploadCommand::new(
            "clip.mp4".to_owned(),
            1_048_576,
            "video/mp4".to_owned(),
            DIGEST.to_owned(),
            3,
        );
        let mut use_case = MockBeginUploadUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(expected_command))
            .return_once(move |_| {
                Box::pin(async move { Ok(BeginUploadResponse::new(7, 5_242_880, expires_at)) })
            });

        // Act
        let (status, payload) = into_parts(send(use_case, 3, request_body()?).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            payload,
            json!({
                "upload_id": 7i64,
                "chunk_size": 5_242_880u64,
                "expires_at": "2026-01-01T12:00:00",
            })
        );
        Ok(())
    }
    #[tokio::test]
    async fn post_upload_unsupported_media_type_returns_unsupported_media_type()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockBeginUploadUseCase::new();
        expect_error(&mut use_case, BeginUploadError::UnsupportedMediaType);

        // Act
        let (status, payload) = into_parts(send(use_case, 3, request_body()?).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(
            payload,
            json!({ "error": "the declared content type is not a supported media type" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_upload_file_too_large_returns_payload_too_large() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockBeginUploadUseCase::new();
        expect_error(&mut use_case, BeginUploadError::FileTooLarge);

        // Act
        let (status, payload) = into_parts(send(use_case, 3, request_body()?).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(
            payload,
            json!({ "error": "the file size exceeds the maximum allowed size" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_upload_invalid_payload_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockBeginUploadUseCase::new();

        // Act
        let (status, payload) =
            into_parts(send(use_case, 3, Body::from("not-json")).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(payload.get("error").is_some());
        Ok(())
    }

    #[tokio::test]
    async fn post_upload_invalid_md5_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockBeginUploadUseCase::new();
        expect_error(&mut use_case, BeginUploadError::InvalidMd5);

        // Act
        let (status, payload) = into_parts(send(use_case, 3, request_body()?).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(payload, json!({ "error": "the md5 digest is invalid" }));
        Ok(())
    }

    #[tokio::test]
    async fn post_upload_dependency_failure_returns_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockBeginUploadUseCase::new();
        expect_error(
            &mut use_case,
            BeginUploadError::Unknown(anyhow::anyhow!("boom")),
        );

        // Act
        let (status, payload) = into_parts(send(use_case, 3, request_body()?).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(payload, json!({ "error": "an unexpected error occurred" }));
        Ok(())
    }
}
