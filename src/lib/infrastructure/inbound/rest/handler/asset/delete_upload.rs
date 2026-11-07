use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse as _, Response};

use crate::application::port::cancel_upload::{CancelUploadCommand, CancelUploadError};
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Map a cancel-upload error to its API error.
impl From<CancelUploadError> for ApiError {
    fn from(err: CancelUploadError) -> Self {
        match err {
            CancelUploadError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            CancelUploadError::NoSuchUpload => Self::NotFound(err.to_string()),
            CancelUploadError::NotCancellable => Self::Conflict(err.to_string()),
            CancelUploadError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Cancel a pending upload session.
///
/// The upload identifier is carried by the path; there is no request body. The
/// session's staged content and its record are deleted and the endpoint answers
/// `204 No Content`. A finished session's content is already the caller's
/// library content, so cancelling it is a `409 Conflict` and deletes nothing; a
/// session being verified is mid-completion and is a conflict too. An unknown
/// or unowned session answers `404 Not Found`, indistinguishable from each
/// other, and a session already cancelled answers `404` as well.
#[utoipa::path(
    delete,
    operation_id = "delete_upload",
    path = "/upload/{upload_id}",
    tag = "asset",
    params(
        ("upload_id" = NumericID, Path, description = "Identifier of the upload session"),
    ),
    responses(
        (
            status = NO_CONTENT,
            description = "Upload session cancelled"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid upload session identifier"
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
            description = "Unknown or unowned upload session"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The upload session is finished or being verified"
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
    summary = "Cancel an upload session"
)]
pub(crate) async fn delete_upload(
    Path(upload_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let Ok(upload_id) = upload_id_value.parse::<NumericID>() else {
        return Err(ApiError::BadRequest(
            "invalid upload session identifier".to_owned(),
        ));
    };
    state
        .asset_use_case_factory()
        .cancel_upload()
        .execute(CancelUploadCommand::new(upload_id, caller.user_id()))
        .await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::port::asset_use_case_factory::MockAssetUseCaseFactory;
    use crate::application::port::cancel_upload::{CancelUploadUseCase, MockCancelUploadUseCase};
    use crate::test_helpers::*;
    use axum::body::Body;
    use axum::extract::Request;
    use axum::routing::delete;
    use mockall::predicate::eq;
    use std::error::Error;
    use std::sync::Arc;
    use tower::ServiceExt as _;

    /// Send a cancellation for `upload_id` through the endpoint router on behalf
    /// of `caller_id`, injecting the caller the way the auth middleware does.
    async fn send(
        use_case: MockCancelUploadUseCase,
        caller_id: NumericID,
        upload_id: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockAssetUseCaseFactory::new();
        factory
            .expect_cancel_upload()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn CancelUploadUseCase>);

        let mut request = Request::builder()
            .method("DELETE")
            .uri(format!("/api/v1/asset/upload/{upload_id}"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/asset/upload/{upload_id}", delete(delete_upload))
            .with_state(app_state_with_asset(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn delete_upload_pending_session_returns_no_content() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCancelUploadUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(CancelUploadCommand::new(7, 3)))
            .return_once(|_| Box::pin(async { Ok(()) }));

        // Act
        let response = send(use_case, 3, "7").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        Ok(())
    }

    #[tokio::test]
    async fn delete_upload_finished_session_returns_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCancelUploadUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(CancelUploadError::NotCancellable) }));

        // Act
        let response = send(use_case, 3, "7").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the upload session cannot be cancelled")
        );
        Ok(())
    }

    #[tokio::test]
    async fn delete_upload_unknown_session_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCancelUploadUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(CancelUploadError::NoSuchUpload) }));

        // Act
        let response = send(use_case, 3, "7").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn delete_upload_invalid_identifier_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockCancelUploadUseCase::new();

        // Act
        let response = send(use_case, 3, "not-a-number").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }
}
