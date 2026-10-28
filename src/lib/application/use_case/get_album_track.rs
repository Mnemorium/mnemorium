use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::get_album_track::GetAlbumTrackCommand;
use crate::application::port::get_album_track::GetAlbumTrackError;
use crate::application::port::get_album_track::GetAlbumTrackResponse;
use crate::application::port::get_album_track::GetAlbumTrackUseCase;
use crate::application::security_event;
use crate::domain::model::permission::Action;
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

/// Use case implementation for fetching one track of a music album medium.
pub struct GetAlbumTrack<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> GetAlbumTrack<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> GetAlbumTrackUseCase for GetAlbumTrack<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: GetAlbumTrackCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<GetAlbumTrackResponse, GetAlbumTrackError>> + Send + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| GetAlbumTrackError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| GetAlbumTrackError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetAlbumTrackError::NoSuchCaller)?;

                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| GetAlbumTrackError::Unknown(error.into()))?;

                let album = unit_of_work
                    .music_albums()
                    .search(&MusicAlbumFilter {
                        id: Some(command.album_id()),
                        ..MusicAlbumFilter::default()
                    })
                    .await
                    .map_err(|error| GetAlbumTrackError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetAlbumTrackError::NoSuchAlbum)?;
                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::CatalogRead, &album)
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(GetAlbumTrackError::NoSuchAlbum);
                }

                // A track of another album or medium is not found: the track is
                // read within the named medium of the album, which is also what
                // makes an album holding no medium hold no track.
                let medium = album
                    .mediums()
                    .iter()
                    .find(|medium| medium.index().index() == command.medium_index())
                    .ok_or(GetAlbumTrackError::NoSuchMedium)?;
                let recording = medium
                    .tracks()
                    .iter()
                    .find(|recording| {
                        recording.index().medium_index() == Some(command.track_index())
                    })
                    .ok_or(GetAlbumTrackError::NoSuchTrack)?;

                Ok(GetAlbumTrackResponse::new(
                    album.id(),
                    medium.index().index(),
                    command.track_index(),
                    recording.id(),
                    recording.file_id(),
                    recording.name().map(str::to_owned),
                    recording.isrc_code().map(str::to_owned),
                    recording.first_release_date(),
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| GetAlbumTrackError::Unknown(error.into()))?;
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

    use crate::application::port::get_album_track::GetAlbumTrackCommand;
    use crate::application::port::get_album_track::GetAlbumTrackError;
    use crate::application::port::get_album_track::GetAlbumTrackUseCase as _;
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
    use crate::test_helpers::album_release_date;
    use crate::test_helpers::no_permissions;

    use super::GetAlbumTrack;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = GetAlbumTrack<TestFactory>;

    /// Build an album holding one `CD` medium sequencing `track_indexes`.
    fn album(
        album_id: i64,
        medium_index: u8,
        track_indexes: &[u8],
    ) -> Result<MusicAlbum, Box<dyn Error>> {
        let tracks = track_indexes
            .iter()
            .map(|track_index| {
                Ok(MusicRecording::try_new(
                    900i64.saturating_add(i64::from(*track_index)),
                    Some(format!("Track {track_index}")),
                    album_release_date(),
                    Some("USRC17607839".to_owned()),
                    i64::from(*track_index),
                    100i64.saturating_add(i64::from(*track_index)),
                    None,
                    TrackIndex::MediumIndex(*track_index),
                )?)
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
        Ok(MusicAlbum::try_new(
            album_id,
            format!("Album {album_id}"),
            album_release_date(),
            None,
            Vec::new(),
            None,
            None,
            vec![MusicMedium::new(MediumIndex::Cd(medium_index), tracks)],
        )?)
    }

    /// Build an album holding no medium.
    #[expect(
        clippy::single_call_fn,
        reason = "the empty-album fixture names the case it builds"
    )]
    fn empty_album(album_id: i64) -> Result<MusicAlbum, Box<dyn Error>> {
        Ok(MusicAlbum::try_new(
            album_id,
            "Empty".to_owned(),
            album_release_date(),
            None,
            Vec::new(),
            None,
            None,
            Vec::new(),
        )?)
    }

    /// Build a Library test harness around the given mocks.
    fn harness_with(
        users: MockUserRepository,
        albums: MockMusicAlbumRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            permissions: no_permissions(),
            music_albums: albums,
            users,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
    }

    /// Expect the caller load to find `caller_id`.
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

    /// Expect the album lookup to find `album`.
    fn expect_album(albums: &mut MockMusicAlbumRepository, album: MusicAlbum) {
        let album_id = album.id();
        albums
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(album_id))
            .return_once(move |_| {
                let found = vec![album.clone()];
                Box::pin(async move { Ok(found) })
            });
    }

    #[tokio::test]
    async fn get_album_track_returns_the_scoped_track() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album(7, 1, &[1])?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = GetAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(GetAlbumTrackCommand::new(1, 7, 1, 1))
            .await?;

        // Assert
        assert_eq!(response.album_id(), 7);
        assert_eq!(response.medium_index(), 1);
        assert_eq!(response.track_index(), 1);
        assert_eq!(response.recording_id(), 901);
        assert_eq!(response.file_id(), 101);
        assert_eq!(response.name(), Some("Track 1"));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_album_track_unknown_album_returns_no_such_album() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        albums
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, albums);
        let use_case: UseCase = GetAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(GetAlbumTrackCommand::new(1, 999, 1, 1))
            .await;

        // Assert
        assert!(matches!(result, Err(GetAlbumTrackError::NoSuchAlbum)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_album_track_unknown_medium_returns_no_such_medium() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album(7, 1, &[1])?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = GetAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(GetAlbumTrackCommand::new(1, 7, 9, 1))
            .await;

        // Assert
        assert!(matches!(result, Err(GetAlbumTrackError::NoSuchMedium)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_album_track_track_outside_the_medium_returns_no_such_track()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album(7, 1, &[1])?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = GetAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(GetAlbumTrackCommand::new(1, 7, 1, 5))
            .await;

        // Assert
        assert!(matches!(result, Err(GetAlbumTrackError::NoSuchTrack)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_album_track_album_without_medium_returns_no_such_medium()
    -> Result<(), Box<dyn Error>> {
        // Arrange: an album with no medium holds no track to find.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, empty_album(7)?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = GetAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(GetAlbumTrackCommand::new(1, 7, 1, 1))
            .await;

        // Assert
        assert!(matches!(result, Err(GetAlbumTrackError::NoSuchMedium)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_album_track_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, MockMusicAlbumRepository::new());
        let use_case: UseCase = GetAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(GetAlbumTrackCommand::new(999, 7, 1, 1))
            .await;

        // Assert
        assert!(matches!(result, Err(GetAlbumTrackError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_album_track_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        albums
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, albums);
        let use_case: UseCase = GetAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(GetAlbumTrackCommand::new(1, 7, 1, 1))
            .await;

        // Assert
        assert!(matches!(result, Err(GetAlbumTrackError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_album_track_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album(7, 1, &[1])?);
        let harness = harness_with(users, albums);
        harness.commit_fails.store(true, Ordering::SeqCst);
        let use_case: UseCase = GetAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(GetAlbumTrackCommand::new(1, 7, 1, 1))
            .await;

        // Assert
        assert!(matches!(result, Err(GetAlbumTrackError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }
}
