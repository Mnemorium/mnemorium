use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::list_album_tracks::ListAlbumTracksCommand;
use crate::application::port::list_album_tracks::ListAlbumTracksError;
use crate::application::port::list_album_tracks::ListAlbumTracksItem;
use crate::application::port::list_album_tracks::ListAlbumTracksResponse;
use crate::application::port::list_album_tracks::ListAlbumTracksUseCase;
use crate::application::security_event;
use crate::application::use_case::library_access;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::music_album_repository::MusicAlbumFilter;
use crate::domain::port::music_album_repository::MusicAlbumRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;

/// Use case implementation for listing the tracks of one medium of one music
/// album.
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

                // The aggregate read carries each medium's recording
                // occurrences, so the requested medium is resolved from the
                // album itself and no track is stitched from a second query.
                let medium = album
                    .mediums()
                    .iter()
                    .find(|medium| medium.index().index() == command.medium_index())
                    .ok_or(ListAlbumTracksError::NoSuchMedium)?;
                let mut tracks = Vec::with_capacity(medium.tracks().len());
                for recording in medium.tracks() {
                    let track_index = recording.index().medium_index().ok_or_else(|| {
                        security_event::application_error("list_album_tracks");
                        ListAlbumTracksError::Unknown(anyhow::anyhow!(
                            "an album medium sequences a playlist placement"
                        ))
                    })?;
                    tracks.push(ListAlbumTracksItem::new(
                        album.id(),
                        medium.index().index(),
                        track_index,
                        recording.id(),
                        recording.file_id(),
                        recording.name().map(str::to_owned),
                        recording.isrc_code().map(str::to_owned),
                        recording.first_release_date(),
                    ));
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
    use std::io as stdio;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::PoisonError;
    use std::sync::atomic::Ordering;

    use tracing::subscriber::DefaultGuard;
    use tracing::subscriber::set_default;
    use tracing_subscriber::fmt;
    use tracing_subscriber::layer::SubscriberExt as _;

    use crate::application::port::list_album_tracks::ListAlbumTracksCommand;
    use crate::application::port::list_album_tracks::ListAlbumTracksError;
    use crate::application::port::list_album_tracks::ListAlbumTracksItem;
    use crate::application::port::list_album_tracks::ListAlbumTracksUseCase as _;
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

    use super::ListAlbumTracks;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = ListAlbumTracks<TestFactory>;

    /// Append-only sink that lets a test read back the lines [`fmt`] emits.
    #[derive(Clone)]
    struct CaptureWriter {
        /// Buffer shared with the test that asserts on the captured output.
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "only the raw `write` and `flush` are meaningful for an in-memory capture buffer"
    )]
    impl stdio::Write for CaptureWriter {
        fn flush(&mut self) -> stdio::Result<()> {
            Ok(())
        }

        fn write(&mut self, buf: &[u8]) -> stdio::Result<usize> {
            let mut buffer = self.buffer.lock().unwrap_or_else(PoisonError::into_inner);
            buffer.extend_from_slice(buf);
            Ok(buf.len())
        }
    }

    /// Hand every formatted event to a fresh clone of the shared buffer.
    #[expect(
        clippy::missing_trait_methods,
        reason = "the default `make_writer_for` already routes through `make_writer`"
    )]
    impl<'writer> fmt::MakeWriter<'writer> for CaptureWriter {
        type Writer = Self;

        fn make_writer(&'writer self) -> Self::Writer {
            self.clone()
        }
    }

    /// Install a capturing subscriber on the current thread and return the
    /// shared buffer plus the guard that keeps it active.
    #[expect(
        clippy::single_call_fn,
        reason = "the capture fixture names the test-only logging setup"
    )]
    fn capture_logs() -> (Arc<Mutex<Vec<u8>>>, DefaultGuard) {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::registry().with(
            fmt::layer().with_ansi(false).with_writer(CaptureWriter {
                buffer: Arc::clone(&buffer),
            }),
        );
        let guard = set_default(subscriber);
        (buffer, guard)
    }

    /// Read the captured bytes back as a lossy UTF-8 string.
    #[expect(
        clippy::single_call_fn,
        reason = "the capture fixture names the test-only logging setup"
    )]
    fn captured_logs(buffer: &Mutex<Vec<u8>>) -> String {
        let bytes = buffer.lock().unwrap_or_else(PoisonError::into_inner);
        String::from_utf8_lossy(bytes.as_slice()).into_owned()
    }

    /// Build a `CD` medium at `medium_index` sequencing one recording per
    /// track index.
    #[expect(
        clippy::single_call_fn,
        reason = "the medium fixture names the case it builds"
    )]
    fn medium(medium_index: u8, track_indexes: &[u8]) -> Result<MusicMedium, Box<dyn Error>> {
        let tracks = track_indexes
            .iter()
            .map(|track_index| {
                Ok(MusicRecording::try_new(
                    900i64.saturating_add(i64::from(*track_index)),
                    Some(format!("Track {track_index}")),
                    album_release_date(),
                    None,
                    i64::from(*track_index),
                    100i64.saturating_add(i64::from(*track_index)),
                    None,
                    TrackIndex::MediumIndex(*track_index),
                )?)
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
        Ok(MusicMedium::new(MediumIndex::Cd(medium_index), tracks))
    }

    /// Build an album holding one medium.
    fn album(
        album_id: i64,
        medium_index: u8,
        track_indexes: &[u8],
    ) -> Result<MusicAlbum, Box<dyn Error>> {
        Ok(MusicAlbum::try_new(
            album_id,
            format!("Album {album_id}"),
            album_release_date(),
            None,
            Vec::new(),
            None,
            vec![medium(medium_index, track_indexes)?],
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
            Vec::new(),
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
        expect_album(&mut albums, album(7, 1, &[1, 2])?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = ListAlbumTracks::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(ListAlbumTracksCommand::new(1, 7, 1))
            .await?;

        // Assert
        assert_eq!(
            response.tracks().first(),
            Some(&ListAlbumTracksItem::new(
                7,
                1,
                1,
                901,
                101,
                Some("Track 1".to_owned()),
                None,
                album_release_date(),
            ))
        );
        assert_eq!(
            response
                .tracks()
                .get(1)
                .map(|item| (item.track_index(), item.recording_id())),
            Some((2, 902))
        );
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn list_album_tracks_medium_without_tracks_returns_empty() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album(7, 1, &[])?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = ListAlbumTracks::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(ListAlbumTracksCommand::new(1, 7, 1))
            .await?;

        // Assert
        assert!(response.tracks().is_empty());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn list_album_tracks_album_without_medium_returns_no_such_medium()
    -> Result<(), Box<dyn Error>> {
        // Arrange: an album with no medium holds no track to list.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, empty_album(7)?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = ListAlbumTracks::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(ListAlbumTracksCommand::new(1, 7, 1)).await;

        // Assert
        assert!(matches!(result, Err(ListAlbumTracksError::NoSuchMedium)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn list_album_tracks_unknown_medium_returns_no_such_medium() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album(7, 1, &[1])?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = ListAlbumTracks::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(ListAlbumTracksCommand::new(1, 7, 9)).await;

        // Assert
        assert!(matches!(result, Err(ListAlbumTracksError::NoSuchMedium)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
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
        let harness = harness_with(users, albums);
        let use_case: UseCase = ListAlbumTracks::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ListAlbumTracksCommand::new(1, 999, 1))
            .await;

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
        let result = use_case
            .execute(ListAlbumTracksCommand::new(999, 7, 1))
            .await;

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
        albums
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, albums);
        let use_case: UseCase = ListAlbumTracks::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(ListAlbumTracksCommand::new(1, 7, 1)).await;

        // Assert
        assert!(matches!(result, Err(ListAlbumTracksError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn list_album_tracks_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album(7, 1, &[1])?);
        let harness = harness_with(users, albums);
        harness.commit_fails.store(true, Ordering::SeqCst);
        let use_case: UseCase = ListAlbumTracks::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(ListAlbumTracksCommand::new(1, 7, 1)).await;

        // Assert
        assert!(matches!(result, Err(ListAlbumTracksError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn list_album_tracks_playlist_placement_in_a_medium_logs_application_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange: a medium holding a playlist placement is an internal
        // invariant violation the use case originates itself.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let recording = MusicRecording::try_new(
            900,
            Some("Playlist placement".to_owned()),
            album_release_date(),
            None,
            1,
            101,
            None,
            TrackIndex::PlaylistIndex(1),
        )?;
        let album = MusicAlbum::try_new(
            7,
            "Album 7".to_owned(),
            album_release_date(),
            None,
            Vec::new(),
            None,
            vec![MusicMedium::new(MediumIndex::Cd(1), vec![recording])],
        )?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album);
        let harness = harness_with(users, albums);
        let use_case: UseCase = ListAlbumTracks::new(Arc::clone(&harness.factory));
        let (buffer, _capture) = capture_logs();

        // Act
        let result = use_case.execute(ListAlbumTracksCommand::new(1, 7, 1)).await;

        // Assert
        assert!(matches!(result, Err(ListAlbumTracksError::Unknown(_))));
        let logs = captured_logs(&buffer);
        assert_eq!(
            logs.matches("ERROR").count(),
            1,
            "the use case must emit exactly one error event: {logs}"
        );
        assert!(
            logs.contains("event=\"application_error\""),
            "the event must be application_error: {logs}"
        );
        assert!(
            logs.contains("operation=list_album_tracks"),
            "the event must name the owning operation: {logs}"
        );
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
