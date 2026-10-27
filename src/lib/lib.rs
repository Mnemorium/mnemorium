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
pub mod application;
pub mod domain;
pub mod infrastructure;

/// Fixtures shared by the in-crate test modules.
#[cfg(test)]
mod test_helpers {
    use std::error::Error;
    use std::future::Future;
    use std::future::ready;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;

    use axum::body::to_bytes;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::response::Response;
    use chrono::NaiveDate;
    use serde_json::Value;

    use crate::application::port::asset_use_case_factory::AssetUseCaseFactory;
    use crate::application::port::asset_use_case_factory::MockAssetUseCaseFactory;
    use crate::application::port::identity_use_case_factory::IdentityUseCaseFactory;
    use crate::application::port::identity_use_case_factory::MockIdentityUseCaseFactory;
    use crate::application::port::library_use_case_factory::LibraryUseCaseFactory;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::user_use_case_factory::MockUserUseCaseFactory;
    use crate::application::port::user_use_case_factory::UserUseCaseFactory;
    use crate::domain::alias::NumericID;
    use crate::domain::model::configuration::Configuration;
    use crate::domain::model::episode::Episode;
    use crate::domain::model::episode::EpisodeIndex;
    use crate::domain::model::instance_state::InstanceState;
    use crate::domain::model::movie::Movie;
    use crate::domain::model::season::Season;
    use crate::domain::model::season::SeasonIndex;
    use crate::domain::model::series::Series;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::model::video::ScanType;
    use crate::domain::model::video::Video;
    use crate::domain::port::asset_unit_of_work::AssetUnitOfWork;
    use crate::domain::port::credential_repository::CredentialRepository;
    use crate::domain::port::credential_repository::MockCredentialRepository;
    use crate::domain::port::error::UnitOfWorkError;
    use crate::domain::port::file_repository::FileRepository;
    use crate::domain::port::file_repository::MockFileRepository;
    use crate::domain::port::gallery_repository::GalleryRepository;
    use crate::domain::port::gallery_repository::MockGalleryRepository;
    use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
    use crate::domain::port::image_repository::ImageRepository;
    use crate::domain::port::image_repository::MockImageRepository;
    use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
    use crate::domain::port::mime_type_repository::MimeTypeRepository;
    use crate::domain::port::mime_type_repository::MockMimeTypeRepository;
    use crate::domain::port::movie_repository::MockMovieRepository;
    use crate::domain::port::movie_repository::MovieRepository;
    use crate::domain::port::music_album_repository::MockMusicAlbumRepository;
    use crate::domain::port::music_album_repository::MusicAlbumRepository;
    use crate::domain::port::music_playlist_repository::MockMusicPlaylistRepository;
    use crate::domain::port::music_playlist_repository::MusicPlaylistRepository;
    use crate::domain::port::series_repository::MockSeriesRepository;
    use crate::domain::port::series_repository::SeriesRepository;
    use crate::domain::port::unit_of_work::UnitOfWork;
    use crate::domain::port::unit_of_work::UnitOfWorkFactory;
    use crate::domain::port::upload_repository::MockUploadRepository;
    use crate::domain::port::upload_repository::UploadRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::domain::port::user_repository::UserRepository;
    use crate::domain::port::user_unit_of_work::UserUnitOfWork;
    use crate::domain::port::video_repository::MockVideoRepository;
    use crate::domain::port::video_repository::VideoRepository;
    use crate::infrastructure::inbound::rest::api_error::ApiErrorMessage;
    use crate::infrastructure::inbound::rest::app_state::AppState;

    /// Digest of the content the asset fixtures persist.
    pub const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    /// Size of every upload fixture chunk, in bytes.
    pub const CHUNK_SIZE: u64 = 4;

    /// A password that satisfies the password policy.
    ///
    /// Use it when a test only needs a valid password: registering a user,
    /// authenticating, or provisioning a credential. Tests whose subject is
    /// password behaviour (policy validation, wrong-password rejection,
    /// change-password, hashing, verification) keep an explicit password.
    pub const SECRET_PASSWORD: &str = "C0rrect!Horse";

    /// JWT secret the test `AppState` fixtures are built with.
    ///
    /// The auth middleware builds its token provider from the instance
    /// identity, so tests mint tokens with this value.
    pub const TEST_JWT_SECRET: &str =
        "0000000000000000000000000000000000000000000000000000000000000000";

    /// Site pepper the test `AppState` fixtures are built with.
    pub const TEST_PEPPER: &str =
        "1111111111111111111111111111111111111111111111111111111111111111";

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
    pub struct TestUnitOfWork {
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
        /// Music playlist repository.
        pub music_playlists: MockMusicPlaylistRepository,
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
                commit_fails: Arc::new(AtomicBool::new(false)),
                committed: Arc::new(AtomicBool::new(false)),
                credentials: MockCredentialRepository::new(),
                files: MockFileRepository::new(),
                galleries: MockGalleryRepository::new(),
                images: MockImageRepository::new(),
                mime_types: MockMimeTypeRepository::new(),
                movies: MockMovieRepository::new(),
                music_albums: MockMusicAlbumRepository::new(),
                music_playlists: MockMusicPlaylistRepository::new(),
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
    }

    impl UserUnitOfWork for TestUnitOfWork {
        fn users(&mut self) -> impl UserRepository + '_ {
            &mut self.users
        }
    }

    impl LibraryUnitOfWork for TestUnitOfWork {
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
    pub struct TestUnitOfWorkFactory {
        /// Popped from the front by each `begin` call.
        pub unit_of_works: UnitOfWorkQueue,
    }

    impl TestUnitOfWorkFactory {
        /// Hand out `unit_of_work` on the first `begin` call.
        #[must_use]
        pub fn new(unit_of_work: TestUow) -> Self {
            Self {
                unit_of_works: Mutex::new(vec![unit_of_work]),
            }
        }
    }

    impl UnitOfWorkFactory for TestUnitOfWorkFactory {
        type Uow = TestUnitOfWork;

        fn begin(&self) -> impl Future<Output = Result<Self::Uow, UnitOfWorkError>> + Send {
            let unit_of_work = match self.unit_of_works.lock() {
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
            };
            ready(unit_of_work.ok_or(UnitOfWorkError::OperationFailed))
        }
    }

    /// The mocked unit of work shared by every fixture.
    pub type TestUow = TestUnitOfWork;

    /// Factory whose unit of work is wired to the `mockall` repository mocks.
    pub type TestFactory = TestUnitOfWorkFactory;

    /// The Asset fixture produced by [`AssetFactoryHarness::new`].
    pub struct AssetFactoryHarness {
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
        pub fn new(unit_of_work: TestUow) -> Self {
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
    pub struct GalleryFactoryHarness {
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
        pub fn new(unit_of_work: TestUow) -> Self {
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
    pub fn instance_state() -> Result<InstanceState, Box<dyn Error>> {
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
    pub fn app_state() -> Result<AppState, Box<dyn Error>> {
        Ok(AppState::new(
            Arc::new(MockAssetUseCaseFactory::new()),
            Arc::new(Configuration::default()),
            Arc::new(MockIdentityUseCaseFactory::new()),
            Arc::new(instance_state()?),
            Arc::new(MockLibraryUseCaseFactory::new()),
            Arc::new(MockUserUseCaseFactory::new()),
        ))
    }

    /// Build an application state around a mocked Identity use-case factory.
    ///
    /// The Library and User factories are mocks the callers never reach.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed identity cannot be built.
    pub fn app_state_with_identity(
        identity_use_case_factory: Arc<dyn IdentityUseCaseFactory>,
    ) -> Result<AppState, Box<dyn Error>> {
        Ok(AppState::new(
            Arc::new(MockAssetUseCaseFactory::new()),
            Arc::new(Configuration::default()),
            identity_use_case_factory,
            Arc::new(instance_state()?),
            Arc::new(MockLibraryUseCaseFactory::new()),
            Arc::new(MockUserUseCaseFactory::new()),
        ))
    }

    /// Build an application state around a mocked User use-case factory.
    ///
    /// The Identity and Library factories are mocks the callers never reach.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed identity cannot be built.
    pub fn app_state_with_user(
        user_use_case_factory: Arc<dyn UserUseCaseFactory>,
    ) -> Result<AppState, Box<dyn Error>> {
        Ok(AppState::new(
            Arc::new(MockAssetUseCaseFactory::new()),
            Arc::new(Configuration::default()),
            Arc::new(MockIdentityUseCaseFactory::new()),
            Arc::new(instance_state()?),
            Arc::new(MockLibraryUseCaseFactory::new()),
            user_use_case_factory,
        ))
    }

    /// Build an application state around a mocked Library use-case factory.
    ///
    /// The Identity and User factories are mocks the callers never reach.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed identity cannot be built.
    pub fn app_state_with_library(
        library_use_case_factory: Arc<dyn LibraryUseCaseFactory>,
    ) -> Result<AppState, Box<dyn Error>> {
        Ok(AppState::new(
            Arc::new(MockAssetUseCaseFactory::new()),
            Arc::new(Configuration::default()),
            Arc::new(MockIdentityUseCaseFactory::new()),
            Arc::new(instance_state()?),
            library_use_case_factory,
            Arc::new(MockUserUseCaseFactory::new()),
        ))
    }

    /// Build an application state around a mocked Asset use-case factory.
    ///
    /// The Identity, Library and User factories are mocks the callers never
    /// reach.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed identity cannot be built.
    pub fn app_state_with_asset(
        asset_use_case_factory: Arc<dyn AssetUseCaseFactory>,
    ) -> Result<AppState, Box<dyn Error>> {
        Ok(AppState::new(
            asset_use_case_factory,
            Arc::new(Configuration::default()),
            Arc::new(MockIdentityUseCaseFactory::new()),
            Arc::new(instance_state()?),
            Arc::new(MockLibraryUseCaseFactory::new()),
            Arc::new(MockUserUseCaseFactory::new()),
        ))
    }

    /// Return the message an `ApiError` attached to `response`.
    ///
    /// The message travels in a response extension, so a handler test can assert
    /// the handler-owned message without depending on the HAL error envelope
    /// that the root middleware builds (`TEST-049`).
    #[must_use]
    pub fn error_message_of(response: &Response) -> Option<String> {
        response
            .extensions()
            .get::<ApiErrorMessage>()
            .map(|message| message.0.clone())
    }

    /// The release date the album fixtures carry.
    #[must_use]
    pub fn album_release_date() -> NaiveDate {
        NaiveDate::from_ymd_opt(1959, 8, 17).unwrap_or_default()
    }

    /// The date the video fixtures air or release on.
    #[must_use]
    pub fn video_date() -> NaiveDate {
        NaiveDate::from_ymd_opt(1990, 4, 8).unwrap_or_default()
    }

    /// Build a progressive video backed by `file_id`.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed video values violate the domain rules.
    pub fn sample_video(video_id: NumericID, file_id: NumericID) -> Result<Video, Box<dyn Error>> {
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
        )?)
    }

    /// Build a film named `Movie {id}`, bearing one genre and one video.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed film values violate the domain rules.
    pub fn sample_movie(id: NumericID) -> Result<Movie, Box<dyn Error>> {
        Ok(Movie::try_new(
            id,
            format!("Movie {id}"),
            "A plot".to_owned(),
            "US".to_owned(),
            video_date(),
            vec!["DRAMA".to_owned()],
            sample_video(id, id)?,
        )?)
    }

    /// Build a show named `Series {id}` with `seasons` and one genre.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed show values violate the domain rules.
    pub fn sample_series(id: NumericID, seasons: Vec<Season>) -> Result<Series, Box<dyn Error>> {
        Ok(Series::try_new(
            id,
            format!("Series {id}"),
            "A plot".to_owned(),
            "US".to_owned(),
            video_date(),
            vec!["DRAMA".to_owned()],
            seasons,
        )?)
    }

    /// Build a season at `season_index` holding `episodes`.
    #[must_use]
    pub fn sample_season(season_index: u32, episodes: Vec<Episode>) -> Season {
        Season::new(SeasonIndex::new(season_index), None, episodes)
    }

    /// Build an episode named `Episode {episode_index}` with one video.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed episode values violate the domain rules.
    pub fn sample_episode(
        episode_index: u32,
        video_id: NumericID,
    ) -> Result<Episode, Box<dyn Error>> {
        Ok(Episode::try_new(
            EpisodeIndex::new(episode_index),
            format!("Episode {episode_index}"),
            "A plot".to_owned(),
            video_date(),
            sample_video(video_id, video_id)?,
        )?)
    }

    /// Expect the caller load to find a standard user named `caller`.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed caller cannot be built.
    pub fn expect_caller(
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

    /// Split `response` into its status, `Content-Type` header and decoded JSON
    /// body.
    ///
    /// # Errors
    ///
    /// Returns an error when the body cannot be read or is not JSON.
    pub async fn into_parts(
        response: Response,
    ) -> Result<(StatusCode, Option<String>, Value), Box<dyn Error>> {
        let status = response.status();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let body = serde_json::from_slice(&bytes)?;
        Ok((status, content_type, body))
    }

    /// Split `response` into its status and decoded JSON body.
    ///
    /// # Errors
    ///
    /// Returns an error when the body cannot be read or is not JSON.
    pub async fn into_status_and_body(
        response: Response,
    ) -> Result<(StatusCode, Value), Box<dyn Error>> {
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        Ok((status, serde_json::from_slice(&bytes)?))
    }
}
