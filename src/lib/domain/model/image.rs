use chrono::NaiveDate;

use crate::domain::alias::NumericID;

/// Error returned when initialising an `Image`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ImageError {
    /// The height is not positive.
    #[error("image height must be greater than zero")]
    InvalidHeightPx,
    /// The name is empty.
    #[error("image name must not be empty")]
    InvalidName,
    /// The width is not positive.
    #[error("image width must be greater than zero")]
    InvalidWidthPx,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Orientation of an image.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize, utoipa::ToSchema,
)]
#[serde(rename_all = "UPPERCASE")]
#[non_exhaustive]
pub enum Orientation {
    /// The width is greater than the height.
    Landscape,
    /// The height is greater than the width.
    Portrait,
    /// The width equals the height.
    Square,
}

/// An image stored on the server.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::struct_field_names,
    reason = "`image_id` is the primary key name mandated by the SQL section and the domain field naming"
)]
pub struct Image {
    /// Date at which the image was created.
    created_at: NaiveDate,
    /// Identifier of the backing file.
    file_id: NumericID,
    /// Height of the image, in pixels.
    height_px: i64,
    /// Unique identifier of the image.
    image_id: NumericID,
    /// Name of the image.
    name: String,
    /// Orientation of the image.
    orientation: Orientation,
    /// Width of the image, in pixels.
    width_px: i64,
}

impl Image {
    /// Return the date at which the image was created.
    #[must_use]
    pub fn created_at(&self) -> NaiveDate {
        self.created_at
    }

    /// Return the identifier of the backing file.
    #[must_use]
    pub fn file_id(&self) -> NumericID {
        self.file_id
    }

    /// Return the height of the image, in pixels.
    #[must_use]
    pub fn height_px(&self) -> i64 {
        self.height_px
    }

    /// Return the unique identifier of the image.
    #[must_use]
    pub fn image_id(&self) -> NumericID {
        self.image_id
    }

    /// Return the name of the image.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the orientation of the image.
    #[must_use]
    pub fn orientation(&self) -> Orientation {
        self.orientation
    }

    /// Initialise a new `Image`, validating `name`, `width_px` and `height_px`.
    ///
    /// # Errors
    ///
    /// Returns [`ImageError::InvalidName`] when `name` is empty,
    /// [`ImageError::InvalidWidthPx`] when `width_px` is not positive, and
    /// [`ImageError::InvalidHeightPx`] when `height_px` is not positive.
    pub fn try_new(
        image_id: NumericID,
        name: String,
        width_px: i64,
        height_px: i64,
        orientation: Orientation,
        created_at: NaiveDate,
        file_id: NumericID,
    ) -> Result<Self, ImageError> {
        if name.is_empty() {
            return Err(ImageError::InvalidName);
        }
        if width_px <= 0 {
            return Err(ImageError::InvalidWidthPx);
        }
        if height_px <= 0 {
            return Err(ImageError::InvalidHeightPx);
        }
        Ok(Self {
            created_at,
            file_id,
            height_px,
            image_id,
            name,
            orientation,
            width_px,
        })
    }

    /// Return the width of the image, in pixels.
    #[must_use]
    pub fn width_px(&self) -> i64 {
        self.width_px
    }
}
