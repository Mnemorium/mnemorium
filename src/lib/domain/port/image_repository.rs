use std::future::Future;

use crate::domain::NumericID;
use crate::domain::model::{Image, Orientation};
use crate::domain::port::RepositoryError;

/// Search filters for [`ImageRepository::search`]. Every field is optional; an
/// all-`None` filter returns every image.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct ImageFilter {
    /// Filter on the identifier of the backing file.
    pub file_id: Option<NumericID>,
    /// Filter on the image identifier.
    pub image_id: Option<NumericID>,
    /// Filter on the image name.
    pub name: Option<String>,
    /// Filter on the image orientation.
    pub orientation: Option<Orientation>,
}

/// Port for persisting and querying `Image`.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait ImageRepository: Send + Sync {
    /// Insert a new `image`, returning the persisted image with its final
    /// identifier.
    ///
    /// The identifier of `image` is ignored: the repository assigns a fresh
    /// identity.
    fn create(
        &mut self,
        image: Image,
    ) -> impl Future<Output = Result<Image, RepositoryError>> + Send;

    /// Delete every image matching `filter`.
    ///
    /// Returns `Ok(true)` when at least one image was deleted, and `Ok(false)`
    /// when none matched. An all-`None` filter deletes every image.
    ///
    /// The resources carrying the image as their cover are retained: the
    /// database clears their cover reference, so an image removal never
    /// deletes a music group, album, playlist, film, show or season.
    fn delete(
        &mut self,
        filter: &ImageFilter,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Search images matching `filter`, returned as `Vec<Image>`.
    ///
    /// Returns an empty list when no image matches; a missing match is a valid
    /// outcome, not an error.
    fn search(
        &mut self,
        filter: &ImageFilter,
    ) -> impl Future<Output = Result<Vec<Image>, RepositoryError>> + Send;
}

#[cfg(test)]
impl<T: ImageRepository + ?Sized> ImageRepository for &mut T {
    fn create(
        &mut self,
        image: Image,
    ) -> impl Future<Output = Result<Image, RepositoryError>> + Send {
        (**self).create(image)
    }

    fn delete(
        &mut self,
        filter: &ImageFilter,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).delete(filter)
    }

    fn search(
        &mut self,
        filter: &ImageFilter,
    ) -> impl Future<Output = Result<Vec<Image>, RepositoryError>> + Send {
        (**self).search(filter)
    }
}
