use crate::application::authorization::authorize;
use crate::domain::model::{ChildKind, ResourceKind, ResourceRef};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use tracing::warn;

use crate::application::port::{
    UpdatePlaylistCommand, UpdatePlaylistError, UpdatePlaylistResponse, UpdatePlaylistUseCase,
};
use crate::application::{FieldUpdate, cover_is_viewable};
use crate::domain::model::{Action, MusicPlaylistError, MusicRecording, TrackIndex, Visibility};
use crate::domain::port::{
    IdentityUnitOfWork, LibraryUnitOfWork, MusicAlbumRepository as _, MusicPlaylistFilter,
    MusicPlaylistRepository as _, MusicRecordingFilter, RepositoryError, ResourceRepository as _,
    UnitOfWork as _, UnitOfWorkFactory, UserFilter, UserRepository as _, UserUnitOfWork,
};
use crate::domain::service::{NamePolicy, NamePolicyError};

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

                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::MusicPlaylist, playlist.id()),
                    Action::View,
                    None,
                )
                .await
                .map_err(|_| UpdatePlaylistError::NoSuchPlaylist)?;

                // Metadata is owner-only; the check covers the whole mixed
                // request, so a non-owner's `add_track` is rejected with it and
                // nothing is applied.
                let metadata_requested = command.name().is_some()
                    || !matches!(command.cover_image_id(), FieldUpdate::Absent);
                if metadata_requested {
                    authorize(
                        &mut unit_of_work.resources(),
                        &caller,
                        ResourceRef::new(ResourceKind::MusicPlaylist, playlist.id()),
                        Action::EditMetadata,
                        None,
                    )
                    .await
                    .map_err(|error| {
                        if error.conceals() {
                            UpdatePlaylistError::NoSuchPlaylist
                        } else {
                            UpdatePlaylistError::Forbidden
                        }
                    })?;
                }
                // Visibility is not metadata: it moves the resource between the
                // grant tiers, so it needs `ManagePermissions` on its own
                // (`REQ-AUTHORIZATION-008`).
                if command.is_public().is_some() {
                    authorize(
                        &mut unit_of_work.resources(),
                        &caller,
                        ResourceRef::new(ResourceKind::MusicPlaylist, playlist.id()),
                        Action::ManagePermissions,
                        None,
                    )
                    .await
                    .map_err(|error| {
                        if error.conceals() {
                            UpdatePlaylistError::NoSuchPlaylist
                        } else {
                            UpdatePlaylistError::Forbidden
                        }
                    })?;
                }
                if command.add_track().is_some() {
                    authorize(
                        &mut unit_of_work.resources(),
                        &caller,
                        ResourceRef::new(ResourceKind::MusicPlaylist, playlist.id()),
                        Action::CreateChild,
                        Some(ChildKind::MusicRecording),
                    )
                    .await
                    .map_err(|error| {
                        if error.conceals() {
                            UpdatePlaylistError::NoSuchPlaylist
                        } else {
                            UpdatePlaylistError::Forbidden
                        }
                    })?;
                }

                if let Some(requested_name) = command.name() {
                    let name = NamePolicy::validate(requested_name.to_owned()).map_err(
                        |error| match error {
                            NamePolicyError::Empty => {
                                warn!(
                                    target: "security",
                                    event = "input_validation_failed",
                                    field = %"name",
                                    reason = %"empty",
                                    "rejected a client-supplied value"
                                );
                                UpdatePlaylistError::InvalidName
                            }
                            NamePolicyError::SurroundingWhitespace => {
                                warn!(
                                    target: "security",
                                    event = "input_validation_failed",
                                    field = %"name",
                                    reason = %"surrounding_whitespace",
                                    "rejected a client-supplied value"
                                );
                                UpdatePlaylistError::InvalidName
                            }
                            NamePolicyError::TooLong => {
                                warn!(
                                    target: "security",
                                    event = "input_validation_failed",
                                    field = %"name",
                                    reason = %"too_long",
                                    "rejected a client-supplied value"
                                );
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
                    // The container check admits the write; the target must also
                    // be readable, which is what `authorization` requires of a
                    // playlist entry (`REQ-AUTHORIZATION-026`).
                    authorize(
                        &mut unit_of_work.resources(),
                        &caller,
                        ResourceRef::new(ResourceKind::MusicRecording, recorded.id()),
                        Action::ViewMetadata,
                        None,
                    )
                    .await
                    .map_err(|_| UpdatePlaylistError::Forbidden)?;
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
                if let FieldUpdate::Set(cover_image_id) = command.cover_image_id() {
                    if !cover_is_viewable(&mut unit_of_work, &caller, Some(cover_image_id)).await {
                        return Err(UpdatePlaylistError::InvalidCover);
                    }
                    let backed = unit_of_work
                        .music_playlists()
                        .search(&MusicPlaylistFilter {
                            cover_image_id: Some(cover_image_id),
                            ..MusicPlaylistFilter::default()
                        })
                        .await
                        .map_err(|error| UpdatePlaylistError::Unknown(error.into()))?;
                    if backed.iter().any(|backer| backer.id() != playlist.id()) {
                        return Err(UpdatePlaylistError::InvalidCover);
                    }
                }
                playlist.set_cover_image_id(
                    command.cover_image_id().resolve(playlist.cover_image_id()),
                );
                playlist.touch(Utc::now().naive_utc());

                let saved = unit_of_work
                    .music_playlists()
                    .save(playlist)
                    .await
                    .map_err(|error| match error {
                        RepositoryError::AlreadyExist => UpdatePlaylistError::NameAlreadyExists,
                        RepositoryError::Conflict | RepositoryError::DataIntegrityViolation => {
                            UpdatePlaylistError::InvalidCover
                        }
                        other => UpdatePlaylistError::Unknown(other.into()),
                    })?;

                // The playlist flag and the resource's visibility are one fact:
                // storing one without the other would leave the representation
                // and the decision disagreeing.
                if let Some(is_public) = command.is_public() {
                    let visibility = if is_public {
                        Visibility::Public
                    } else {
                        Visibility::Private
                    };
                    let updated = unit_of_work
                        .resources()
                        .set_visibility(
                            ResourceRef::new(ResourceKind::MusicPlaylist, saved.id()),
                            visibility,
                        )
                        .await
                        .map_err(|error| UpdatePlaylistError::Unknown(error.into()))?;
                    if !updated {
                        return Err(UpdatePlaylistError::Unknown(anyhow::anyhow!(
                            "the playlist has no stored resource"
                        )));
                    }
                }

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
    use crate::application::FieldUpdate;

    use super::*;
    use crate::domain::model::{MusicPlaylist, MusicRecording, Role, TrackIndex, User};

    use crate::domain::port::{
        MockMusicAlbumRepository, MockMusicPlaylistRepository, MockResourceRepository,
        MockUserRepository,
    };

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

    #[tokio::test]
    async fn update_playlist_renames_and_sets_visibility() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository,
               caller_id: i64,
               role: Role|
              -> Result<(), Box<dyn Error>> {
            let caller = User::try_new(caller_id, "caller".to_owned(), None, caller_id, role)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        let stored = playlist(3, 4, "Mine", false, Vec::new());
        let loaded_at = stored.last_modified_at();
        (move |playlists: &mut MockMusicPlaylistRepository, playlist: MusicPlaylist| {
            let playlist_id = playlist.id();
            playlists
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(playlist_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![playlist]) }));
        })(&mut playlists, stored);
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
        let harness = (move |users: MockUserRepository,
                             playlists: MockMusicPlaylistRepository|
              -> GalleryFactoryHarness {
            let unit_of_work = TestUnitOfWork {
                music_playlists: playlists,
                users,
                ..TestUnitOfWork::default()
            };
            GalleryFactoryHarness::new(unit_of_work)
        })(users, playlists);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(UpdatePlaylistCommand::new(
                4,
                3,
                Some("New".to_owned()),
                Some(true),
                None,
                FieldUpdate::Absent,
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
        (move |users: &mut MockUserRepository,
               caller_id: i64,
               role: Role|
              -> Result<(), Box<dyn Error>> {
            let caller = User::try_new(caller_id, "caller".to_owned(), None, caller_id, role)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 0, Role::Admin)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        let stored = playlist(3, 4, "Mine", false, Vec::new());
        let loaded_at = stored.last_modified_at();
        (move |playlists: &mut MockMusicPlaylistRepository, playlist: MusicPlaylist| {
            let playlist_id = playlist.id();
            playlists
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(playlist_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![playlist]) }));
        })(&mut playlists, stored);
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
        let harness = (move |users: MockUserRepository,
                             playlists: MockMusicPlaylistRepository|
              -> GalleryFactoryHarness {
            let unit_of_work = TestUnitOfWork {
                music_playlists: playlists,
                users,
                ..TestUnitOfWork::default()
            };
            GalleryFactoryHarness::new(unit_of_work)
        })(users, playlists);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(UpdatePlaylistCommand::new(
                0,
                3,
                Some("Root".to_owned()),
                None,
                None,
                FieldUpdate::Absent,
            ))
            .await?;

        // Assert
        assert_eq!(response.playlist().name(), "Root");
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }
    #[tokio::test]
    async fn update_playlist_empty_body_returns_the_unchanged_playlist()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository,
               caller_id: i64,
               role: Role|
              -> Result<(), Box<dyn Error>> {
            let caller = User::try_new(caller_id, "caller".to_owned(), None, caller_id, role)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        let stored = playlist(3, 4, "Mine", false, Vec::new());
        let loaded_at = stored.last_modified_at();
        (move |playlists: &mut MockMusicPlaylistRepository, playlist: MusicPlaylist| {
            let playlist_id = playlist.id();
            playlists
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(playlist_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![playlist]) }));
        })(&mut playlists, stored);
        (move |playlists: &mut MockMusicPlaylistRepository,
               expected_name: &'static str,
               loaded_at: NaiveDateTime| {
            playlists
                .expect_save()
                .times(1)
                .withf(move |saved| {
                    saved.name() == expected_name && saved.last_modified_at() > loaded_at
                })
                .returning(|saved| Box::pin(async move { Ok(saved) }));
        })(&mut playlists, "Mine", loaded_at);
        let harness = (move |users: MockUserRepository,
                             playlists: MockMusicPlaylistRepository|
              -> GalleryFactoryHarness {
            let unit_of_work = TestUnitOfWork {
                music_playlists: playlists,
                users,
                ..TestUnitOfWork::default()
            };
            GalleryFactoryHarness::new(unit_of_work)
        })(users, playlists);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(UpdatePlaylistCommand::new(
                4,
                3,
                None,
                None,
                None,
                FieldUpdate::Absent,
            ))
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
        (move |users: &mut MockUserRepository,
               caller_id: i64,
               role: Role|
              -> Result<(), Box<dyn Error>> {
            let caller = User::try_new(caller_id, "caller".to_owned(), None, caller_id, role)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        (move |playlists: &mut MockMusicPlaylistRepository, playlist: MusicPlaylist| {
            let playlist_id = playlist.id();
            playlists
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(playlist_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![playlist]) }));
        })(&mut playlists, playlist(3, 4, "Mine", false, Vec::new()));
        let collision = playlist(5, 4, "Taken", false, Vec::new());
        playlists.expect_search().times(1).return_once(move |_| {
            let found = collision.clone();
            Box::pin(async move { Ok(vec![found]) })
        });
        playlists.expect_save().times(0);
        let harness = (move |users: MockUserRepository,
                             playlists: MockMusicPlaylistRepository|
              -> GalleryFactoryHarness {
            let unit_of_work = TestUnitOfWork {
                music_playlists: playlists,
                users,
                ..TestUnitOfWork::default()
            };
            GalleryFactoryHarness::new(unit_of_work)
        })(users, playlists);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdatePlaylistCommand::new(
                4,
                3,
                Some("Taken".to_owned()),
                None,
                None,
                FieldUpdate::Absent,
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
        (move |users: &mut MockUserRepository,
               caller_id: i64,
               role: Role|
              -> Result<(), Box<dyn Error>> {
            let caller = User::try_new(caller_id, "caller".to_owned(), None, caller_id, role)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        (move |playlists: &mut MockMusicPlaylistRepository, playlist: MusicPlaylist| {
            let playlist_id = playlist.id();
            playlists
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(playlist_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![playlist]) }));
        })(&mut playlists, playlist(3, 4, "Mine", false, Vec::new()));
        playlists.expect_save().times(0);
        let harness = (move |users: MockUserRepository,
                             playlists: MockMusicPlaylistRepository|
              -> GalleryFactoryHarness {
            let unit_of_work = TestUnitOfWork {
                music_playlists: playlists,
                users,
                ..TestUnitOfWork::default()
            };
            GalleryFactoryHarness::new(unit_of_work)
        })(users, playlists);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdatePlaylistCommand::new(
                4,
                3,
                Some("   ".to_owned()),
                None,
                None,
                FieldUpdate::Absent,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdatePlaylistError::InvalidName)));
        Ok(())
    }
    #[tokio::test]
    async fn update_playlist_adds_a_recording_for_a_reader() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository,
               caller_id: i64,
               role: Role|
              -> Result<(), Box<dyn Error>> {
            let caller = User::try_new(caller_id, "caller".to_owned(), None, caller_id, role)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 9, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        let stored = playlist(3, 4, "Public", true, Vec::new());
        let loaded_at = stored.last_modified_at();
        (move |playlists: &mut MockMusicPlaylistRepository, playlist: MusicPlaylist| {
            let playlist_id = playlist.id();
            playlists
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(playlist_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![playlist]) }));
        })(&mut playlists, stored);
        let added = track(30, 0)?;
        let recorded = added.clone();
        let mut albums = MockMusicAlbumRepository::new();
        albums
            .expect_search()
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
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
            music_playlists: playlists,
            users,
            ..TestUnitOfWork::default()
        };
        let harness = GalleryFactoryHarness::new(unit_of_work);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdatePlaylistCommand::new(
                9,
                3,
                None,
                None,
                Some(30),
                FieldUpdate::Absent,
            ))
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
        (move |users: &mut MockUserRepository,
               caller_id: i64,
               role: Role|
              -> Result<(), Box<dyn Error>> {
            let caller = User::try_new(caller_id, "caller".to_owned(), None, caller_id, role)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        (move |playlists: &mut MockMusicPlaylistRepository, playlist: MusicPlaylist| {
            let playlist_id = playlist.id();
            playlists
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(playlist_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![playlist]) }));
        })(&mut playlists, playlist(3, 4, "Mine", false, Vec::new()));
        let mut albums = MockMusicAlbumRepository::new();
        albums
            .expect_search_recordings()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        playlists.expect_save().times(0);
        let unit_of_work = TestUnitOfWork {
            music_albums: albums,
            music_playlists: playlists,
            users,
            ..TestUnitOfWork::default()
        };
        let harness = GalleryFactoryHarness::new(unit_of_work);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdatePlaylistCommand::new(
                4,
                3,
                None,
                None,
                Some(999),
                FieldUpdate::Absent,
            ))
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
        (move |users: &mut MockUserRepository,
               caller_id: i64,
               role: Role|
              -> Result<(), Box<dyn Error>> {
            let caller = User::try_new(caller_id, "caller".to_owned(), None, caller_id, role)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        (move |playlists: &mut MockMusicPlaylistRepository, playlist: MusicPlaylist| {
            let playlist_id = playlist.id();
            playlists
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(playlist_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![playlist]) }));
        })(
            &mut playlists,
            playlist(3, 4, "Mine", false, vec![track(30, 0)?]),
        );
        playlists.expect_save().times(0);
        let harness = (move |users: MockUserRepository,
                             playlists: MockMusicPlaylistRepository|
              -> GalleryFactoryHarness {
            let unit_of_work = TestUnitOfWork {
                music_playlists: playlists,
                users,
                ..TestUnitOfWork::default()
            };
            GalleryFactoryHarness::new(unit_of_work)
        })(users, playlists);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdatePlaylistCommand::new(
                4,
                3,
                None,
                None,
                Some(30),
                FieldUpdate::Absent,
            ))
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
        (move |users: &mut MockUserRepository,
               caller_id: i64,
               role: Role|
              -> Result<(), Box<dyn Error>> {
            let caller = User::try_new(caller_id, "caller".to_owned(), None, caller_id, role)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        (move |playlists: &mut MockMusicPlaylistRepository, playlist: MusicPlaylist| {
            let playlist_id = playlist.id();
            playlists
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(playlist_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![playlist]) }));
        })(&mut playlists, playlist(3, 4, "Mine", false, Vec::new()));
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
            music_playlists: playlists,
            users,
            ..TestUnitOfWork::default()
        };
        let harness = GalleryFactoryHarness::new(unit_of_work);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdatePlaylistCommand::new(
                4,
                3,
                None,
                None,
                Some(30),
                FieldUpdate::Absent,
            ))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(UpdatePlaylistError::NameAlreadyExists)
        ));
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
        let harness = (move |users: MockUserRepository,
                             playlists: MockMusicPlaylistRepository|
              -> GalleryFactoryHarness {
            let unit_of_work = TestUnitOfWork {
                music_playlists: playlists,
                users,
                ..TestUnitOfWork::default()
            };
            GalleryFactoryHarness::new(unit_of_work)
        })(users, MockMusicPlaylistRepository::new());
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdatePlaylistCommand::new(
                999,
                3,
                None,
                None,
                None,
                FieldUpdate::Absent,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdatePlaylistError::NoSuchCaller)));
        Ok(())
    }

    #[tokio::test]
    async fn update_playlist_save_failure_rolls_back() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository,
               caller_id: i64,
               role: Role|
              -> Result<(), Box<dyn Error>> {
            let caller = User::try_new(caller_id, "caller".to_owned(), None, caller_id, role)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        (move |playlists: &mut MockMusicPlaylistRepository, playlist: MusicPlaylist| {
            let playlist_id = playlist.id();
            playlists
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(playlist_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![playlist]) }));
        })(&mut playlists, playlist(3, 4, "Mine", false, Vec::new()));
        playlists
            .expect_save()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = (move |users: MockUserRepository,
                             playlists: MockMusicPlaylistRepository|
              -> GalleryFactoryHarness {
            let unit_of_work = TestUnitOfWork {
                music_playlists: playlists,
                users,
                ..TestUnitOfWork::default()
            };
            GalleryFactoryHarness::new(unit_of_work)
        })(users, playlists);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdatePlaylistCommand::new(
                4,
                3,
                None,
                Some(true),
                None,
                FieldUpdate::Absent,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdatePlaylistError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_playlist_replaces_the_cover() -> Result<(), Box<dyn Error>> {
        // Arrange: the named image is viewable and no other playlist backs it.
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .withf(|filter| filter.id == Some(4))
            .returning(|_| {
                let found = User::try_new(4, "caller".to_owned(), None, 4, Role::Standard);
                Box::pin(async move { Ok(found.into_iter().collect()) })
            });
        let stored = playlist(3, 4, "Mine", false, Vec::new());
        let playlist_id = stored.id();
        let mut playlists = MockMusicPlaylistRepository::new();
        playlists
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(playlist_id))
            .return_once(move |_| Box::pin(async move { Ok(vec![stored]) }));
        playlists
            .expect_search()
            .times(1)
            .withf(|filter| filter.cover_image_id == Some(19))
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        playlists
            .expect_save()
            .times(1)
            .withf(|saved| saved.cover_image_id() == Some(19))
            .returning(|saved| Box::pin(async move { Ok(saved) }));
        let unit_of_work = TestUnitOfWork {
            music_playlists: playlists,
            users,
            ..TestUnitOfWork::default()
        };
        let harness = GalleryFactoryHarness::new(unit_of_work);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(UpdatePlaylistCommand::new(
                4,
                3,
                None,
                None,
                None,
                FieldUpdate::Set(19),
            ))
            .await?;

        // Assert
        assert_eq!(response.playlist().cover_image_id(), Some(19));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_playlist_cover_backed_by_another_playlist_returns_invalid_cover()
    -> Result<(), Box<dyn Error>> {
        // Arrange: another playlist already carries the named image.
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .withf(|filter| filter.id == Some(4))
            .returning(|_| {
                let found = User::try_new(4, "caller".to_owned(), None, 4, Role::Standard);
                Box::pin(async move { Ok(found.into_iter().collect()) })
            });
        let stored = playlist(3, 4, "Mine", false, Vec::new());
        let playlist_id = stored.id();
        let mut playlists = MockMusicPlaylistRepository::new();
        playlists
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(playlist_id))
            .return_once(move |_| Box::pin(async move { Ok(vec![stored]) }));
        let backer = playlist(5, 6, "Theirs", false, Vec::new());
        playlists
            .expect_search()
            .times(1)
            .withf(|filter| filter.cover_image_id == Some(19))
            .return_once(move |_| {
                let found = backer.clone();
                Box::pin(async move { Ok(vec![found]) })
            });
        playlists.expect_save().times(0);
        let unit_of_work = TestUnitOfWork {
            music_playlists: playlists,
            users,
            ..TestUnitOfWork::default()
        };
        let harness = GalleryFactoryHarness::new(unit_of_work);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdatePlaylistCommand::new(
                4,
                3,
                None,
                None,
                None,
                FieldUpdate::Set(19),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdatePlaylistError::InvalidCover)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_playlist_unreadable_cover_returns_invalid_cover() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the caller may edit the playlist but not view the image.
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .withf(|filter| filter.id == Some(4))
            .returning(|_| {
                let found = User::try_new(4, "caller".to_owned(), None, 4, Role::Standard);
                Box::pin(async move { Ok(found.into_iter().collect()) })
            });
        let stored = playlist(3, 4, "Mine", false, Vec::new());
        let playlist_id = stored.id();
        let mut playlists = MockMusicPlaylistRepository::new();
        playlists
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(playlist_id))
            .return_once(move |_| Box::pin(async move { Ok(vec![stored]) }));
        playlists.expect_save().times(0);
        let mut resources = MockResourceRepository::new();
        resources.expect_search().returning(|filter, caller| {
            let user_id = caller.id();
            if filter.kind == Some(crate::domain::model::ResourceKind::File) {
                return Box::pin(async { Ok(Vec::new()) });
            }
            let resource = crate::domain::model::Resource::from_stored(
                1,
                filter
                    .kind
                    .unwrap_or(crate::domain::model::ResourceKind::Library),
                filter.entity_id.unwrap_or_default(),
                Some(user_id),
                None,
                crate::domain::model::Visibility::Private,
            );
            let grant = crate::domain::model::Grant::from_stored(
                1,
                crate::domain::model::Principal::User(user_id),
                1,
                crate::domain::model::ResourceRole::Downloader,
                crate::domain::model::ChildKind::All,
                crate::domain::model::GrantEffect::Allow,
            );
            Box::pin(async move {
                Ok(vec![crate::domain::model::ResourceAccess::new(
                    resource,
                    Vec::new(),
                    vec![grant],
                    Vec::new(),
                )])
            })
        });
        let unit_of_work = TestUnitOfWork {
            music_playlists: playlists,
            resources,
            users,
            ..TestUnitOfWork::default()
        };
        let harness = GalleryFactoryHarness::new(unit_of_work);
        let use_case: UseCase = UpdatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdatePlaylistCommand::new(
                4,
                3,
                None,
                None,
                None,
                FieldUpdate::Set(19),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdatePlaylistError::InvalidCover)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
