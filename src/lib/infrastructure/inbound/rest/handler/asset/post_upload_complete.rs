use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::http::HeaderValue;
use axum::http::header;
use axum::response::IntoResponse as _;
use axum::response::Response;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::application::port::complete_upload::CompleteUploadCommand;
use crate::application::port::complete_upload::CompleteUploadError;
use crate::application::port::complete_upload::CompleteUploadResponse as CompleteUploadResponseData;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::HAL_CONTENT_TYPE;
use crate::infrastructure::inbound::rest::hal::SelfLinks;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// File finalized by a successful completion.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct PostUploadCompleteResponse {
    /// Unique identifier of the stored file.
    pub file_id: NumericID,
    /// Whether the upload session has been finished.
    pub is_finished: bool,
    /// Link to the completed upload session.
    #[serde(rename = "_links")]
    pub links: SelfLinks,
}

// TODO(file-link): add a `file` link to this response, and to the upload-session
// responses, once a file endpoint exists to address the stored file.

impl PostUploadCompleteResponse {
    /// Map the completed upload onto its HTTP representation.
    #[must_use]
    pub fn new(upload_id: NumericID, response: &CompleteUploadResponseData) -> Self {
        Self {
            file_id: response.file_id(),
            is_finished: response.is_finished(),
            links: SelfLinks::new(&format!("/api/v1/asset/upload/{upload_id}")),
        }
    }
}

/// Map a complete-upload error to its API error.
impl From<CompleteUploadError> for ApiError {
    fn from(err: CompleteUploadError) -> Self {
        match err {
            CompleteUploadError::Incomplete
            | CompleteUploadError::IntegrityMismatch
            | CompleteUploadError::UnsupportedMedia => Self::UnprocessableEntity(err.to_string()),
            CompleteUploadError::Expired => Self::Gone(err.to_string()),
            CompleteUploadError::NoSuchUpload => Self::NotFound(err.to_string()),
            CompleteUploadError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Complete an upload session.
///
/// The upload identifier is carried by the path; there is no request body. The
/// staged content's integrity hash is recomputed and verified against the hash
/// declared when the upload was initialized. When the caller already owns a
/// file with that hash, no second copy is stored: the existing file's
/// identifier is returned and the upload session is left unfinished. Otherwise
/// the staged file is promoted, its record registered, and the upload marked
/// finished. Completion is idempotent: completing an already finished upload
/// returns the caller's file for the same digest.
#[utoipa::path(
    post,
    operation_id = "post_upload_complete",
    path = "/asset/upload/{upload_id}/complete",
    tag = "asset",
    params(
        ("upload_id" = NumericID, Path, description = "Identifier of the upload session"),
    ),
    responses(
        (
            status = OK,
            body = PostUploadCompleteResponse,
            content_type = "application/hal+json",
            description = "Upload completed"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid upload session identifier"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The upload is incomplete, its integrity does not match, or its content is not supported media"
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
            description = "Unknown upload session"
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
            status = GONE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Upload session expired"
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
    summary = "Complete an upload session"
)]
pub async fn post_upload_complete(
    Path(upload_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let Ok(upload_id) = upload_id_value.parse::<NumericID>() else {
        return Err(ApiError::BadRequest(
            "invalid upload session identifier".to_owned(),
        ));
    };
    let response = state
        .asset_use_case_factory()
        .complete_upload()
        .execute(CompleteUploadCommand::new(upload_id, caller.user_id()))
        .await?;
    let body = PostUploadCompleteResponse::new(upload_id, &response);
    let mut http_response = Json(body).into_response();
    http_response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(HAL_CONTENT_TYPE),
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
    use crate::test_helpers::error_message_of;

    /// Send a completion POST for `upload_id` through the endpoint router on
    /// behalf of `caller_id`, injecting the caller the way the auth middleware
    /// does. The endpoint carries no request body.
    async fn send(
        use_case: MockCompleteUploadUseCase,
        caller_id: NumericID,
        upload_id: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockAssetUseCaseFactory::new();
        factory
            .expect_complete_upload()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn CompleteUploadUseCase>);

        let mut request = Request::builder()
            .method("POST")
            .uri(format!("/api/v1/asset/upload/{upload_id}/complete"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/asset/upload/{upload_id}/complete",
                post(post_upload_complete),
            )
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
    fn expect_error(use_case: &mut MockCompleteUploadUseCase, error: CompleteUploadError) {
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));
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
            .return_once(|_| Box::pin(async { Ok(CompleteUploadResponse::new(11, true)) }));

        // Act
        let response = send(use_case, 3, "7").await?;
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/hal+json"),
            "the response must declare the HAL media type"
        );
        let (status, payload) = into_parts(response).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            payload,
            json!({
                "file_id": 11i64,
                "is_finished": true,
                "_links": { "self": { "href": "/api/v1/asset/upload/7" } },
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_upload_complete_incomplete_returns_unprocessable_entity()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCompleteUploadUseCase::new();
        expect_error(&mut use_case, CompleteUploadError::Incomplete);

        // Act
        let response = send(use_case, 3, "7").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the upload session is not complete")
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_upload_complete_integrity_mismatch_returns_unprocessable_entity()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCompleteUploadUseCase::new();
        expect_error(&mut use_case, CompleteUploadError::IntegrityMismatch);

        // Act
        let response = send(use_case, 3, "7").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the integrity hash does not match the declared one")
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_upload_complete_unsupported_media_returns_unprocessable_entity()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCompleteUploadUseCase::new();
        expect_error(&mut use_case, CompleteUploadError::UnsupportedMedia);

        // Act
        let response = send(use_case, 3, "7").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the completed upload is not a supported media file")
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_upload_complete_unknown_upload_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCompleteUploadUseCase::new();
        expect_error(&mut use_case, CompleteUploadError::NoSuchUpload);

        // Act
        let response = send(use_case, 3, "7").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("no upload session matches this identifier")
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_upload_complete_expired_upload_returns_gone() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCompleteUploadUseCase::new();
        expect_error(&mut use_case, CompleteUploadError::Expired);

        // Act
        let response = send(use_case, 3, "7").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::GONE);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the upload session has expired")
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_upload_complete_invalid_identifier_returns_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockCompleteUploadUseCase::new();

        // Act
        let response = send(use_case, 3, "abc").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("invalid upload session identifier")
        );
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
        let response = send(use_case, 3, "7").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("an unexpected error occurred")
        );
        Ok(())
    }
}
