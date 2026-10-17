use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::get_album_track::GetAlbumTrackCommand;
use crate::application::port::get_album_track::GetAlbumTrackError;
use crate::application::port::get_album_track::GetAlbumTrackResponse;
use crate::application::port::get_album_track::GetAlbumTrackUseCase;
use crate::application::use_case::library_access;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::music_album_repository::MusicAlbumFilter;
use crate::domain::port::music_album_repository::MusicAlbumRepository as _;
use crate::domain::port::music_album_repository::MusicTrackFilter;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;

/// Use case implementation for fetching one track of a music album.
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
    F::Uow: LibraryUnitOfWork + UserUnitOfWork,
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
                library_access::load_caller(&mut unit_of_work, command.caller_id())
                    .await
                    .map_err(|error| GetAlbumTrackError::Unknown(error.into()))?
                    .ok_or(GetAlbumTrackError::NoSuchCaller)?;

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

                // A track of another album is not found: the track is read
                // within the album's own mediums, which is also what makes an
                // album holding no medium hold no track.
                let mut found = None;
                for medium in album.mediums() {
                    let tracks = unit_of_work
                        .music_albums()
                        .search_tracks(&MusicTrackFilter {
                            id: Some(command.track_id()),
                            medium_id: Some(medium.id()),
                            ..MusicTrackFilter::default()
                        })
                        .await
                        .map_err(|error| GetAlbumTrackError::Unknown(error.into()))?;
                    if let Some(track) = tracks.into_iter().next() {
                        found = Some((medium, track));
                        break;
                    }
                }
                let (medium, track) = found.ok_or(GetAlbumTrackError::NoSuchTrack)?;
                let recording = medium
                    .tracks()
                    .iter()
                    .find(|recording| recording.id() == track.recording_id())
                    .ok_or_else(|| {
                        GetAlbumTrackError::Unknown(anyhow::anyhow!(
                            "the track's recording is missing from its medium"
                        ))
                    })?;

                Ok(GetAlbumTrackResponse::new(
                    track.id(),
                    album.id(),
                    medium.medium_index(),
                    track.track_index(),
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
                    if unit_of_work.rollback().await.is_err() {
                        // The unit-of-work adapter owns the rollback-failure log (OBS-002).
                    }
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
    use crate::domain::model::music_medium::MusicMedium;
    use crate::domain::model::music_recording::MusicRecording;
    use crate::domain::model::music_track::MusicTrack;
    use crate::domain::model::music_track::MusicTrackPosition;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::music_album_repository::MockMusicAlbumRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::GalleryFactoryHarness;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::TestUnitOfWork;
    use crate::test_helpers::album_release_date;

    use super::GetAlbumTrack;

    /// Identifier of the medium the album fixtures hold.
    const MEDIUM_ID: i64 = 40;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = GetAlbumTrack<TestFactory>;

    /// Build an album holding one medium with the recordings of `track_ids`.
    fn album(album_id: i64, track_ids: &[i64]) -> Result<MusicAlbum, Box<dyn Error>> {
        let tracks = track_ids
            .iter()
            .map(|track_id| {
                Ok(MusicRecording::try_new(
                    900i64.saturating_add(*track_id),
                    Some(format!("Track {track_id}")),
                    album_release_date(),
                    Some("USRC17607839".to_owned()),
                    *track_id,
                    100i64.saturating_add(*track_id),
                    None,
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
            vec![MusicMedium::new(
                MEDIUM_ID,
                Some(1),
                "CD".to_owned(),
                tracks,
            )],
        )?)
    }

    /// Build a Library test harness around the given mocks.
    fn harness_with(
        users: MockUserRepository,
        albums: MockMusicAlbumRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
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
        expect_album(&mut albums, album(7, &[101])?);
        // The album fixture above pairs track 101 with recording 1001.
        let track = MusicTrack::try_new(101, MusicTrackPosition::Medium(MEDIUM_ID), 1001, 1)?;
        albums
            .expect_search_tracks()
            .times(1)
            .withf(|filter| filter.medium_id == Some(MEDIUM_ID) && filter.id == Some(101))
            .return_once(move |_| {
                let found = vec![track.clone()];
                Box::pin(async move { Ok(found) })
            });
        let harness = harness_with(users, albums);
        let use_case: UseCase = GetAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(GetAlbumTrackCommand::new(1, 7, 101))
            .await?;

        // Assert
        assert_eq!(response.track_id(), 101);
        assert_eq!(response.album_id(), 7);
        assert_eq!(response.file_id(), 201);
        assert_eq!(response.name(), Some("Track 101"));
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
        albums.expect_search_tracks().times(0);
        let harness = harness_with(users, albums);
        let use_case: UseCase = GetAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(GetAlbumTrackCommand::new(1, 999, 101))
            .await;

        // Assert
        assert!(matches!(result, Err(GetAlbumTrackError::NoSuchAlbum)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_album_track_track_outside_the_album_returns_no_such_track()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album(7, &[101])?);
        albums
            .expect_search_tracks()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, albums);
        let use_case: UseCase = GetAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetAlbumTrackCommand::new(1, 7, 555)).await;

        // Assert
        assert!(matches!(result, Err(GetAlbumTrackError::NoSuchTrack)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_album_track_album_without_medium_returns_no_such_track()
    -> Result<(), Box<dyn Error>> {
        // Arrange: an album with no medium holds no track to find.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(
            &mut albums,
            MusicAlbum::try_new(
                7,
                "Empty".to_owned(),
                album_release_date(),
                None,
                Vec::new(),
                None,
                Vec::new(),
            )?,
        );
        albums.expect_search_tracks().times(0);
        let harness = harness_with(users, albums);
        let use_case: UseCase = GetAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetAlbumTrackCommand::new(1, 7, 101)).await;

        // Assert
        assert!(matches!(result, Err(GetAlbumTrackError::NoSuchTrack)));
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
            .execute(GetAlbumTrackCommand::new(999, 7, 101))
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
        expect_album(&mut albums, album(7, &[101])?);
        albums
            .expect_search_tracks()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, albums);
        let use_case: UseCase = GetAlbumTrack::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetAlbumTrackCommand::new(1, 7, 101)).await;

        // Assert
        assert!(matches!(result, Err(GetAlbumTrackError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
