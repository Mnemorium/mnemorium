use crate::domain::alias::NumericID;
use crate::domain::model::video::ScanType as DomainScanType;

/// Scan type of a video, matching the `chk_video_scan_type` check constraint in
/// the `video` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "UPPERCASE")]
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

/// Data model for the `video` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Video {
    /// Codec of the video.
    pub codec: String,
    /// Identifier of the colour space of the video.
    pub color_id: String,
    /// Duration of the video, in milliseconds.
    pub duration_ms: f64,
    /// Identifier of the backing file.
    pub file_id: NumericID,
    /// Number of frames in the video.
    pub frame_count: i64,
    /// Height of the video, in pixels.
    pub height: i64,
    /// Scan type of the video.
    pub scan_type: ScanType,
    /// Unique identifier of the video.
    #[sqlx(primary_key)]
    pub video_id: NumericID,
    /// Width of the video, in pixels.
    pub width: i64,
}

/// Parse a stored scan-type token into its domain value.
///
/// Returns `None` when `value` is not one of the tokens the
/// `chk_video_scan_type` check constraint admits, so an adapter can report a
/// value the domain cannot hold as a data-integrity failure rather than serving
/// it.
#[must_use]
pub(crate) fn parse_scan_type(value: &str) -> Option<DomainScanType> {
    match value {
        "INTERLACED" => Some(DomainScanType::Interlaced),
        "MBAFF" => Some(DomainScanType::Mbaff),
        "PAFF" => Some(DomainScanType::Paff),
        "PROGRESSIVE" => Some(DomainScanType::Progressive),
        _ => None,
    }
}
