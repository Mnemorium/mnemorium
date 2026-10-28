use axum::Json;
use axum::body::Bytes;
use axum::extract::Path;
use axum::extract::State;
use axum::extract::rejection::BytesRejection;
use axum::http::HeaderMap;
use axum::http::HeaderValue;
use axum::http::header;
use axum::response::IntoResponse as _;
use axum::response::Response;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::port::write_upload_chunk::WriteUploadChunkCommand;
use crate::application::port::write_upload_chunk::WriteUploadChunkError;
use crate::domain::alias::NumericID;
use crate::domain::model::integrity_hash::DIGEST_HEX_LENGTH;
use crate::domain::model::integrity_hash::IntegrityHash;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::HAL_CONTENT_TYPE;
use crate::infrastructure::inbound::rest::handler::asset::UploadSessionResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Number of raw bytes a content digest carries.
#[expect(
    clippy::integer_division,
    reason = "two hexadecimal characters encode one byte"
)]
const DIGEST_BYTES: usize = DIGEST_HEX_LENGTH / 2;

/// Raw bytes of one chunk.
///
/// The handler extracts the body as raw bytes. It is declared as a binary
/// `String` so the `OpenAPI` schema describes the raw `application/octet-stream`
/// payload instead of the JSON array of integers a bare `Vec<u8>` would produce.
#[derive(Debug, Deserialize, ToSchema)]
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
            WriteUploadChunkError::NoSuchCaller => Self::Unauthorized(err.to_string()),
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
            description = "Malformed identifier, chunk number or header syntax"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "A required header is missing, or the chunk number, range or digest fails validation"
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
    raw_body: Result<Bytes, BytesRejection>,
) -> Result<Response, ApiError> {
    // Resolve the extractor result first: an oversized body must produce the
    // `413` and its catalogued `limit_exceeded` event regardless of the path
    // parameters or headers, which fail independently (`OBS-006`, `API-041`).
    let body = raw_body.map_err(ApiError::from)?;

    let Ok(upload_id) = upload_id_value.parse::<NumericID>() else {
        return Err(ApiError::BadRequest(
            "invalid upload session identifier".to_owned(),
        ));
    };
    let Ok(chunk_number) = chunk_number_value.parse::<u64>() else {
        return Err(ApiError::BadRequest("invalid chunk number".to_owned()));
    };

    let raw = match headers.get(header::CONTENT_RANGE) {
        None => {
            return Err(ApiError::UnprocessableEntity(
                "missing Content-Range header".to_owned(),
            ));
        }
        Some(value) => value
            .to_str()
            .map_err(|_| ApiError::BadRequest("malformed Content-Range header".to_owned()))?,
    };
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
        return Err(ApiError::UnprocessableEntity(
            "the Content-Range end precedes its start".to_owned(),
        ));
    }

    let raw_digest = match headers.get("content-digest") {
        None => {
            return Err(ApiError::UnprocessableEntity(
                "missing Content-Digest header".to_owned(),
            ));
        }
        Some(value) => value
            .to_str()
            .map_err(|_| ApiError::BadRequest("malformed Content-Digest header".to_owned()))?,
    };
    let mut parsed_any = false;
    let mut parsed_digest = None;
    for parameter in raw_digest.split(',').map(str::trim) {
        let Some((algorithm, value)) = parameter.split_once('=') else {
            continue;
        };
        if algorithm.trim().is_empty() {
            continue;
        }
        parsed_any = true;
        if algorithm.trim() != "sha-256" {
            continue;
        }
        let encoded = value
            .trim()
            .strip_prefix(':')
            .and_then(|after_colon| after_colon.strip_suffix(':'))
            .ok_or_else(|| ApiError::BadRequest("malformed Content-Digest header".to_owned()))?;
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|_| ApiError::BadRequest("malformed Content-Digest header".to_owned()))?;
        if bytes.len() != DIGEST_BYTES {
            return Err(ApiError::UnprocessableEntity(
                "the Content-Digest value is not a 32-byte SHA-256 digest".to_owned(),
            ));
        }
        let mut hex = String::with_capacity(bytes.len().saturating_mul(2));
        {
            use std::fmt::Write as _;

            for byte in &bytes {
                let _result = write!(hex, "{byte:02x}");
            }
        }
        parsed_digest = Some(IntegrityHash::try_new(hex).map_err(|_| {
            ApiError::UnprocessableEntity(
                "the Content-Digest value is not a valid SHA-256 digest".to_owned(),
            )
        })?);
        break;
    }
    let Some(content_digest) = parsed_digest else {
        let error = if parsed_any {
            ApiError::UnprocessableEntity("unsupported Content-Digest algorithm".to_owned())
        } else {
            ApiError::BadRequest("malformed Content-Digest header".to_owned())
        };
        return Err(error);
    };

    let body_len = u64::try_from(body.len())
        .map_err(|_| ApiError::BadRequest("invalid chunk length".to_owned()))?;
    if end.saturating_sub(start).saturating_add(1) != body_len {
        return Err(ApiError::UnprocessableEntity(
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

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;

    use axum::extract::Request;
    use axum::http::HeaderValue;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::response::Response;
    use axum::routing::put;
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD;
    use chrono::NaiveDate;
    use chrono::NaiveDateTime;
    use mockall::predicate::eq;
    use rstest::rstest;

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
    use crate::test_helpers::into_status_and_body;

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

    /// Send a chunk PUT whose `Content-Range` and `Content-Digest` headers carry
    /// raw bytes, exercising each header's `to_str` failure (`API-041`).
    ///
    /// A value that is not visible ASCII cannot be expressed as a `&str`, so this
    /// builder is local to the module and bypasses the `&str` parameters of
    /// [`send`].
    async fn send_with_raw_header_values(
        use_case: MockWriteUploadChunkUseCase,
        caller_id: NumericID,
        uri: &str,
        content_range: &[u8],
        content_digest: &[u8],
        body: Body,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockAssetUseCaseFactory::new();
        factory
            .expect_write_upload_chunk()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn WriteUploadChunkUseCase>);

        let mut request = Request::builder()
            .method("PUT")
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .header(
                header::CONTENT_RANGE,
                HeaderValue::from_bytes(content_range)?,
            )
            .header("content-digest", HeaderValue::from_bytes(content_digest)?)
            .body(body)?;
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

    /// Make the mocked use case fail with `error`.
    fn expect_error(use_case: &mut MockWriteUploadChunkUseCase, error: WriteUploadChunkError) {
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));
    }

    /// Fixed expiry used by the stubbed success response.
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
        let (status, payload) = into_status_and_body(response).await?;

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
    async fn put_upload_chunk_missing_content_range_returns_unprocessable_entity()
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
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
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
    async fn put_upload_chunk_range_length_mismatch_returns_unprocessable_entity()
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
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the declared range does not match the body length")
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_reversed_range_returns_unprocessable_entity()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockWriteUploadChunkUseCase::new();

        // Act
        let response = send(
            use_case,
            3,
            "/api/v1/asset/upload/7/chunk/0",
            Some("bytes 9-0/12"),
            Some(&chunk_content_digest()),
            Body::from(CHUNK),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the Content-Range end precedes its start")
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_missing_content_digest_returns_unprocessable_entity()
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
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("missing Content-Digest header")
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_content_digest_without_equals_returns_bad_request()
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

    #[rstest]
    #[case::unwrapped_value("sha-256=AAAA")]
    #[case::undecodable_value("sha-256=:not-base64:")]
    #[tokio::test]
    async fn put_upload_chunk_malformed_content_digest_value_returns_bad_request(
        #[case] content_digest: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockWriteUploadChunkUseCase::new();

        // Act
        let response = send(
            use_case,
            3,
            "/api/v1/asset/upload/7/chunk/0",
            Some(RANGE_CHUNK_ZERO),
            Some(content_digest),
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
    async fn put_upload_chunk_unsupported_content_digest_algorithm_returns_unprocessable_entity()
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
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("unsupported Content-Digest algorithm")
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_content_digest_wrong_length_returns_unprocessable_entity()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockWriteUploadChunkUseCase::new();

        // Act
        let response = send(
            use_case,
            3,
            "/api/v1/asset/upload/7/chunk/0",
            Some(RANGE_CHUNK_ZERO),
            Some("sha-256=:AAAA:"),
            Body::from(CHUNK),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the Content-Digest value is not a 32-byte SHA-256 digest")
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_content_digest_mixed_algorithms_stores_chunk()
    -> Result<(), Box<dyn Error>> {
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
        let digest = format!("sha-512=:AAAA:, {}", chunk_content_digest());

        // Act
        let response = send(
            use_case,
            3,
            "/api/v1/asset/upload/7/chunk/0",
            Some(RANGE_CHUNK_ZERO),
            Some(&digest),
            Body::from(CHUNK),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_non_utf8_content_digest_returns_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockWriteUploadChunkUseCase::new();

        // Act
        let response = send_with_raw_header_values(
            use_case,
            3,
            "/api/v1/asset/upload/7/chunk/0",
            RANGE_CHUNK_ZERO.as_bytes(),
            b"sha-256=:\xff\xfe:",
            Body::from(CHUNK),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let message = error_message_of(&response);
        assert_eq!(message.as_deref(), Some("malformed Content-Digest header"));
        assert_ne!(
            message.as_deref(),
            Some("missing Content-Digest header"),
            "a present but undecodable header must not be reported as missing (`API-041`)"
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_non_utf8_content_range_returns_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockWriteUploadChunkUseCase::new();

        // Act
        let response = send_with_raw_header_values(
            use_case,
            3,
            "/api/v1/asset/upload/7/chunk/0",
            b"bytes 0-3/\xff",
            chunk_content_digest().as_bytes(),
            Body::from(CHUNK),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let message = error_message_of(&response);
        assert_eq!(message.as_deref(), Some("malformed Content-Range header"));
        assert_ne!(
            message.as_deref(),
            Some("missing Content-Range header"),
            "a present but undecodable header must not be reported as missing (`API-041`)"
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_upload_chunk_content_digest_missing_algorithm_returns_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockWriteUploadChunkUseCase::new();

        // Act
        let response = send(
            use_case,
            3,
            "/api/v1/asset/upload/7/chunk/0",
            Some(RANGE_CHUNK_ZERO),
            Some("=:AAAA:"),
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
