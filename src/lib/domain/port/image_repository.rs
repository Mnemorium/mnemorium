use std::future::Future;

use crate::domain::alias::NumericID;
use crate::domain::model::image::Image;
use crate::domain::model::image::Orientation;
use crate::domain::port::error::RepositoryError;

/// Search filters for [`ImageRepository::search`]. Every field is optional; an
/// all-`None` filter returns every image.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ImageFilter {
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
pub trait ImageRepository: Send + Sync {
    /// Insert a new `image`, returning the persisted image with its final
    /// identifier.
    ///
    /// The identifier of `image` is ignored: the repository assigns a fresh
    /// identity.
    fn create(
        &mut self,
        image: Image,
    ) -> impl Future<Output = Result<Image, RepositoryError>> + Send;

    /// Delete the image identified by `id`.
    ///
    /// Returns `Ok(true)` when an image matched `id` and was deleted, and
    /// `Ok(false)` when no image matched. A missing image is a valid outcome,
    /// not an error.
    fn delete(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Return the image identified by `id`, if any.
    ///
    /// A missing image is `Ok(None)`, not an error.
    fn get(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<Option<Image>, RepositoryError>> + Send;

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
        id: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).delete(id)
    }

    fn get(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<Option<Image>, RepositoryError>> + Send {
        (**self).get(id)
    }

    fn search(
        &mut self,
        filter: &ImageFilter,
    ) -> impl Future<Output = Result<Vec<Image>, RepositoryError>> + Send {
        (**self).search(filter)
    }
}
