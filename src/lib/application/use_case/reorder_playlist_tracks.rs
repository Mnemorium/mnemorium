use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use crate::application::port::reorder_playlist_tracks::ReorderPlaylistTracksCommand;
use crate::application::port::reorder_playlist_tracks::ReorderPlaylistTracksError;
use crate::application::port::reorder_playlist_tracks::ReorderPlaylistTracksResponse;
use crate::application::port::reorder_playlist_tracks::ReorderPlaylistTracksUseCase;
use crate::application::security_event;
use crate::domain::model::music_playlist::MusicPlaylistError;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::music_playlist_repository::MusicPlaylistFilter;
use crate::domain::port::music_playlist_repository::MusicPlaylistRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::service::library_access::LibraryAccess;

/// Use case implementation for reordering the tracks of a playlist.
pub struct ReorderPlaylistTracks<F> {
    /// Factory opening the unit of work wrapping the reorder.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> ReorderPlaylistTracks<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> ReorderPlaylistTracksUseCase for ReorderPlaylistTracks<F>
where
    F: UnitOfWorkFactory,
    F::Uow: LibraryUnitOfWork + UserUnitOfWork,
{
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "MusicPlaylistError is non_exhaustive, so a catch-all arm is required"
    )]
    fn execute<'future>(
        &'future self,
        command: ReorderPlaylistTracksCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ReorderPlaylistTracksResponse, ReorderPlaylistTracksError>>
                + Send
                + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| ReorderPlaylistTracksError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| ReorderPlaylistTracksError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(ReorderPlaylistTracksError::NoSuchCaller)?;

                let mut playlist = unit_of_work
                    .music_playlists()
                    .search(&MusicPlaylistFilter {
                        id: Some(command.playlist_id()),
                        ..MusicPlaylistFilter::default()
                    })
                    .await
                    .map_err(|error| ReorderPlaylistTracksError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(ReorderPlaylistTracksError::NoSuchPlaylist)?;

                if !LibraryAccess::can_read_playlist(&playlist, &caller) {
                    // The response stays indistinguishable from a missing
                    // playlist (`API-044`).
                    security_event::authorization_failed(caller.id(), "reorder", "music_playlist");
                    return Err(ReorderPlaylistTracksError::NoSuchPlaylist);
                }

                let order =
                    playlist
                        .validate_order(command.order())
                        .map_err(|error| match error {
                            MusicPlaylistError::InvalidOrder => {
                                security_event::input_validation_failed("order", "invalid_order");
                                ReorderPlaylistTracksError::InvalidOrder
                            }
                            _ => ReorderPlaylistTracksError::Unknown(anyhow::Error::new(error)),
                        })?;

                unit_of_work
                    .music_playlists()
                    .replace_tracks(command.playlist_id(), &order)
                    .await
                    .map_err(|error| ReorderPlaylistTracksError::Unknown(error.into()))?;

                playlist.touch(Utc::now().naive_utc());
                let saved = unit_of_work
                    .music_playlists()
                    .save(playlist)
                    .await
                    .map_err(|error| ReorderPlaylistTracksError::Unknown(error.into()))?;

                Ok(ReorderPlaylistTracksResponse::new(saved))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| ReorderPlaylistTracksError::Unknown(error.into()))?;
                    Ok(value)
                }
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

    use crate::application::port::reorder_playlist_tracks::ReorderPlaylistTracksCommand;
    use crate::application::port::reorder_playlist_tracks::ReorderPlaylistTracksError;
    use crate::application::port::reorder_playlist_tracks::ReorderPlaylistTracksUseCase as _;
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

    use super::ReorderPlaylistTracks;

    type UseCase = ReorderPlaylistTracks<TestFactory>;

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
            tracks,
        )
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
    async fn reorder_playlist_tracks_replaces_with_the_permutation() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        let stored = playlist(3, 4, true, vec![track(30, 0)?, track(31, 1)?]);
        let loaded_at = stored.last_modified_at();
        expect_playlist(&mut playlists, stored.clone());
        playlists
            .expect_replace_tracks()
            .times(1)
            .withf(|playlist_id, order| *playlist_id == 3 && order == [(0, 31), (1, 30)])
            .returning(|_, _| Box::pin(async { Ok(()) }));
        playlists
            .expect_save()
            .times(1)
            .withf(move |saved| saved.last_modified_at() > loaded_at)
            .return_once(move |_| {
                let found = stored.clone();
                Box::pin(async move { Ok(found) })
            });
        let harness = harness_with(users, playlists);
        let use_case: UseCase = ReorderPlaylistTracks::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ReorderPlaylistTracksCommand::new(9, 3, vec![1, 0]))
            .await?;

        // Assert
        assert_eq!(result.playlist().id(), 3);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn reorder_playlist_tracks_empty_order_succeeds() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        let stored = playlist(3, 4, false, Vec::new());
        let loaded_at = stored.last_modified_at();
        expect_playlist(&mut playlists, stored.clone());
        playlists
            .expect_replace_tracks()
            .times(1)
            .withf(|_, order| order.is_empty())
            .returning(|_, _| Box::pin(async { Ok(()) }));
        playlists
            .expect_save()
            .times(1)
            .withf(move |saved| saved.last_modified_at() > loaded_at)
            .return_once(move |_| {
                let found = stored.clone();
                Box::pin(async move { Ok(found) })
            });
        let harness = harness_with(users, playlists);
        let use_case: UseCase = ReorderPlaylistTracks::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ReorderPlaylistTracksCommand::new(4, 3, Vec::new()))
            .await?;

        // Assert
        assert!(result.playlist().tracks().is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn reorder_playlist_tracks_incomplete_order_returns_invalid_order()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(
            &mut playlists,
            playlist(3, 4, false, vec![track(30, 0)?, track(31, 1)?]),
        );
        playlists.expect_replace_tracks().times(0);
        let harness = harness_with(users, playlists);
        let use_case: UseCase = ReorderPlaylistTracks::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ReorderPlaylistTracksCommand::new(4, 3, vec![0]))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(ReorderPlaylistTracksError::InvalidOrder)
        ));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn reorder_playlist_tracks_duplicate_index_returns_invalid_order()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(
            &mut playlists,
            playlist(3, 4, false, vec![track(30, 0)?, track(31, 1)?]),
        );
        playlists.expect_replace_tracks().times(0);
        let harness = harness_with(users, playlists);
        let use_case: UseCase = ReorderPlaylistTracks::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ReorderPlaylistTracksCommand::new(4, 3, vec![1, 1]))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(ReorderPlaylistTracksError::InvalidOrder)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn reorder_playlist_tracks_unknown_index_returns_invalid_order()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(
            &mut playlists,
            playlist(3, 4, false, vec![track(30, 0)?, track(31, 1)?]),
        );
        playlists.expect_replace_tracks().times(0);
        let harness = harness_with(users, playlists);
        let use_case: UseCase = ReorderPlaylistTracks::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ReorderPlaylistTracksCommand::new(4, 3, vec![0, 7]))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(ReorderPlaylistTracksError::InvalidOrder)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn reorder_playlist_tracks_invisible_playlist_returns_no_such_playlist()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(&mut playlists, playlist(3, 4, false, vec![track(30, 0)?]));
        playlists.expect_replace_tracks().times(0);
        let harness = harness_with(users, playlists);
        let use_case: UseCase = ReorderPlaylistTracks::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ReorderPlaylistTracksCommand::new(9, 3, vec![0]))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(ReorderPlaylistTracksError::NoSuchPlaylist)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn reorder_playlist_tracks_replace_failure_rolls_back() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(
            &mut playlists,
            playlist(3, 4, false, vec![track(30, 0)?, track(31, 1)?]),
        );
        playlists
            .expect_replace_tracks()
            .times(1)
            .returning(|_, _| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        playlists.expect_save().times(0);
        let harness = harness_with(users, playlists);
        let use_case: UseCase = ReorderPlaylistTracks::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ReorderPlaylistTracksCommand::new(4, 3, vec![1, 0]))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(ReorderPlaylistTracksError::Unknown(_))
        ));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
