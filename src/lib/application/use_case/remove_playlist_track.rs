use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use tracing::warn;

use crate::application::port::{
    RemovePlaylistTrackCommand, RemovePlaylistTrackError, RemovePlaylistTrackUseCase,
};
use crate::domain::model::Action;
use crate::domain::port::{
    IdentityUnitOfWork, LibraryUnitOfWork, MusicPlaylistFilter, MusicPlaylistRepository as _,
    PermissionFilter, PermissionRepository as _, UnitOfWork as _, UnitOfWorkFactory, UserFilter,
    UserRepository as _, UserUnitOfWork,
};
use crate::domain::service::{Authorization, Decision};

/// Use case implementation for removing one track of a playlist.
pub(crate) struct RemovePlaylistTrack<F> {
    /// Factory opening the unit of work wrapping the removal.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> RemovePlaylistTrack<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> RemovePlaylistTrackUseCase for RemovePlaylistTrack<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: RemovePlaylistTrackCommand,
    ) -> Pin<Box<dyn Future<Output = Result<(), RemovePlaylistTrackError>> + Send + 'future>> {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| RemovePlaylistTrackError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| RemovePlaylistTrackError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(RemovePlaylistTrackError::NoSuchCaller)?;

                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| RemovePlaylistTrackError::Unknown(error.into()))?;

                let mut playlist = unit_of_work
                    .music_playlists()
                    .search(&MusicPlaylistFilter {
                        id: Some(command.playlist_id()),
                        ..MusicPlaylistFilter::default()
                    })
                    .await
                    .map_err(|error| RemovePlaylistTrackError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(RemovePlaylistTrackError::NoSuchPlaylist)?;

                if let Decision::Deny(denial) = Authorization::decide(
                    &caller,
                    &permissions,
                    Action::PlaylistModerate,
                    &playlist,
                ) {
                    warn!(
                        target: "security",
                        event = "authz_failed",
                        actor = caller.id(),
                        action = %denial.action().verb(),
                        target = %denial.action().target(),
                        "denied an action"
                    );
                    return Err(if denial.conceals() {
                        RemovePlaylistTrackError::NoSuchPlaylist
                    } else {
                        RemovePlaylistTrackError::Forbidden
                    });
                }

                let removed = playlist.remove_track(command.track_index());
                if !removed {
                    return Err(RemovePlaylistTrackError::NoSuchTrack);
                }

                playlist.touch(Utc::now().naive_utc());
                unit_of_work
                    .music_playlists()
                    .save(playlist)
                    .await
                    .map_err(|error| RemovePlaylistTrackError::Unknown(error.into()))?;
                Ok(())
            }
            .await;

            match result {
                Ok(()) => unit_of_work
                    .commit()
                    .await
                    .map_err(|error| RemovePlaylistTrackError::Unknown(error.into())),
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
    use crate::domain::model::{MusicPlaylist, MusicRecording, Role, TrackIndex, User};

    use crate::domain::port::{MockMusicPlaylistRepository, MockUserRepository, RepositoryError};

    use crate::test_helpers::*;
    use chrono::NaiveDateTime;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    type UseCase = RemovePlaylistTrack<TestFactory>;

    fn track(recording_id: i64, index: u32) -> Result<MusicRecording, Box<dyn Error>> {
        Ok(MusicRecording::try_new(
            recording_id,
            Some(format!("track {recording_id}")),
            None,
            recording_id,
            recording_id,
            None,
            TrackIndex::PlaylistIndex(index),
        )?)
    }

    fn playlist(
        id: i64,
        user_id: i64,
        is_public: bool,
        tracks: Vec<MusicRecording>,
    ) -> MusicPlaylist {
        MusicPlaylist::new(
            id,
            "Mix".to_owned(),
            NaiveDateTime::default(),
            NaiveDateTime::default(),
            user_id,
            is_public,
            None,
            tracks,
        )
    }

    fn harness_with(
        users: MockUserRepository,
        playlists: MockMusicPlaylistRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            permissions: no_permissions(),
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
    async fn remove_playlist_track_reader_removes_the_track() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        let stored = playlist(3, 4, true, vec![track(30, 0)?, track(31, 1)?]);
        let loaded_at = stored.last_modified_at();
        expect_playlist(&mut playlists, stored);
        playlists
            .expect_save()
            .times(1)
            .withf(move |saved| {
                saved.tracks().len() == 1
                    && saved.tracks().first().is_some_and(|track| track.id() == 31)
                    && saved.last_modified_at() > loaded_at
            })
            .returning(|saved| Box::pin(async move { Ok(saved) }));
        let harness = harness_with(users, playlists);
        let use_case: UseCase = RemovePlaylistTrack::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(RemovePlaylistTrackCommand::new(9, 3, 0))
            .await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn remove_playlist_track_missing_index_returns_no_such_track()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(&mut playlists, playlist(3, 4, false, vec![track(30, 0)?]));
        playlists.expect_save().times(0);
        let harness = harness_with(users, playlists);
        let use_case: UseCase = RemovePlaylistTrack::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(RemovePlaylistTrackCommand::new(4, 3, 5))
            .await;

        // Assert
        assert!(matches!(result, Err(RemovePlaylistTrackError::NoSuchTrack)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn remove_playlist_track_invisible_playlist_returns_no_such_playlist()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(&mut playlists, playlist(3, 4, false, vec![track(30, 0)?]));
        playlists.expect_save().times(0);
        let harness = harness_with(users, playlists);
        let use_case: UseCase = RemovePlaylistTrack::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(RemovePlaylistTrackCommand::new(9, 3, 0))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(RemovePlaylistTrackError::NoSuchPlaylist)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn remove_playlist_track_repository_failure_rolls_back() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(&mut playlists, playlist(3, 4, false, vec![track(30, 0)?]));
        playlists
            .expect_save()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, playlists);
        let use_case: UseCase = RemovePlaylistTrack::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(RemovePlaylistTrackCommand::new(4, 3, 0))
            .await;

        // Assert
        assert!(matches!(result, Err(RemovePlaylistTrackError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
