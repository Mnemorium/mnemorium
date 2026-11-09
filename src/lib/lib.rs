#![cfg_attr(
    test,
    allow(
        clippy::missing_trait_methods,
        reason = "We are less strict about test code"
    ),
    allow(
        clippy::too_many_arguments,
        reason = "We are less strict about test code"
    )
)]

pub mod app;
/// The application layer: use-case ports and their implementations.
pub(crate) mod application;
/// The domain layer: models, ports and services.
pub(crate) mod domain;
/// The infrastructure layer: inbound and outbound adapters.
pub(crate) mod infrastructure;

/// Fixtures shared by the in-crate test modules.
#[cfg(test)]
mod test_helpers {
    use std::error::Error;
    use std::future::{Future, ready};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    use axum::body::to_bytes;
    use axum::http::StatusCode;
    use axum::response::Response;
    use chrono::{NaiveDate, NaiveDateTime};
    use serde_json::Value;

    use crate::application::port::asset_use_case_factory::MockAssetUseCaseFactory;
    use crate::application::port::identity_use_case_factory::MockIdentityUseCaseFactory;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::user_use_case_factory::MockUserUseCaseFactory;
    use crate::domain::alias::{NonNegativeF64, NumericID, PositiveI64};
    use crate::domain::model::audio::ProbedAudio;
    use crate::domain::model::configuration::Configuration;
    use crate::domain::model::episode::{Episode, EpisodeIndex};
    use crate::domain::model::file::File;
    use crate::domain::model::image::{Orientation, ProbedImage};
    use crate::domain::model::instance_state::InstanceState;
    use crate::domain::model::integrity_hash::{DIGEST_HEX_LENGTH, IntegrityHash};
    use crate::domain::model::movie::Movie;
    use crate::domain::model::music_album::MusicAlbum;
    use crate::domain::model::music_medium::MusicMedium;
    use crate::domain::model::permission::Permission;
    use crate::domain::model::season::{Season, SeasonIndex};
    use crate::domain::model::series::Series;
    use crate::domain::model::user::{Role, User};
    use crate::domain::model::video::{ProbedVideo, ScanType, Video};
    use crate::domain::port::asset_unit_of_work::AssetUnitOfWork;
    use crate::domain::port::audio_repository::{AudioRepository, MockAudioRepository};
    use crate::domain::port::credential_repository::{
        CredentialRepository, MockCredentialRepository,
    };
    use crate::domain::port::error::UnitOfWorkError;
    use crate::domain::port::file_repository::{FileRepository, MockFileRepository};
    use crate::domain::port::gallery_repository::{GalleryRepository, MockGalleryRepository};
    use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
    use crate::domain::port::image_repository::{ImageRepository, MockImageRepository};
    use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
    use crate::domain::port::mime_type_repository::{MimeTypeRepository, MockMimeTypeRepository};
    use crate::domain::port::movie_repository::{MockMovieRepository, MovieRepository};
    use crate::domain::port::music_album_repository::{
        MockMusicAlbumRepository, MusicAlbumRepository,
    };
    use crate::domain::port::music_group_repository::{
        MockMusicGroupRepository, MusicGroupRepository,
    };
    use crate::domain::port::music_playlist_repository::{
        MockMusicPlaylistRepository, MusicPlaylistRepository,
    };
    use crate::domain::port::permission_repository::{
        MockPermissionRepository, PermissionRepository,
    };
    use crate::domain::port::series_repository::{MockSeriesRepository, SeriesRepository};
    use crate::domain::port::unit_of_work::{UnitOfWork, UnitOfWorkFactory};
    use crate::domain::port::upload_repository::{MockUploadRepository, UploadRepository};
    use crate::domain::port::user_repository::{MockUserRepository, UserRepository};
    use crate::domain::port::user_unit_of_work::UserUnitOfWork;
    use crate::domain::port::video_repository::{MockVideoRepository, VideoRepository};
    use crate::infrastructure::inbound::rest::api_error::ApiErrorMessage;
    use crate::infrastructure::inbound::rest::app_state::AppState;

    /// Digest of the content the asset fixtures persist.
    pub(crate) const DIGEST: &str =
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    /// Size of every upload fixture chunk, in bytes.
    pub(crate) const CHUNK_SIZE: u64 = 4;

    /// A password that satisfies the password policy.
    ///
    /// Use it when a test only needs a valid password: registering a user,
    /// authenticating, or provisioning a credential. Tests whose subject is
    /// password behaviour (policy validation, wrong-password rejection,
    /// change-password, hashing, verification) keep an explicit password.
    pub(crate) const SECRET_PASSWORD: &str = "C0rrect!Horse";

    /// JWT secret the test `AppState` fixtures are built with.
    ///
    /// The auth middleware builds its token provider from the instance
    /// identity, so tests mint tokens with this value.
    pub(crate) const TEST_JWT_SECRET: &str =
        "0000000000000000000000000000000000000000000000000000000000000000";

    /// Site pepper the test `AppState` fixtures are built with.
    const TEST_PEPPER: &str = "1111111111111111111111111111111111111111111111111111111111111111";

    /// Fake unit of work wiring mocked repositories and recording its outcome.
    ///
    /// Its accessors return mutable views over the mocked repositories, which
    /// is why `UnitOfWork` cannot be `mockall`-generated. `committed` and
    /// `rolled_back` are shared with the test so it can assert the transaction
    /// lifecycle after the use case has consumed the unit of work.
    ///
    /// One type serves every bounded context, and it implements `Default`: a
    /// test builds one from `..TestUnitOfWork::default()` and names only the
    /// repositories it stubs, so adding a repository to a context touches this
    /// fixture alone.
    pub(crate) struct TestUnitOfWork {
        /// Audio repository.
        pub audios: MockAudioRepository,
        /// Set to force `commit` to fail.
        pub commit_fails: Arc<AtomicBool>,
        /// Set when `commit` is called.
        pub committed: Arc<AtomicBool>,
        /// Credential repository.
        pub credentials: MockCredentialRepository,
        /// File repository.
        pub files: MockFileRepository,
        /// Gallery repository.
        pub galleries: MockGalleryRepository,
        /// Image repository.
        pub images: MockImageRepository,
        /// Mime type repository.
        pub mime_types: MockMimeTypeRepository,
        /// Movie repository.
        pub movies: MockMovieRepository,
        /// Music album repository.
        pub music_albums: MockMusicAlbumRepository,
        /// Music group repository.
        pub music_groups: MockMusicGroupRepository,
        /// Music playlist repository.
        pub music_playlists: MockMusicPlaylistRepository,
        /// Permission repository.
        pub permissions: MockPermissionRepository,
        /// Set when `rollback` is called.
        pub rolled_back: Arc<AtomicBool>,
        /// Series repository.
        pub series: MockSeriesRepository,
        /// Upload repository.
        pub uploads: MockUploadRepository,
        /// User repository.
        pub users: MockUserRepository,
        /// Video repository.
        pub videos: MockVideoRepository,
    }

    impl Default for TestUnitOfWork {
        /// A unit of work holding a fresh mock per repository.
        fn default() -> Self {
            Self {
                audios: MockAudioRepository::new(),
                commit_fails: Arc::new(AtomicBool::new(false)),
                committed: Arc::new(AtomicBool::new(false)),
                credentials: MockCredentialRepository::new(),
                files: MockFileRepository::new(),
                galleries: MockGalleryRepository::new(),
                images: MockImageRepository::new(),
                mime_types: MockMimeTypeRepository::new(),
                movies: MockMovieRepository::new(),
                music_albums: MockMusicAlbumRepository::new(),
                music_groups: MockMusicGroupRepository::new(),
                music_playlists: MockMusicPlaylistRepository::new(),
                permissions: MockPermissionRepository::new(),
                rolled_back: Arc::new(AtomicBool::new(false)),
                series: MockSeriesRepository::new(),
                uploads: MockUploadRepository::new(),
                users: MockUserRepository::new(),
                videos: MockVideoRepository::new(),
            }
        }
    }

    impl IdentityUnitOfWork for TestUnitOfWork {
        fn credentials(&mut self) -> impl CredentialRepository + '_ {
            &mut self.credentials
        }

        fn permissions(&mut self) -> impl PermissionRepository + '_ {
            &mut self.permissions
        }
    }

    impl UserUnitOfWork for TestUnitOfWork {
        fn users(&mut self) -> impl UserRepository + '_ {
            &mut self.users
        }
    }

    impl LibraryUnitOfWork for TestUnitOfWork {
        fn audios(&mut self) -> impl AudioRepository + '_ {
            &mut self.audios
        }

        fn galleries(&mut self) -> impl GalleryRepository + '_ {
            &mut self.galleries
        }

        fn images(&mut self) -> impl ImageRepository + '_ {
            &mut self.images
        }

        fn movies(&mut self) -> impl MovieRepository + '_ {
            &mut self.movies
        }

        fn music_albums(&mut self) -> impl MusicAlbumRepository + '_ {
            &mut self.music_albums
        }

        fn music_groups(&mut self) -> impl MusicGroupRepository + '_ {
            &mut self.music_groups
        }

        fn music_playlists(&mut self) -> impl MusicPlaylistRepository + '_ {
            &mut self.music_playlists
        }

        fn series(&mut self) -> impl SeriesRepository + '_ {
            &mut self.series
        }

        fn videos(&mut self) -> impl VideoRepository + '_ {
            &mut self.videos
        }
    }

    impl AssetUnitOfWork for TestUnitOfWork {
        fn files(&mut self) -> impl FileRepository + '_ {
            &mut self.files
        }

        fn mime_types(&mut self) -> impl MimeTypeRepository + '_ {
            &mut self.mime_types
        }

        fn uploads(&mut self) -> impl UploadRepository + '_ {
            &mut self.uploads
        }
    }

    impl UnitOfWork for TestUnitOfWork {
        fn commit(self) -> impl Future<Output = Result<(), UnitOfWorkError>> + Send {
            self.committed.store(true, Ordering::SeqCst);
            if self.commit_fails.load(Ordering::SeqCst) {
                return ready(Err(UnitOfWorkError::OperationFailed));
            }
            ready(Ok(()))
        }

        fn rollback(self) -> impl Future<Output = Result<(), UnitOfWorkError>> + Send {
            self.rolled_back.store(true, Ordering::SeqCst);
            ready(Ok(()))
        }
    }

    /// Units of work handed out one per `begin` call.
    type UnitOfWorkQueue = Mutex<Vec<TestUnitOfWork>>;

    /// Fake factory handing out a queue of prepared [`TestUnitOfWork`]s, one per
    /// `begin` call, so retry loops can bind a unit of work per attempt.
    pub(crate) struct TestUnitOfWorkFactory {
        /// Popped from the front by each `begin` call.
        pub unit_of_works: UnitOfWorkQueue,
    }

    impl TestUnitOfWorkFactory {
        /// Hand out `unit_of_work` on the first `begin` call.
        #[must_use]
        pub(crate) fn new(unit_of_work: TestUow) -> Self {
            Self {
                unit_of_works: Mutex::new(vec![unit_of_work]),
            }
        }

        /// Pop the next prepared unit of work, tolerating a poisoned lock.
        fn take(&self) -> Option<TestUnitOfWork> {
            match self.unit_of_works.lock() {
                Ok(mut guard) if !guard.is_empty() => Some(guard.remove(0)),
                Ok(_) => None,
                Err(poisoned) => {
                    let mut guard = poisoned.into_inner();
                    if guard.is_empty() {
                        None
                    } else {
                        Some(guard.remove(0))
                    }
                }
            }
        }
    }

    impl UnitOfWorkFactory for TestUnitOfWorkFactory {
        type Uow = TestUnitOfWork;

        fn begin(&self) -> impl Future<Output = Result<Self::Uow, UnitOfWorkError>> + Send {
            ready(self.take().ok_or(UnitOfWorkError::OperationFailed))
        }

        /// The double issues no SQL, so the intent changes nothing: both
        /// openers pop the same prepared unit of work, and a write-intent call
        /// consumes no extra queue entry.
        fn begin_write(&self) -> impl Future<Output = Result<Self::Uow, UnitOfWorkError>> + Send {
            self.begin()
        }
    }

    /// The mocked unit of work shared by every fixture.
    pub(crate) type TestUow = TestUnitOfWork;

    /// Factory whose unit of work is wired to the `mockall` repository mocks.
    pub(crate) type TestFactory = TestUnitOfWorkFactory;

    /// The Asset fixture produced by [`AssetFactoryHarness::new`].
    pub(crate) struct AssetFactoryHarness {
        /// Set to force `commit` to fail.
        pub commit_fails: Arc<AtomicBool>,
        /// Set when the unit of work is committed.
        pub committed: Arc<AtomicBool>,
        /// The factory handed to the use case under test.
        pub factory: Arc<TestFactory>,
        /// Set when the unit of work is rolled back.
        pub rolled_back: Arc<AtomicBool>,
    }

    impl AssetFactoryHarness {
        /// Wrap an Asset `unit_of_work` in a factory, sharing its
        /// transaction-lifecycle flags.
        #[must_use]
        pub(crate) fn new(unit_of_work: TestUow) -> Self {
            let committed = Arc::clone(&unit_of_work.committed);
            let commit_fails = Arc::clone(&unit_of_work.commit_fails);
            let rolled_back = Arc::clone(&unit_of_work.rolled_back);
            Self {
                committed,
                commit_fails,
                factory: Arc::new(TestFactory::new(unit_of_work)),
                rolled_back,
            }
        }
    }

    /// The Library fixture produced by [`GalleryFactoryHarness::new`].
    pub(crate) struct GalleryFactoryHarness {
        /// Set to force `commit` to fail.
        pub commit_fails: Arc<AtomicBool>,
        /// Set when the unit of work is committed.
        pub committed: Arc<AtomicBool>,
        /// The factory handed to the use case under test.
        pub factory: Arc<TestFactory>,
        /// Set when the unit of work is rolled back.
        pub rolled_back: Arc<AtomicBool>,
    }

    impl GalleryFactoryHarness {
        /// Wrap a Library `unit_of_work` in a factory, sharing its
        /// transaction-lifecycle flags.
        #[must_use]
        pub(crate) fn new(unit_of_work: TestUow) -> Self {
            let committed = Arc::clone(&unit_of_work.committed);
            let commit_fails = Arc::clone(&unit_of_work.commit_fails);
            let rolled_back = Arc::clone(&unit_of_work.rolled_back);
            Self {
                committed,
                commit_fails,
                factory: Arc::new(TestFactory::new(unit_of_work)),
                rolled_back,
            }
        }
    }

    /// Build the instance identity the test `AppState` fixtures carry.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed identity cannot be built.
    pub(crate) fn instance_state() -> Result<InstanceState, Box<dyn Error>> {
        Ok(InstanceState::try_new(
            0,
            TEST_PEPPER.to_owned(),
            TEST_JWT_SECRET.to_owned(),
        )?)
    }

    /// Build an application state whose use-case factories are mocks and whose
    /// configuration and identity are fixed valid values.
    ///
    /// The auth middleware builds its token provider from that identity, so
    /// tests mint tokens with [`TEST_JWT_SECRET`].
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed identity cannot be built.
    pub(crate) fn app_state() -> Result<AppState, Box<dyn Error>> {
        Ok(AppState::new(
            Arc::new(MockAssetUseCaseFactory::new()),
            Arc::new(Configuration::default()),
            Arc::new(MockIdentityUseCaseFactory::new()),
            Arc::new(instance_state()?),
            Arc::new(MockLibraryUseCaseFactory::new()),
            Arc::new(MockUserUseCaseFactory::new()),
        ))
    }

    /// Return the message an `ApiError` attached to `response`.
    ///
    /// The message travels in a response extension, so a test can assert the
    /// handler-owned message without depending on the HAL error envelope that
    /// the root middleware builds.
    #[must_use]
    pub(crate) fn error_message_of(response: &Response) -> Option<String> {
        response
            .extensions()
            .get::<ApiErrorMessage>()
            .map(|message| message.0.clone())
    }

    /// The release date the album fixtures carry.
    #[must_use]
    pub(crate) fn album_release_date() -> NaiveDate {
        NaiveDate::from_ymd_opt(1959, 8, 17).unwrap_or_default()
    }

    /// The date the video fixtures air or release on.
    #[must_use]
    pub(crate) fn video_date() -> NaiveDate {
        NaiveDate::from_ymd_opt(1990, 4, 8).unwrap_or_default()
    }

    /// Build a file of `mime_type_id` owned by `owner_id`.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixture path is empty.
    pub(crate) fn sample_file(
        id: NumericID,
        owner_id: NumericID,
        mime_type_id: &str,
    ) -> Result<File, Box<dyn Error>> {
        Ok(File::try_new(
            id,
            format!("{id}"),
            owner_id,
            false,
            mime_type_id.to_owned(),
            NaiveDate::default(),
            10,
            NaiveDateTime::default(),
            None,
            None,
        )?)
    }

    /// Build a progressive video backed by `file_id`.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed video values violate the domain rules.
    pub(crate) fn sample_video(
        video_id: NumericID,
        file_id: NumericID,
    ) -> Result<Video, Box<dyn Error>> {
        Ok(Video::try_new(
            video_id,
            1000.0,
            "H264".to_owned(),
            30,
            1920,
            1080,
            "YCbCr".to_owned(),
            ScanType::Progressive,
            file_id,
            None,
        )?)
    }

    /// Build a film named `Movie {id}`, bearing one genre and one video.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed film values violate the domain rules.
    pub(crate) fn sample_movie(id: NumericID) -> Result<Movie, Box<dyn Error>> {
        Ok(Movie::try_new(
            id,
            format!("Movie {id}"),
            "A plot".to_owned(),
            Some("US".to_owned()),
            Some(video_date()),
            vec!["DRAMA".to_owned()],
            None,
            sample_video(id, id)?,
        )?)
    }

    /// Build an album named `Album {id}` with no medium.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed album values violate the domain rules.
    pub(crate) fn sample_album(
        id: NumericID,
        mediums: Vec<MusicMedium>,
    ) -> Result<MusicAlbum, Box<dyn Error>> {
        Ok(MusicAlbum::try_new(
            id,
            format!("Album {id}"),
            Some(album_release_date()),
            None,
            Vec::new(),
            0,
            None,
            None,
            mediums,
        )?)
    }

    /// Build a show named `Series {id}` with `seasons` and one genre.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed show values violate the domain rules.
    pub(crate) fn sample_series(
        id: NumericID,
        seasons: Vec<Season>,
    ) -> Result<Series, Box<dyn Error>> {
        Ok(Series::try_new(
            id,
            format!("Series {id}"),
            "A plot".to_owned(),
            Some("US".to_owned()),
            Some(video_date()),
            vec!["DRAMA".to_owned()],
            None,
            seasons,
        )?)
    }

    /// Build a season at `season_index` holding `episodes`.
    #[must_use]
    pub(crate) fn sample_season(season_index: u32, episodes: Vec<Episode>) -> Season {
        Season::new(SeasonIndex::new(season_index), None, None, episodes)
    }

    /// Build an episode named `Episode {episode_index}` with one video.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed episode values violate the domain rules.
    pub(crate) fn sample_episode(
        episode_index: u32,
        video_id: NumericID,
    ) -> Result<Episode, Box<dyn Error>> {
        Ok(Episode::try_new(
            EpisodeIndex::new(episode_index),
            format!("Episode {episode_index}"),
            "A plot".to_owned(),
            Some(video_date()),
            sample_video(video_id, video_id)?,
        )?)
    }

    /// Expect the caller load to find a standard user named `caller`.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed caller cannot be built.
    pub(crate) fn expect_caller(
        users: &mut MockUserRepository,
        caller_id: NumericID,
    ) -> Result<(), Box<dyn Error>> {
        let caller = User::try_new(
            caller_id,
            "caller".to_owned(),
            None,
            caller_id,
            Role::Standard,
        )?;
        users
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(caller_id))
            .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
        Ok(())
    }

    /// Expect the permission load to find `overrides` for `caller_id`.
    pub(crate) fn expect_permissions(
        permissions: &mut MockPermissionRepository,
        caller_id: NumericID,
        overrides: Vec<Permission>,
    ) {
        permissions
            .expect_search()
            .times(1)
            .withf(move |filter| filter.user_id == Some(caller_id) && filter.action.is_none())
            .return_once(move |_| Box::pin(async move { Ok(overrides) }));
    }

    /// Return a user repository resolving any requested identifier.
    ///
    /// A use case whose test does not exercise the caller load stubs it with
    /// this repository: identifier `0` resolves to the Root Admin and any other
    /// identifier to a Standard user.
    #[must_use]
    pub(crate) fn any_caller() -> MockUserRepository {
        let mut users = MockUserRepository::new();
        users.expect_search().returning(|filter| {
            let found = filter.id.and_then(|id| {
                let role = if id == 0 { Role::Admin } else { Role::Standard };
                User::try_new(id, "caller".to_owned(), None, id.max(1), role).ok()
            });
            Box::pin(async move { Ok(found.into_iter().collect()) })
        });
        users
    }

    /// Return a permission repository that finds no override, for any caller.
    ///
    /// A use case whose test does not exercise a permission stubs the load
    /// with this repository instead of repeating the expectation.
    #[must_use]
    pub(crate) fn no_permissions() -> MockPermissionRepository {
        let mut permissions = MockPermissionRepository::new();
        permissions
            .expect_search()
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        permissions
    }

    /// Split `response` into its status and decoded JSON body.
    ///
    /// # Errors
    ///
    /// Returns an error when the body cannot be read or is not JSON.
    pub(crate) async fn into_status_and_body(
        response: Response,
    ) -> Result<(StatusCode, Value), Box<dyn Error>> {
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        Ok((status, serde_json::from_slice(&bytes)?))
    }

    /// Build a positive [`PositiveI64`] from a literal.
    #[must_use]
    #[expect(
        clippy::expect_used,
        reason = "the fixtures pass only values that satisfy the scalar's invariant"
    )]
    pub(crate) fn positive_i64(value: i64) -> PositiveI64 {
        PositiveI64::try_new(value).expect("the fixture scalar is positive")
    }

    /// Build a non-negative [`NonNegativeF64`] from a literal.
    #[must_use]
    #[expect(
        clippy::expect_used,
        reason = "the fixtures pass only values that satisfy the scalar's invariant"
    )]
    pub(crate) fn non_negative_f64(value: f64) -> NonNegativeF64 {
        NonNegativeF64::try_new(value).expect("the fixture scalar is non-negative")
    }

    /// Build an [`IntegrityHash`] from a literal digest.
    #[must_use]
    #[expect(
        clippy::expect_used,
        reason = "the fixtures pass only valid 64-character hexadecimal digests"
    )]
    pub(crate) fn hash(digest: &str) -> IntegrityHash<DIGEST_HEX_LENGTH> {
        IntegrityHash::try_new(digest.to_owned()).expect("the fixture digest is valid")
    }

    /// Build the image the probe tests compare against.
    #[must_use]
    pub(crate) fn probed_image() -> ProbedImage {
        ProbedImage {
            height_px: positive_i64(480),
            orientation: Orientation::Landscape,
            width_px: positive_i64(640),
        }
    }

    /// Build the video the probe tests compare against.
    #[must_use]
    pub(crate) fn probed_video() -> ProbedVideo {
        ProbedVideo {
            codec: "h264".to_owned(),
            color_id: "YCbCr".to_owned(),
            duration_ms: non_negative_f64(1_000.0f64),
            frame_count: positive_i64(30),
            height: positive_i64(480),
            scan_type: ScanType::Progressive,
            width: positive_i64(640),
        }
    }

    /// Build an audio probe result that reports its measurements.
    #[must_use]
    pub(crate) fn probed_audio() -> ProbedAudio {
        ProbedAudio {
            bit_depth: Some(16),
            block_size: Some(1_152),
            codec: "mp3".to_owned(),
            duration_ms: Some(non_negative_f64(1_000.0f64)),
            sample_rate_hz: Some(non_negative_f64(44_100.0f64)),
        }
    }

    /// Build an audio probe result whose measurements are absent.
    #[must_use]
    pub(crate) fn probed_audio_without_measurements() -> ProbedAudio {
        ProbedAudio {
            bit_depth: None,
            block_size: None,
            codec: "mp3".to_owned(),
            duration_ms: Some(non_negative_f64(1_000.0f64)),
            sample_rate_hz: Some(non_negative_f64(44_100.0f64)),
        }
    }
}
