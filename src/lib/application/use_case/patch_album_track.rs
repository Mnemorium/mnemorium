use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::field_update::FieldUpdate;
use crate::application::port::patch_album_track::PatchAlbumTrackCommand;
use crate::application::port::patch_album_track::PatchAlbumTrackError;
use crate::application::port::patch_album_track::PatchAlbumTrackResponse;
use crate::application::port::patch_album_track::PatchAlbumTrackUseCase;
use crate::application::security_event;
use crate::domain::model::music_recording::MusicRecordingError;
use crate::domain::model::permission::Action;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::music_album_repository::MusicAlbumFilter;
use crate::domain::port::music_album_repository::MusicAlbumRepository as _;
use crate::domain::port::permission_repository::PermissionFilter;
use crate::domain::port::permission_repository::PermissionRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::service::authorization::Authorization;
use crate::domain::service::authorization::Decision;

/// Use case implementation for editing the shared recording behind a track.
pub struct PatchAlbumTrack<F> {
    /// Factory opening the unit of work wrapping the write.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> PatchAlbumTrack<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> PatchAlbumTrackUseCase for PatchAlbumTrack<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: PatchAlbumTrackCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PatchAlbumTrackResponse, PatchAlbumTrackError>>
                + Send
                + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| PatchAlbumTrackError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| PatchAlbumTrackError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(PatchAlbumTrackError::NoSuchCaller)?;

                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| PatchAlbumTrackError::Unknown(error.into()))?;

                let album = unit_of_work
                    .music_albums()
                    .search(&MusicAlbumFilter {
                        id: Some(command.album_id()),
                        ..MusicAlbumFilter::default()
                    })
                    .await
                    .map_err(|error| PatchAlbumTrackError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(PatchAlbumTrackError::NoSuchAlbum)?;

                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::LibraryRead, &album)
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(PatchAlbumTrackError::NoSuchAlbum);
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
                        PatchAlbumTrackError::NoSuchAlbum
                    } else {
                        PatchAlbumTrackError::Forbidden
                    });
                }

                if album.id() == 0 {
                    return Err(PatchAlbumTrackError::DefaultAlbum);
                }

                let medium = album
                    .mediums()
                    .iter()
                    .find(|medium| medium.index().index() == command.medium_index())
                    .ok_or(PatchAlbumTrackError::NoSuchMedium)?;
                let mut recording = medium
                    .tracks()
                    .iter()
                    .find(|track| track.index().medium_index() == Some(command.track_index()))
                    .cloned()
                    .ok_or(PatchAlbumTrackError::NoSuchTrack)?;

                match command.name().clone() {
                    FieldUpdate::Absent => {}
                    FieldUpdate::Clear => {
                        recording.set_name(None).map_err(name_error)?;
                    }
                    FieldUpdate::Set(name) => {
                        recording.set_name(Some(name)).map_err(name_error)?;
                    }
                }
                match command.isrc_code().clone() {
                    FieldUpdate::Absent => {}
                    FieldUpdate::Clear => {
                        recording.set_isrc_code(None).map_err(isrc_error)?;
                    }
                    FieldUpdate::Set(isrc_code) => {
                        recording
                            .set_isrc_code(Some(isrc_code))
                            .map_err(isrc_error)?;
                    }
                }

                unit_of_work
                    .music_albums()
                    .save_recording(recording)
                    .await
                    .map_err(|error| {
                        if matches!(error, RepositoryError::AlreadyExist) {
                            PatchAlbumTrackError::IsrcAlreadyExists
                        } else if matches!(error, RepositoryError::Conflict) {
                            PatchAlbumTrackError::Conflict
                        } else {
                            PatchAlbumTrackError::Unknown(error.into())
                        }
                    })?
                    .ok_or(PatchAlbumTrackError::NoSuchTrack)?;

                let stored = unit_of_work
                    .music_albums()
                    .search(&MusicAlbumFilter {
                        id: Some(album.id()),
                        ..MusicAlbumFilter::default()
                    })
                    .await
                    .map_err(|error| PatchAlbumTrackError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(PatchAlbumTrackError::NoSuchAlbum)?;

                Ok(PatchAlbumTrackResponse::new(
                    stored,
                    command.medium_index(),
                    command.track_index(),
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| PatchAlbumTrackError::Unknown(error.into()))?;
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

/// Map a failed recording-name write to the use-case error.
fn name_error(error: MusicRecordingError) -> PatchAlbumTrackError {
    match error {
        MusicRecordingError::InvalidIsrcCode => PatchAlbumTrackError::InvalidIsrc,
        MusicRecordingError::InvalidName => {
            security_event::input_validation_failed("name", "empty");
            PatchAlbumTrackError::InvalidName
        }
        MusicRecordingError::Unknown(source) => PatchAlbumTrackError::Unknown(source),
    }
}

/// Map a failed recording-ISRC write to the use-case error.
fn isrc_error(error: MusicRecordingError) -> PatchAlbumTrackError {
    match error {
        MusicRecordingError::InvalidIsrcCode => {
            security_event::input_validation_failed("isrc_code", "invalid_length");
            PatchAlbumTrackError::InvalidIsrc
        }
        MusicRecordingError::InvalidName => PatchAlbumTrackError::InvalidName,
        MusicRecordingError::Unknown(source) => PatchAlbumTrackError::Unknown(source),
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use crate::application::field_update::FieldUpdate;
    use crate::application::port::patch_album_track::PatchAlbumTrackCommand;
    use crate::application::port::patch_album_track::PatchAlbumTrackError;
    use crate::application::port::patch_album_track::PatchAlbumTrackUseCase as _;
    use crate::domain::alias::NumericID;
    use crate::domain::model::music_album::MusicAlbum;
    use crate::domain::model::music_medium::MediumIndex;
    use crate::domain::model::music_medium::MusicMedium;
    use crate::domain::model::music_recording::MusicRecording;
    use crate::domain::model::music_recording::TrackIndex;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::music_album_repository::MockMusicAlbumRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::GalleryFactoryHarness;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::TestUnitOfWork;
    use crate::test_helpers::expect_caller;
    use crate::test_helpers::no_permissions;
    use crate::test_helpers::sample_album;

    use super::PatchAlbumTrack;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = PatchAlbumTrack<TestFactory>;

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

    /// The album holding one `CD` medium with one recording.
    fn album_with_track(id: NumericID) -> Result<MusicAlbum, Box<dyn Error>> {
        let recording = MusicRecording::try_new(
            5,
            Some("So What".to_owned()),
            Some("USSM15900001".to_owned()),
            1,
            2,
            None,
            TrackIndex::MediumIndex(1),
        )?;
        sample_album(
            id,
            vec![MusicMedium::new(MediumIndex::Cd(1), vec![recording])],
        )
    }

    /// Expect the post-write album reload.
    fn expect_reload(albums: &mut MockMusicAlbumRepository, album: MusicAlbum) {
        albums.expect_search().times(1).return_once(move |_| {
            let found = album.clone();
            Box::pin(async move { Ok(vec![found]) })
        });
    }

    #[tokio::test]
    async fn patch_album_track_renames_the_recording() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album_with_track(7)?);
        albums
            .expect_save_recording()
            .times(1)
            .withf(|recording| {
                recording.id() == 5 && recording.name() == Some("So What (Remastered)")
            })
            .returning(|recording| Box::pin(async move { Ok(Some(recording)) }));
        expect_reload(&mut albums, album_with_track(7)?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = PatchAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(PatchAlbumTrackCommand::new(
                1,
                7,
                1,
                1,
                FieldUpdate::Set("So What (Remastered)".to_owned()),
                FieldUpdate::Absent,
            ))
            .await?;

        // Assert
        assert_eq!(response.track_index(), 1);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn patch_album_track_clears_the_isrc() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album_with_track(7)?);
        albums
            .expect_save_recording()
            .times(1)
            .withf(|recording| recording.isrc_code().is_none())
            .returning(|recording| Box::pin(async move { Ok(Some(recording)) }));
        expect_reload(&mut albums, album_with_track(7)?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = PatchAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(PatchAlbumTrackCommand::new(
                1,
                7,
                1,
                1,
                FieldUpdate::Absent,
                FieldUpdate::Clear,
            ))
            .await?;

        // Assert
        assert_eq!(response.medium_index(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn patch_album_track_duplicate_isrc_is_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album_with_track(7)?);
        albums
            .expect_save_recording()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::AlreadyExist) }));
        let harness = harness_with(users, albums);
        let use_case: UseCase = PatchAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(PatchAlbumTrackCommand::new(
                1,
                7,
                1,
                1,
                FieldUpdate::Absent,
                FieldUpdate::Set("USSM15900002".to_owned()),
            ))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(PatchAlbumTrackError::IsrcAlreadyExists)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn patch_album_track_invalid_isrc_is_unprocessable() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album_with_track(7)?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = PatchAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(PatchAlbumTrackCommand::new(
                1,
                7,
                1,
                1,
                FieldUpdate::Absent,
                FieldUpdate::Set("short".to_owned()),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(PatchAlbumTrackError::InvalidIsrc)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn patch_album_track_unknown_track_is_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album_with_track(7)?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = PatchAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(PatchAlbumTrackCommand::new(
                1,
                7,
                1,
                9,
                FieldUpdate::Set("Renamed".to_owned()),
                FieldUpdate::Absent,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(PatchAlbumTrackError::NoSuchTrack)));
        Ok(())
    }

    #[tokio::test]
    async fn patch_album_track_standard_user_is_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album_with_track(7)?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = PatchAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(PatchAlbumTrackCommand::new(
                1,
                7,
                1,
                1,
                FieldUpdate::Set("Renamed".to_owned()),
                FieldUpdate::Absent,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(PatchAlbumTrackError::Forbidden)));
        Ok(())
    }
}
