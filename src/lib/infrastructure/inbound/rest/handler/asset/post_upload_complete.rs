use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::application::port::complete_upload::CompleteUploadCommand;
use crate::application::port::complete_upload::CompleteUploadError;
use crate::application::port::complete_upload::CompleteUploadResponse as CompleteUploadResponseData;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload completing an upload session.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct PostUploadCompleteRequest {
    /// Unique identifier of the upload session to complete.
    pub upload_id: NumericID,
}

/// File finalized by a successful completion.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct PostUploadCompleteResponse {
    /// Unique identifier of the stored file.
    pub file_id: NumericID,
}

// TODO(file-link): add a `file` link to this response, and to `get_upload`, once
// a file endpoint exists to address the stored file.

/// Map the complete-upload response onto its HTTP representation.
impl From<CompleteUploadResponseData> for PostUploadCompleteResponse {
    fn from(response: CompleteUploadResponseData) -> Self {
        Self {
            file_id: response.file_id(),
        }
    }
}

/// Map a complete-upload error to its API error.
impl From<CompleteUploadError> for ApiError {
    fn from(err: CompleteUploadError) -> Self {
        match err {
            CompleteUploadError::Incomplete | CompleteUploadError::IntegrityMismatch => {
                Self::BadRequest(err.to_string())
            }
            CompleteUploadError::Conflict => Self::Conflict(err.to_string()),
            CompleteUploadError::Expired => Self::Gone(err.to_string()),
            CompleteUploadError::NoSuchUpload => Self::NotFound(err.to_string()),
            CompleteUploadError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Complete an upload session and promote its staged file.
///
/// Verifies the recomputed MD5 digest, deduplicates against the caller's
/// existing files and, on success, registers the file and marks the upload as
/// finished. Completion is idempotent: completing an already finished upload
/// returns the caller's file for the same digest.
#[utoipa::path(
    post,
    operation_id = "post_upload_complete",
    path = "/asset/upload/complete",
    tag = "asset",
    request_body(
        content_type = "application/json",
        content = PostUploadCompleteRequest,
    ),
    responses(
        (
            status = OK,
            body = PostUploadCompleteResponse,
            description = "Upload completed"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            description = "Invalid payload, incomplete upload or integrity mismatch"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            description = "Missing or invalid credentials"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            description = "Unknown upload session"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            description = "The digest is already owned by another user"
        ),
        (
            status = GONE,
            body = ErrorBody,
            description = "Upload session expired"
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
    summary = "Complete an upload session"
)]
pub async fn post_upload_complete(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PostUploadCompleteRequest>, JsonRejection>,
) -> Result<Json<PostUploadCompleteResponse>, ApiError> {
    let Json(request) = payload.map_err(ApiError::from)?;
    let response = state
        .asset_use_case_factory()
        .complete_upload()
        .execute(CompleteUploadCommand::new(
            request.upload_id,
            caller.user_id(),
        ))
        .await?;
    Ok(Json(PostUploadCompleteResponse::from(response)))
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
    use mockall::predicate::eq;
    use serde_json::Value;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::post_upload_complete;
    use crate::application::port::asset_use_case_factory::MockAssetUseCaseFactory;
    use crate::application::port::complete_upload::CompleteUploadCommand;
    use crate::application::port::complete_upload::CompleteUploadError;
    use crate::application::port::complete_upload::CompleteUploadResponse;
    use crate::application::port::complete_upload::CompleteUploadUseCase;
    use crate::application::port::complete_upload::MockCompleteUploadUseCase;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_asset;

    /// Send `body` through the endpoint router on behalf of `caller_id`,
    /// injecting the caller the way the auth middleware does.
    async fn send(
        use_case: MockCompleteUploadUseCase,
        caller_id: NumericID,
        body: Body,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockAssetUseCaseFactory::new();
        factory
            .expect_complete_upload()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn CompleteUploadUseCase>);

        let mut request = Request::builder()
            .method("POST")
            .uri("/api/v1/asset/upload/complete")
            .header(header::CONTENT_TYPE, "application/json")
            .body(body)?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/asset/upload/complete", post(post_upload_complete))
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
    fn expect_error(use_case: &mut MockCompleteUploadUseCase, error: CompleteUploadError) {
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));
    }

    fn request_body() -> Result<Body, Box<dyn Error>> {
        Ok(Body::from(serde_json::to_vec(&json!({ "upload_id": 7 }))?))
    }

    #[tokio::test]
    async fn post_upload_complete_valid_request_returns_file_id() -> Result<(), Box<dyn Error>> {
        // Arrange
        let expected_command = CompleteUploadCommand::new(7, 3);
        let mut use_case = MockCompleteUploadUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(expected_command))
            .return_once(|_| Box::pin(async { Ok(CompleteUploadResponse::new(11)) }));

        // Act
        let (status, payload) = into_parts(send(use_case, 3, request_body()?).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload, json!({ "file_id": 11i64 }));
        Ok(())
    }

    #[tokio::test]
    async fn post_upload_complete_incomplete_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCompleteUploadUseCase::new();
        expect_error(&mut use_case, CompleteUploadError::Incomplete);

        // Act
        let (status, payload) = into_parts(send(use_case, 3, request_body()?).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            payload,
            json!({ "error": "the upload session is not complete" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_upload_complete_integrity_mismatch_returns_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCompleteUploadUseCase::new();
        expect_error(&mut use_case, CompleteUploadError::IntegrityMismatch);

        // Act
        let (status, payload) = into_parts(send(use_case, 3, request_body()?).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            payload,
            json!({ "error": "the md5 digest does not match the declared one" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_upload_complete_unknown_upload_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCompleteUploadUseCase::new();
        expect_error(&mut use_case, CompleteUploadError::NoSuchUpload);

        // Act
        let (status, payload) = into_parts(send(use_case, 3, request_body()?).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            payload,
            json!({ "error": "no upload session matches this identifier" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_upload_complete_cross_user_digest_returns_conflict() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut use_case = MockCompleteUploadUseCase::new();
        expect_error(&mut use_case, CompleteUploadError::Conflict);

        // Act
        let (status, payload) = into_parts(send(use_case, 3, request_body()?).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(
            payload,
            json!({ "error": "a file with this md5 digest already exists and is owned by another user" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_upload_complete_expired_upload_returns_gone() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCompleteUploadUseCase::new();
        expect_error(&mut use_case, CompleteUploadError::Expired);

        // Act
        let (status, payload) = into_parts(send(use_case, 3, request_body()?).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::GONE);
        assert_eq!(
            payload,
            json!({ "error": "the upload session has expired" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_upload_complete_invalid_payload_returns_bad_request() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = MockCompleteUploadUseCase::new();

        // Act
        let (status, payload) =
            into_parts(send(use_case, 3, Body::from("not-json")).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(payload.get("error").is_some());
        Ok(())
    }

    #[tokio::test]
    async fn post_upload_complete_dependency_failure_returns_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCompleteUploadUseCase::new();
        expect_error(
            &mut use_case,
            CompleteUploadError::Unknown(anyhow::anyhow!("boom")),
        );

        // Act
        let (status, payload) = into_parts(send(use_case, 3, request_body()?).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(payload, json!({ "error": "an unexpected error occurred" }));
        Ok(())
    }
}
