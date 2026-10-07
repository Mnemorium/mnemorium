use std::future::Future;
use std::path::Path;

use crate::domain::model::image::Orientation;
use crate::domain::model::video::ScanType;
use crate::domain::port::error::MediaProbeError;

/// Metadata extracted from an image file.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ProbedImage {
    /// Height of the image, in pixels.
    pub height_px: i64,
    /// Orientation of the image.
    pub orientation: Orientation,
    /// Width of the image, in pixels.
    pub width_px: i64,
}

/// Metadata extracted from a video file.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ProbedVideo {
    /// Codec of the video.
    pub codec: String,
    /// Identifier of the colour space of the video.
    pub color_id: String,
    /// Duration of the video, in milliseconds.
    pub duration_ms: f64,
    /// Number of frames in the video.
    pub frame_count: i64,
    /// Height of the video, in pixels.
    pub height: i64,
    /// Scan type of the video.
    pub scan_type: ScanType,
    /// Width of the video, in pixels.
    pub width: i64,
}

/// Port for probing media files for their metadata.
#[cfg_attr(test, mockall::automock)]
pub trait MediaProbe: Send + Sync {
    /// Probe the image stored at `path`.
    ///
    /// A file absent at `path` is a valid outcome, reported as `Ok(None)`; a
    /// missing file is not an error.
    ///
    /// # Errors
    ///
    /// Returns [`MediaProbeError::UnsupportedMedia`] when the file is not a
    /// supported image, [`MediaProbeError::Timeout`] when probing exceeds the
    /// allowed time, and [`MediaProbeError::OperationFailed`] or
    /// [`MediaProbeError::Unknown`] when the probe cannot complete.
    fn probe_image(
        &self,
        path: &Path,
    ) -> impl Future<Output = Result<Option<ProbedImage>, MediaProbeError>> + Send;

    /// Probe the video stored at `path`.
    ///
    /// A file absent at `path` is a valid outcome, reported as `Ok(None)`; a
    /// missing file is not an error.
    ///
    /// # Errors
    ///
    /// Returns [`MediaProbeError::UnsupportedMedia`] when the file is not a
    /// supported video, [`MediaProbeError::Timeout`] when probing exceeds the
    /// allowed time, and [`MediaProbeError::OperationFailed`] or
    /// [`MediaProbeError::Unknown`] when the probe cannot complete.
    fn probe_video(
        &self,
        path: &Path,
    ) -> impl Future<Output = Result<Option<ProbedVideo>, MediaProbeError>> + Send;
}
