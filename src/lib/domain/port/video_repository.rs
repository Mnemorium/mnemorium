use std::future::Future;

use crate::domain::alias::NumericID;
use crate::domain::model::video::ScanType;
use crate::domain::model::video::Video;
use crate::domain::port::error::RepositoryError;

/// Search filters for [`VideoRepository::search`]. Every field is optional; an
/// all-`None` filter returns every video.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct VideoFilter {
    /// Filter on the video codec.
    pub codec: Option<String>,
    /// Filter on the identifier of the colour space.
    pub color_id: Option<String>,
    /// Filter on the identifier of the backing file.
    pub file_id: Option<NumericID>,
    /// Filter on the video scan type.
    pub scan_type: Option<ScanType>,
    /// Filter on the video identifier.
    pub video_id: Option<NumericID>,
}

/// Port for persisting and querying `Video`.
#[cfg_attr(test, mockall::automock)]
pub trait VideoRepository: Send + Sync {
    /// Insert a new `video`, returning the persisted video with its final
    /// identifier.
    ///
    /// The identifier of `video` is ignored: the repository assigns a fresh
    /// identity.
    fn create(
        &mut self,
        video: Video,
    ) -> impl Future<Output = Result<Video, RepositoryError>> + Send;

    /// Delete the video identified by `id`.
    ///
    /// Returns `Ok(true)` when a video matched `id` and was deleted, and
    /// `Ok(false)` when no video matched. A missing video is a valid outcome,
    /// not an error.
    fn delete(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Return the video identified by `id`, if any.
    ///
    /// A missing video is `Ok(None)`, not an error.
    fn get(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<Option<Video>, RepositoryError>> + Send;

    /// Search videos matching `filter`, returned as `Vec<Video>`.
    ///
    /// Returns an empty list when no video matches; a missing match is a valid
    /// outcome, not an error.
    fn search(
        &mut self,
        filter: &VideoFilter,
    ) -> impl Future<Output = Result<Vec<Video>, RepositoryError>> + Send;
}

#[cfg(test)]
impl<T: VideoRepository + ?Sized> VideoRepository for &mut T {
    fn create(
        &mut self,
        video: Video,
    ) -> impl Future<Output = Result<Video, RepositoryError>> + Send {
        (**self).create(video)
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
    ) -> impl Future<Output = Result<Option<Video>, RepositoryError>> + Send {
        (**self).get(id)
    }

    fn search(
        &mut self,
        filter: &VideoFilter,
    ) -> impl Future<Output = Result<Vec<Video>, RepositoryError>> + Send {
        (**self).search(filter)
    }
}
