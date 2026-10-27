use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use crate::application::port::update_playlist::UpdatePlaylistCommand;
use crate::application::port::update_playlist::UpdatePlaylistError;
use crate::application::port::update_playlist::UpdatePlaylistResponse;
use crate::application::port::update_playlist::UpdatePlaylistUseCase;
use crate::application::security_event;
use crate::domain::model::music_playlist::MusicPlaylistError;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::music_playlist_repository::MusicPlaylistFilter;
use crate::domain::port::music_playlist_repository::MusicPlaylistRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::service::library_access::LibraryAccess;
use crate::domain::service::name_policy::NamePolicy;
use crate::domain::service::name_policy::NamePolicyError;

/// Use case implementation for updating a playlist's metadata or tracks.
pub struct UpdatePlaylist<F> {
    /// Factory opening the unit of work wrapping the update.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> UpdatePlaylist<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> UpdatePlaylistUseCase for UpdatePlaylist<F>
where
    F: UnitOfWorkFactory,
    F::Uow: LibraryUnitOfWork + UserUnitOfWork,
{
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "RepositoryError is non_exhaustive, so a catch-all arm is required"
    )]
    fn execute<'future>(
        &'future self,
        command: UpdatePlaylistCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<UpdatePlaylistResponse, UpdatePlaylistError>>
                + Send
                + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| UpdatePlaylistError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| UpdatePlaylistError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(UpdatePlaylistError::NoSuchCaller)?;

                let mut playlist = unit_of_work
                    .music_playlists()
                    .search(&MusicPlaylistFilter {
                        id: Some(command.playlist_id()),
                        ..MusicPlaylistFilter::default()
                    })
                    .await
                    .map_err(|error| UpdatePlaylistError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(UpdatePlaylistError::NoSuchPlaylist)?;

                if !LibraryAccess::can_read_playlist(&playlist, &caller) {
                    // The response stays indistinguishable from a missing
                    // playlist (`API-044`).
                    security_event::authorization_failed(caller.id(), "update", "music_playlist");
                    return Err(UpdatePlaylistError::NoSuchPlaylist);
                }

                // Metadata is owner-only; the check covers the whole mixed
                // request, so a non-owner's `add_track` is rejected with it and
                // nothing is applied.
                let owner_only = command.name().is_some() || command.is_public().is_some();
                if owner_only && !playlist.is_owned_by(caller.id()) {
                    security_event::authorization_failed(caller.id(), "update", "music_playlist");
                    return Err(UpdatePlaylistError::Forbidden);
                }

                if let Some(requested_name) = command.name() {
                    let name = NamePolicy::validate(requested_name.to_owned()).map_err(
                        |error| match error {
                            NamePolicyError::Empty => {
                                security_event::input_validation_failed("name", "empty");
                                UpdatePlaylistError::InvalidName
                            }
                            NamePolicyError::TooLong => {
                                security_event::input_validation_failed("name", "too_long");
                                UpdatePlaylistError::NameTooLong
                            }
                        },
                    )?;
                    playlist.set_name(name);

                    let collisions = unit_of_work
                        .music_playlists()
                        .search(&MusicPlaylistFilter {
                            name: Some(playlist.name().to_owned()),
                            user_id: Some(playlist.user_id()),
                            ..MusicPlaylistFilter::default()
                        })
                        .await
                        .map_err(|error| UpdatePlaylistError::Unknown(error.into()))?;
                    if collisions
                        .iter()
                        .any(|collision| collision.id() != playlist.id())
                    {
                        return Err(UpdatePlaylistError::NameAlreadyExists);
                    }
                }

                if let Some(recording_id) = command.add_track() {
                    match playlist.ensure_recording_absent(recording_id) {
                        Ok(()) => {}
                        Err(MusicPlaylistError::RecordingAlreadyPresent) => {
                            return Err(UpdatePlaylistError::RecordingAlreadyPresent);
                        }
                        Err(error) => {
                            return Err(UpdatePlaylistError::Unknown(anyhow::Error::new(error)));
                        }
                    }
                    match unit_of_work
                        .music_playlists()
                        .add_track(playlist.id(), recording_id)
                        .await
                    {
                        Ok(Some(_)) => {}
                        Ok(None) => return Err(UpdatePlaylistError::NoSuchRecording),
                        // The unique index rejects a concurrent duplicate or an
                        // append that lost the position race. The generic
                        // conflict does not claim which.
                        Err(RepositoryError::AlreadyExist) => {
                            return Err(UpdatePlaylistError::Conflict);
                        }
                        Err(error) => {
                            return Err(UpdatePlaylistError::Unknown(error.into()));
                        }
                    }
                }

                if let Some(is_public) = command.is_public() {
                    playlist.set_is_public(is_public);
                }
                playlist.touch(Utc::now().naive_utc());

                let saved = unit_of_work
                    .music_playlists()
                    .save(playlist)
                    .await
                    .map_err(|error| match error {
                        RepositoryError::AlreadyExist => UpdatePlaylistError::NameAlreadyExists,
                        other => UpdatePlaylistError::Unknown(other.into()),
                    })?;

                Ok(UpdatePlaylistResponse::new(saved))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| UpdatePlaylistError::Unknown(error.into()))?;
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

    use crate::application::port::update_playlist::UpdatePlaylistCommand;
    use crate::application::port::update_playlist::UpdatePlaylistError;
    use crate::application::port::update_playlist::UpdatePlaylistUseCase as _;
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

    use super::UpdatePlaylist;

    type UseCase = UpdatePlaylist<TestFactory>;

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

    fn save_echo(
        playlists: &mut MockMusicPlaylistRepository,
        expected_name: &'static str,
        loaded_at: NaiveDateTime,
    ) {
        playlists
            .expect_save()
            .times(1)
            .withf(move |saved| {
                saved.name() == expected_name && saved.last_modified_at() > loaded_at
            })
            .returning(|saved| Box::pin(async move { Ok(saved) }));
    }

    #[tokio::test]
    async fn update_playlist_renames_and_sets_visibility() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        let stored = playlist(3, 4, "Mine", false, Vec::new());
        let loaded_at = stored.last_modified_at();
        expect_playlist(&mut playlists, stored);
        playlists
            .expect_search()
            .times(1)
            .withf(|filter| filter.name.as_deref() == Some("New") && filter.user_id == Some(4))
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        playlists
            .expect_save()
            .times(1)
            .withf(move |saved| {
                saved.name() == "New" && saved.is_public() && saved.last_modified_at() > loaded_at
            })
            .returning(|saved| Box::pin(async move { Ok(saved) }));
        let harness = harness_with(users, playlists);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(UpdatePlaylistCommand::new(
                4,
                3,
                Some("New".to_owned()),
                Some(true),
                None,
            ))
            .await?;

        // Assert
        assert_eq!(response.playlist().name(), "New");
        assert!(response.playlist().is_public());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_playlist_empty_body_returns_the_unchanged_playlist()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        let stored = playlist(3, 4, "Mine", false, Vec::new());
        let loaded_at = stored.last_modified_at();
        expect_playlist(&mut playlists, stored);
        save_echo(&mut playlists, "Mine", loaded_at);
        let harness = harness_with(users, playlists);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(UpdatePlaylistCommand::new(4, 3, None, None, None))
            .await?;

        // Assert
        assert_eq!(response.playlist().name(), "Mine");
        assert!(!response.playlist().is_public());
        Ok(())
    }

    #[tokio::test]
    async fn update_playlist_duplicate_name_returns_name_already_exists()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(&mut playlists, playlist(3, 4, "Mine", false, Vec::new()));
        let collision = playlist(5, 4, "Taken", false, Vec::new());
        playlists.expect_search().times(1).return_once(move |_| {
            let found = collision.clone();
            Box::pin(async move { Ok(vec![found]) })
        });
        playlists.expect_save().times(0);
        let harness = harness_with(users, playlists);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdatePlaylistCommand::new(
                4,
                3,
                Some("Taken".to_owned()),
                None,
                None,
            ))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(UpdatePlaylistError::NameAlreadyExists)
        ));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_playlist_invalid_name_returns_invalid_name() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(&mut playlists, playlist(3, 4, "Mine", false, Vec::new()));
        playlists.expect_save().times(0);
        let harness = harness_with(users, playlists);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdatePlaylistCommand::new(
                4,
                3,
                Some("   ".to_owned()),
                None,
                None,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdatePlaylistError::InvalidName)));
        Ok(())
    }

    #[tokio::test]
    async fn update_playlist_non_owner_metadata_returns_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange: the caller can read the public playlist but does not own it.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(&mut playlists, playlist(3, 4, "Public", true, Vec::new()));
        playlists.expect_save().times(0);
        let harness = harness_with(users, playlists);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdatePlaylistCommand::new(
                9,
                3,
                Some("Renamed".to_owned()),
                None,
                None,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdatePlaylistError::Forbidden)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_playlist_adds_a_recording_for_a_reader() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        let stored = playlist(3, 4, "Public", true, Vec::new());
        let loaded_at = stored.last_modified_at();
        expect_playlist(&mut playlists, stored);
        let added = track(30, 0)?;
        let saved_track = added.clone();
        playlists
            .expect_add_track()
            .times(1)
            .withf(|playlist_id, recording_id| *playlist_id == 3 && *recording_id == 30)
            .return_once(move |_, _| Box::pin(async move { Ok(Some(added)) }));
        // The adapter re-reads the aggregate after the placement insert, so the
        // saved playlist answers with the recording the use case appended.
        let saved = playlist(3, 4, "Public", true, vec![saved_track]);
        playlists
            .expect_save()
            .times(1)
            .withf(move |candidate| {
                candidate.tracks().is_empty() && candidate.last_modified_at() > loaded_at
            })
            .return_once(move |_| {
                let found = saved.clone();
                Box::pin(async move { Ok(found) })
            });
        let harness = harness_with(users, playlists);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdatePlaylistCommand::new(9, 3, None, None, Some(30)))
            .await?;

        // Assert
        assert_eq!(result.playlist().id(), 3);
        let tracks = result.playlist().tracks();
        assert_eq!(tracks.len(), 1);
        let track = tracks.first().ok_or("the added track is missing")?;
        assert_eq!(track.id(), 30);
        assert_eq!(track.index().playlist_index(), Some(0));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_playlist_unknown_recording_returns_no_such_recording()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(&mut playlists, playlist(3, 4, "Mine", false, Vec::new()));
        playlists
            .expect_add_track()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(None) }));
        playlists.expect_save().times(0);
        let harness = harness_with(users, playlists);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdatePlaylistCommand::new(4, 3, None, None, Some(999)))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdatePlaylistError::NoSuchRecording)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_playlist_duplicate_recording_returns_already_present()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(
            &mut playlists,
            playlist(3, 4, "Mine", false, vec![track(30, 0)?]),
        );
        playlists.expect_add_track().times(0);
        let harness = harness_with(users, playlists);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdatePlaylistCommand::new(4, 3, None, None, Some(30)))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(UpdatePlaylistError::RecordingAlreadyPresent)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn update_playlist_raced_duplicate_returns_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange: the schema guard rejects a concurrent duplicate add.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(&mut playlists, playlist(3, 4, "Mine", false, Vec::new()));
        playlists
            .expect_add_track()
            .times(1)
            .returning(|_, _| Box::pin(async { Err(RepositoryError::AlreadyExist) }));
        let harness = harness_with(users, playlists);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdatePlaylistCommand::new(4, 3, None, None, Some(30)))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdatePlaylistError::Conflict)));
        Ok(())
    }

    #[tokio::test]
    async fn update_playlist_mixed_owner_only_and_add_rejects_everything()
    -> Result<(), Box<dyn Error>> {
        // Arrange: a non-owner sends a rename together with an add.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(&mut playlists, playlist(3, 4, "Public", true, Vec::new()));
        playlists.expect_add_track().times(0);
        playlists.expect_save().times(0);
        let harness = harness_with(users, playlists);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdatePlaylistCommand::new(
                9,
                3,
                Some("Renamed".to_owned()),
                None,
                Some(30),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdatePlaylistError::Forbidden)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_playlist_invisible_playlist_returns_no_such_playlist()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(&mut playlists, playlist(3, 4, "Mine", false, Vec::new()));
        let harness = harness_with(users, playlists);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdatePlaylistCommand::new(9, 3, None, None, Some(30)))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdatePlaylistError::NoSuchPlaylist)));
        Ok(())
    }

    #[tokio::test]
    async fn update_playlist_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, MockMusicPlaylistRepository::new());
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdatePlaylistCommand::new(999, 3, None, None, None))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdatePlaylistError::NoSuchCaller)));
        Ok(())
    }

    #[tokio::test]
    async fn update_playlist_save_failure_rolls_back() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(&mut playlists, playlist(3, 4, "Mine", false, Vec::new()));
        playlists
            .expect_save()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, playlists);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdatePlaylistCommand::new(4, 3, None, Some(true), None))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdatePlaylistError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
