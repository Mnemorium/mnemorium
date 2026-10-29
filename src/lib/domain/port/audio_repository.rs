use std::future::Future;

use crate::domain::alias::NumericID;
use crate::domain::model::audio::Audio;
use crate::domain::port::error::RepositoryError;

/// Search filters for [`AudioRepository::search`]. Every field is optional; an
/// all-`None` filter returns every audio row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct AudioFilter {
    /// Filter on the identifier of the backing file.
    pub file_id: Option<NumericID>,
}

/// Port for persisting and querying `Audio`.
#[cfg_attr(test, mockall::automock)]
pub trait AudioRepository: Send + Sync {
    /// Insert a new `audio`, returning the persisted audio with its final
    /// identifier.
    ///
    /// The identifier of `audio` is ignored: the repository assigns a fresh
    /// identity.
    fn create(
        &mut self,
        audio: Audio,
    ) -> impl Future<Output = Result<Audio, RepositoryError>> + Send;

    /// Search audio rows matching `filter`, returned as `Vec<Audio>`.
    ///
    /// Returns an empty list when no audio matches; a missing match is a valid
    /// outcome, not an error.
    fn search(
        &mut self,
        filter: &AudioFilter,
    ) -> impl Future<Output = Result<Vec<Audio>, RepositoryError>> + Send;
}

#[cfg(test)]
impl<T: AudioRepository + ?Sized> AudioRepository for &mut T {
    fn create(
        &mut self,
        audio: Audio,
    ) -> impl Future<Output = Result<Audio, RepositoryError>> + Send {
        (**self).create(audio)
    }

    fn search(
        &mut self,
        filter: &AudioFilter,
    ) -> impl Future<Output = Result<Vec<Audio>, RepositoryError>> + Send {
        (**self).search(filter)
    }
}
