use crate::domain::alias::NumericID;

/// Error returned when initialising a `Video`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum VideoError {
    /// The frame count is not positive.
    #[error("video frame count must be greater than zero")]
    InvalidFrameCount,
    /// The height is not positive.
    #[error("video height must be greater than zero")]
    InvalidHeight,
    /// The width is not positive.
    #[error("video width must be greater than zero")]
    InvalidWidth,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Scan type of a video stream.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize, utoipa::ToSchema,
)]
#[serde(rename_all = "UPPERCASE")]
#[non_exhaustive]
pub enum ScanType {
    /// Interlaced scan of two separate fields per frame.
    Interlaced,
    /// Macroblock-adaptive frame/field scan.
    Mbaff,
    /// Picture-adaptive frame/field scan.
    Paff,
    /// Progressive scan of one frame at a time.
    Progressive,
}

/// A video stored on the server.
#[derive(Debug, Clone, PartialEq)]
#[expect(
    clippy::struct_field_names,
    reason = "`video_id` is the primary key name mandated by the SQL section and the domain field naming"
)]
pub struct Video {
    /// Codec of the video.
    codec: String,
    /// Identifier of the colour space of the video.
    color_id: String,
    /// Duration of the video, in milliseconds.
    duration_ms: f64,
    /// Identifier of the backing file.
    file_id: NumericID,
    /// Number of frames in the video.
    frame_count: i64,
    /// Height of the video, in pixels.
    height: i64,
    /// Scan type of the video.
    scan_type: ScanType,
    /// Unique identifier of the video.
    video_id: NumericID,
    /// Width of the video, in pixels.
    width: i64,
}

impl Video {
    /// Return the codec of the video.
    #[must_use]
    pub fn codec(&self) -> &str {
        &self.codec
    }

    /// Return the identifier of the colour space of the video.
    #[must_use]
    pub fn color_id(&self) -> &str {
        &self.color_id
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

    /// Return the number of frames in the video.
    #[must_use]
    pub fn frame_count(&self) -> i64 {
        self.frame_count
    }

    /// Return the height of the video, in pixels.
    #[must_use]
    pub fn height(&self) -> i64 {
        self.height
    }

    /// Return the scan type of the video.
    #[must_use]
    pub fn scan_type(&self) -> ScanType {
        self.scan_type
    }

    /// Initialise a new `Video`, validating `frame_count`, `width` and
    /// `height`.
    ///
    /// # Errors
    ///
    /// Returns [`VideoError::InvalidFrameCount`] when `frame_count` is not
    /// positive, [`VideoError::InvalidWidth`] when `width` is not positive, and
    /// [`VideoError::InvalidHeight`] when `height` is not positive.
    #[expect(
        clippy::too_many_arguments,
        reason = "try_new mirrors every video column"
    )]
    pub fn try_new(
        video_id: NumericID,
        duration_ms: f64,
        codec: String,
        frame_count: i64,
        width: i64,
        height: i64,
        color_id: String,
        scan_type: ScanType,
        file_id: NumericID,
    ) -> Result<Self, VideoError> {
        if frame_count <= 0 {
            return Err(VideoError::InvalidFrameCount);
        }
        if width <= 0 {
            return Err(VideoError::InvalidWidth);
        }
        if height <= 0 {
            return Err(VideoError::InvalidHeight);
        }
        Ok(Self {
            codec,
            color_id,
            duration_ms,
            file_id,
            frame_count,
            height,
            scan_type,
            video_id,
            width,
        })
    }

    /// Return the unique identifier of the video.
    #[must_use]
    pub fn video_id(&self) -> NumericID {
        self.video_id
    }

    /// Return the width of the video, in pixels.
    #[must_use]
    pub fn width(&self) -> i64 {
        self.width
    }
}
