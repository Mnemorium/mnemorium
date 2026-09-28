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

    use crate::application::port::asset_use_case_factory::AssetUseCaseFactory;
    use crate::application::port::asset_use_case_factory::MockAssetUseCaseFactory;
    use crate::application::port::identity_use_case_factory::IdentityUseCaseFactory;
    use crate::application::port::identity_use_case_factory::MockIdentityUseCaseFactory;
    use crate::application::port::user_use_case_factory::MockUserUseCaseFactory;
    use crate::application::port::user_use_case_factory::UserUseCaseFactory;
    use crate::domain::model::asset::Asset;
    use crate::domain::model::configuration::Configuration;
    use crate::domain::model::jwt::Jwt;
    use crate::domain::model::logging::Logging;
    use crate::domain::model::logging::Rotation;
    use crate::domain::model::persistence::Persistence;
    use crate::domain::model::security::Security;
    use crate::domain::model::sqlite3::Sqlite3;
    use crate::domain::port::asset_unit_of_work::AssetUnitOfWork;
    use crate::domain::port::configuration_repository::ConfigurationRepository;
    use crate::domain::port::configuration_repository::MockConfigurationRepository;
    use crate::domain::port::configuration_unit_of_work::ConfigurationUnitOfWork;
    use crate::domain::port::credential_repository::CredentialRepository;
    use crate::domain::port::credential_repository::MockCredentialRepository;
    use crate::domain::port::error::UnitOfWorkError;
    use crate::domain::port::file_repository::FileRepository;
    use crate::domain::port::file_repository::MockFileRepository;
    use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
    use crate::domain::port::mime_type_repository::MimeTypeRepository;
    use crate::domain::port::mime_type_repository::MockMimeTypeRepository;
    use crate::domain::port::unit_of_work::UnitOfWork;
    use crate::domain::port::unit_of_work::UnitOfWorkFactory;
    use crate::domain::port::upload_repository::MockUploadRepository;
    use crate::domain::port::upload_repository::UploadRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::domain::port::user_repository::UserRepository;
    use crate::domain::port::user_unit_of_work::UserUnitOfWork;
    use crate::infrastructure::inbound::rest::app_state::AppState;
    use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;

    /// A password that satisfies the password policy.
    ///
    /// Use it when a test only needs a valid password: registering a user,
    /// authenticating, or provisioning a credential. Tests whose subject is
    /// password behaviour (policy validation, wrong-password rejection,
    /// change-password, hashing, verification) keep an explicit password.
    pub const SECRET_PASSWORD: &str = "C0rrect!Horse";

    /// Fake unit of work wiring mocked repositories and recording its outcome.
    ///
    /// Its accessors return mutable views over the mocked repositories, which
    /// is why `UnitOfWork` cannot be `mockall`-generated. `committed` and
    /// `rolled_back` are shared with the test so it can assert the transaction
    /// lifecycle after the use case has consumed the unit of work.
    ///
    /// One type serves every bounded context: a context a test does not
    /// exercise holds fresh mocks supplied by the constructors below.
    pub struct TestUnitOfWork<U, C, K, F, M, P> {
        /// Set when `commit` is called.
        pub committed: Arc<AtomicBool>,
        /// Configuration repository.
        pub configuration: K,
        /// Credential repository.
        pub credentials: C,
        /// File repository.
        pub files: F,
        /// Mime type repository.
        pub mime_types: M,
        /// Set when `rollback` is called.
        pub rolled_back: Arc<AtomicBool>,
        /// Upload repository.
        pub uploads: P,
        /// User repository.
        pub users: U,
    }

    impl<
        U: UserRepository,
        C: CredentialRepository,
        K: ConfigurationRepository,
        F: FileRepository,
        M: MimeTypeRepository,
        P: UploadRepository,
    > ConfigurationUnitOfWork for TestUnitOfWork<U, C, K, F, M, P>
    {
        fn configuration(&mut self) -> impl ConfigurationRepository + '_ {
            &mut self.configuration
        }
    }

    impl<
        U: UserRepository,
        C: CredentialRepository,
        K: ConfigurationRepository,
        F: FileRepository,
        M: MimeTypeRepository,
        P: UploadRepository,
    > IdentityUnitOfWork for TestUnitOfWork<U, C, K, F, M, P>
    {
        fn credentials(&mut self) -> impl CredentialRepository + '_ {
            &mut self.credentials
        }
    }

    impl<
        U: UserRepository,
        C: CredentialRepository,
        K: ConfigurationRepository,
        F: FileRepository,
        M: MimeTypeRepository,
        P: UploadRepository,
    > UserUnitOfWork for TestUnitOfWork<U, C, K, F, M, P>
    {
        fn users(&mut self) -> impl UserRepository + '_ {
            &mut self.users
        }
    }

    impl<
        U: UserRepository,
        C: CredentialRepository,
        K: ConfigurationRepository,
        F: FileRepository,
        M: MimeTypeRepository,
        P: UploadRepository,
    > AssetUnitOfWork for TestUnitOfWork<U, C, K, F, M, P>
    {
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

    impl<
        U: UserRepository,
        C: CredentialRepository,
        K: ConfigurationRepository,
        F: FileRepository,
        M: MimeTypeRepository,
        P: UploadRepository,
    > UnitOfWork for TestUnitOfWork<U, C, K, F, M, P>
    {
        fn commit(self) -> impl Future<Output = Result<(), UnitOfWorkError>> + Send {
            self.committed.store(true, Ordering::SeqCst);
            ready(Ok(()))
        }

        fn rollback(self) -> impl Future<Output = Result<(), UnitOfWorkError>> + Send {
            self.rolled_back.store(true, Ordering::SeqCst);
            ready(Ok(()))
        }
    }

    /// Units of work handed out one per `begin` call.
    type UnitOfWorkQueue<U, C, K, F, M, P> = Mutex<Vec<TestUnitOfWork<U, C, K, F, M, P>>>;

    /// Fake factory handing out a queue of prepared [`TestUnitOfWork`]s, one per
    /// `begin` call, so retry loops can bind a unit of work per attempt.
    pub struct TestUnitOfWorkFactory<U, C, K, F, M, P> {
        /// Popped from the front by each `begin` call.
        pub unit_of_works: UnitOfWorkQueue<U, C, K, F, M, P>,
    }

    impl<
        U: UserRepository,
        C: CredentialRepository,
        K: ConfigurationRepository,
        F: FileRepository,
        M: MimeTypeRepository,
        P: UploadRepository,
    > UnitOfWorkFactory for TestUnitOfWorkFactory<U, C, K, F, M, P>
    {
        type Uow = TestUnitOfWork<U, C, K, F, M, P>;

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
    pub type TestUow = TestUnitOfWork<
        MockUserRepository,
        MockCredentialRepository,
        MockConfigurationRepository,
        MockFileRepository,
        MockMimeTypeRepository,
        MockUploadRepository,
    >;

    /// Factory whose unit of work is wired to the `mockall` repository mocks.
    pub type TestFactory = TestUnitOfWorkFactory<
        MockUserRepository,
        MockCredentialRepository,
        MockConfigurationRepository,
        MockFileRepository,
        MockMimeTypeRepository,
        MockUploadRepository,
    >;

    /// The unit-of-work fixtures produced by [`asset_factory`].
    pub struct AssetFactoryHarness {
        /// Set when the unit of work is committed.
        pub committed: Arc<AtomicBool>,
        /// The factory handed to the use case under test.
        pub factory: Arc<TestFactory>,
        /// Set when the unit of work is rolled back.
        pub rolled_back: Arc<AtomicBool>,
    }

    /// Build a [`TestFactory`] holding the given User, Identity and
    /// Configuration repositories and transaction-lifecycle flags.
    ///
    /// The File, Mime type and Upload repositories are fresh mocks the caller
    /// never reaches.
    pub fn unit_of_work_factory(
        users: MockUserRepository,
        credentials: MockCredentialRepository,
        configuration: MockConfigurationRepository,
        committed: Arc<AtomicBool>,
        rolled_back: Arc<AtomicBool>,
    ) -> TestFactory {
        TestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![TestUnitOfWork {
                committed,
                configuration,
                credentials,
                files: MockFileRepository::new(),
                mime_types: MockMimeTypeRepository::new(),
                rolled_back,
                uploads: MockUploadRepository::new(),
                users,
            }]),
        }
    }

    /// Build an Asset [`TestFactory`] around the given mocked repositories.
    ///
    /// The User, Identity and Configuration repositories are fresh mocks the
    /// caller never reaches.
    #[must_use]
    pub fn asset_factory(
        uploads: MockUploadRepository,
        files: MockFileRepository,
        mime_types: MockMimeTypeRepository,
    ) -> AssetFactoryHarness {
        let committed = Arc::new(AtomicBool::new(false));
        let rolled_back = Arc::new(AtomicBool::new(false));
        let factory = TestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![asset_unit_of_work_with(
                uploads,
                files,
                mime_types,
                Arc::clone(&committed),
                Arc::clone(&rolled_back),
            )]),
        };
        AssetFactoryHarness {
            committed,
            factory: Arc::new(factory),
            rolled_back,
        }
    }

    /// Build one Asset [`TestUow`] and its lifecycle flags.
    #[must_use]
    pub fn asset_unit_of_work(
        uploads: MockUploadRepository,
        files: MockFileRepository,
        mime_types: MockMimeTypeRepository,
    ) -> (TestUow, Arc<AtomicBool>, Arc<AtomicBool>) {
        let committed = Arc::new(AtomicBool::new(false));
        let rolled_back = Arc::new(AtomicBool::new(false));
        let unit_of_work = asset_unit_of_work_with(
            uploads,
            files,
            mime_types,
            Arc::clone(&committed),
            Arc::clone(&rolled_back),
        );
        (unit_of_work, committed, rolled_back)
    }

    /// Assemble an Asset [`TestUow`] around the given mocks and lifecycle flags.
    fn asset_unit_of_work_with(
        uploads: MockUploadRepository,
        files: MockFileRepository,
        mime_types: MockMimeTypeRepository,
        committed: Arc<AtomicBool>,
        rolled_back: Arc<AtomicBool>,
    ) -> TestUow {
        TestUnitOfWork {
            committed,
            configuration: MockConfigurationRepository::new(),
            credentials: MockCredentialRepository::new(),
            files,
            mime_types,
            rolled_back,
            uploads,
            users: MockUserRepository::new(),
        }
    }

    /// Build an application state whose use-case factories are mocks and whose
    /// configuration is a fixed valid snapshot.
    ///
    /// Only the token provider is exercised by the callers; the factories are
    /// never reached.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed configuration cannot be built.
    #[expect(
        clippy::single_call_fn,
        reason = "only the auth middleware tests build an application state around a token provider"
    )]
    pub fn app_state(token_provider: Arc<JwtTokenProvider>) -> Result<AppState, Box<dyn Error>> {
        let jwt = Jwt::try_new("0".repeat(64), 3600)?;
        let security = Security::try_new(jwt, "1".repeat(64), true)?;
        let sqlite3 = Sqlite3::try_new(":memory:".to_owned(), 1)?;
        let logging = Logging::try_new(false, "debug,sqlx=warn".to_owned(), 7, Rotation::Daily)?;
        let configuration = Configuration::new(
            Persistence::new(sqlite3),
            security,
            logging,
            Asset::default(),
        );
        Ok(AppState::new(
            Arc::new(MockAssetUseCaseFactory::new()),
            Arc::new(ArcSwap::from_pointee(configuration)),
            Arc::new(MockIdentityUseCaseFactory::new()),
            token_provider,
            Arc::new(MockUserUseCaseFactory::new()),
        ))
    }

    /// Build an application state around a mocked Identity use-case factory.
    ///
    /// The token provider and the User factory are mocks the callers never
    /// reach.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed configuration cannot be built.
    pub fn app_state_with_identity(
        identity_use_case_factory: Arc<dyn IdentityUseCaseFactory>,
    ) -> Result<AppState, Box<dyn Error>> {
        let jwt = Jwt::try_new("0".repeat(64), 3600)?;
        let security = Security::try_new(jwt, "1".repeat(64), true)?;
        let sqlite3 = Sqlite3::try_new(":memory:".to_owned(), 1)?;
        let logging = Logging::try_new(false, "debug,sqlx=warn".to_owned(), 7, Rotation::Daily)?;
        let configuration = Configuration::new(
            Persistence::new(sqlite3),
            security,
            logging,
            Asset::default(),
        );
        Ok(AppState::new(
            Arc::new(MockAssetUseCaseFactory::new()),
            Arc::new(ArcSwap::from_pointee(configuration)),
            identity_use_case_factory,
            Arc::new(JwtTokenProvider::new("tmptmp".to_owned(), 3600)),
            Arc::new(MockUserUseCaseFactory::new()),
        ))
    }

    /// Build an application state around a mocked User use-case factory.
    ///
    /// The token provider and the Identity factory are mocks the callers never
    /// reach.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed configuration cannot be built.
    pub fn app_state_with_user(
        user_use_case_factory: Arc<dyn UserUseCaseFactory>,
    ) -> Result<AppState, Box<dyn Error>> {
        let jwt = Jwt::try_new("0".repeat(64), 3600)?;
        let security = Security::try_new(jwt, "1".repeat(64), true)?;
        let sqlite3 = Sqlite3::try_new(":memory:".to_owned(), 1)?;
        let logging = Logging::try_new(false, "debug,sqlx=warn".to_owned(), 7, Rotation::Daily)?;
        let configuration = Configuration::new(
            Persistence::new(sqlite3),
            security,
            logging,
            Asset::default(),
        );
        Ok(AppState::new(
            Arc::new(MockAssetUseCaseFactory::new()),
            Arc::new(ArcSwap::from_pointee(configuration)),
            Arc::new(MockIdentityUseCaseFactory::new()),
            Arc::new(JwtTokenProvider::new("tmptmp".to_owned(), 3600)),
            user_use_case_factory,
        ))
    }

    /// Build an application state around a mocked Asset use-case factory.
    ///
    /// The token provider and the Identity and User factories are mocks the
    /// callers never reach.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed configuration cannot be built.
    pub fn app_state_with_asset(
        asset_use_case_factory: Arc<dyn AssetUseCaseFactory>,
    ) -> Result<AppState, Box<dyn Error>> {
        let jwt = Jwt::try_new("0".repeat(64), 3600)?;
        let security = Security::try_new(jwt, "1".repeat(64), true)?;
        let sqlite3 = Sqlite3::try_new(":memory:".to_owned(), 1)?;
        let logging = Logging::try_new(false, "debug,sqlx=warn".to_owned(), 7, Rotation::Daily)?;
        let configuration = Configuration::new(
            Persistence::new(sqlite3),
            security,
            logging,
            Asset::default(),
        );
        Ok(AppState::new(
            asset_use_case_factory,
            Arc::new(ArcSwap::from_pointee(configuration)),
            Arc::new(MockIdentityUseCaseFactory::new()),
            Arc::new(JwtTokenProvider::new("tmptmp".to_owned(), 3600)),
            Arc::new(MockUserUseCaseFactory::new()),
        ))
    }
}
