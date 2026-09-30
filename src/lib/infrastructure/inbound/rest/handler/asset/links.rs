//! HAL links of an upload-session representation.

use serde::Deserialize;
use serde::Serialize;
use utoipa::ToSchema;

use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::hal::Link;

/// HAL links exposed by an upload-session representation.
///
/// The `self` link addresses the session itself; the templated `chunk` link
/// lets clients discover where to store a chunk instead of building the URL
/// from the identifiers (`API-037`).
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct UploadSessionLinks {
    /// URI template for storing one chunk of the upload session.
    pub chunk: Link,
    /// Link to the upload-session resource itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

impl UploadSessionLinks {
    /// Build the links of the upload session identified by `upload_id`.
    #[must_use]
    pub fn for_upload(upload_id: NumericID) -> Self {
        Self {
            chunk: Link::templated(&format!(
                "/api/v1/asset/upload/{upload_id}/chunk/{{chunk_number}}"
            )),
            self_link: Link::new(&format!("/api/v1/asset/upload/{upload_id}")),
        }
    }
}
