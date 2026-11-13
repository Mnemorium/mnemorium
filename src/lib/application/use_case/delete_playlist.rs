use crate::application::authorization::authorize;
use crate::domain::model::{ResourceKind, ResourceRef};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::{DeletePlaylistCommand, DeletePlaylistError, DeletePlaylistUseCase};
use crate::domain::model::Action;
use crate::domain::port::{
    IdentityUnitOfWork, LibraryUnitOfWork, MusicPlaylistFilter, MusicPlaylistRepository as _,
    UnitOfWork as _, UnitOfWorkFactory, UserFilter, UserRepository as _, UserUnitOfWork,
};

/// Use case implementation for deleting a playlist.
pub(crate) struct DeletePlaylist<F> {
    /// Factory opening the unit of work wrapping the deletion.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> DeletePlaylist<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> DeletePlaylistUseCase for DeletePlaylist<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: DeletePlaylistCommand,
    ) -> Pin<Box<dyn Future<Output = Result<(), DeletePlaylistError>> + Send + 'future>> {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| DeletePlaylistError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| DeletePlaylistError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(DeletePlaylistError::NoSuchCaller)?;

                let playlist = unit_of_work
                    .music_playlists()
                    .search(&MusicPlaylistFilter {
                        id: Some(command.playlist_id()),
                        ..MusicPlaylistFilter::default()
                    })
                    .await
                    .map_err(|error| DeletePlaylistError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(DeletePlaylistError::NoSuchPlaylist)?;

                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::MusicPlaylist, playlist.id()),
                    Action::Delete,
                    None,
                )
                .await
                .map_err(|error| {
                    if error.conceals() {
                        DeletePlaylistError::NoSuchPlaylist
                    } else {
                        DeletePlaylistError::Forbidden
                    }
                })?;

                let deleted = unit_of_work
                    .music_playlists()
                    .delete(&MusicPlaylistFilter {
                        id: Some(command.playlist_id()),
                        ..MusicPlaylistFilter::default()
                    })
                    .await
                    .map_err(|error| DeletePlaylistError::Unknown(error.into()))?;
                if !deleted {
                    return Err(DeletePlaylistError::NoSuchPlaylist);
                }
                Ok(())
            }
            .await;

            match result {
                Ok(()) => unit_of_work
                    .commit()
                    .await
                    .map_err(|error| DeletePlaylistError::Unknown(error.into())),
                Err(error) => {
                    // The unit-of-work adapter owns the rollback-failure log (OBS-002).
                    let _rollback = unit_of_work.rollback().await;
                    Err(error)
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::{MusicPlaylist, Role, User};

    use crate::domain::port::{MockMusicPlaylistRepository, MockUserRepository, RepositoryError};

    use crate::test_helpers::*;
    use chrono::NaiveDateTime;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    type UseCase = DeletePlaylist<TestFactory>;

    fn playlist(id: i64, user_id: i64, name: &str, is_public: bool) -> MusicPlaylist {
        MusicPlaylist::new(
            id,
            name.to_owned(),
            NaiveDateTime::default(),
            NaiveDateTime::default(),
            user_id,
            is_public,
            None,
            Vec::new(),
        )
    }

    fn harness_with(
        users: MockUserRepository,
        playlists: MockMusicPlaylistRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            resources: no_resources(),
            music_playlists: playlists,
            users,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
    }

    fn expect_caller(
        users: &mut MockUserRepository,
        caller_id: i64,
        role: Role,
    ) -> Result<(), Box<dyn Error>> {
        let caller = User::try_new(caller_id, "caller".to_owned(), None, caller_id, role)?;
        users
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(caller_id))
            .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
        Ok(())
    }

    fn expect_playlist(playlists: &mut MockMusicPlaylistRepository, playlist: MusicPlaylist) {
        let playlist_id = playlist.id();
        playlists
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(playlist_id))
            .return_once(move |_| Box::pin(async move { Ok(vec![playlist]) }));
    }

    #[tokio::test]
    async fn delete_playlist_owner_deletes_playlist() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(&mut playlists, playlist(3, 4, "Mine", false));
        playlists
            .expect_delete()
            .times(1)
            .withf(|filter| filter.id == Some(3))
            .returning(|_| Box::pin(async { Ok(true) }));
        let harness = harness_with(users, playlists);
        let use_case: UseCase = DeletePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(DeletePlaylistCommand::new(4, 3)).await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_playlist_root_admin_deletes_another_users_playlist()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 0, Role::Admin)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(&mut playlists, playlist(3, 4, "Private", false));
        playlists
            .expect_delete()
            .times(1)
            .withf(|filter| filter.id == Some(3))
            .returning(|_| Box::pin(async { Ok(true) }));
        let harness = harness_with(users, playlists);
        let use_case: UseCase = DeletePlaylist::new(Arc::clone(&harness.factory));

        // Act: the Root Admin manages every playlist.
        let result = use_case.execute(DeletePlaylistCommand::new(0, 3)).await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }
    #[tokio::test]
    async fn delete_playlist_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, MockMusicPlaylistRepository::new());
        let use_case: UseCase = DeletePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(DeletePlaylistCommand::new(999, 3)).await;

        // Assert
        assert!(matches!(result, Err(DeletePlaylistError::NoSuchCaller)));
        Ok(())
    }

    #[tokio::test]
    async fn delete_playlist_repository_failure_rolls_back() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(&mut playlists, playlist(3, 4, "Mine", false));
        playlists
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, playlists);
        let use_case: UseCase = DeletePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(DeletePlaylistCommand::new(4, 3)).await;

        // Assert
        assert!(matches!(result, Err(DeletePlaylistError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
