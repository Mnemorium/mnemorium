use axum::Json;
use axum::body::Bytes;
use axum::extract::Path;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::HeaderValue;
use axum::http::header;
use axum::response::IntoResponse as _;
use axum::response::Response;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::application::port::write_upload_chunk::WriteUploadChunkCommand;
use crate::application::port::write_upload_chunk::WriteUploadChunkError;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::HAL_CONTENT_TYPE;
use crate::infrastructure::inbound::rest::handler::asset::upload_session::UploadSessionResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Required length of the `Content-MD5` header, in characters.
const MD5_HEX_LENGTH: usize = 32;

/// Raw bytes of one chunk.
///
/// The handler extracts the body as raw bytes. It is declared as a binary
/// `String` so the `OpenAPI` schema describes the raw `application/octet-stream`
/// payload instead of the JSON array of integers a bare `Vec<u8>` would produce.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(value_type = String, format = Binary)]
#[non_exhaustive]
pub struct PutUploadChunkRequest(
    /// Raw bytes of the chunk.
    pub Vec<u8>,
);

/// Map a write-upload-chunk error to its API error.
impl From<WriteUploadChunkError> for ApiError {
    fn from(err: WriteUploadChunkError) -> Self {
        match err {
            WriteUploadChunkError::InvalidChunk
            | WriteUploadChunkError::InvalidChunkNumber
            | WriteUploadChunkError::InvalidChunkRange
            | WriteUploadChunkError::InvalidMd5 => Self::BadRequest(err.to_string()),
            WriteUploadChunkError::Expired => Self::Gone(err.to_string()),
            WriteUploadChunkError::AlreadyFinished => Self::Conflict(err.to_string()),
            WriteUploadChunkError::NoSuchUpload => Self::NotFound(err.to_string()),
            WriteUploadChunkError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Store one chunk of an upload session.
///
/// The request body carries the raw bytes of the chunk. The `Content-Range`
/// header declares the inclusive byte range as `bytes {start}-{end}/{total}` and
/// the `Content-MD5` header carries the lowercase hexadecimal MD5 digest of the
/// body. The chunk's `start` is validated against the chunk size persisted when
/// the session began, not the current configuration. The chunk is idempotent:
/// re-uploading it with the same digest succeeds without changing the stored
/// content.
#[utoipa::path(
    put,
    operation_id = "put_upload_chunk",
    path = "/asset/upload/{upload_id}/chunk/{chunk_number}",
    tag = "asset",
    request_body(
        content_type = "application/octet-stream",
        content = PutUploadChunkRequest,
    ),
    params(
        ("upload_id" = NumericID, Path, description = "Identifier of the upload session"),
        ("chunk_number" = u64, Path, description = "Zero-based number of the chunk"),
        ("Content-Range" = String, Header, description = "Inclusive byte range, `bytes {start}-{end}/{total}`"),
        ("Content-MD5" = String, Header, description = "Lowercase hexadecimal MD5 digest of the chunk"),
    ),
    responses(
        (
            status = OK,
            body = UploadSessionResponse,
            content_type = "application/hal+json",
            description = "Chunk stored"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid identifier, chunk number, range, digest or body"
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
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Upload session already finished"
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
    summary = "Upload one chunk"
)]
pub async fn put_upload_chunk(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    Path((upload_id_value, chunk_number_value)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    let Ok(upload_id) = upload_id_value.parse::<NumericID>() else {
        return Err(ApiError::BadRequest(
            "invalid upload session identifier".to_owned(),
        ));
    };
    let Ok(chunk_number) = chunk_number_value.parse::<u64>() else {
        return Err(ApiError::BadRequest("invalid chunk number".to_owned()));
    };

    let (start, end) = parse_content_range(&headers)?;
    let content_md5 = parse_content_md5(&headers)?;

    let body_len = u64::try_from(body.len())
        .map_err(|_| ApiError::BadRequest("invalid chunk length".to_owned()))?;
    if end.saturating_sub(start).saturating_add(1) != body_len {
        return Err(ApiError::BadRequest(
            "the declared range does not match the body length".to_owned(),
        ));
    }

    let response = state
        .asset_use_case_factory()
        .write_upload_chunk()
        .execute(WriteUploadChunkCommand::new(
            upload_id,
            chunk_number,
            start,
            body.to_vec(),
            content_md5,
            caller.user_id(),
        ))
        .await?;
    let session = UploadSessionResponse::new(
        upload_id,
        response.bitmap().to_owned(),
        response.total_chunks(),
        response.expires_at(),
        response.is_finished(),
        response.file_id(),
    );
    let mut http_response = Json(session).into_response();
    http_response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(HAL_CONTENT_TYPE),
    );
    Ok(http_response)
}

/// Parse the `Content-Range` header.
///
/// The header must be `bytes {start}-{end}/{total}` with `end >= start`. The
/// `start` offset is checked against the session's persisted chunk size by the
/// use case.
#[expect(
    clippy::single_call_fn,
    reason = "the header parser is named after the format it decodes"
)]
fn parse_content_range(headers: &HeaderMap) -> Result<(u64, u64), ApiError> {
    let raw = headers
        .get(header::CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::BadRequest("missing Content-Range header".to_owned()))?;
    let rest = raw
        .strip_prefix("bytes ")
        .ok_or_else(|| ApiError::BadRequest("malformed Content-Range header".to_owned()))?;
    let (range, total) = rest
        .split_once('/')
        .ok_or_else(|| ApiError::BadRequest("malformed Content-Range header".to_owned()))?;
    let (start_raw, end_raw) = range
        .split_once('-')
        .ok_or_else(|| ApiError::BadRequest("malformed Content-Range header".to_owned()))?;
    let start = start_raw
        .parse::<u64>()
        .map_err(|_| ApiError::BadRequest("malformed Content-Range header".to_owned()))?;
    let end = end_raw
        .parse::<u64>()
        .map_err(|_| ApiError::BadRequest("malformed Content-Range header".to_owned()))?;
    let _total = total
        .parse::<u64>()
        .map_err(|_| ApiError::BadRequest("malformed Content-Range header".to_owned()))?;

    if end < start {
        return Err(ApiError::BadRequest(
            "malformed Content-Range header".to_owned(),
        ));
    }
    Ok((start, end))
}

/// Parse and validate the `Content-MD5` header.
#[expect(
    clippy::single_call_fn,
    reason = "the header parser is named after the format it decodes"
)]
fn parse_content_md5(headers: &HeaderMap) -> Result<String, ApiError> {
    let raw = headers
        .get("content-md5")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::BadRequest("missing Content-MD5 header".to_owned()))?;
    let is_hex =
        raw.len() == MD5_HEX_LENGTH && raw.chars().all(|character| character.is_ascii_hexdigit());
    if !is_hex {
        return Err(ApiError::BadRequest(
            "malformed Content-MD5 header".to_owned(),
        ));
    }
    Ok(raw.to_ascii_lowercase())
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
    use axum::routing::put;
    use chrono::NaiveDate;
    use chrono::NaiveDateTime;
    use md5::Digest as _;
    use md5::Md5;
    use mockall::predicate::eq;
    use serde_json::Value;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::put_upload_chunk;
    use crate::application::port::asset_use_case_factory::MockAssetUseCaseFactory;
    use crate::application::port::write_upload_chunk::MockWriteUploadChunkUseCase;
    use crate::application::port::write_upload_chunk::WriteUploadChunkCommand;
    use crate::application::port::write_upload_chunk::WriteUploadChunkError;
    use crate::application::port::write_upload_chunk::WriteUploadChunkResponse;
    use crate::application::port::write_upload_chunk::WriteUploadChunkUseCase;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_asset;

    /// Chunk the tests upload, and its digest.
    const CHUNK: &[u8] = b"1234";

    /// Valid `Content-Range` for chunk `0` under the default chunk size.
    const RANGE_CHUNK_ZERO: &str = "bytes 0-3/4";

    fn chunk_md5() -> String {
        use std::fmt::Write as _;

        let mut hex = String::new();
        for byte in Md5::digest(CHUNK) {
            let _result = write!(hex, "{byte:02x}");
        }
        hex
    }

    /// Send a chunk PUT through the endpoint router on behalf of `caller_id`,
    /// injecting the caller identifier the way the auth middleware does.
    async fn send(
        use_case: MockWriteUploadChunkUseCase,
        caller_id: NumericID,
        uri: &str,
        content_range: Option<&str>,
        content_md5: Option<&str>,
        body: Body,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockAssetUseCaseFactory::new();
        factory
            .expect_write_upload_chunk()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn WriteUploadChunkUseCase>);

        let mut builder = Request::builder()
            .method("PUT")
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/octet-stream");
        if let Some(value) = content_range {
            builder = builder.header(header::CONTENT_RANGE, value);
        }
        if let Some(value) = content_md5 {
            builder = builder.header("content-md5", value);
        }
        let mut request = builder.body(body)?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/asset/upload/{upload_id}/chunk/{chunk_number}",
                put(put_upload_chunk),
            )
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
    fn expect_error(use_case: &mut MockWriteUploadChunkUseCase, error: WriteUploadChunkError) {
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));
    }

    /// Fixed expiry used by the stubbed success response.
    #[expect(
        clippy::single_call_fn,
        reason = "the fixture is named for readability"
    )]
    fn expiry() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 1, 1)
            .unwrap_or_default()
            .and_hms_opt(12, 0, 0)
            .unwrap_or_default()
    }

    #[tokio::test]
    async fn put_upload_chunk_valid_request_stores_chunk() -> Result<(), Box<dyn Error>> {
        // Arrange
        let expected_command =
            WriteUploadChunkCommand::new(7, 0, 0, CHUNK.to_vec(), chunk_md5(), 3);
        let mut use_case = MockWriteUploadChunkUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(expected_command))
            .return_once(move |_| {
                Box::pin(async move {
                    Ok(WriteUploadChunkResponse::new(
                        "100".to_owned(),
                        expiry(),
                        None,
                        false,
                        3,
                    ))
                })
            });

        // Act
        let response = send(
            use_case,
            3,
            "/api/v1/asset/upload/7/chunk/0",
            Some(RANGE_CHUNK_ZERO),
            Some(&chunk_md5()),
            Body::from(CHUNK),
        )
        .await?;
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
                "bitmap": "100",
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
    async fn put_upload_chunk_missing_content_range_returns_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockWriteUploadChunkUseCase::new();

        // Act
        let (status, payload) = into_parts(
            send(
                use_case,
                3,
                "/api/v1/asset/upload/7/chunk/2",
                None,
                Some(&chunk_md5()),
                Body::from(CHUNK),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(payload, json!({ "error": "missing Content-Range header" }));
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_malformed_content_range_returns_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockWriteUploadChunkUseCase::new();

        // Act
        let (status, payload) = into_parts(
            send(
                use_case,
                3,
                "/api/v1/asset/upload/7/chunk/2",
                Some("8-11/12"),
                Some(&chunk_md5()),
                Body::from(CHUNK),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            payload,
            json!({ "error": "malformed Content-Range header" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_range_not_matching_chunk_returns_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockWriteUploadChunkUseCase::new();
        expect_error(&mut use_case, WriteUploadChunkError::InvalidChunkRange);

        // Act
        let (status, payload) = into_parts(
            send(
                use_case,
                3,
                "/api/v1/asset/upload/7/chunk/2",
                Some("bytes 0-3/12"),
                Some(&chunk_md5()),
                Body::from(CHUNK),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            payload,
            json!({ "error": "the chunk range does not match the chunk number" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_range_length_mismatch_returns_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockWriteUploadChunkUseCase::new();

        // Act
        let (status, payload) = into_parts(
            send(
                use_case,
                3,
                "/api/v1/asset/upload/7/chunk/0",
                Some("bytes 0-9/12"),
                Some(&chunk_md5()),
                Body::from(CHUNK),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            payload,
            json!({ "error": "the declared range does not match the body length" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_missing_content_md5_returns_bad_request() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = MockWriteUploadChunkUseCase::new();

        // Act
        let (status, payload) = into_parts(
            send(
                use_case,
                3,
                "/api/v1/asset/upload/7/chunk/0",
                Some(RANGE_CHUNK_ZERO),
                None,
                Body::from(CHUNK),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(payload, json!({ "error": "missing Content-MD5 header" }));
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_malformed_content_md5_returns_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockWriteUploadChunkUseCase::new();

        // Act
        let (status, payload) = into_parts(
            send(
                use_case,
                3,
                "/api/v1/asset/upload/7/chunk/0",
                Some(RANGE_CHUNK_ZERO),
                Some("oops"),
                Body::from(CHUNK),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(payload, json!({ "error": "malformed Content-MD5 header" }));
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_invalid_upload_id_returns_bad_request() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = MockWriteUploadChunkUseCase::new();

        // Act
        let (status, payload) = into_parts(
            send(
                use_case,
                3,
                "/api/v1/asset/upload/abc/chunk/0",
                Some("bytes 0-3/4"),
                Some(&chunk_md5()),
                Body::from(CHUNK),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            payload,
            json!({ "error": "invalid upload session identifier" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_invalid_chunk_number_returns_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockWriteUploadChunkUseCase::new();

        // Act
        let (status, payload) = into_parts(
            send(
                use_case,
                3,
                "/api/v1/asset/upload/7/chunk/abc",
                Some(RANGE_CHUNK_ZERO),
                Some(&chunk_md5()),
                Body::from(CHUNK),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(payload, json!({ "error": "invalid chunk number" }));
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_unknown_upload_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockWriteUploadChunkUseCase::new();
        expect_error(&mut use_case, WriteUploadChunkError::NoSuchUpload);

        // Act
        let (status, payload) = into_parts(
            send(
                use_case,
                3,
                "/api/v1/asset/upload/7/chunk/0",
                Some(RANGE_CHUNK_ZERO),
                Some(&chunk_md5()),
                Body::from(CHUNK),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            payload,
            json!({ "error": "no upload session matches this identifier" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_expired_upload_returns_gone() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockWriteUploadChunkUseCase::new();
        expect_error(&mut use_case, WriteUploadChunkError::Expired);

        // Act
        let (status, payload) = into_parts(
            send(
                use_case,
                3,
                "/api/v1/asset/upload/7/chunk/0",
                Some(RANGE_CHUNK_ZERO),
                Some(&chunk_md5()),
                Body::from(CHUNK),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::GONE);
        assert_eq!(
            payload,
            json!({ "error": "the upload session has expired" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_finished_upload_returns_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockWriteUploadChunkUseCase::new();
        expect_error(&mut use_case, WriteUploadChunkError::AlreadyFinished);

        // Act
        let (status, payload) = into_parts(
            send(
                use_case,
                3,
                "/api/v1/asset/upload/7/chunk/0",
                Some(RANGE_CHUNK_ZERO),
                Some(&chunk_md5()),
                Body::from(CHUNK),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(
            payload,
            json!({ "error": "the upload session has already been finished" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_invalid_chunk_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockWriteUploadChunkUseCase::new();
        expect_error(&mut use_case, WriteUploadChunkError::InvalidChunk);

        // Act
        let (status, payload) = into_parts(
            send(
                use_case,
                3,
                "/api/v1/asset/upload/7/chunk/0",
                Some(RANGE_CHUNK_ZERO),
                Some(&chunk_md5()),
                Body::from(CHUNK),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(payload, json!({ "error": "the chunk is invalid" }));
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_dependency_failure_returns_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockWriteUploadChunkUseCase::new();
        expect_error(
            &mut use_case,
            WriteUploadChunkError::Unknown(anyhow::anyhow!("boom")),
        );

        // Act
        let (status, payload) = into_parts(
            send(
                use_case,
                3,
                "/api/v1/asset/upload/7/chunk/0",
                Some(RANGE_CHUNK_ZERO),
                Some(&chunk_md5()),
                Body::from(CHUNK),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(payload, json!({ "error": "an unexpected error occurred" }));
        Ok(())
    }
}
