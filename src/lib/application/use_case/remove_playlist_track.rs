use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use crate::application::port::remove_playlist_track::RemovePlaylistTrackCommand;
use crate::application::port::remove_playlist_track::RemovePlaylistTrackError;
use crate::application::port::remove_playlist_track::RemovePlaylistTrackUseCase;
use crate::application::security_event;
use crate::domain::model::permission::Action;
use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::music_playlist_repository::MusicPlaylistFilter;
use crate::domain::port::music_playlist_repository::MusicPlaylistRepository as _;
use crate::domain::port::permission_repository::PermissionFilter;
use crate::domain::port::permission_repository::PermissionRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::service::authorization::Authorization;
use crate::domain::service::authorization::Decision;

/// Use case implementation for removing one track of a playlist.
pub struct RemovePlaylistTrack<F> {
    /// Factory opening the unit of work wrapping the removal.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> RemovePlaylistTrack<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
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
                .begin()
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
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(if denial.conceals() {
                        RemovePlaylistTrackError::NoSuchPlaylist
                    } else {
                        RemovePlaylistTrackError::Forbidden
                    });
                }

                let removed = unit_of_work
                    .music_playlists()
                    .remove_track(command.playlist_id(), command.track_index())
                    .await
                    .map_err(|error| RemovePlaylistTrackError::Unknown(error.into()))?;
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
    use std::error::Error;
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use chrono::NaiveDate;
    use chrono::NaiveDateTime;

    use crate::application::port::remove_playlist_track::RemovePlaylistTrackCommand;
    use crate::application::port::remove_playlist_track::RemovePlaylistTrackError;
    use crate::application::port::remove_playlist_track::RemovePlaylistTrackUseCase as _;
    use crate::domain::model::music_playlist::MusicPlaylist;
    use crate::domain::model::music_recording::MusicRecording;
    use crate::domain::model::music_recording::TrackIndex;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::music_playlist_repository::MockMusicPlaylistRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::GalleryFactoryHarness;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::TestUnitOfWork;
    use crate::test_helpers::no_permissions;

    use super::RemovePlaylistTrack;

    type UseCase = RemovePlaylistTrack<TestFactory>;

    fn track(recording_id: i64, index: u32) -> Result<MusicRecording, Box<dyn Error>> {
        Ok(MusicRecording::try_new(
            recording_id,
            Some(format!("track {recording_id}")),
            NaiveDate::from_ymd_opt(2026, 10, 20).ok_or("invalid fixture date")?,
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
            .expect_remove_track()
            .times(1)
            .withf(|playlist_id, index| *playlist_id == 3 && *index == 0)
            .returning(|_, _| Box::pin(async { Ok(true) }));
        playlists
            .expect_save()
            .times(1)
            .withf(move |saved| saved.last_modified_at() > loaded_at)
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
        playlists
            .expect_remove_track()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(false) }));
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
        playlists.expect_remove_track().times(0);
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
            .expect_remove_track()
            .times(1)
            .returning(|_, _| Box::pin(async { Err(RepositoryError::OperationFailed) }));
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
