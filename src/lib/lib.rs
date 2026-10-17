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

    use arc_swap::ArcSwap;
    use axum::response::Response;

    use crate::application::port::asset_use_case_factory::AssetUseCaseFactory;
    use crate::application::port::asset_use_case_factory::MockAssetUseCaseFactory;
    use crate::application::port::identity_use_case_factory::IdentityUseCaseFactory;
    use crate::application::port::identity_use_case_factory::MockIdentityUseCaseFactory;
    use crate::application::port::library_use_case_factory::LibraryUseCaseFactory;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::user_use_case_factory::MockUserUseCaseFactory;
    use crate::application::port::user_use_case_factory::UserUseCaseFactory;
    use crate::domain::model::configuration::Asset;
    use crate::domain::model::configuration::Configuration;
    use crate::domain::model::configuration::Jwt;
    use crate::domain::model::configuration::Logging;
    use crate::domain::model::configuration::Persistence;
    use crate::domain::model::configuration::Rotation;
    use crate::domain::model::configuration::Security;
    use crate::domain::model::configuration::Sqlite3;
    use crate::domain::port::asset_unit_of_work::AssetUnitOfWork;
    use crate::domain::port::configuration_repository::ConfigurationRepository;
    use crate::domain::port::configuration_repository::MockConfigurationRepository;
    use crate::domain::port::configuration_unit_of_work::ConfigurationUnitOfWork;
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
    use crate::domain::port::music_album_repository::MockMusicAlbumRepository;
    use crate::domain::port::music_album_repository::MusicAlbumRepository;
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

    /// A password that satisfies the password policy.
    ///
    /// Use it when a test only needs a valid password: registering a user,
    /// authenticating, or provisioning a credential. Tests whose subject is
    /// password behaviour (policy validation, wrong-password rejection,
    /// change-password, hashing, verification) keep an explicit password.
    pub const SECRET_PASSWORD: &str = "C0rrect!Horse";

    /// JWT secret the test `AppState` fixtures are built with.
    ///
    /// The auth middleware builds its token provider from the live
    /// configuration, so tests mint tokens with this value.
    pub const TEST_JWT_SECRET: &str =
        "0000000000000000000000000000000000000000000000000000000000000000";

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
        /// Configuration repository.
        pub configuration: MockConfigurationRepository,
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
        /// Music album repository.
        pub music_albums: MockMusicAlbumRepository,
        /// Set when `rollback` is called.
        pub rolled_back: Arc<AtomicBool>,
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
                configuration: MockConfigurationRepository::new(),
                credentials: MockCredentialRepository::new(),
                files: MockFileRepository::new(),
                galleries: MockGalleryRepository::new(),
                images: MockImageRepository::new(),
                mime_types: MockMimeTypeRepository::new(),
                music_albums: MockMusicAlbumRepository::new(),
                rolled_back: Arc::new(AtomicBool::new(false)),
                uploads: MockUploadRepository::new(),
                users: MockUserRepository::new(),
                videos: MockVideoRepository::new(),
            }
        }
    }

    impl ConfigurationUnitOfWork for TestUnitOfWork {
        fn configuration(&mut self) -> impl ConfigurationRepository + '_ {
            &mut self.configuration
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

        fn music_albums(&mut self) -> impl MusicAlbumRepository + '_ {
            &mut self.music_albums
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

    /// Build a configuration whose JWT settings use `jwt_secret`.
    ///
    /// All other settings are fixed valid values, and the pepper is `"1"`
    /// repeated 64 times.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed configuration cannot be built.
    pub fn configuration_with_jwt_secret(
        jwt_secret: &str,
    ) -> Result<Configuration, Box<dyn Error>> {
        let jwt = Jwt::try_new(jwt_secret.to_owned(), 3600)?;
        let security = Security::try_new(jwt, "1".repeat(64), true)?;
        let sqlite3 = Sqlite3::try_new(":memory:".to_owned(), 1)?;
        let logging = Logging::try_new(false, "debug,sqlx=warn".to_owned(), 7, Rotation::Daily)?;
        Ok(Configuration::new(
            Persistence::new(sqlite3),
            security,
            logging,
            Asset::default(),
        ))
    }

    /// Build an application state whose use-case factories are mocks and whose
    /// configuration is a fixed valid snapshot.
    ///
    /// The auth middleware builds its token provider from that configuration,
    /// so tests mint tokens with [`TEST_JWT_SECRET`].
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed configuration cannot be built.
    pub fn app_state() -> Result<AppState, Box<dyn Error>> {
        Ok(AppState::new(
            Arc::new(MockAssetUseCaseFactory::new()),
            Arc::new(ArcSwap::from_pointee(configuration_with_jwt_secret(
                TEST_JWT_SECRET,
            )?)),
            Arc::new(MockIdentityUseCaseFactory::new()),
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
    /// Returns an error when the fixed configuration cannot be built.
    pub fn app_state_with_identity(
        identity_use_case_factory: Arc<dyn IdentityUseCaseFactory>,
    ) -> Result<AppState, Box<dyn Error>> {
        Ok(AppState::new(
            Arc::new(MockAssetUseCaseFactory::new()),
            Arc::new(ArcSwap::from_pointee(configuration_with_jwt_secret(
                TEST_JWT_SECRET,
            )?)),
            identity_use_case_factory,
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
    /// Returns an error when the fixed configuration cannot be built.
    pub fn app_state_with_user(
        user_use_case_factory: Arc<dyn UserUseCaseFactory>,
    ) -> Result<AppState, Box<dyn Error>> {
        Ok(AppState::new(
            Arc::new(MockAssetUseCaseFactory::new()),
            Arc::new(ArcSwap::from_pointee(configuration_with_jwt_secret(
                TEST_JWT_SECRET,
            )?)),
            Arc::new(MockIdentityUseCaseFactory::new()),
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
    /// Returns an error when the fixed configuration cannot be built.
    pub fn app_state_with_library(
        library_use_case_factory: Arc<dyn LibraryUseCaseFactory>,
    ) -> Result<AppState, Box<dyn Error>> {
        Ok(AppState::new(
            Arc::new(MockAssetUseCaseFactory::new()),
            Arc::new(ArcSwap::from_pointee(configuration_with_jwt_secret(
                TEST_JWT_SECRET,
            )?)),
            Arc::new(MockIdentityUseCaseFactory::new()),
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
    /// Returns an error when the fixed configuration cannot be built.
    pub fn app_state_with_asset(
        asset_use_case_factory: Arc<dyn AssetUseCaseFactory>,
    ) -> Result<AppState, Box<dyn Error>> {
        Ok(AppState::new(
            asset_use_case_factory,
            Arc::new(ArcSwap::from_pointee(configuration_with_jwt_secret(
                TEST_JWT_SECRET,
            )?)),
            Arc::new(MockIdentityUseCaseFactory::new()),
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
}
