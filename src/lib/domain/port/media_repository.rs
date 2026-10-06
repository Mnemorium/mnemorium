use std::future::Future;

use crate::domain::alias::NumericID;
use crate::domain::model::image::Image;
use crate::domain::model::image::Orientation;
use crate::domain::model::video::ScanType;
use crate::domain::model::video::Video;
use crate::domain::port::error::RepositoryError;

/// Search filters for [`MediaRepository::search_images`]. Every field is
/// optional; an all-`None` filter returns every image.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ImageFilter {
    /// Filter on the identifier of the backing file.
    pub file_id: Option<NumericID>,
    /// Filter on the image identifier.
    pub id: Option<NumericID>,
    /// Filter on the image name.
    pub name: Option<String>,
    /// Filter on the image orientation.
    pub orientation: Option<Orientation>,
}

/// Search filters for [`MediaRepository::search_videos`]. Every field is
/// optional; an all-`None` filter returns every video.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct VideoFilter {
    /// Filter on the video codec.
    pub codec: Option<String>,
    /// Filter on the identifier of the colour space.
    pub color_id: Option<String>,
    /// Filter on the identifier of the backing file.
    pub file_id: Option<NumericID>,
    /// Filter on the video identifier.
    pub id: Option<NumericID>,
    /// Filter on the video scan type.
    pub scan_type: Option<ScanType>,
}

/// Port for persisting and querying `Image` and `Video`.
#[cfg_attr(test, mockall::automock)]
pub trait MediaRepository: Send + Sync {
    /// Insert a new `image`, returning the persisted image with its final
    /// identifier.
    ///
    /// The identifier of `image` is ignored: the repository assigns a fresh
    /// identity.
    fn create_image(
        &mut self,
        image: Image,
    ) -> impl Future<Output = Result<Image, RepositoryError>> + Send;

    /// Insert a new `video`, returning the persisted video with its final
    /// identifier.
    ///
    /// The identifier of `video` is ignored: the repository assigns a fresh
    /// identity.
    fn create_video(
        &mut self,
        video: Video,
    ) -> impl Future<Output = Result<Video, RepositoryError>> + Send;

    /// Delete the image identified by `id`.
    ///
    /// Returns `Ok(true)` when an image matched `id` and was deleted, and
    /// `Ok(false)` when no image matched. A missing image is a valid outcome,
    /// not an error.
    fn delete_image(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Delete the video identified by `id`.
    ///
    /// Returns `Ok(true)` when a video matched `id` and was deleted, and
    /// `Ok(false)` when no video matched. A missing video is a valid outcome,
    /// not an error.
    fn delete_video(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Return the image identified by `id`, if any.
    ///
    /// A missing image is `Ok(None)`, not an error.
    fn get_image(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<Option<Image>, RepositoryError>> + Send;

    /// Return the video identified by `id`, if any.
    ///
    /// A missing video is `Ok(None)`, not an error.
    fn get_video(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<Option<Video>, RepositoryError>> + Send;

    /// Search images matching `filter`, returned as `Vec<Image>`.
    ///
    /// Returns an empty list when no image matches; a missing match is a valid
    /// outcome, not an error.
    fn search_images(
        &mut self,
        filter: &ImageFilter,
    ) -> impl Future<Output = Result<Vec<Image>, RepositoryError>> + Send;

    /// Search videos matching `filter`, returned as `Vec<Video>`.
    ///
    /// Returns an empty list when no video matches; a missing match is a valid
    /// outcome, not an error.
    fn search_videos(
        &mut self,
        filter: &VideoFilter,
    ) -> impl Future<Output = Result<Vec<Video>, RepositoryError>> + Send;
}

#[cfg(test)]
impl<T: MediaRepository + ?Sized> MediaRepository for &mut T {
    fn create_image(
        &mut self,
        image: Image,
    ) -> impl Future<Output = Result<Image, RepositoryError>> + Send {
        (**self).create_image(image)
    }

    fn create_video(
        &mut self,
        video: Video,
    ) -> impl Future<Output = Result<Video, RepositoryError>> + Send {
        (**self).create_video(video)
    }

    fn delete_image(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).delete_image(id)
    }

    fn delete_video(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).delete_video(id)
    }

    fn get_image(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<Option<Image>, RepositoryError>> + Send {
        (**self).get_image(id)
    }

    fn get_video(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<Option<Video>, RepositoryError>> + Send {
        (**self).get_video(id)
    }

    fn search_images(
        &mut self,
        filter: &ImageFilter,
    ) -> impl Future<Output = Result<Vec<Image>, RepositoryError>> + Send {
        (**self).search_images(filter)
    }

    fn search_videos(
        &mut self,
        filter: &VideoFilter,
    ) -> impl Future<Output = Result<Vec<Video>, RepositoryError>> + Send {
        (**self).search_videos(filter)
    }
}
