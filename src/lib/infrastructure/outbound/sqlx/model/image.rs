use chrono::NaiveDate;

use crate::domain::alias::NumericID;

/// Orientation of an image, matching the `chk_image_orientation` check
/// constraint in the `image` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "UPPERCASE")]
#[non_exhaustive]
pub enum Orientation {
    /// The width is greater than the height.
    Landscape,
    /// The height is greater than the width.
    Portrait,
    /// The width equals the height.
    Square,
}

/// Data model for the `image` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Image {
    /// Date at which the image was created.
    pub created_at: NaiveDate,
    /// Identifier of the backing file.
    pub file_id: NumericID,
    /// Height of the image, in pixels.
    pub height_px: i64,
    /// Unique identifier of the image.
    #[sqlx(primary_key)]
    pub image_id: NumericID,
    /// Name of the image.
    pub name: String,
    /// Orientation of the image.
    pub orientation: Orientation,
    /// Width of the image, in pixels.
    pub width_px: i64,
}
