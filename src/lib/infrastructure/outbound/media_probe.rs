//! Media probe decoding images in-process and videos through `ffprobe`.
//!
//! The adapter owns the failure log for every fault it classifies: it emits the
//! catalogued `port_fault` with a stable `kind` and the `media_probe` operation
//! and never logs a path, an argument or the probe's standard error (`OBS-002`,
//! `OBS-003`). The video probe is invoked with an explicit argument vector, a
//! bounded standard output and a timeout; no shell is involved.

use std::cmp::Ordering;
use std::ffi::OsString;
use std::future::Future;
use std::io::Error;
use std::io::ErrorKind;
use std::path::Path;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;

use serde::Deserialize;
use tokio::fs;
use tokio::io::AsyncReadExt as _;
use tokio::process::Command;
use tokio::sync::Semaphore;
use tokio::task::spawn_blocking;
use tokio::time::timeout;
use tracing::error;
use tracing::warn;

use crate::domain::model::image::Orientation;
use crate::domain::model::video::ScanType;
use crate::domain::port::error::MediaProbeError;
use crate::domain::port::media_probe::MediaProbe;
use crate::domain::port::media_probe::ProbedImage;
use crate::domain::port::media_probe::ProbedVideo;

/// Name of the video probe binary resolved from `PATH`.
const FFPROBE: &str = "ffprobe";

/// Maximum number of bytes read from the probe's standard output.
const MAX_PROBE_STDOUT_BYTES: u64 = 1024 * 1024;

/// Maximum number of video probes allowed to run at once.
///
/// Sized for a single-container deployment: each permit bounds one `ffprobe`
/// child, so concurrent completions cannot exhaust the host's process limits.
const MAX_CONCURRENT_PROBES: usize = 4;

/// Number of milliseconds in a second, as a float for duration conversion.
const MILLIS_PER_SECOND: f64 = 1000.0;

/// Security-event operation name owned by this adapter (`OBS-002`).
const OPERATION: &str = "media_probe";

/// Maximum time a video probe may take before it is killed.
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);

/// Default colour space assigned to a video whose pixel format is unmapped.
const DEFAULT_COLOR_ID: &str = "YCbCr";

/// Media probe backed by the `image` crate and the local `ffprobe` binary.
pub struct LocalMediaProbe {
    /// Maximum time a video probe may take.
    probe_timeout: Duration,
    /// Program resolved from `PATH` by default.
    program: OsString,
}

impl LocalMediaProbe {
    /// Create a probe that runs the `ffprobe` binary resolved from `PATH`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            probe_timeout: PROBE_TIMEOUT,
            program: OsString::from(FFPROBE),
        }
    }
}

#[cfg(test)]
impl LocalMediaProbe {
    /// Create a probe bound to an explicit `program` and `probe_timeout`.
    ///
    /// Test-only: production resolves `ffprobe` from `PATH` with the default
    /// timeout.
    pub(crate) fn with_program(program: impl Into<OsString>, probe_timeout: Duration) -> Self {
        Self {
            probe_timeout,
            program: program.into(),
        }
    }
}

impl Default for LocalMediaProbe {
    fn default() -> Self {
        Self::new()
    }
}

/// The `ffprobe` JSON document, reduced to the fields the adapter reads.
#[derive(Deserialize)]
struct ProbeOutput {
    /// Container-level metadata.
    format: Option<ProbeFormat>,
    /// One entry per elementary stream.
    streams: Option<Vec<ProbeStream>>,
}

/// Container-level metadata of an `ffprobe` document.
#[derive(Deserialize)]
struct ProbeFormat {
    /// Container duration, in seconds, as a string.
    duration: Option<String>,
}

/// One elementary stream of an `ffprobe` document.
#[derive(Deserialize)]
struct ProbeStream {
    /// Average frame rate, as a `"numerator/denominator"` string.
    avg_frame_rate: Option<String>,
    /// Codec name, for example `"h264"`.
    codec_name: Option<String>,
    /// Stream kind, for example `"video"`.
    codec_type: Option<String>,
    /// Stream duration, in seconds, as a string.
    duration: Option<String>,
    /// Field order, for example `"progressive"` or `"tt"`.
    field_order: Option<String>,
    /// Stream height, in pixels.
    height: Option<u32>,
    /// Declared number of frames, as a string.
    nb_frames: Option<String>,
    /// Pixel format, for example `"yuv420p"`.
    pix_fmt: Option<String>,
    /// Nominal frame rate, as a `"numerator/denominator"` string.
    r_frame_rate: Option<String>,
    /// Stream width, in pixels.
    width: Option<u32>,
}

impl MediaProbe for LocalMediaProbe {
    fn probe_image(
        &self,
        path: &Path,
    ) -> impl Future<Output = Result<Option<ProbedImage>, MediaProbeError>> + Send {
        let owned = path.to_path_buf();
        async move {
            // Decoding untrusted, potentially large bytes blocks; run it on the
            // blocking pool so a crafted image cannot stall an async worker.
            match spawn_blocking(move || decode_image(&owned)).await {
                Ok(result) => result,
                Err(error) => {
                    log_fault("join_failure");
                    Err(MediaProbeError::Unknown(error.into()))
                }
            }
        }
    }

    fn probe_video(
        &self,
        path: &Path,
    ) -> impl Future<Output = Result<Option<ProbedVideo>, MediaProbeError>> + Send {
        let owned = path.to_path_buf();
        let program = self.program.clone();
        let probe_timeout = self.probe_timeout;
        async move { run_video_probe(&program, probe_timeout, &owned).await }
    }
}

/// Build the `ffprobe` invocation for `path`.
///
/// The path is passed as a single argument to the program; no shell interprets
/// it, so a path containing shell metacharacters cannot inject a command.
#[expect(
    clippy::single_call_fn,
    reason = "the argument vector is named after the rule it encodes"
)]
fn probe_command(program: &OsString, path: &Path) -> Command {
    let mut command = Command::new(program);
    command.kill_on_drop(true);
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    // The probe's stderr is never surfaced: discarding it means a chatty
    // `ffprobe` cannot fill the pipe and stall the probe into a false timeout.
    command.stderr(Stdio::null());
    command.args([
        "-v",
        "error",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
    ]);
    command.arg(path);
    command
}

/// Decode an image and return its dimensions.
///
/// A file absent at `path` is `Ok(None)`; a malformed or unsupported image is
/// an error.
#[expect(
    clippy::single_call_fn,
    reason = "the image decoding is named after the step it performs"
)]
fn decode_image(path: &Path) -> Result<Option<ProbedImage>, MediaProbeError> {
    let reader = match image::ImageReader::open(path) {
        Ok(reader) => reader,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(io_fault(err)),
    };
    let (width, height) = reader
        .with_guessed_format()
        .map_err(io_fault)?
        .into_dimensions()
        .map_err(|error| {
            let reason = if matches!(error, image::ImageError::Unsupported(_)) {
                "unsupported_media"
            } else {
                "decode_failure"
            };
            log_validation_failure(reason);
            MediaProbeError::UnsupportedMedia
        })?;

    let orientation = match width.cmp(&height) {
        Ordering::Greater => Orientation::Landscape,
        Ordering::Less => Orientation::Portrait,
        Ordering::Equal => Orientation::Square,
    };

    Ok(Some(ProbedImage {
        height_px: i64::from(height),
        orientation,
        width_px: i64::from(width),
    }))
}

/// Map an I/O error onto the port error and log its stable `kind`.
fn io_fault(err: Error) -> MediaProbeError {
    log_fault("io_failure");
    MediaProbeError::Unknown(err.into())
}

/// Run `ffprobe` against `path` and map its JSON output onto [`ProbedVideo`].
///
/// # Errors
///
/// Returns [`MediaProbeError::Timeout`] when the probe exceeds `probe_timeout`,
/// [`MediaProbeError::UnsupportedMedia`] when the file has no usable video
/// stream, and [`MediaProbeError::OperationFailed`] or
/// [`MediaProbeError::Unknown`] when the probe cannot complete.
#[expect(
    clippy::single_call_fn,
    reason = "the video probe is named after the step it performs"
)]
async fn run_video_probe(
    program: &OsString,
    probe_timeout: Duration,
    path: &Path,
) -> Result<Option<ProbedVideo>, MediaProbeError> {
    match fs::metadata(path).await {
        Ok(_) => {}
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(io_fault(err)),
    }

    // Untrusted uploaded bytes are handed to `ffprobe` as itself, so the work
    // is bounded here; a fuller sandbox is a separate architectural decision.
    let Ok(_permit) = probe_permits().acquire().await else {
        log_fault("semaphore_closed");
        return Err(MediaProbeError::OperationFailed);
    };

    let mut child = match probe_command(program, path).spawn() {
        Ok(child) => child,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            log_fault("missing_binary");
            return Err(MediaProbeError::OperationFailed);
        }
        Err(err) => {
            log_fault("spawn_failure");
            return Err(MediaProbeError::Unknown(err.into()));
        }
    };

    let piped_stdout = child.stdout.take();
    let read = timeout(probe_timeout, async {
        let mut buffer = Vec::new();
        if let Some(stdout) = piped_stdout {
            stdout
                .take(MAX_PROBE_STDOUT_BYTES)
                .read_to_end(&mut buffer)
                .await?;
        }
        let truncated = u64::try_from(buffer.len()).unwrap_or(u64::MAX) >= MAX_PROBE_STDOUT_BYTES;
        if truncated {
            // Kill before waiting so a probe flooding the pipe cannot deadlock.
            child.kill().await?;
        }
        let status = child.wait().await?;
        Ok::<_, Error>((status, buffer, truncated))
    })
    .await;

    let (status, buffer, truncated) = match read {
        Ok(Ok(output)) => output,
        Ok(Err(err)) => return Err(io_fault(err)),
        Err(_elapsed) => {
            log_fault("timeout");
            return Err(MediaProbeError::Timeout);
        }
    };

    if truncated {
        log_validation_failure("output_too_large");
        return Err(MediaProbeError::OperationFailed);
    }
    if !status.success() {
        // A valid binary that exits non-zero on existing bytes reports input it
        // cannot decode, a client rejection rather than a dependency fault.
        log_validation_failure("exit_status");
        return Err(MediaProbeError::UnsupportedMedia);
    }

    Ok(Some(parse_video(&buffer)?))
}

/// Parse the `ffprobe` JSON document `buffer` into [`ProbedVideo`].
///
/// # Errors
///
/// Returns [`MediaProbeError::UnsupportedMedia`] when the document carries no
/// usable video stream, and [`MediaProbeError::OperationFailed`] when the
/// document is malformed.
#[expect(
    clippy::single_call_fn,
    reason = "the output parsing is named after the step it performs"
)]
fn parse_video(buffer: &[u8]) -> Result<ProbedVideo, MediaProbeError> {
    let output: ProbeOutput = serde_json::from_slice(buffer).map_err(|_| {
        log_validation_failure("malformed_output");
        MediaProbeError::OperationFailed
    })?;

    let streams = output.streams.unwrap_or_default();
    let Some(stream) = streams
        .iter()
        .find(|stream| stream.codec_type.as_deref() == Some("video"))
    else {
        log_validation_failure("unsupported_media");
        return Err(MediaProbeError::UnsupportedMedia);
    };

    let Some(width) = stream.width.map(i64::from).filter(|value| *value > 0) else {
        log_validation_failure("unsupported_media");
        return Err(MediaProbeError::UnsupportedMedia);
    };
    let Some(height) = stream.height.map(i64::from).filter(|value| *value > 0) else {
        log_validation_failure("unsupported_media");
        return Err(MediaProbeError::UnsupportedMedia);
    };
    let Some(codec) = stream.codec_name.clone().filter(|name| !name.is_empty()) else {
        log_validation_failure("unsupported_media");
        return Err(MediaProbeError::UnsupportedMedia);
    };

    let duration_s = output
        .format
        .as_ref()
        .and_then(|format| format.duration.as_deref())
        .or(stream.duration.as_deref())
        .and_then(parse_finite)
        .unwrap_or(0.0f64);
    let frames_per_second = stream
        .avg_frame_rate
        .as_deref()
        .or(stream.r_frame_rate.as_deref())
        .map_or(0.0f64, parse_frames_per_second);

    let frame_count = stream
        .nb_frames
        .as_deref()
        .and_then(|frames| frames.parse::<i64>().ok())
        .filter(|frames| *frames > 0)
        .or_else(|| frames_from_rate(duration_s, frames_per_second))
        .ok_or_else(|| {
            log_validation_failure("missing_frame_count");
            MediaProbeError::UnsupportedMedia
        })?;

    Ok(ProbedVideo {
        codec,
        color_id: color_id(stream.pix_fmt.as_deref()),
        duration_ms: duration_s * MILLIS_PER_SECOND,
        frame_count,
        height,
        scan_type: scan_type(stream.field_order.as_deref()),
        width,
    })
}

/// Parse a non-negative, finite floating-point value, or `None`.
fn parse_finite(value: &str) -> Option<f64> {
    value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|parsed| parsed.is_finite() && *parsed >= 0.0f64)
}

/// Parse an `ffprobe` frame rate (`"30000/1001"`) into frames per second.
#[expect(
    clippy::single_call_fn,
    reason = "the frame-rate parsing is named after the grammar it decodes"
)]
fn parse_frames_per_second(rate: &str) -> f64 {
    let mut parts = rate.split('/');
    let numerator = parts.next().and_then(parse_finite).unwrap_or(0.0f64);
    let denominator = parts.next().and_then(parse_finite).unwrap_or(1.0f64);
    if denominator < f64::EPSILON {
        0.0f64
    } else {
        numerator / denominator
    }
}

/// Derive a frame count from `duration_s` and `frames_per_second`.
///
/// Returns `None` when either input is unusable or the product does not reach a
/// single frame.
#[expect(
    clippy::single_call_fn,
    reason = "the frame-count fallback is named after the rule it encodes"
)]
fn frames_from_rate(duration_s: f64, frames_per_second: f64) -> Option<i64> {
    if frames_per_second <= 0.0f64 {
        return None;
    }
    let frames = (duration_s * frames_per_second).round();
    if frames < 1.0f64 {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the rounded frame count is bounded by the video's duration"
    )]
    Some(frames as i64)
}

/// Map an `ffprobe` pixel format onto a seeded `color` identifier.
///
/// The mapping follows the accepted design: `yuv*`/`nv12` are `YCbCr`,
/// `gray*` is `Grayscale`, and `rgb*`/`bgr*` are `sRGB`; every other format
/// defaults to `YCbCr`, the common video colour space.
#[expect(
    clippy::single_call_fn,
    reason = "the colour-space mapping is named after the rule it encodes"
)]
fn color_id(pix_fmt: Option<&str>) -> String {
    let Some(pixel_format) = pix_fmt else {
        return DEFAULT_COLOR_ID.to_owned();
    };
    if pixel_format.starts_with("yuv") || pixel_format.starts_with("nv12") {
        "YCbCr"
    } else if pixel_format.starts_with("gray") {
        "Grayscale"
    } else if pixel_format.starts_with("rgb") || pixel_format.starts_with("bgr") {
        "sRGB"
    } else {
        DEFAULT_COLOR_ID
    }
    .to_owned()
}

/// Map an `ffprobe` `field_order` value onto a [`ScanType`].
///
/// A field-first order is interlaced; `progressive`, absent or unknown values
/// are progressive.
#[expect(
    clippy::single_call_fn,
    reason = "the scan-type mapping is named after the rule it encodes"
)]
fn scan_type(field_order: Option<&str>) -> ScanType {
    match field_order {
        Some("tt" | "bb" | "tb" | "bt") => ScanType::Interlaced,
        _ => ScanType::Progressive,
    }
}

/// Return the process-wide semaphore bounding concurrent video probes.
#[expect(
    clippy::single_call_fn,
    reason = "the bounded-probe semaphore is named after the limit it enforces"
)]
fn probe_permits() -> &'static Semaphore {
    static PERMITS: OnceLock<Semaphore> = OnceLock::new();
    PERMITS.get_or_init(|| Semaphore::new(MAX_CONCURRENT_PROBES))
}

/// Emit the catalogued `output_validation_failed` (`warn`) for stored bytes
/// that do not decode (`OBS-005`, `OBS-006`).
fn log_validation_failure(reason: &str) {
    warn!(
        target: "security",
        event = "output_validation_failed",
        reason = %reason,
        "stored data failed to decode"
    );
}

/// Emit the catalogued `port_fault` (`error`) for a classified probe failure.
fn log_fault(kind: &str) {
    error!(
        target: "security",
        event = "port_fault",
        kind = %kind,
        operation = OPERATION,
        "an outbound dependency failed"
    );
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::time::Duration;

    use tempfile::tempdir;

    use crate::domain::model::image::Orientation;
    use crate::domain::model::video::ScanType;
    use crate::domain::port::error::MediaProbeError;
    use crate::domain::port::media_probe::MediaProbe as _;

    use super::LocalMediaProbe;

    /// A generous probe timeout so a loaded test runner cannot starve a stub.
    const STUB_TIMEOUT: Duration = Duration::from_secs(10);

    /// Write an executable `script` into `dir` and return its path.
    #[cfg(unix)]
    fn stub(dir: &Path, name: &str, body: &str) -> Result<PathBuf, Box<dyn Error>> {
        use std::os::unix::fs::PermissionsExt as _;

        let path = dir.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n"))?;
        let mut permissions = fs::metadata(&path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions)?;
        Ok(path)
    }

    #[tokio::test]
    async fn probe_image_reads_landscape_png() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let path = tmp.path().join("wide.png");
        image::RgbImage::from_pixel(4, 2, image::Rgb([10, 20, 30])).save(&path)?;
        let probe = LocalMediaProbe::new();

        // Act
        let probed = probe
            .probe_image(&path)
            .await?
            .ok_or("the image should exist")?;

        // Assert
        assert_eq!(probed.width_px, 4);
        assert_eq!(probed.height_px, 2);
        assert_eq!(probed.orientation, Orientation::Landscape);
        Ok(())
    }

    #[tokio::test]
    async fn probe_image_reads_portrait_jpeg() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let path = tmp.path().join("tall.jpg");
        image::RgbImage::from_pixel(2, 5, image::Rgb([1, 2, 3])).save(&path)?;
        let probe = LocalMediaProbe::new();

        // Act
        let probed = probe
            .probe_image(&path)
            .await?
            .ok_or("the image should exist")?;

        // Assert
        assert_eq!(probed.width_px, 2);
        assert_eq!(probed.height_px, 5);
        assert_eq!(probed.orientation, Orientation::Portrait);
        Ok(())
    }

    #[tokio::test]
    async fn probe_image_missing_file_returns_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let probe = LocalMediaProbe::new();

        // Act
        let probed = probe.probe_image(&tmp.path().join("absent.png")).await?;

        // Assert
        assert!(probed.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn probe_image_malformed_file_returns_error() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let path = tmp.path().join("broken.png");
        fs::write(&path, b"definitely not an image")?;
        let probe = LocalMediaProbe::new();

        // Act
        let result = probe.probe_image(&path).await;

        // Assert
        assert!(matches!(result, Err(MediaProbeError::UnsupportedMedia)));
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn probe_video_maps_ffprobe_json() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let program = stub(
            tmp.path(),
            "ffprobe",
            "cat <<'JSON'\n{\"streams\":[{\"codec_type\":\"video\",\"codec_name\":\"h264\",\"width\":640,\"height\":480,\"pix_fmt\":\"yuv420p\",\"field_order\":\"tt\",\"nb_frames\":\"48\"}],\"format\":{\"duration\":\"2.0\"}}\nJSON",
        )?;
        let media = tmp.path().join("clip.mp4");
        fs::write(&media, b"stub")?;
        let probe = LocalMediaProbe::with_program(program, STUB_TIMEOUT);

        // Act
        let probed = probe
            .probe_video(&media)
            .await?
            .ok_or("the video should exist")?;

        // Assert
        assert_eq!(probed.codec, "h264");
        assert_eq!(probed.width, 640);
        assert_eq!(probed.height, 480);
        assert_eq!(probed.color_id, "YCbCr");
        assert_eq!(probed.scan_type, ScanType::Interlaced);
        assert_eq!(probed.frame_count, 48);
        assert!((probed.duration_ms - 2000.0f64).abs() < f64::EPSILON);
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn probe_video_derives_frame_count_when_nb_frames_is_absent() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let tmp = tempdir()?;
        let program = stub(
            tmp.path(),
            "ffprobe",
            "cat <<'JSON'\n{\"streams\":[{\"codec_type\":\"video\",\"codec_name\":\"vp9\",\"width\":100,\"height\":50,\"pix_fmt\":\"gray8\",\"field_order\":\"progressive\",\"avg_frame_rate\":\"10/1\",\"duration\":\"3.0\"}]}\nJSON",
        )?;
        let media = tmp.path().join("clip.webm");
        fs::write(&media, b"stub")?;
        let probe = LocalMediaProbe::with_program(program, STUB_TIMEOUT);

        // Act
        let probed = probe
            .probe_video(&media)
            .await?
            .ok_or("the video should exist")?;

        // Assert
        assert_eq!(probed.frame_count, 30);
        assert_eq!(probed.color_id, "Grayscale");
        assert_eq!(probed.scan_type, ScanType::Progressive);
        Ok(())
    }

    #[tokio::test]
    async fn probe_video_missing_file_returns_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let probe = LocalMediaProbe::new();

        // Act
        let probed = probe.probe_video(&tmp.path().join("absent.mp4")).await?;

        // Assert
        assert!(probed.is_none());
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn probe_video_non_zero_exit_returns_unsupported_media() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let program = stub(tmp.path(), "ffprobe", "exit 1")?;
        let media = tmp.path().join("clip.mp4");
        fs::write(&media, b"stub")?;
        let probe = LocalMediaProbe::with_program(program, STUB_TIMEOUT);

        // Act
        let result = probe.probe_video(&media).await;

        // Assert
        assert!(matches!(result, Err(MediaProbeError::UnsupportedMedia)));
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn probe_video_malformed_output_returns_operation_failed() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let program = stub(tmp.path(), "ffprobe", "echo not-json")?;
        let media = tmp.path().join("clip.mp4");
        fs::write(&media, b"stub")?;
        let probe = LocalMediaProbe::with_program(program, STUB_TIMEOUT);

        // Act
        let result = probe.probe_video(&media).await;

        // Assert
        assert!(matches!(result, Err(MediaProbeError::OperationFailed)));
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn probe_video_no_video_stream_returns_unsupported_media() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let program = stub(
            tmp.path(),
            "ffprobe",
            "cat <<'JSON'\n{\"streams\":[{\"codec_type\":\"audio\",\"codec_name\":\"aac\"}],\"format\":{\"duration\":\"1.0\"}}\nJSON",
        )?;
        let media = tmp.path().join("clip.mp4");
        fs::write(&media, b"stub")?;
        let probe = LocalMediaProbe::with_program(program, STUB_TIMEOUT);

        // Act
        let result = probe.probe_video(&media).await;

        // Assert
        assert!(matches!(result, Err(MediaProbeError::UnsupportedMedia)));
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn probe_video_timeout_returns_timeout() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let program = stub(tmp.path(), "ffprobe", "sleep 10")?;
        let media = tmp.path().join("clip.mp4");
        fs::write(&media, b"stub")?;
        let probe = LocalMediaProbe::with_program(program, Duration::from_millis(200));

        // Act
        let result = probe.probe_video(&media).await;

        // Assert
        assert!(matches!(result, Err(MediaProbeError::Timeout)));
        Ok(())
    }

    #[tokio::test]
    async fn probe_video_missing_binary_returns_operation_failed() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let media = tmp.path().join("clip.mp4");
        fs::write(&media, b"stub")?;
        let missing = tmp.path().join("no-such-ffprobe");
        let probe = LocalMediaProbe::with_program(missing, STUB_TIMEOUT);

        // Act
        let result = probe.probe_video(&media).await;

        // Assert
        assert!(matches!(result, Err(MediaProbeError::OperationFailed)));
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn probe_video_passes_the_path_as_one_argument_without_a_shell()
    -> Result<(), Box<dyn Error>> {
        // Arrange: a path carrying shell metacharacters, and a stub recording
        // each received argument on its own line.
        let tmp = tempdir()?;
        let program = stub(
            tmp.path(),
            "ffprobe",
            "printf '%s\\n' \"$@\" > \"$(dirname \"$0\")/args.txt\"\ncat <<'JSON'\n{\"streams\":[{\"codec_type\":\"video\",\"codec_name\":\"h264\",\"width\":10,\"height\":10,\"nb_frames\":\"1\"}]}\nJSON",
        )?;
        let media = tmp.path().join("clip; touch pwned &.mp4");
        fs::write(&media, b"stub")?;
        let probe = LocalMediaProbe::with_program(program, STUB_TIMEOUT);

        // Act
        let probed = probe
            .probe_video(&media)
            .await?
            .ok_or("the video should exist")?;
        assert_eq!(probed.width, 10);
        let recorded = fs::read_to_string(tmp.path().join("args.txt"))?;

        // Assert: the path is exactly one argv element; a shell would have split
        // it on the semicolon and ampersand.
        assert!(
            recorded
                .lines()
                .any(|arg| arg == media.to_string_lossy().as_ref()),
            "the probe must receive the path as one argument: {recorded}"
        );
        Ok(())
    }
}
