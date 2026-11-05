use std::future::Future;

use crate::domain::alias::NumericID;
use crate::domain::model::music_group::MusicGroup;
use crate::domain::port::error::RepositoryError;

/// Search filters for [`MusicGroupRepository::search`]. Every field is
/// optional; an all-`None` filter returns every group.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct MusicGroupFilter {
    /// Filter on the group identifier.
    pub id: Option<NumericID>,
    /// Filter on the group stage name.
    pub stage_name: Option<String>,
}

/// Port persisting the music groups an album belongs to.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait MusicGroupRepository: Send + Sync {
    /// Persist a new group and return it with the identifier it was assigned.
    ///
    /// The database enforces the unique stage name and the cover contract, so
    /// a duplicate name is reported as [`RepositoryError::AlreadyExist`] and a
    /// `USER` cover image as [`RepositoryError::Conflict`].
    fn create(
        &mut self,
        group: MusicGroup,
    ) -> impl Future<Output = Result<MusicGroup, RepositoryError>> + Send;

    /// Return the group identified by `id`, or `None` when no group matches.
    fn get(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<Option<MusicGroup>, RepositoryError>> + Send;

    /// Search the groups matching `filter`, returned as `Vec<MusicGroup>`.
    ///
    /// Returns an empty list when no group matches; a missing match is a valid
    /// outcome, not an error.
    fn search(
        &mut self,
        filter: &MusicGroupFilter,
    ) -> impl Future<Output = Result<Vec<MusicGroup>, RepositoryError>> + Send;

    /// Persist `group` over the stored group carrying its identifier and return
    /// the stored value, or `None` when no group matches.
    fn update(
        &mut self,
        group: MusicGroup,
    ) -> impl Future<Output = Result<Option<MusicGroup>, RepositoryError>> + Send;
}

#[cfg(test)]
impl<T: MusicGroupRepository + ?Sized> MusicGroupRepository for &mut T {
    fn create(
        &mut self,
        group: MusicGroup,
    ) -> impl Future<Output = Result<MusicGroup, RepositoryError>> + Send {
        (**self).create(group)
    }

    fn get(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<Option<MusicGroup>, RepositoryError>> + Send {
        (**self).get(id)
    }

    fn search(
        &mut self,
        filter: &MusicGroupFilter,
    ) -> impl Future<Output = Result<Vec<MusicGroup>, RepositoryError>> + Send {
        (**self).search(filter)
    }

    fn update(
        &mut self,
        group: MusicGroup,
    ) -> impl Future<Output = Result<Option<MusicGroup>, RepositoryError>> + Send {
        (**self).update(group)
    }
}
