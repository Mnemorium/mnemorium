use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::get_playlist::GetPlaylistCommand;
use crate::application::port::get_playlist::GetPlaylistError;
use crate::application::port::get_playlist::GetPlaylistResponse;
use crate::application::port::get_playlist::GetPlaylistUseCase;
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

/// Use case implementation for fetching one playlist.
pub struct GetPlaylist<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> GetPlaylist<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> GetPlaylistUseCase for GetPlaylist<F>
where
    F: UnitOfWorkFactory,
    F::Uow: LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: GetPlaylistCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetPlaylistResponse, GetPlaylistError>> + Send + 'future>>
    {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| GetPlaylistError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| GetPlaylistError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetPlaylistError::NoSuchCaller)?;

                let playlist = unit_of_work
                    .music_playlists()
                    .search(&MusicPlaylistFilter {
                        id: Some(command.playlist_id()),
                        ..MusicPlaylistFilter::default()
                    })
                    .await
                    .map_err(|error| GetPlaylistError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetPlaylistError::NoSuchPlaylist)?;

                if !LibraryAccess::can_read_playlist(&playlist, &caller) {
                    // The response stays indistinguishable from a missing
                    // playlist (`API-044`).
                    security_event::authorization_failed(caller.id(), "read", "music_playlist");
                    return Err(GetPlaylistError::NoSuchPlaylist);
                }

                Ok(GetPlaylistResponse::new(playlist))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| GetPlaylistError::Unknown(error.into()))?;
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
    #![expect(
        clippy::single_call_fn,
        reason = "named fixtures keep the tests readable"
    )]

    use std::error::Error;
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use chrono::NaiveDate;
    use chrono::NaiveDateTime;

    use crate::application::port::get_playlist::GetPlaylistCommand;
    use crate::application::port::get_playlist::GetPlaylistError;
    use crate::application::port::get_playlist::GetPlaylistUseCase as _;
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

    use super::GetPlaylist;

    type UseCase = GetPlaylist<TestFactory>;

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
        name: &str,
        is_public: bool,
        tracks: Vec<MusicRecording>,
    ) -> MusicPlaylist {
        MusicPlaylist::new(
            id,
            name.to_owned(),
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
    async fn get_playlist_owner_reads_private_playlist_with_tracks() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(
            &mut playlists,
            playlist(3, 4, "Mine", false, vec![track(30, 0)?]),
        );
        let harness = harness_with(users, playlists);
        let use_case: UseCase = GetPlaylist::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(GetPlaylistCommand::new(4, 3)).await?;

        // Assert
        assert_eq!(response.playlist().id(), 3);
        assert_eq!(response.playlist().tracks().len(), 1);
        assert_eq!(
            response
                .playlist()
                .tracks()
                .first()
                .and_then(|track| track.index().playlist_index()),
            Some(0)
        );
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_playlist_any_caller_reads_a_public_playlist() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(&mut playlists, playlist(2, 3, "Public", true, Vec::new()));
        let harness = harness_with(users, playlists);
        let use_case: UseCase = GetPlaylist::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(GetPlaylistCommand::new(9, 2)).await?;

        // Assert
        assert_eq!(response.playlist().id(), 2);
        Ok(())
    }

    #[tokio::test]
    async fn get_playlist_root_admin_reads_a_private_playlist() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 0, Role::Admin)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(&mut playlists, playlist(3, 4, "Mine", false, Vec::new()));
        let harness = harness_with(users, playlists);
        let use_case: UseCase = GetPlaylist::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(GetPlaylistCommand::new(0, 3)).await?;

        // Assert
        assert_eq!(response.playlist().id(), 3);
        Ok(())
    }

    #[tokio::test]
    async fn get_playlist_carries_the_cover_of_a_playlist_with_artwork()
    -> Result<(), Box<dyn Error>> {
        // Arrange: a playlist whose artwork is file 19.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let covered = MusicPlaylist::new(
            3,
            "Mine".to_owned(),
            NaiveDateTime::default(),
            NaiveDateTime::default(),
            4,
            false,
            Some(19),
            Vec::new(),
        );
        assert_eq!(
            covered.cover_file_id(),
            Some(19),
            "a playlist built with artwork must report the identifier"
        );
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(&mut playlists, covered);
        let harness = harness_with(users, playlists);
        let use_case: UseCase = GetPlaylist::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(GetPlaylistCommand::new(4, 3)).await?;

        // Assert
        assert_eq!(response.playlist().cover_file_id(), Some(19));
        Ok(())
    }

    #[tokio::test]
    async fn get_playlist_invisible_private_playlist_returns_no_such_playlist()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(&mut playlists, playlist(3, 4, "Mine", false, Vec::new()));
        let harness = harness_with(users, playlists);
        let use_case: UseCase = GetPlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetPlaylistCommand::new(9, 3)).await;

        // Assert
        assert!(matches!(result, Err(GetPlaylistError::NoSuchPlaylist)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_playlist_unknown_playlist_returns_no_such_playlist() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        playlists
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, playlists);
        let use_case: UseCase = GetPlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetPlaylistCommand::new(1, 999)).await;

        // Assert
        assert!(matches!(result, Err(GetPlaylistError::NoSuchPlaylist)));
        Ok(())
    }

    #[tokio::test]
    async fn get_playlist_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, MockMusicPlaylistRepository::new());
        let use_case: UseCase = GetPlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetPlaylistCommand::new(999, 1)).await;

        // Assert
        assert!(matches!(result, Err(GetPlaylistError::NoSuchCaller)));
        Ok(())
    }

    #[tokio::test]
    async fn get_playlist_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        playlists
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, playlists);
        let use_case: UseCase = GetPlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetPlaylistCommand::new(1, 2)).await;

        // Assert
        assert!(matches!(result, Err(GetPlaylistError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
