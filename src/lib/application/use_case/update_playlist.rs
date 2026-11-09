use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use crate::application::port::update_playlist::{
    UpdatePlaylistCommand, UpdatePlaylistError, UpdatePlaylistResponse, UpdatePlaylistUseCase,
};
use crate::application::security_event;
use crate::domain::model::music_playlist::MusicPlaylistError;
use crate::domain::model::music_recording::{MusicRecording, TrackIndex};
use crate::domain::model::permission::Action;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::music_album_repository::{
    MusicAlbumRepository as _, MusicRecordingFilter,
};
use crate::domain::port::music_playlist_repository::{
    MusicPlaylistFilter, MusicPlaylistRepository as _,
};
use crate::domain::port::permission_repository::{PermissionFilter, PermissionRepository as _};
use crate::domain::port::unit_of_work::{UnitOfWork as _, UnitOfWorkFactory};
use crate::domain::port::user_repository::{UserFilter, UserRepository as _};
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::service::authorization::{Authorization, Decision};
use crate::domain::service::name_policy::{NamePolicy, NamePolicyError};

/// Use case implementation for updating a playlist's metadata or tracks.
pub(crate) struct UpdatePlaylist<F> {
    /// Factory opening the unit of work wrapping the update.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> UpdatePlaylist<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> UpdatePlaylistUseCase for UpdatePlaylist<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
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
                .begin_write()
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

                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| UpdatePlaylistError::Unknown(error.into()))?;

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

                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::PlaylistRead, &playlist)
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(UpdatePlaylistError::NoSuchPlaylist);
                }

                // Metadata is owner-only; the check covers the whole mixed
                // request, so a non-owner's `add_track` is rejected with it and
                // nothing is applied.
                let metadata_requested = command.name().is_some() || command.is_public().is_some();
                if metadata_requested
                    && let Decision::Deny(denial) = Authorization::decide(
                        &caller,
                        &permissions,
                        Action::PlaylistManage,
                        &playlist,
                    )
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(if denial.conceals() {
                        UpdatePlaylistError::NoSuchPlaylist
                    } else {
                        UpdatePlaylistError::Forbidden
                    });
                }
                if command.add_track().is_some()
                    && let Decision::Deny(denial) = Authorization::decide(
                        &caller,
                        &permissions,
                        Action::PlaylistModerate,
                        &playlist,
                    )
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(if denial.conceals() {
                        UpdatePlaylistError::NoSuchPlaylist
                    } else {
                        UpdatePlaylistError::Forbidden
                    });
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
                    let recorded = unit_of_work
                        .music_albums()
                        .search_recordings(&MusicRecordingFilter {
                            id: Some(recording_id),
                            ..MusicRecordingFilter::default()
                        })
                        .await
                        .map_err(|error| UpdatePlaylistError::Unknown(error.into()))?
                        .into_iter()
                        .next()
                        .ok_or(UpdatePlaylistError::NoSuchRecording)?;
                    let next_index = playlist
                        .tracks()
                        .iter()
                        .filter_map(|track| track.index().playlist_index())
                        .max()
                        .map_or(0, |index| index.saturating_add(1));
                    let placed = MusicRecording::try_new(
                        recorded.id(),
                        recorded.name().map(str::to_owned),
                        recorded.isrc_code().map(str::to_owned),
                        recorded.audio_id(),
                        recorded.file_id(),
                        recorded.music_group_ids().map(<[_]>::to_vec),
                        TrackIndex::PlaylistIndex(next_index),
                    )
                    .map_err(|error| UpdatePlaylistError::Unknown(error.into()))?;
                    playlist
                        .add_track(placed)
                        .map_err(|error| UpdatePlaylistError::Unknown(error.into()))?;
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

    use super::*;
    use crate::domain::model::music_playlist::MusicPlaylist;
    use crate::domain::model::music_recording::{MusicRecording, TrackIndex};
    use crate::domain::model::permission::{Permission, Scope};
    use crate::domain::model::user::{Role, User};
    use crate::domain::port::music_album_repository::MockMusicAlbumRepository;
    use crate::domain::port::music_playlist_repository::MockMusicPlaylistRepository;
    use crate::domain::port::permission_repository::MockPermissionRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::*;
    use chrono::NaiveDateTime;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    type UseCase = UpdatePlaylist<TestFactory>;

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
    async fn update_playlist_root_admin_renames_another_users_playlist()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 0, Role::Admin)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        let stored = playlist(3, 4, "Mine", false, Vec::new());
        let loaded_at = stored.last_modified_at();
        expect_playlist(&mut playlists, stored);
        playlists
            .expect_search()
            .times(1)
            .withf(|filter| filter.name.as_deref() == Some("Root"))
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        playlists
            .expect_save()
            .times(1)
            .withf(move |saved| saved.name() == "Root" && saved.last_modified_at() > loaded_at)
            .returning(|saved| Box::pin(async move { Ok(saved) }));
        let harness = harness_with(users, playlists);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(UpdatePlaylistCommand::new(
                0,
                3,
                Some("Root".to_owned()),
                None,
                None,
            ))
            .await?;

        // Assert
        assert_eq!(response.playlist().name(), "Root");
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_playlist_readable_but_not_moderatable_add_track_returns_forbidden()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(&mut playlists, playlist(3, 4, "Public", true, Vec::new()));
        playlists.expect_save().times(0);
        let mut permissions = MockPermissionRepository::new();
        expect_permissions(
            &mut permissions,
            9,
            vec![Permission::try_new(
                9,
                Action::PlaylistModerate,
                Scope::None,
            )?],
        );
        let unit_of_work = TestUnitOfWork {
            music_playlists: playlists,
            permissions,
            users,
            ..TestUnitOfWork::default()
        };
        let harness = GalleryFactoryHarness::new(unit_of_work);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdatePlaylistCommand::new(9, 3, None, None, Some(7)))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdatePlaylistError::Forbidden)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
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
        let recorded = added.clone();
        let mut albums = MockMusicAlbumRepository::new();
        albums
            .expect_search_recordings()
            .times(1)
            .withf(|filter| filter.id == Some(30))
            .return_once(move |_| Box::pin(async move { Ok(vec![recorded]) }));
        // The use case appends the recording to the aggregate in memory and
        // saves the whole playlist, so the persisted value carries the track.
        let saved = playlist(3, 4, "Public", true, vec![added]);
        playlists
            .expect_save()
            .times(1)
            .withf(move |candidate| {
                candidate.tracks().len() == 1
                    && candidate
                        .tracks()
                        .first()
                        .is_some_and(|track| track.id() == 30)
                    && candidate.last_modified_at() > loaded_at
            })
            .return_once(move |_| {
                let found = saved.clone();
                Box::pin(async move { Ok(found) })
            });
        let unit_of_work = TestUnitOfWork {
            music_albums: albums,
            permissions: no_permissions(),
            music_playlists: playlists,
            users,
            ..TestUnitOfWork::default()
        };
        let harness = GalleryFactoryHarness::new(unit_of_work);
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
        let mut albums = MockMusicAlbumRepository::new();
        albums
            .expect_search_recordings()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        playlists.expect_save().times(0);
        let unit_of_work = TestUnitOfWork {
            music_albums: albums,
            permissions: no_permissions(),
            music_playlists: playlists,
            users,
            ..TestUnitOfWork::default()
        };
        let harness = GalleryFactoryHarness::new(unit_of_work);
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
        playlists.expect_save().times(0);
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
    async fn update_playlist_raced_duplicate_returns_name_already_exists()
    -> Result<(), Box<dyn Error>> {
        // Arrange: saving the aggregate races another writer of the same name.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_playlist(&mut playlists, playlist(3, 4, "Mine", false, Vec::new()));
        let recorded = track(30, 0)?;
        let mut albums = MockMusicAlbumRepository::new();
        albums
            .expect_search_recordings()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![recorded]) }));
        playlists
            .expect_save()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::AlreadyExist) }));
        let unit_of_work = TestUnitOfWork {
            music_albums: albums,
            permissions: no_permissions(),
            music_playlists: playlists,
            users,
            ..TestUnitOfWork::default()
        };
        let harness = GalleryFactoryHarness::new(unit_of_work);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdatePlaylistCommand::new(4, 3, None, None, Some(30)))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(UpdatePlaylistError::NameAlreadyExists)
        ));
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
