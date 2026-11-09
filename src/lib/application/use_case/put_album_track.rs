use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::put_album_track::{
    PutAlbumTrackCommand, PutAlbumTrackError, PutAlbumTrackResponse, PutAlbumTrackUseCase,
};
use crate::application::security_event;
use crate::domain::model::music_medium::MusicMedium;
use crate::domain::model::music_recording::{MusicRecording, TrackIndex};
use crate::domain::model::permission::Action;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::music_album_repository::{
    MusicAlbumFilter, MusicAlbumRepository as _, MusicRecordingFilter,
};
use crate::domain::port::permission_repository::{PermissionFilter, PermissionRepository as _};
use crate::domain::port::unit_of_work::{UnitOfWork as _, UnitOfWorkFactory};
use crate::domain::port::user_repository::{UserFilter, UserRepository as _};
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::service::authorization::{Authorization, Decision};

/// Use case implementation for placing a recording at a medium track address.
pub(crate) struct PutAlbumTrack<F> {
    /// Factory opening the unit of work wrapping the write.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> PutAlbumTrack<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> PutAlbumTrackUseCase for PutAlbumTrack<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: PutAlbumTrackCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PutAlbumTrackResponse, PutAlbumTrackError>> + Send + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| PutAlbumTrackError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| PutAlbumTrackError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(PutAlbumTrackError::NoSuchCaller)?;

                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| PutAlbumTrackError::Unknown(error.into()))?;

                let album = unit_of_work
                    .music_albums()
                    .search(&MusicAlbumFilter {
                        id: Some(command.album_id()),
                        ..MusicAlbumFilter::default()
                    })
                    .await
                    .map_err(|error| PutAlbumTrackError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(PutAlbumTrackError::NoSuchAlbum)?;

                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::LibraryRead, &album)
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(PutAlbumTrackError::NoSuchAlbum);
                }
                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::LibraryManage, &album)
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(if denial.conceals() {
                        PutAlbumTrackError::NoSuchAlbum
                    } else {
                        PutAlbumTrackError::Forbidden
                    });
                }

                if album.id() == 0 {
                    return Err(PutAlbumTrackError::DefaultAlbum);
                }
                let medium = album
                    .mediums()
                    .iter()
                    .find(|medium| medium.index().index() == command.medium_index())
                    .ok_or(PutAlbumTrackError::NoSuchMedium)?;
                let destination = medium
                    .tracks()
                    .iter()
                    .find(|track| track.index().medium_index() == Some(command.track_index()));
                if destination.is_some_and(|track| track.id() != command.recording_id()) {
                    return Err(PutAlbumTrackError::Conflict);
                }
                let recorded = unit_of_work
                    .music_albums()
                    .search_recordings(&MusicRecordingFilter {
                        id: Some(command.recording_id()),
                        ..MusicRecordingFilter::default()
                    })
                    .await
                    .map_err(|error| PutAlbumTrackError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(PutAlbumTrackError::NoSuchRecording)?;
                let created = destination.is_none() && recorded.index().medium_index().is_none();

                let mut tracks: Vec<MusicRecording> = medium
                    .tracks()
                    .iter()
                    .filter(|track| {
                        track.index().medium_index() != Some(command.track_index())
                            && track.id() != command.recording_id()
                    })
                    .cloned()
                    .collect();
                tracks.push(
                    MusicRecording::try_new(
                        recorded.id(),
                        recorded.name().map(str::to_owned),
                        recorded.isrc_code().map(str::to_owned),
                        recorded.audio_id(),
                        recorded.file_id(),
                        recorded.music_group_ids().map(<[_]>::to_vec),
                        TrackIndex::MediumIndex(command.track_index()),
                    )
                    .map_err(|_| PutAlbumTrackError::NoSuchRecording)?,
                );

                unit_of_work
                    .music_albums()
                    .save_medium(album.id(), MusicMedium::new(medium.index(), tracks))
                    .await
                    .map_err(|error| {
                        if matches!(
                            error,
                            RepositoryError::Conflict | RepositoryError::AlreadyExist
                        ) {
                            PutAlbumTrackError::Conflict
                        } else {
                            PutAlbumTrackError::Unknown(error.into())
                        }
                    })?
                    .ok_or(PutAlbumTrackError::NoSuchMedium)?;

                let stored = unit_of_work
                    .music_albums()
                    .search(&MusicAlbumFilter {
                        id: Some(album.id()),
                        ..MusicAlbumFilter::default()
                    })
                    .await
                    .map_err(|error| PutAlbumTrackError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(PutAlbumTrackError::NoSuchAlbum)?;

                Ok(PutAlbumTrackResponse::new(
                    stored,
                    command.medium_index(),
                    command.track_index(),
                    created,
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| PutAlbumTrackError::Unknown(error.into()))?;
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
    use crate::domain::alias::NumericID;
    use crate::domain::model::music_album::MusicAlbum;
    use crate::domain::model::music_medium::{MediumIndex, MusicMedium};
    use crate::domain::model::music_recording::{MusicRecording, TrackIndex};
    use crate::domain::model::user::{Role, User};
    use crate::domain::port::music_album_repository::MockMusicAlbumRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::*;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = PutAlbumTrack<TestFactory>;

    /// Expect the caller load to find an administrator.
    fn expect_admin(
        users: &mut MockUserRepository,
        caller_id: NumericID,
    ) -> Result<(), Box<dyn Error>> {
        let caller = User::try_new(caller_id, "admin".to_owned(), None, caller_id, Role::Admin)?;
        users
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(caller_id))
            .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
        Ok(())
    }

    /// Build a Library test harness around the given repository.
    fn harness_with(
        users: MockUserRepository,
        albums: MockMusicAlbumRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            music_albums: albums,
            permissions: no_permissions(),
            users,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
    }

    /// Expect the album load to find `album`.
    fn expect_album(albums: &mut MockMusicAlbumRepository, album: MusicAlbum) {
        let album_id = album.id();
        albums
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(album_id))
            .return_once(move |_| {
                let found = album.clone();
                Box::pin(async move { Ok(vec![found]) })
            });
    }

    /// The album holding one empty `CD` medium at index 1.
    fn album_with_medium(id: NumericID) -> Result<MusicAlbum, Box<dyn Error>> {
        sample_album(id, vec![MusicMedium::new(MediumIndex::Cd(1), Vec::new())])
    }

    /// The recording every placement test moves, at `index`.
    fn recording_at(index: TrackIndex) -> Result<MusicRecording, Box<dyn Error>> {
        Ok(MusicRecording::try_new(
            5,
            Some("So What".to_owned()),
            None,
            1,
            2,
            None,
            index,
        )?)
    }

    /// The recording every placement test moves, already placed at `index`.
    fn recording(index: u8) -> Result<MusicRecording, Box<dyn Error>> {
        recording_at(TrackIndex::MediumIndex(index))
    }

    /// The album holding one `CD` medium with `recording_id` at `track_index`.
    fn album_with_recording(
        id: NumericID,
        recording_id: NumericID,
        track_index: u8,
    ) -> Result<MusicAlbum, Box<dyn Error>> {
        let placed = MusicRecording::try_new(
            recording_id,
            Some("So What".to_owned()),
            None,
            1,
            2,
            None,
            TrackIndex::MediumIndex(track_index),
        )?;
        sample_album(id, vec![MusicMedium::new(MediumIndex::Cd(1), vec![placed])])
    }

    /// Expect the recording lookup backing a placement to find `recorded`.
    fn expect_recording(albums: &mut MockMusicAlbumRepository, recorded: MusicRecording) {
        let recording_id = recorded.id();
        albums
            .expect_search_recordings()
            .times(1)
            .withf(move |filter| filter.id == Some(recording_id))
            .return_once(move |_| {
                let found = recorded.clone();
                Box::pin(async move { Ok(vec![found]) })
            });
    }

    /// Expect the post-write album reload.
    fn expect_reload(albums: &mut MockMusicAlbumRepository, album: MusicAlbum) {
        albums.expect_search().times(1).return_once(move |_| {
            let found = album.clone();
            Box::pin(async move { Ok(vec![found]) })
        });
    }

    #[tokio::test]
    async fn put_album_track_places_a_recording() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album_with_medium(7)?);
        let placed = recording_at(TrackIndex::Unplaced)?;
        expect_recording(&mut albums, placed);
        albums
            .expect_save_medium()
            .times(1)
            .withf(|album_id, medium| {
                *album_id == 7
                    && medium.index() == MediumIndex::Cd(1)
                    && medium.tracks().len() == 1
                    && medium.tracks().first().is_some_and(|track| {
                        track.id() == 5 && track.index().medium_index() == Some(1)
                    })
            })
            .return_once(|_, medium| Box::pin(async move { Ok(Some(medium)) }));
        expect_reload(&mut albums, album_with_medium(7)?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = PutAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(PutAlbumTrackCommand::new(1, 7, 1, 1, 5))
            .await?;

        // Assert
        assert!(response.created());
        assert_eq!(response.track_index(), 1);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn put_album_track_moves_a_recording() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album_with_medium(7)?);
        expect_recording(&mut albums, recording(2)?);
        albums
            .expect_save_medium()
            .times(1)
            .return_once(|_, medium| Box::pin(async move { Ok(Some(medium)) }));
        expect_reload(&mut albums, album_with_medium(7)?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = PutAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(PutAlbumTrackCommand::new(1, 7, 1, 2, 5))
            .await?;

        // Assert
        assert!(!response.created());
        Ok(())
    }

    #[tokio::test]
    async fn put_album_track_same_address_is_unchanged() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album_with_recording(7, 5, 1)?);
        expect_recording(&mut albums, recording(1)?);
        albums
            .expect_save_medium()
            .times(1)
            .return_once(|_, medium| Box::pin(async move { Ok(Some(medium)) }));
        expect_reload(&mut albums, album_with_medium(7)?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = PutAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(PutAlbumTrackCommand::new(1, 7, 1, 1, 5))
            .await?;

        // Assert
        assert!(!response.created());
        Ok(())
    }

    #[tokio::test]
    async fn put_album_track_occupied_destination_is_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album_with_recording(7, 6, 1)?);
        albums.expect_save_medium().times(0);
        let harness = harness_with(users, albums);
        let use_case: UseCase = PutAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(PutAlbumTrackCommand::new(1, 7, 1, 1, 5))
            .await;

        // Assert
        assert!(matches!(result, Err(PutAlbumTrackError::Conflict)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn put_album_track_unknown_recording_is_unprocessable() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album_with_medium(7)?);
        albums
            .expect_search_recordings()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        albums.expect_save_medium().times(0);
        let harness = harness_with(users, albums);
        let use_case: UseCase = PutAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(PutAlbumTrackCommand::new(1, 7, 1, 1, 404))
            .await;

        // Assert
        assert!(matches!(result, Err(PutAlbumTrackError::NoSuchRecording)));
        Ok(())
    }

    #[tokio::test]
    async fn put_album_track_unknown_medium_is_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album_with_medium(7)?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = PutAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(PutAlbumTrackCommand::new(1, 7, 2, 1, 5))
            .await;

        // Assert
        assert!(matches!(result, Err(PutAlbumTrackError::NoSuchMedium)));
        Ok(())
    }

    #[tokio::test]
    async fn put_album_track_rejects_the_default_album() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album_with_medium(0)?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = PutAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(PutAlbumTrackCommand::new(1, 0, 1, 1, 5))
            .await;

        // Assert
        assert!(matches!(result, Err(PutAlbumTrackError::DefaultAlbum)));
        Ok(())
    }

    #[tokio::test]
    async fn put_album_track_dependency_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange: the placement check fails at the port.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album_with_medium(7)?);
        albums
            .expect_search_recordings()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::Unavailable) }));
        let harness = harness_with(users, albums);
        let use_case: UseCase = PutAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(PutAlbumTrackCommand::new(1, 7, 1, 1, 5))
            .await;

        // Assert
        assert!(matches!(result, Err(PutAlbumTrackError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn put_album_track_standard_user_is_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album_with_medium(7)?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = PutAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(PutAlbumTrackCommand::new(1, 7, 1, 1, 5))
            .await;

        // Assert
        assert!(matches!(result, Err(PutAlbumTrackError::Forbidden)));
        Ok(())
    }
}
