use chrono::NaiveDateTime;

use crate::domain::alias::NumericID;

/// Error returned when initialising a `GalleryItem`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GalleryItemError {
    /// The item index is negative.
    #[error("gallery item index must not be negative")]
    InvalidIndex,
    /// The item does not reference exactly one medium.
    #[error("a gallery item must reference exactly one medium")]
    InvalidMedia,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// The single medium a `GalleryItem` references.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum GalleryItemMedia {
    /// The item references an image.
    Image(NumericID),
    /// The item references a video.
    Video(NumericID),
}

impl GalleryItemMedia {
    /// Return the identifier of the referenced medium.
    #[must_use]
    pub fn media_id(&self) -> NumericID {
        match *self {
            Self::Image(media_id) | Self::Video(media_id) => media_id,
        }
    }
}

/// An ordered membership of one medium in a `Gallery`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::struct_field_names,
    reason = "`gallery_item_id` is the primary key name mandated by the SQL section and the domain field naming"
)]
pub struct GalleryItem {
    /// Date and time at which the item was added.
    added_at: NaiveDateTime,
    /// Identifier of the gallery the item belongs to.
    gallery_id: NumericID,
    /// Unique identifier of the item.
    gallery_item_id: NumericID,
    /// Position of the item within its gallery.
    item_index: i64,
    /// Medium the item references.
    media: GalleryItemMedia,
}

impl GalleryItem {
    /// Return the date and time at which the item was added.
    #[must_use]
    pub fn added_at(&self) -> NaiveDateTime {
        self.added_at
    }

    /// Return the identifier of the gallery the item belongs to.
    #[must_use]
    pub fn gallery_id(&self) -> NumericID {
        self.gallery_id
    }

    /// Return the unique identifier of the item.
    #[must_use]
    pub fn gallery_item_id(&self) -> NumericID {
        self.gallery_item_id
    }

    /// Return the position of the item within its gallery.
    #[must_use]
    pub fn item_index(&self) -> i64 {
        self.item_index
    }

    /// Return the medium the item references.
    #[must_use]
    pub fn media(&self) -> GalleryItemMedia {
        self.media
    }

    /// Initialise a new `GalleryItem`, validating that exactly one medium is
    /// referenced and that `item_index` is non-negative.
    ///
    /// # Errors
    ///
    /// Returns [`GalleryItemError::InvalidMedia`] when both or neither of
    /// `image_id` and `video_id` are set, and
    /// [`GalleryItemError::InvalidIndex`] when `item_index` is negative.
    pub fn try_new(
        gallery_item_id: NumericID,
        gallery_id: NumericID,
        image_id: Option<NumericID>,
        video_id: Option<NumericID>,
        item_index: i64,
        added_at: NaiveDateTime,
    ) -> Result<Self, GalleryItemError> {
        let media = match (image_id, video_id) {
            (Some(image), None) => GalleryItemMedia::Image(image),
            (None, Some(video)) => GalleryItemMedia::Video(video),
            (Some(_), Some(_)) | (None, None) => return Err(GalleryItemError::InvalidMedia),
        };
        if item_index < 0 {
            return Err(GalleryItemError::InvalidIndex);
        }
        Ok(Self {
            added_at,
            gallery_id,
            gallery_item_id,
            item_index,
            media,
        })
    }
}
