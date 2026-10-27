use crate::domain::alias::NumericID;
use crate::domain::model::video::ScanType;
use crate::domain::model::video::Video;

/// The video backing a film or an episode, projected to the fields the
/// representations expose.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct VideoSummary {
    /// Codec of the video.
    codec: String,
    /// Duration of the video, in milliseconds.
    duration_ms: f64,
    /// Identifier of the backing file.
    file_id: NumericID,
    /// Height of the video, in pixels.
    height: i64,
    /// Unique identifier of the video.
    id: NumericID,
    /// Scan type of the video.
    scan_type: ScanType,
    /// Width of the video, in pixels.
    width: i64,
}

impl VideoSummary {
    /// Return the codec of the video.
    #[must_use]
    pub fn codec(&self) -> &str {
        &self.codec
    }

    /// Return the duration of the video, in milliseconds.
    #[must_use]
    pub fn duration_ms(&self) -> f64 {
        self.duration_ms
    }

    /// Return the identifier of the backing file.
    #[must_use]
    pub fn file_id(&self) -> NumericID {
        self.file_id
    }

    /// Return the height of the video, in pixels.
    #[must_use]
    pub fn height(&self) -> i64 {
        self.height
    }

    /// Return the unique identifier of the video.
    #[must_use]
    pub fn id(&self) -> NumericID {
        self.id
    }

    /// Return the scan type of the video.
    #[must_use]
    pub fn scan_type(&self) -> ScanType {
        self.scan_type
    }

    /// Return the width of the video, in pixels.
    #[must_use]
    pub fn width(&self) -> i64 {
        self.width
    }
}

impl From<&Video> for VideoSummary {
    fn from(video: &Video) -> Self {
        Self {
            codec: video.codec().to_owned(),
            duration_ms: video.duration_ms(),
            file_id: video.file_id(),
            height: video.height(),
            id: video.video_id(),
            scan_type: video.scan_type(),
            width: video.width(),
        }
    }
}
