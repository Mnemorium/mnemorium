use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDate;

use crate::domain::NumericID;
use crate::domain::model::Orientation;

/// Command to fetch one typed image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct GetImageCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Identifier of the image to fetch.
    image_id: NumericID,
}

impl GetImageCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the identifier of the image to fetch.
    #[must_use]
    pub(crate) fn image_id(&self) -> NumericID {
        self.image_id
    }

    /// Create a new fetch-image command.
    #[must_use]
    pub(crate) fn new(caller_id: NumericID, image_id: NumericID) -> Self {
        Self {
            caller_id,
            image_id,
        }
    }
}

/// Response of a successful image lookup: one image and its backing file.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct GetImageResponse {
    /// Date at which the image was created.
    created_at: NaiveDate,
    /// Identifier of the backing file.
    file_id: NumericID,
    /// Height of the image, in pixels.
    height_px: i64,
    /// Unique identifier of the image.
    id: NumericID,
    /// Name of the image.
    name: String,
    /// Orientation of the image.
    orientation: Orientation,
    /// Width of the image, in pixels.
    width_px: i64,
}

impl GetImageResponse {
    /// Return the date at which the image was created.
    #[must_use]
    pub(crate) fn created_at(&self) -> NaiveDate {
        self.created_at
    }

    /// Return the identifier of the backing file.
    #[must_use]
    pub(crate) fn file_id(&self) -> NumericID {
        self.file_id
    }

    /// Return the height of the image, in pixels.
    #[must_use]
    pub(crate) fn height_px(&self) -> i64 {
        self.height_px
    }

    /// Return the unique identifier of the image.
    #[must_use]
    pub(crate) fn id(&self) -> NumericID {
        self.id
    }

    /// Return the name of the image.
    #[must_use]
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Create a new fetch-image response.
    #[must_use]
    pub(crate) fn new(
        id: NumericID,
        name: String,
        width_px: i64,
        height_px: i64,
        orientation: Orientation,
        created_at: NaiveDate,
        file_id: NumericID,
    ) -> Self {
        Self {
            created_at,
            file_id,
            height_px,
            id,
            name,
            orientation,
            width_px,
        }
    }

    /// Return the orientation of the image.
    #[must_use]
    pub(crate) fn orientation(&self) -> Orientation {
        self.orientation
    }

    /// Return the width of the image, in pixels.
    #[must_use]
    pub(crate) fn width_px(&self) -> i64 {
        self.width_px
    }
}

/// Error returned when fetching a typed image fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum GetImageError {
    /// The caller does not exist.
    #[error("the caller does not exist")]
    NoSuchCaller,
    /// The image does not exist, or the caller may not read it.
    ///
    /// Both outcomes are reported the same way so a caller cannot probe for
    /// images it may not read.
    #[error("the image does not exist")]
    NoSuchImage,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case fetching one typed image.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait GetImageUseCase: Send + Sync {
    /// Fetch the requested image.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as `Arc<dyn GetImageUseCase>`.
    fn execute<'future>(
        &'future self,
        command: GetImageCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetImageResponse, GetImageError>> + Send + 'future>>;
}
