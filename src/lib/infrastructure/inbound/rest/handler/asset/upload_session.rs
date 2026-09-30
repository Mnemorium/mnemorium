//! Shared HTTP representation of an upload session's state.

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::handler::asset::links::UploadSessionLinks;

/// State of an upload session returned by a successful lookup or chunk store.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct UploadSessionResponse {
    /// One character per chunk, `1` when received and `0` otherwise.
    #[schema(example = json!("110"))]
    pub bitmap: String,
    /// Date and time at which the upload session expires, as
    /// `YYYY-MM-DDTHH:MM:SS`.
    #[schema(example = json!("2026-01-01T12:00:00"))]
    pub expires_at: String,
    /// Unique identifier of the caller's file for the session digest, when one
    /// exists.
    pub file_id: Option<NumericID>,
    /// Whether the upload session has been finished.
    pub is_finished: bool,
    /// Links to the upload session, its chunk endpoint and its completion.
    #[serde(rename = "_links")]
    pub links: UploadSessionLinks,
    /// Total number of chunks the upload is split into.
    pub total_chunks: usize,
}

impl UploadSessionResponse {
    /// Map the state of the upload session identified by `upload_id` onto its
    /// HTTP representation.
    #[must_use]
    pub fn new(
        upload_id: NumericID,
        bitmap: String,
        total_chunks: usize,
        expires_at: NaiveDateTime,
        is_finished: bool,
        file_id: Option<NumericID>,
    ) -> Self {
        Self {
            bitmap,
            expires_at: expires_at.format("%Y-%m-%dT%H:%M:%S").to_string(),
            file_id,
            is_finished,
            links: UploadSessionLinks::for_upload(upload_id),
            total_chunks,
        }
    }
}
