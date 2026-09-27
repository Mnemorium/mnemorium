use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::application::port::get_upload::GetUploadCommand;
use crate::application::port::get_upload::GetUploadError;
use crate::application::port::get_upload::GetUploadResponse as GetUploadResponseData;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// State of an upload session returned by a successful lookup.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct GetUploadResponse {
    /// One character per chunk, `1` when received and `0` otherwise.
    #[schema(example = json!("110"))]
    pub bitmap: String,
    /// Date and time at which the upload session expires, as
    /// `YYYY-MM-DDTHH:MM:SS`.
    #[schema(example = json!("2026-01-01T12:00:00"))]
    pub expires_at: String,
    /// Unique identifier of the finished file, when the upload is finished.
    pub file_id: Option<NumericID>,
    /// Whether the upload session has been finished.
    pub is_finished: bool,
    /// Total number of chunks the upload is split into.
    pub total_chunks: usize,
}

/// Map the get-upload response onto its HTTP representation.
impl From<GetUploadResponseData> for GetUploadResponse {
    fn from(response: GetUploadResponseData) -> Self {
        Self {
            bitmap: response.bitmap().to_owned(),
            expires_at: response
                .expires_at()
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string(),
            file_id: response.file_id(),
            is_finished: response.is_finished(),
            total_chunks: response.total_chunks(),
        }
    }
}

/// Map a get-upload error to its API error.
impl From<GetUploadError> for ApiError {
    fn from(err: GetUploadError) -> Self {
        match err {
            GetUploadError::Expired => Self::Gone(err.to_string()),
            GetUploadError::NoSuchUpload => Self::NotFound(err.to_string()),
            GetUploadError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Fetch the state of an upload session.
///
/// Returns the received-chunk bitmap, the total number of chunks, the expiry
/// instant, whether the upload is finished and, when it is, the identifier of
/// the finished file.
#[utoipa::path(
    get,
    operation_id = "get_upload",
    path = "/asset/upload/{upload_id}",
    tag = "asset",
    params(
        ("upload_id" = NumericID, Path, description = "Identifier of the upload session"),
    ),
    responses(
        (status = OK, body = GetUploadResponse, description = "Upload session found"),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            description = "Invalid upload session identifier"
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
    summary = "Fetch the state of an upload session"
)]
pub async fn get_upload(
    Path(upload_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Json<GetUploadResponse>, ApiError> {
    let Ok(upload_id) = upload_id_value.parse::<NumericID>() else {
        return Err(ApiError::BadRequest(
            "invalid upload session identifier".to_owned(),
        ));
    };
    let response = state
        .asset_use_case_factory()
        .get_upload()
        .execute(GetUploadCommand::new(upload_id, caller.user_id()))
        .await?;
    Ok(Json(GetUploadResponse::from(response)))
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::body::to_bytes;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::response::Response;
    use axum::routing::get;
    use chrono::NaiveDate;
    use chrono::NaiveDateTime;
    use mockall::predicate::eq;
    use serde_json::Value;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::get_upload;
    use crate::application::port::asset_use_case_factory::MockAssetUseCaseFactory;
    use crate::application::port::get_upload::GetUploadCommand;
    use crate::application::port::get_upload::GetUploadError;
    use crate::application::port::get_upload::GetUploadResponse as GetUploadResponseData;
    use crate::application::port::get_upload::GetUploadUseCase;
    use crate::application::port::get_upload::MockGetUploadUseCase;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_asset;

    /// Send a GET to `/api/v1/asset/upload/{upload_id}` on behalf of
    /// `caller_id`, injecting the caller the way the auth middleware does.
    async fn send(
        use_case: MockGetUploadUseCase,
        caller_id: NumericID,
        upload_id: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockAssetUseCaseFactory::new();
        factory
            .expect_get_upload()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn GetUploadUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/asset/upload/{upload_id}"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/asset/upload/{upload_id}", get(get_upload))
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
    fn expect_error(use_case: &mut MockGetUploadUseCase, error: GetUploadError) {
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));
    }

    fn expiry() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 1, 1)
            .unwrap_or_default()
            .and_hms_opt(12, 0, 0)
            .unwrap_or_default()
    }

    #[tokio::test]
    async fn get_upload_in_progress_returns_bitmap() -> Result<(), Box<dyn Error>> {
        // Arrange
        let expected_command = GetUploadCommand::new(7, 3);
        let mut use_case = MockGetUploadUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(expected_command))
            .return_once(move |_| {
                Box::pin(async move {
                    Ok(GetUploadResponseData::new(
                        "101".to_owned(),
                        3,
                        expiry(),
                        false,
                        None,
                    ))
                })
            });

        // Act
        let (status, payload) = into_parts(send(use_case, 3, "7").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            payload,
            json!({
                "bitmap": "101",
                "total_chunks": 3usize,
                "expires_at": "2026-01-01T12:00:00",
                "is_finished": false,
                "file_id": null,
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_upload_finished_returns_file_id() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetUploadUseCase::new();
        use_case.expect_execute().times(1).return_once(move |_| {
            Box::pin(async move {
                Ok(GetUploadResponseData::new(
                    "1".to_owned(),
                    1,
                    expiry(),
                    true,
                    Some(11),
                ))
            })
        });

        // Act
        let (status, payload) = into_parts(send(use_case, 3, "7").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            payload,
            json!({
                "bitmap": "1",
                "total_chunks": 1usize,
                "expires_at": "2026-01-01T12:00:00",
                "is_finished": true,
                "file_id": 11i64,
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_upload_unknown_upload_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetUploadUseCase::new();
        expect_error(&mut use_case, GetUploadError::NoSuchUpload);

        // Act
        let (status, payload) = into_parts(send(use_case, 3, "7").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            payload,
            json!({ "error": "no upload session matches this identifier" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_upload_expired_upload_returns_gone() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetUploadUseCase::new();
        expect_error(&mut use_case, GetUploadError::Expired);

        // Act
        let (status, payload) = into_parts(send(use_case, 3, "7").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::GONE);
        assert_eq!(
            payload,
            json!({ "error": "the upload session has expired" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_upload_invalid_id_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockGetUploadUseCase::new();

        // Act
        let (status, payload) = into_parts(send(use_case, 3, "abc").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            payload,
            json!({ "error": "invalid upload session identifier" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_upload_dependency_failure_returns_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetUploadUseCase::new();
        expect_error(
            &mut use_case,
            GetUploadError::Unknown(anyhow::anyhow!("boom")),
        );

        // Act
        let (status, payload) = into_parts(send(use_case, 3, "7").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(payload, json!({ "error": "an unexpected error occurred" }));
        Ok(())
    }
}
