use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::http::HeaderValue;
use axum::http::header;
use axum::response::IntoResponse as _;
use axum::response::Response;

use crate::application::port::get_upload::GetUploadCommand;
use crate::application::port::get_upload::GetUploadError;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::HAL_CONTENT_TYPE;
use crate::infrastructure::inbound::rest::handler::asset::upload_session::UploadSessionResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Map a get-upload error to its API error.
impl From<GetUploadError> for ApiError {
    fn from(err: GetUploadError) -> Self {
        match err {
            GetUploadError::NoSuchUpload => Self::NotFound(err.to_string()),
            GetUploadError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Fetch the state of an upload session.
///
/// Returns the received-chunk bitmap, the total number of chunks, the expiry
/// instant, whether the upload is finished and the identifier of the caller's
/// file for the session digest, when one exists. The file identifier is
/// reported whether or not the session is finished: a caller-scoped duplicate
/// completion leaves the session open and returns this same identifier.
///
/// An expired session is reported as `404`, exactly like an unknown one, so a
/// retried read never changes the result. As a side effect the read lazily
/// deletes the expired session's row and staged file before answering.
#[utoipa::path(
    get,
    operation_id = "get_upload",
    path = "/asset/upload/{upload_id}",
    tag = "asset",
    params(
        ("upload_id" = NumericID, Path, description = "Identifier of the upload session"),
    ),
    responses(
        (
            status = OK,
            body = UploadSessionResponse,
            content_type = "application/hal+json",
            description = "Upload session found"
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
            description = "Unknown or expired upload session"
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
            status = INTERNAL_SERVER_ERROR,
            body = ErrorBody,
            content_type = "application/hal+json",
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
) -> Result<Response, ApiError> {
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
    let body = UploadSessionResponse::new(
        upload_id,
        response.bitmap().to_owned(),
        response.total_chunks(),
        response.expires_at(),
        response.is_finished(),
        response.file_id(),
    );
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
                "bitmap": "101",
                "total_chunks": 3usize,
                "expires_at": "2026-01-01T12:00:00",
                "is_finished": false,
                "file_id": null,
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
                "bitmap": "1",
                "total_chunks": 1usize,
                "expires_at": "2026-01-01T12:00:00",
                "is_finished": true,
                "file_id": 11i64,
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
