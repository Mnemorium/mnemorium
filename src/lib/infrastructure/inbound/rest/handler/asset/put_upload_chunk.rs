use axum::Json;
use axum::body::Bytes;
use axum::extract::Path;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::HeaderValue;
use axum::http::header;
use axum::response::IntoResponse as _;
use axum::response::Response;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::application::port::write_upload_chunk::WriteUploadChunkCommand;
use crate::application::port::write_upload_chunk::WriteUploadChunkError;
use crate::domain::alias::NumericID;
use crate::domain::model::integrity_hash::IntegrityHash;
use crate::domain::model::integrity_hash::SHA256_HEX_LENGTH;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::HAL_CONTENT_TYPE;
use crate::infrastructure::inbound::rest::handler::asset::upload_session::UploadSessionResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Number of raw bytes a SHA-256 digest carries.
const SHA256_BYTE_LENGTH: usize = 32;

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
            | WriteUploadChunkError::InvalidContentDigest => {
                Self::UnprocessableEntity(err.to_string())
            }
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
/// the `Content-Digest` header carries the SHA-256 digest of the body as an RFC
/// 9530 Byte Sequence, `sha-256=:<base64>:`. The chunk's `start` is validated
/// against the chunk size persisted when the session began, not the current
/// configuration. The chunk is idempotent: re-uploading it with the same digest
/// succeeds without changing the stored content.
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
        (
            "Content-Digest" = String,
            Header,
            description = "RFC 9530 content digest of the chunk, `sha-256=:<base64 of the raw 32-byte digest>:`",
            example = "sha-256=:A6xnQhbz4Vx2HuGl4lXwZ5U2I8iziLRFnhP5eNfIRvQ=:"
        ),
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
            description = "Malformed identifier, chunk number, Content-Range or Content-Digest header"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The chunk number, range or digest fails validation"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
        ),
        (
            status = PAYLOAD_TOO_LARGE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body exceeds the maximum chunk size"
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
    let content_digest = parse_content_digest(&headers)?;

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
            content_digest,
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

/// Parse and validate the RFC 9530 `Content-Digest` header.
///
/// The header must carry a `sha-256` member whose value is a Byte Sequence, the
/// base64-encoded raw 32-byte digest wrapped in colons (`sha-256=:<base64>:`).
/// The decoded bytes are re-encoded as lowercase hexadecimal.
#[expect(
    clippy::single_call_fn,
    reason = "the header parser is named after the format it decodes"
)]
fn parse_content_digest(headers: &HeaderMap) -> Result<IntegrityHash<SHA256_HEX_LENGTH>, ApiError> {
    let raw = headers
        .get("content-digest")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::BadRequest("missing Content-Digest header".to_owned()))?;

    let mut parsed_any = false;
    for parameter in raw.split(',').map(str::trim) {
        let Some((algorithm, value)) = parameter.split_once('=') else {
            continue;
        };
        parsed_any = true;
        if algorithm.trim() != "sha-256" {
            continue;
        }
        let encoded = value
            .trim()
            .strip_prefix(':')
            .and_then(|rest| rest.strip_suffix(':'))
            .ok_or_else(|| ApiError::BadRequest("malformed Content-Digest header".to_owned()))?;
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|_| ApiError::BadRequest("malformed Content-Digest header".to_owned()))?;
        if bytes.len() != SHA256_BYTE_LENGTH {
            return Err(ApiError::BadRequest(
                "malformed Content-Digest header".to_owned(),
            ));
        }
        return IntegrityHash::try_new(hex_encode(&bytes))
            .map_err(|_| ApiError::BadRequest("malformed Content-Digest header".to_owned()));
    }

    let error = if parsed_any {
        "unsupported Content-Digest algorithm"
    } else {
        "malformed Content-Digest header"
    };
    Err(ApiError::BadRequest(error.to_owned()))
}

/// Hexadecimal-encode `bytes`, lowercase.
#[expect(
    clippy::single_call_fn,
    reason = "the digest encoding is named for readability"
)]
fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut hex = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        let _result = write!(hex, "{byte:02x}");
    }
    hex
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
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD;
    use chrono::NaiveDate;
    use chrono::NaiveDateTime;
    use mockall::predicate::eq;
    use serde_json::Value;
    use serde_json::json;
    use sha2::Digest as _;
    use sha2::Sha256;
    use tower::ServiceExt as _;

    use super::put_upload_chunk;
    use crate::application::port::asset_use_case_factory::MockAssetUseCaseFactory;
    use crate::application::port::write_upload_chunk::MockWriteUploadChunkUseCase;
    use crate::application::port::write_upload_chunk::WriteUploadChunkCommand;
    use crate::application::port::write_upload_chunk::WriteUploadChunkError;
    use crate::application::port::write_upload_chunk::WriteUploadChunkResponse;
    use crate::application::port::write_upload_chunk::WriteUploadChunkUseCase;
    use crate::domain::alias::NumericID;
    use crate::domain::model::integrity_hash::IntegrityHash;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_asset;
    use crate::test_helpers::error_message_of;

    /// Chunk the tests upload, and its digest.
    const CHUNK: &[u8] = b"1234";

    /// Valid `Content-Range` for chunk `0` under the default chunk size.
    const RANGE_CHUNK_ZERO: &str = "bytes 0-3/4";

    /// Build the `Content-Digest` header value for SHA-256 of `CHUNK`.
    fn chunk_content_digest() -> String {
        let digest = Sha256::digest(CHUNK);
        format!("sha-256=:{}:", STANDARD.encode(digest))
    }

    /// Build the domain digest of `CHUNK`.
    #[expect(
        clippy::expect_used,
        clippy::single_call_fn,
        reason = "the helper drives a valid SHA-256 digest through the value object and is named for readability"
    )]
    fn chunk_integrity_hash() -> IntegrityHash<64> {
        let digest = Sha256::digest(CHUNK);
        let hex = digest.iter().fold(String::new(), |mut accumulator, byte| {
            use std::fmt::Write as _;
            let _result = write!(accumulator, "{byte:02x}");
            accumulator
        });
        IntegrityHash::try_new(hex).expect("a SHA-256 digest is a valid integrity hash")
    }

    /// Send a chunk PUT through the endpoint router on behalf of `caller_id`,
    /// injecting the caller identifier the way the auth middleware does.
    async fn send(
        use_case: MockWriteUploadChunkUseCase,
        caller_id: NumericID,
        uri: &str,
        content_range: Option<&str>,
        content_digest: Option<&str>,
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
        if let Some(value) = content_digest {
            builder = builder.header("content-digest", value);
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
            WriteUploadChunkCommand::new(7, 0, 0, CHUNK.to_vec(), chunk_integrity_hash(), 3);
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
            Some(&chunk_content_digest()),
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
        let response = send(
            use_case,
            3,
            "/api/v1/asset/upload/7/chunk/2",
            None,
            Some(&chunk_content_digest()),
            Body::from(CHUNK),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("missing Content-Range header")
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_malformed_content_range_returns_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockWriteUploadChunkUseCase::new();

        // Act
        let response = send(
            use_case,
            3,
            "/api/v1/asset/upload/7/chunk/2",
            Some("8-11/12"),
            Some(&chunk_content_digest()),
            Body::from(CHUNK),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("malformed Content-Range header")
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_range_not_matching_chunk_returns_unprocessable_entity()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockWriteUploadChunkUseCase::new();
        expect_error(&mut use_case, WriteUploadChunkError::InvalidChunkRange);

        // Act
        let response = send(
            use_case,
            3,
            "/api/v1/asset/upload/7/chunk/2",
            Some("bytes 0-3/12"),
            Some(&chunk_content_digest()),
            Body::from(CHUNK),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the chunk range does not match the chunk number")
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_range_length_mismatch_returns_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockWriteUploadChunkUseCase::new();

        // Act
        let response = send(
            use_case,
            3,
            "/api/v1/asset/upload/7/chunk/0",
            Some("bytes 0-9/12"),
            Some(&chunk_content_digest()),
            Body::from(CHUNK),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the declared range does not match the body length")
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_missing_content_digest_returns_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockWriteUploadChunkUseCase::new();

        // Act
        let response = send(
            use_case,
            3,
            "/api/v1/asset/upload/7/chunk/0",
            Some(RANGE_CHUNK_ZERO),
            None,
            Body::from(CHUNK),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("missing Content-Digest header")
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_malformed_content_digest_returns_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockWriteUploadChunkUseCase::new();

        // Act
        let response = send(
            use_case,
            3,
            "/api/v1/asset/upload/7/chunk/0",
            Some(RANGE_CHUNK_ZERO),
            Some("oops"),
            Body::from(CHUNK),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("malformed Content-Digest header")
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_unsupported_content_digest_algorithm_returns_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockWriteUploadChunkUseCase::new();

        // Act
        let response = send(
            use_case,
            3,
            "/api/v1/asset/upload/7/chunk/0",
            Some(RANGE_CHUNK_ZERO),
            Some("sha-512=:AAAA:"),
            Body::from(CHUNK),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("unsupported Content-Digest algorithm")
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_invalid_upload_id_returns_bad_request() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = MockWriteUploadChunkUseCase::new();

        // Act
        let response = send(
            use_case,
            3,
            "/api/v1/asset/upload/abc/chunk/0",
            Some("bytes 0-3/4"),
            Some(&chunk_content_digest()),
            Body::from(CHUNK),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("invalid upload session identifier")
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_invalid_chunk_number_returns_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockWriteUploadChunkUseCase::new();

        // Act
        let response = send(
            use_case,
            3,
            "/api/v1/asset/upload/7/chunk/abc",
            Some(RANGE_CHUNK_ZERO),
            Some(&chunk_content_digest()),
            Body::from(CHUNK),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("invalid chunk number")
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_unknown_upload_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockWriteUploadChunkUseCase::new();
        expect_error(&mut use_case, WriteUploadChunkError::NoSuchUpload);

        // Act
        let response = send(
            use_case,
            3,
            "/api/v1/asset/upload/7/chunk/0",
            Some(RANGE_CHUNK_ZERO),
            Some(&chunk_content_digest()),
            Body::from(CHUNK),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("no upload session matches this identifier")
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_expired_upload_returns_gone() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockWriteUploadChunkUseCase::new();
        expect_error(&mut use_case, WriteUploadChunkError::Expired);

        // Act
        let response = send(
            use_case,
            3,
            "/api/v1/asset/upload/7/chunk/0",
            Some(RANGE_CHUNK_ZERO),
            Some(&chunk_content_digest()),
            Body::from(CHUNK),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::GONE);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the upload session has expired")
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_finished_upload_returns_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockWriteUploadChunkUseCase::new();
        expect_error(&mut use_case, WriteUploadChunkError::AlreadyFinished);

        // Act
        let response = send(
            use_case,
            3,
            "/api/v1/asset/upload/7/chunk/0",
            Some(RANGE_CHUNK_ZERO),
            Some(&chunk_content_digest()),
            Body::from(CHUNK),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the upload session has already been finished")
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_invalid_chunk_returns_unprocessable_entity()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockWriteUploadChunkUseCase::new();
        expect_error(&mut use_case, WriteUploadChunkError::InvalidChunk);

        // Act
        let response = send(
            use_case,
            3,
            "/api/v1/asset/upload/7/chunk/0",
            Some(RANGE_CHUNK_ZERO),
            Some(&chunk_content_digest()),
            Body::from(CHUNK),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the chunk is invalid")
        );
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
        let response = send(
            use_case,
            3,
            "/api/v1/asset/upload/7/chunk/0",
            Some(RANGE_CHUNK_ZERO),
            Some(&chunk_content_digest()),
            Body::from(CHUNK),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("an unexpected error occurred")
        );
        Ok(())
    }
}
