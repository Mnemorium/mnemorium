use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::IntoResponse as _;
use axum::response::Response;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::application::port::begin_upload::BeginUploadCommand;
use crate::application::port::begin_upload::BeginUploadError;
use crate::application::port::begin_upload::BeginUploadResponse as BeginUploadResponseData;
use crate::domain::alias::NumericID;
use crate::domain::model::integrity_hash::IntegrityHash;
use crate::domain::model::integrity_hash::SHA256_HEX_LENGTH;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::HAL_CONTENT_TYPE;
use crate::infrastructure::inbound::rest::handler::asset::links::UploadSessionLinks;
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
    /// Integrity hash of the complete file, as a 64-character lowercase
    /// hexadecimal SHA-256 string.
    #[schema(
        min_length = 64,
        max_length = 64,
        pattern = "^[0-9a-f]{64}$",
        example = json!("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
    )]
    pub integrity_hash: String,
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
    /// Links to the created upload session.
    #[serde(rename = "_links")]
    pub links: UploadSessionLinks,
    /// Unique identifier of the upload session.
    pub upload_id: NumericID,
}

/// Map the begin-upload response onto its HTTP representation.
impl From<BeginUploadResponseData> for PostUploadResponse {
    fn from(response: BeginUploadResponseData) -> Self {
        let upload_id = response.upload_id();
        Self {
            chunk_size: response.chunk_size(),
            expires_at: response
                .expires_at()
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string(),
            links: UploadSessionLinks::for_upload(upload_id),
            upload_id,
        }
    }
}

/// Map a begin-upload error to its API error.
impl From<BeginUploadError> for ApiError {
    fn from(err: BeginUploadError) -> Self {
        match err {
            BeginUploadError::InvalidFileName | BeginUploadError::InvalidFileSize => {
                Self::BadRequest(err.to_string())
            }
            BeginUploadError::FileTooLarge => Self::PayloadTooLarge(err.to_string()),
            BeginUploadError::Unknown(_) => Self::InternalServerError,
            BeginUploadError::UnsupportedMediaType => Self::UnsupportedMediaType(err.to_string()),
        }
    }
}

/// Initialize an upload session.
///
/// Validates the declared content type, file name, size and integrity hash,
/// creates the upload session and its staging file, and returns the session
/// identifier, the fixed chunk size and the expiry instant.
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
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "Canonical URI of the created upload session"),
            ),
            description = "Upload session created"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid payload"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
        ),
        (
            status = METHOD_NOT_ALLOWED,
            body = ErrorBody,
            content_type = "application/hal+json",
            headers(
                ("Allow" = String, description = "HTTP methods accepted by this path"),
            ),
            description = "Method not allowed"
        ),
        (
            status = PAYLOAD_TOO_LARGE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The declared file size or the request body exceeds the maximum allowed size"
        ),
        (
            status = UNSUPPORTED_MEDIA_TYPE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The declared content type is not a supported media type"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body does not match the expected schema"
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
    summary = "Initialize an upload session"
)]
pub async fn post_upload(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PostUploadRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(request) = payload.map_err(ApiError::from)?;
    // The handler is where the wire form is parsed into the domain value object:
    // a malformed digest never reaches the use case.
    let integrity_hash = IntegrityHash::<SHA256_HEX_LENGTH>::try_new(request.integrity_hash)
        .map_err(|_| ApiError::BadRequest("the integrity hash is invalid".to_owned()))?;
    let response = state
        .asset_use_case_factory()
        .begin_upload()
        .execute(BeginUploadCommand::new(
            request.file_name,
            request.file_size,
            request.content_type,
            integrity_hash,
            caller.user_id(),
        ))
        .await?;
    let body = PostUploadResponse::from(response);
    let location = body.links.self_link.href.clone();
    let mut http_response = Json(body).into_response();
    *http_response.status_mut() = StatusCode::CREATED;
    let headers = http_response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(HAL_CONTENT_TYPE),
    );
    headers.insert(
        header::LOCATION,
        HeaderValue::from_str(&location).map_err(|_| ApiError::InternalServerError)?,
    );
    Ok(http_response)
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
    use crate::domain::model::integrity_hash::IntegrityHash;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_asset;
    use crate::test_helpers::error_message_of;

    const DIGEST: &str = "d41d8cd98f00b204e9800998ecf8427ed41d8cd98f00b204e9800998ecf8427e";

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
    #[expect(
        clippy::single_call_fn,
        reason = "the success test reads the decoded body through a named helper"
    )]
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
            "integrity_hash": DIGEST,
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
            IntegrityHash::try_new(DIGEST.to_owned())?,
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
        let response = send(use_case, 3, request_body()?).await?;
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/hal+json"),
            "the response must declare the HAL media type"
        );
        assert_eq!(
            response
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some("/api/v1/asset/upload/7"),
            "the response must carry the canonical location"
        );
        let (status, payload) = into_parts(response).await?;

        // Assert
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            payload,
            json!({
                "upload_id": 7i64,
                "chunk_size": 5_242_880u64,
                "expires_at": "2026-01-01T12:00:00",
                "_links": {
                    "self": { "href": "/api/v1/asset/upload/7" },
                    "chunk": {
                        "href": "/api/v1/asset/upload/7/chunk/{chunk_number}",
                        "templated": true,
                    },
                    "complete": { "href": "/api/v1/asset/upload/7/complete" },
                },
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
        let response = send(use_case, 3, request_body()?).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the declared content type is not a supported media type")
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_upload_file_too_large_returns_payload_too_large() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockBeginUploadUseCase::new();
        expect_error(&mut use_case, BeginUploadError::FileTooLarge);

        // Act
        let response = send(use_case, 3, request_body()?).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the file size exceeds the maximum allowed size")
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_upload_invalid_payload_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockBeginUploadUseCase::new();

        // Act
        let response = send(use_case, 3, Body::from("not-json")).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(error_message_of(&response).is_some());
        Ok(())
    }

    #[tokio::test]
    async fn post_upload_invalid_integrity_hash_returns_bad_request() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = MockBeginUploadUseCase::new();
        let body = Body::from(serde_json::to_vec(&json!({
            "file_name": "clip.mp4",
            "file_size": 1_048_576u64,
            "content_type": "video/mp4",
            "integrity_hash": "not-a-valid-digest",
        }))?);

        // Act
        let response = send(use_case, 3, body).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the integrity hash is invalid")
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_upload_non_hexadecimal_integrity_hash_returns_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockBeginUploadUseCase::new();
        let body = Body::from(serde_json::to_vec(&json!({
            "file_name": "clip.mp4",
            "file_size": 1_048_576u64,
            "content_type": "video/mp4",
            "integrity_hash": "g".repeat(64),
        }))?);

        // Act
        let response = send(use_case, 3, body).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the integrity hash is invalid")
        );
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
        let response = send(use_case, 3, request_body()?).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("an unexpected error occurred")
        );
        Ok(())
    }
}
