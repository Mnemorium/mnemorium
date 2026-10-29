#![cfg_attr(
    test,
    expect(clippy::struct_field_names, reason = "It's a valid prefix method name")
)]

use std::future::Future;
use std::path::Path;

use crate::domain::model::audio::ProbedAudio;
use crate::domain::model::image::ProbedImage;
use crate::domain::model::video::ProbedVideo;
use crate::domain::port::error::MediaProbeError;

/// Port for probing media files for their metadata.
#[cfg_attr(test, mockall::automock)]
pub trait MediaProbe: Send + Sync {
    /// Probe the audio stored at `path`.
    ///
    /// A file absent at `path` is a valid outcome, reported as `Ok(None)`; a
    /// missing file is not an error. A bit depth or block size the content does
    /// not report is absent rather than fabricated.
    ///
    /// # Errors
    ///
    /// Returns [`MediaProbeError::UnsupportedMedia`] when the file is not a
    /// supported audio file, [`MediaProbeError::Timeout`] when probing exceeds
    /// the allowed time, and [`MediaProbeError::OperationFailed`] or
    /// [`MediaProbeError::Unknown`] when the probe cannot complete.
    fn probe_audio(
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
