//! Port reporting the media type of a file from its content.
//!
//! The port owns no policy: it answers what the content is and nothing else.
//! The caller owns the supported set and decides what an unsupported file
//! means, so two callers with different accept-lists can share it.

use std::future::Future;
use std::path::Path;

use crate::domain::port::error::ContentTypeSnifferError;

/// Port for determining a file's media type from its content.
#[cfg_attr(test, mockall::automock)]
pub trait ContentTypeSniffer: Send + Sync {
    /// Return the media type of the content stored at `path`.
    ///
    /// The type is identified from the content, never from the file name or
    /// extension. Absent, unreadable in a way that cannot classify the content,
    /// or unrecognised content is `Ok(None)`; a missing file is not an error.
    ///
    /// The caller owns the supported set: this port reports every type it
    /// recognises, including one the caller does not accept.
    ///
    /// # Errors
    ///
    /// Returns [`ContentTypeSnifferError::OperationFailed`] or
    /// [`ContentTypeSnifferError::Unknown`] when the content cannot be read.
    fn sniff(
        &self,
        path: &Path,
    ) -> impl Future<Output = Result<Option<String>, ContentTypeSnifferError>> + Send;
}
