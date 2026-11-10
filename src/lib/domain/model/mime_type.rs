/// Error returned when initialising a `MimeType`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum MimeTypeError {
    /// The mime type identifier is empty.
    #[error("the mime type identifier must not be empty")]
    EmptyId,
}

/// A supported mime type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MimeType {
    /// Identifier of the mime type.
    mime_type_id: String,
}

impl MimeType {
    /// Return the mime type identifier.
    #[must_use]
    pub(crate) fn mime_type_id(&self) -> &str {
        &self.mime_type_id
    }

    /// Initialise a `MimeType`, validating `mime_type_id` is non-empty.
    ///
    /// # Errors
    ///
    /// Returns [`MimeTypeError::EmptyId`] when `mime_type_id` is empty.
    pub(crate) fn try_new(mime_type_id: String) -> Result<Self, MimeTypeError> {
        if mime_type_id.is_empty() {
            return Err(MimeTypeError::EmptyId);
        }
        Ok(Self { mime_type_id })
    }
}
