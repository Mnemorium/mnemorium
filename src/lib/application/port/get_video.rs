use std::future::Future;
use std::pin::Pin;

use crate::domain::NumericID;
use crate::domain::model::ScanType;

/// Command to fetch one typed video.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct GetVideoCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Identifier of the video to fetch.
    video_id: NumericID,
}

impl GetVideoCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Create a new fetch-video command.
    #[must_use]
    pub(crate) fn new(caller_id: NumericID, video_id: NumericID) -> Self {
        Self {
            caller_id,
            video_id,
        }
    }

    /// Return the identifier of the video to fetch.
    #[must_use]
    pub(crate) fn video_id(&self) -> NumericID {
        self.video_id
    }
}

/// Response of a successful video lookup: one video and its backing file.
#[non_exhaustive]
pub(crate) struct GetVideoResponse {
    /// Codec of the video.
    codec: String,
    /// Identifier of the colour space.
    color_id: String,
    /// Duration of the video, in milliseconds.
    duration_ms: f64,
    /// Identifier of the backing file.
    file_id: NumericID,
    /// Number of frames.
    frame_count: i64,
    /// Height in pixels.
    height: i64,
    /// Unique identifier of the video.
    id: NumericID,
    /// Scan type.
    scan_type: ScanType,
    /// Width in pixels.
    width: i64,
}

impl GetVideoResponse {
    /// Return the codec.
    #[must_use]
    pub(crate) fn codec(&self) -> &str {
        &self.codec
    }

    /// Return the identifier of the colour space.
    #[must_use]
    pub(crate) fn color_id(&self) -> &str {
        &self.color_id
    }

    /// Return the duration, in milliseconds.
    #[must_use]
    pub(crate) fn duration_ms(&self) -> f64 {
        self.duration_ms
    }

    /// Return the identifier of the backing file.
    #[must_use]
    pub(crate) fn file_id(&self) -> NumericID {
        self.file_id
    }

    /// Return the number of frames.
    #[must_use]
    pub(crate) fn frame_count(&self) -> i64 {
        self.frame_count
    }

    /// Return the height in pixels.
    #[must_use]
    pub(crate) fn height(&self) -> i64 {
        self.height
    }

    /// Return the unique identifier of the video.
    #[must_use]
    pub(crate) fn id(&self) -> NumericID {
        self.id
    }

    /// Create a new fetch-video response.
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the response's fields one-to-one"
    )]
    pub(crate) fn new(
        id: NumericID,
        codec: String,
        color_id: String,
        duration_ms: f64,
        frame_count: i64,
        height: i64,
        width: i64,
        scan_type: ScanType,
        file_id: NumericID,
    ) -> Self {
        Self {
            codec,
            color_id,
            duration_ms,
            file_id,
            frame_count,
            height,
            id,
            scan_type,
            width,
        }
    }

    /// Return the scan type.
    #[must_use]
    pub(crate) fn scan_type(&self) -> ScanType {
        self.scan_type
    }

    /// Return the width in pixels.
    #[must_use]
    pub(crate) fn width(&self) -> i64 {
        self.width
    }
}

/// Error returned when fetching a typed video fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum GetVideoError {
    /// The caller does not exist.
    #[error("the caller does not exist")]
    NoSuchCaller,
    /// The video does not exist, or the caller may not read it.
    ///
    /// Both outcomes are reported the same way so a caller cannot probe for
    /// video it may not read.
    #[error("the video does not exist")]
    NoSuchVideo,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case fetching one typed video.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait GetVideoUseCase: Send + Sync {
    /// Fetch the requested video.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as `Arc<dyn GetVideoUseCase>`.
    fn execute<'future>(
        &'future self,
        command: GetVideoCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetVideoResponse, GetVideoError>> + Send + 'future>>;
}
