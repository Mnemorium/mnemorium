use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::list_album_tracks::ListAlbumTracksCommand;
use crate::application::port::list_album_tracks::ListAlbumTracksError;
use crate::application::port::list_album_tracks::ListAlbumTracksItem;
use crate::application::port::list_album_tracks::ListAlbumTracksResponse;
use crate::application::port::list_album_tracks::ListAlbumTracksUseCase;
use crate::application::use_case::library_access;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::music_album_repository::MusicAlbumFilter;
use crate::domain::port::music_album_repository::MusicAlbumRepository as _;
use crate::domain::port::music_album_repository::MusicTrackFilter;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;

/// Use case implementation for listing the tracks of one music album.
pub struct ListAlbumTracks<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> ListAlbumTracks<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> ListAlbumTracksUseCase for ListAlbumTracks<F>
where
    F: UnitOfWorkFactory,
    F::Uow: LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: ListAlbumTracksCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ListAlbumTracksResponse, ListAlbumTracksError>>
                + Send
                + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| ListAlbumTracksError::Unknown(error.into()))?;

            let result = async {
                library_access::load_caller(&mut unit_of_work, command.caller_id())
                    .await
                    .map_err(|error| ListAlbumTracksError::Unknown(error.into()))?
                    .ok_or(ListAlbumTracksError::NoSuchCaller)?;

                let album = unit_of_work
                    .music_albums()
                    .search(&MusicAlbumFilter {
                        id: Some(command.album_id()),
                        ..MusicAlbumFilter::default()
                    })
                    .await
                    .map_err(|error| ListAlbumTracksError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(ListAlbumTracksError::NoSuchAlbum)?;

                // An album holds at most one medium
                // (`uq_music_medium_music_album_id`), so this reads the album's
                // tracks in one query; the album also carries the recordings
                // they play, which is where the display metadata comes from.
                let mut tracks = Vec::new();
                for medium in album.mediums() {
                    let found = unit_of_work
                        .music_albums()
                        .search_tracks(&MusicTrackFilter {
                            medium_id: Some(medium.id()),
                            ..MusicTrackFilter::default()
                        })
                        .await
                        .map_err(|error| ListAlbumTracksError::Unknown(error.into()))?;
                    for track in found {
                        let recording = medium
                            .tracks()
                            .iter()
                            .find(|recording| recording.id() == track.recording_id())
                            .ok_or_else(|| {
                                ListAlbumTracksError::Unknown(anyhow::anyhow!(
                                    "the track's recording is missing from its medium"
                                ))
                            })?;
                        tracks.push(ListAlbumTracksItem::new(
                            track.id(),
                            album.id(),
                            medium.medium_index(),
                            track.track_index(),
                            recording.file_id(),
                            recording.name().map(str::to_owned),
                            recording.isrc_code().map(str::to_owned),
                            recording.first_release_date(),
                        ));
                    }
                }

                Ok(ListAlbumTracksResponse::new(tracks))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| ListAlbumTracksError::Unknown(error.into()))?;
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

    use crate::application::port::list_album_tracks::ListAlbumTracksCommand;
    use crate::application::port::list_album_tracks::ListAlbumTracksError;
    use crate::application::port::list_album_tracks::ListAlbumTracksItem;
    use crate::application::port::list_album_tracks::ListAlbumTracksUseCase as _;
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

    use super::ListAlbumTracks;

    /// Identifier of the medium the album fixtures hold.
    const MEDIUM_ID: i64 = 40;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = ListAlbumTracks<TestFactory>;

    /// Build an album holding one medium with the recordings of `track_ids`.
    fn album(album_id: i64, track_ids: &[i64]) -> Result<MusicAlbum, Box<dyn Error>> {
        let tracks = track_ids
            .iter()
            .map(|track_id| {
                Ok(MusicRecording::try_new(
                    900i64.saturating_add(*track_id),
                    Some(format!("Track {track_id}")),
                    album_release_date(),
                    None,
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

    /// Build one track of the album fixture, positioned at `track_index`.
    fn track(track_id: i64, track_index: i64) -> Result<MusicTrack, Box<dyn Error>> {
        Ok(MusicTrack::try_new(
            track_id,
            MusicTrackPosition::Medium(MEDIUM_ID),
            900i64.saturating_add(track_id),
            track_index,
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
    async fn list_album_tracks_returns_the_ordered_tracks() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album(7, &[101, 102])?);
        let first = track(101, 1)?;
        let second = track(102, 2)?;
        albums
            .expect_search_tracks()
            .times(1)
            .withf(|filter| filter.medium_id == Some(MEDIUM_ID) && filter.id.is_none())
            .return_once(move |_| {
                let found = vec![first.clone(), second.clone()];
                Box::pin(async move { Ok(found) })
            });
        let harness = harness_with(users, albums);
        let use_case: UseCase = ListAlbumTracks::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(ListAlbumTracksCommand::new(1, 7)).await?;

        // Assert
        assert_eq!(
            response.tracks().first(),
            Some(&ListAlbumTracksItem::new(
                101,
                7,
                1,
                1,
                201,
                Some("Track 101".to_owned()),
                None,
                album_release_date(),
            ))
        );
        assert_eq!(
            response.tracks().get(1).map(ListAlbumTracksItem::track_id),
            Some(102)
        );
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn list_album_tracks_album_without_medium_returns_no_tracks() -> Result<(), Box<dyn Error>>
    {
        // Arrange: an album with no medium can hold no track.
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
        let use_case: UseCase = ListAlbumTracks::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(ListAlbumTracksCommand::new(1, 7)).await?;

        // Assert
        assert!(response.tracks().is_empty());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn list_album_tracks_unknown_album_returns_no_such_album() -> Result<(), Box<dyn Error>> {
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
        let use_case: UseCase = ListAlbumTracks::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(ListAlbumTracksCommand::new(1, 999)).await;

        // Assert
        assert!(matches!(result, Err(ListAlbumTracksError::NoSuchAlbum)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn list_album_tracks_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, MockMusicAlbumRepository::new());
        let use_case: UseCase = ListAlbumTracks::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(ListAlbumTracksCommand::new(999, 7)).await;

        // Assert
        assert!(matches!(result, Err(ListAlbumTracksError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn list_album_tracks_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
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
        let use_case: UseCase = ListAlbumTracks::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(ListAlbumTracksCommand::new(1, 7)).await;

        // Assert
        assert!(matches!(result, Err(ListAlbumTracksError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
