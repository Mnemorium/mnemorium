use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDateTime;

use crate::domain::NumericID;
use crate::domain::port::FileReader;

/// Command to read the content of one asset.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct GetAssetContentCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Whether the request is a `HEAD`, which answers headers without a body.
    head: bool,
    /// `If-None-Match` value, when the request carries one.
    if_none_match: Option<String>,
    /// `If-Range` value, when the request carries one.
    if_range: Option<String>,
    /// The request's `Range` header value, when the request carries one.
    range: Option<String>,
    /// Asset whose bytes are requested.
    target: AssetTarget,
}

impl GetAssetContentCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return whether the request answers headers only.
    #[must_use]
    pub(crate) fn head(&self) -> bool {
        self.head
    }

    /// Return the `If-None-Match` value, if any.
    #[must_use]
    pub(crate) fn if_none_match(&self) -> Option<&str> {
        self.if_none_match.as_deref()
    }

    /// Return the `If-Range` value, if any.
    #[must_use]
    pub(crate) fn if_range(&self) -> Option<&str> {
        self.if_range.as_deref()
    }

    /// Create a new asset-content command for an authenticated caller.
    #[must_use]
    pub(crate) fn new(caller_id: NumericID, target: AssetTarget) -> Self {
        Self {
            caller_id,
            head: false,
            if_none_match: None,
            if_range: None,
            range: None,
            target,
        }
    }

    /// Return the request's `Range` header value, if any.
    #[must_use]
    pub(crate) fn range(&self) -> Option<&str> {
        self.range.as_deref()
    }

    /// Return the asset whose bytes are requested.
    #[must_use]
    pub(crate) fn target(&self) -> AssetTarget {
        self.target
    }

    /// Answer headers only for a `HEAD` request.
    #[must_use]
    pub(crate) fn with_head(mut self, head: bool) -> Self {
        self.head = head;
        self
    }

    /// Carry the request's `If-None-Match` value.
    #[must_use]
    pub(crate) fn with_if_none_match(mut self, value: Option<String>) -> Self {
        self.if_none_match = value;
        self
    }

    /// Carry the request's `If-Range` value.
    #[must_use]
    pub(crate) fn with_if_range(mut self, value: Option<String>) -> Self {
        self.if_range = value;
        self
    }

    /// Carry the request's `Range` header value.
    #[must_use]
    pub(crate) fn with_range(mut self, range: Option<String>) -> Self {
        self.range = range;
        self
    }
}

/// The HTTP outcome a content read resolves to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum ContentOutcome {
    /// The whole content is served.
    Full,
    /// The content is unchanged and no body is served.
    NotModified,
    /// A byte range of the content is served.
    Partial,
    /// The requested range cannot be satisfied.
    Unsatisfiable,
}

/// Response of a successful asset-content read: the reader and its metadata.
#[non_exhaustive]
pub(crate) struct GetAssetContentResponse {
    /// Validator derived from the stored whole-file digest, when present.
    etag: Option<String>,
    /// Original name a save surface offers, present for file content.
    file_name: Option<String>,
    /// Identifier of the stored media type of the asset.
    mime_type_id: String,
    /// Instant at which the content was last modified.
    modified_at: NaiveDateTime,
    /// Whether the whole content, a range, a `304` or a `416` is answered.
    outcome: ContentOutcome,
    /// Reader over the requested bytes, absent for `HEAD`, `304` and `416`.
    reader: Option<FileReader>,
    /// Inclusive start and end of the served span, when a range is served.
    selected_range: Option<(u64, u64)>,
    /// Size of the opened file, in bytes.
    size_bytes: u64,
}

impl GetAssetContentResponse {
    /// Return the `ETag`, when the stored digest supplies one.
    #[must_use]
    pub(crate) fn etag(&self) -> Option<&str> {
        self.etag.as_deref()
    }

    /// Return the original name a save surface offers, when present.
    #[must_use]
    pub(crate) fn file_name(&self) -> Option<&str> {
        self.file_name.as_deref()
    }

    /// Take the reader over the requested bytes, if any.
    #[must_use]
    pub(crate) fn into_reader(self) -> Option<FileReader> {
        self.reader
    }

    /// Return the identifier of the stored media type of the asset.
    #[must_use]
    pub(crate) fn mime_type_id(&self) -> &str {
        &self.mime_type_id
    }

    /// Return the instant at which the content was last modified.
    #[must_use]
    pub(crate) fn modified_at(&self) -> NaiveDateTime {
        self.modified_at
    }

    /// Create a new asset-content response.
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the response's fields one-to-one"
    )]
    pub(crate) fn new(
        mime_type_id: String,
        size_bytes: u64,
        modified_at: NaiveDateTime,
        etag: Option<String>,
        file_name: Option<String>,
        outcome: ContentOutcome,
        selected_range: Option<(u64, u64)>,
        reader: Option<FileReader>,
    ) -> Self {
        Self {
            etag,
            file_name,
            mime_type_id,
            modified_at,
            outcome,
            reader,
            selected_range,
            size_bytes,
        }
    }

    /// Return the HTTP outcome the read resolved to.
    #[must_use]
    pub(crate) fn outcome(&self) -> ContentOutcome {
        self.outcome
    }

    /// Return the inclusive start and end of the served span, when partial.
    #[must_use]
    pub(crate) fn selected_range(&self) -> Option<(u64, u64)> {
        self.selected_range
    }

    /// Return the size of the opened file, in bytes.
    #[must_use]
    pub(crate) fn size_bytes(&self) -> u64 {
        self.size_bytes
    }
}

/// The asset a content request addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum AssetTarget {
    /// A typed audio addressed by its identifier, resolved to its file.
    Audio(NumericID),
    /// A stored file addressed by its own identifier.
    File(NumericID),
    /// A typed image addressed by its identifier, resolved to its file.
    Image(NumericID),
    /// A typed video addressed by its identifier, resolved to its file.
    Video(NumericID),
}

/// Error returned when reading the content of an asset fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum GetAssetContentError {
    /// The registered file's bytes are absent while the library root exists.
    #[error("the file record exists but its bytes are absent")]
    BytesMissing,
    /// The caller can discover the asset but its action refuses.
    #[error("the caller may not read the asset")]
    Forbidden,
    /// The library root is unavailable.
    #[error("the library root is unavailable")]
    LibraryUnavailable,
    /// The asset does not exist, or the caller cannot discover it.
    ///
    /// Both outcomes are reported the same way so a caller cannot probe for
    /// assets it may not discover, and no bytes are read for an unreadable
    /// asset.
    #[error("the asset does not exist")]
    NoSuchAsset,
    /// The caller does not exist.
    #[error("the caller does not exist")]
    NoSuchCaller,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case reading the content of one asset.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait GetAssetContentUseCase: Send + Sync {
    /// Read the requested asset content.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as `Arc<dyn GetAssetContentUseCase>`.
    fn execute<'future>(
        &'future self,
        command: GetAssetContentCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<GetAssetContentResponse, GetAssetContentError>>
                + Send
                + 'future,
        >,
    >;
}
