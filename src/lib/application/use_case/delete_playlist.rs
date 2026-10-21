use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::delete_playlist::DeletePlaylistCommand;
use crate::application::port::delete_playlist::DeletePlaylistError;
use crate::application::port::delete_playlist::DeletePlaylistUseCase;
use crate::application::security_event;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::music_playlist_repository::MusicPlaylistFilter;
use crate::domain::port::music_playlist_repository::MusicPlaylistRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::service::library_access::LibraryAccess;

/// Use case implementation for deleting a playlist.
pub struct DeletePlaylist<F> {
    /// Factory opening the unit of work wrapping the deletion.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> DeletePlaylist<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> DeletePlaylistUseCase for DeletePlaylist<F>
where
    F: UnitOfWorkFactory,
    F::Uow: LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: DeletePlaylistCommand,
    ) -> Pin<Box<dyn Future<Output = Result<(), DeletePlaylistError>> + Send + 'future>> {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
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

                if !LibraryAccess::can_read_playlist(&playlist, &caller) {
                    // An invisible playlist is reported exactly like a missing
                    // one (`API-044`).
                    security_event::authorization_failed(caller.id(), "delete", "music_playlist");
                    return Err(DeletePlaylistError::NoSuchPlaylist);
                }
                if !playlist.is_owned_by(caller.id()) {
                    security_event::authorization_failed(caller.id(), "delete", "music_playlist");
                    return Err(DeletePlaylistError::Forbidden);
                }

                let deleted = unit_of_work
                    .music_playlists()
                    .delete(command.playlist_id())
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
    use std::error::Error;
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use chrono::NaiveDateTime;

    use crate::application::port::delete_playlist::DeletePlaylistCommand;
    use crate::application::port::delete_playlist::DeletePlaylistError;
    use crate::application::port::delete_playlist::DeletePlaylistUseCase as _;
    use crate::domain::model::music_playlist::MusicPlaylist;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::music_playlist_repository::MockMusicPlaylistRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::GalleryFactoryHarness;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::TestUnitOfWork;

    use super::DeletePlaylist;

    type UseCase = DeletePlaylist<TestFactory>;

    fn playlist(
        id: i64,
        user_id: i64,
        name: &str,
        is_public: bool,
    ) -> Result<MusicPlaylist, Box<dyn Error>> {
        Ok(MusicPlaylist::try_new(
            id,
            name.to_owned(),
            NaiveDateTime::default(),
            NaiveDateTime::default(),
            user_id,
            is_public,
            Vec::new(),
        )?)
    }

    fn harness_with(
        users: MockUserRepository,
        playlists: MockMusicPlaylistRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
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
        expect_playlist(&mut playlists, playlist(3, 4, "Mine", false)?);
        playlists
            .expect_delete()
            .times(1)
            .withf(|id| *id == 3)
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
    async fn delete_playlist_visible_non_owner_returns_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 0, Role::Admin)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(&mut playlists, playlist(3, 4, "Public", true)?);
        playlists.expect_delete().times(0);
        let harness = harness_with(users, playlists);
        let use_case: UseCase = DeletePlaylist::new(Arc::clone(&harness.factory));

        // Act: the Root Admin can read but does not own the playlist.
        let result = use_case.execute(DeletePlaylistCommand::new(0, 3)).await;

        // Assert
        assert!(matches!(result, Err(DeletePlaylistError::Forbidden)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_playlist_invisible_returns_no_such_playlist() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(&mut playlists, playlist(3, 4, "Mine", false)?);
        playlists.expect_delete().times(0);
        let harness = harness_with(users, playlists);
        let use_case: UseCase = DeletePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(DeletePlaylistCommand::new(9, 3)).await;

        // Assert
        assert!(matches!(result, Err(DeletePlaylistError::NoSuchPlaylist)));
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
        expect_playlist(&mut playlists, playlist(3, 4, "Mine", false)?);
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
