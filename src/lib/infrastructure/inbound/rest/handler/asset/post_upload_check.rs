use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::application::port::check_upload::CheckUploadCommand;
use crate::application::port::check_upload::CheckUploadError;
use crate::application::port::check_upload::CheckUploadResponse as CheckUploadResponseData;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload checking for an already stored file.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct PostUploadCheckRequest {
    /// MD5 digest of the complete file, as a 32-character hexadecimal string.
    #[schema(min_length = 32, max_length = 32, example = json!("d41d8cd98f00b204e9800998ecf8427e"))]
    pub md5: String,
}

/// Result of a check for an already stored file.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct PostUploadCheckResponse {
    /// Unique identifier of the matching file, when the caller owns one.
    pub file_id: Option<NumericID>,
}

/// Map the check-upload response onto its HTTP representation.
impl From<CheckUploadResponseData> for PostUploadCheckResponse {
    fn from(response: CheckUploadResponseData) -> Self {
        Self {
            file_id: response.file_id(),
        }
    }
}

/// Map a check-upload error to its API error.
impl From<CheckUploadError> for ApiError {
    fn from(err: CheckUploadError) -> Self {
        match err {
            CheckUploadError::InvalidMd5 => Self::BadRequest(err.to_string()),
            CheckUploadError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Check whether the caller already owns a file with the given MD5 digest.
///
/// Returns the identifier of the matching file, or `null` when the caller owns
/// no file with that digest. The lookup is caller-scoped: a file owned by
/// another user is not reported.
#[utoipa::path(
    post,
    operation_id = "post_upload_check",
    path = "/asset/upload/check",
    tag = "asset",
    request_body(
        content_type = "application/json",
        content = PostUploadCheckRequest,
    ),
    responses(
        (
            status = OK,
            body = PostUploadCheckResponse,
            description = "Check completed"
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
            status = INTERNAL_SERVER_ERROR,
            body = ErrorBody,
            description = "Unexpected error"
        ),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "Check for an existing file"
)]
pub async fn post_upload_check(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PostUploadCheckRequest>, JsonRejection>,
) -> Result<Json<PostUploadCheckResponse>, ApiError> {
    let Json(request) = payload.map_err(ApiError::from)?;
    let response = state
        .asset_use_case_factory()
        .check_upload()
        .execute(CheckUploadCommand::new(request.md5, caller.user_id()))
        .await?;
    Ok(Json(PostUploadCheckResponse::from(response)))
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

    use super::post_upload_check;
    use crate::application::port::asset_use_case_factory::MockAssetUseCaseFactory;
    use crate::application::port::check_upload::CheckUploadCommand;
    use crate::application::port::check_upload::CheckUploadError;
    use crate::application::port::check_upload::CheckUploadResponse;
    use crate::application::port::check_upload::CheckUploadUseCase;
    use crate::application::port::check_upload::MockCheckUploadUseCase;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_asset;

    const DIGEST: &str = "d41d8cd98f00b204e9800998ecf8427e";

    /// Send `body` through the endpoint router on behalf of `caller_id`,
    /// injecting the caller the way the auth middleware does.
    async fn send(
        use_case: MockCheckUploadUseCase,
        caller_id: NumericID,
        body: Body,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockAssetUseCaseFactory::new();
        factory
            .expect_check_upload()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn CheckUploadUseCase>);

        let mut request = Request::builder()
            .method("POST")
            .uri("/api/v1/asset/upload/check")
            .header(header::CONTENT_TYPE, "application/json")
            .body(body)?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/asset/upload/check", post(post_upload_check))
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
    fn expect_error(use_case: &mut MockCheckUploadUseCase, error: CheckUploadError) {
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));
    }

    fn request_body() -> Result<Body, Box<dyn Error>> {
        Ok(Body::from(serde_json::to_vec(&json!({ "md5": DIGEST }))?))
    }

    #[tokio::test]
    async fn post_upload_check_owned_file_returns_file_id() -> Result<(), Box<dyn Error>> {
        // Arrange
        let expected_command = CheckUploadCommand::new(DIGEST.to_owned(), 3);
        let mut use_case = MockCheckUploadUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(expected_command))
            .return_once(|_| Box::pin(async { Ok(CheckUploadResponse::new(Some(11))) }));

        // Act
        let (status, payload) = into_parts(send(use_case, 3, request_body()?).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload, json!({ "file_id": 11i64 }));
        Ok(())
    }

    #[tokio::test]
    async fn post_upload_check_unknown_digest_returns_null_file_id() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCheckUploadUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Ok(CheckUploadResponse::new(None)) }));

        // Act
        let (status, payload) = into_parts(send(use_case, 3, request_body()?).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload, json!({ "file_id": null }));
        Ok(())
    }

    #[tokio::test]
    async fn post_upload_check_invalid_md5_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCheckUploadUseCase::new();
        expect_error(&mut use_case, CheckUploadError::InvalidMd5);

        // Act
        let (status, payload) = into_parts(send(use_case, 3, request_body()?).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(payload, json!({ "error": "the md5 digest is invalid" }));
        Ok(())
    }

    #[tokio::test]
    async fn post_upload_check_invalid_payload_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockCheckUploadUseCase::new();

        // Act
        let (status, payload) =
            into_parts(send(use_case, 3, Body::from("not-json")).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(payload.get("error").is_some());
        Ok(())
    }

    #[tokio::test]
    async fn post_upload_check_dependency_failure_returns_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCheckUploadUseCase::new();
        expect_error(
            &mut use_case,
            CheckUploadError::Unknown(anyhow::anyhow!("boom")),
        );

        // Act
        let (status, payload) = into_parts(send(use_case, 3, request_body()?).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(payload, json!({ "error": "an unexpected error occurred" }));
        Ok(())
    }
}
