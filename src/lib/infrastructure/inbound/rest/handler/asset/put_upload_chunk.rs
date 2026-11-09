use axum::Json;
use axum::body::Bytes;
use axum::extract::rejection::BytesRejection;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse as _, Response};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::port::write_upload_chunk::{
    WriteUploadChunkCommand, WriteUploadChunkError,
};
use crate::domain::alias::NumericID;
use crate::domain::model::integrity_hash::{DIGEST_HEX_LENGTH, IntegrityHash};
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::HAL_CONTENT_TYPE;
use crate::infrastructure::inbound::rest::handler::asset::UploadSessionResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Number of raw bytes a content digest carries: two hexadecimal characters
/// encode one byte.
const DIGEST_BYTES: usize = DIGEST_HEX_LENGTH >> 1;

/// Raw bytes of one chunk.
///
/// The handler extracts the body as raw bytes. It is declared as a binary
/// `String` so the `OpenAPI` schema describes the raw `application/octet-stream`
/// payload instead of the JSON array of integers a bare `Vec<u8>` would produce.
#[derive(Debug, Deserialize, ToSchema)]
#[schema(value_type = String, format = Binary)]
#[non_exhaustive]
struct PutUploadChunkRequest(
    /// Raw bytes of the chunk.
    #[expect(
        dead_code,
        reason = "the extractor carries the chunk bytes; the handler reads them through the body"
    )]
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
            WriteUploadChunkError::AlreadyFinished
            | WriteUploadChunkError::CompletionInProgress => Self::Conflict(err.to_string()),
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
    path = "/upload/{upload_id}/chunk/{chunk_number}",
    tag = "asset",
    request_body(
        content_type = "application/octet-stream",
        content = PutUploadChunkRequest,
    ),
    params(
        ("upload_id" = NumericID, Path, description = "Identifier of the upload session"),
        ("chunk_number" = u64, Path, description = "Zero-based number of the chunk", maximum = 9_223_372_036_854_775_807u64),
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
pub(crate) async fn put_upload_chunk(
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
