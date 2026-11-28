use std::future::Future;
use std::path::Path;

use crate::domain::model::{ProbedAudio, ProbedImage, ProbedVideo};
use crate::domain::port::MediaProbeError;

/// Port for probing media files for their metadata.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait MediaProbe: Send + Sync {
    /// Probe the audio stored at `path`.
    ///
    /// A file absent at `path` is a valid outcome, reported as `Ok(None)`; a
    /// missing file is not an error. A bit depth, block size, duration or sample
    /// rate the content does not report is absent rather than fabricated.
    ///
    /// # Errors
    ///
    /// Returns [`MediaProbeError::UnsupportedMedia`] when the file is not a
    /// supported audio file or a measurement it reports cannot be represented,
    /// [`MediaProbeError::Timeout`] when probing exceeds the allowed time, and
    /// [`MediaProbeError::OperationFailed`] or [`MediaProbeError::Unknown`] when
    /// the probe cannot complete.
    fn audio(
        &self,
        path: &Path,
    ) -> impl Future<Output = Result<Option<ProbedAudio>, MediaProbeError>> + Send;

    /// Probe the image stored at `path`.
    ///
    /// A file absent at `path` is a valid outcome, reported as `Ok(None)`; a
    /// missing file is not an error.
    ///
    /// # Errors
    ///
    /// Returns [`MediaProbeError::UnsupportedMedia`] when the file is not a
    /// supported image or a dimension it reports cannot be represented,
    /// [`MediaProbeError::Timeout`] when probing exceeds the allowed time, and
    /// [`MediaProbeError::OperationFailed`] or [`MediaProbeError::Unknown`] when
    /// the probe cannot complete.
    fn image(
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
    /// supported video or a measurement it reports cannot be represented,
    /// [`MediaProbeError::Timeout`] when probing exceeds the allowed time, and
    /// [`MediaProbeError::OperationFailed`] or [`MediaProbeError::Unknown`] when
    /// the probe cannot complete.
    fn video(
        &self,
        path: &Path,
    ) -> impl Future<Output = Result<Option<ProbedVideo>, MediaProbeError>> + Send;
}
