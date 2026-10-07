use chrono::NaiveDateTime;

use crate::domain::alias::NumericID;

/// Data model for the `gallery_item` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GalleryItem {
    /// Date and time at which the item was added.
    pub added_at: NaiveDateTime,
    /// Identifier of the gallery the item belongs to.
    pub gallery_id: NumericID,
    /// Unique identifier of the item.
    #[sqlx(primary_key)]
    pub gallery_item_id: NumericID,
    /// Identifier of the referenced image, when the item shows an image.
    pub image_id: Option<NumericID>,
    /// Position of the item within its gallery.
    pub item_index: i64,
    /// Identifier of the referenced video, when the item shows a video.
    pub video_id: Option<NumericID>,
}

/// Read-model row for a gallery item joined to its medium and backing file.
///
/// Assembled from `gallery_item LEFT JOIN image/video LEFT JOIN file`; `name`
/// falls back to the file path for a video, and `file_id` is the identifier of
/// the backing file whichever medium the item references.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GalleryItemDetail {
    /// Date and time at which the item was added.
    pub added_at: NaiveDateTime,
    /// Identifier of the backing file.
    pub file_id: NumericID,
    /// Identifier of the gallery the item belongs to.
    pub gallery_id: NumericID,
    /// Unique identifier of the item.
    #[sqlx(primary_key)]
    pub gallery_item_id: NumericID,
    /// Identifier of the referenced image, when the item shows an image.
    pub image_id: Option<NumericID>,
    /// Position of the item within its gallery.
    pub item_index: i64,
    /// Name of the item: the image name, or the file path for a video.
    pub name: String,
    /// Identifier of the referenced video, when the item shows a video.
    pub video_id: Option<NumericID>,
}
