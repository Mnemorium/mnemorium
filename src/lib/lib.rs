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

    use crate::application::port::identity_use_case_factory::IdentityUseCaseFactory;
    use crate::application::port::identity_use_case_factory::MockIdentityUseCaseFactory;
    use crate::application::port::user_use_case_factory::MockUserUseCaseFactory;
    use crate::application::port::user_use_case_factory::UserUseCaseFactory;
    use crate::domain::model::configuration::Configuration;
    use crate::domain::model::jwt::Jwt;
    use crate::domain::model::logging::Logging;
    use crate::domain::model::logging::Rotation;
    use crate::domain::model::persistence::Persistence;
    use crate::domain::model::security::Security;
    use crate::domain::model::sqlite3::Sqlite3;
    use crate::domain::port::configuration_repository::ConfigurationRepository;
    use crate::domain::port::configuration_repository::MockConfigurationRepository;
    use crate::domain::port::configuration_unit_of_work::ConfigurationUnitOfWork;
    use crate::domain::port::credential_repository::CredentialRepository;
    use crate::domain::port::credential_repository::MockCredentialRepository;
    use crate::domain::port::error::UnitOfWorkError;
    use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
    use crate::domain::port::unit_of_work::UnitOfWork;
    use crate::domain::port::unit_of_work::UnitOfWorkFactory;
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
    pub struct TestUnitOfWork<U, C, K> {
        /// Set when `commit` is called.
        pub committed: Arc<AtomicBool>,
        /// Configuration repository.
        pub configuration: K,
        /// Credential repository.
        pub credentials: C,
        /// Set when `rollback` is called.
        pub rolled_back: Arc<AtomicBool>,
        /// User repository.
        pub users: U,
    }

    impl<U: UserRepository, C: CredentialRepository, K: ConfigurationRepository>
        ConfigurationUnitOfWork for TestUnitOfWork<U, C, K>
    {
        fn configuration(&mut self) -> impl ConfigurationRepository + '_ {
            &mut self.configuration
        }
    }

    impl<U: UserRepository, C: CredentialRepository, K: ConfigurationRepository> IdentityUnitOfWork
        for TestUnitOfWork<U, C, K>
    {
        fn credentials(&mut self) -> impl CredentialRepository + '_ {
            &mut self.credentials
        }
    }

    impl<U: UserRepository, C: CredentialRepository, K: ConfigurationRepository> UserUnitOfWork
        for TestUnitOfWork<U, C, K>
    {
        fn users(&mut self) -> impl UserRepository + '_ {
            &mut self.users
        }
    }

    impl<U: UserRepository, C: CredentialRepository, K: ConfigurationRepository> UnitOfWork
        for TestUnitOfWork<U, C, K>
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

    /// Fake factory handing out a single prepared [`TestUnitOfWork`].
    pub struct TestUnitOfWorkFactory<U, C, K> {
        /// Taken by the first `begin` call.
        pub unit_of_work: Mutex<Option<TestUnitOfWork<U, C, K>>>,
    }

    impl<U: UserRepository, C: CredentialRepository, K: ConfigurationRepository> UnitOfWorkFactory
        for TestUnitOfWorkFactory<U, C, K>
    {
        type Uow = TestUnitOfWork<U, C, K>;

        fn begin(&self) -> impl Future<Output = Result<Self::Uow, UnitOfWorkError>> + Send {
            let unit_of_work = match self.unit_of_work.lock() {
                Ok(mut guard) => guard.take(),
                Err(poisoned) => poisoned.into_inner().take(),
            };
            ready(unit_of_work.ok_or(UnitOfWorkError::OperationFailed))
        }
    }

    /// Factory whose unit of work is wired to the `mockall` repository mocks.
    pub type TestFactory = TestUnitOfWorkFactory<
        MockUserRepository,
        MockCredentialRepository,
        MockConfigurationRepository,
    >;

    /// Build a [`TestFactory`] holding the given mocked repositories and
    /// transaction-lifecycle flags.
    pub fn unit_of_work_factory(
        users: MockUserRepository,
        credentials: MockCredentialRepository,
        configuration: MockConfigurationRepository,
        committed: Arc<AtomicBool>,
        rolled_back: Arc<AtomicBool>,
    ) -> TestFactory {
        TestUnitOfWorkFactory {
            unit_of_work: Mutex::new(Some(TestUnitOfWork {
                committed,
                configuration,
                credentials,
                rolled_back,
                users,
            })),
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
        let configuration =
            Configuration::try_new(Persistence::try_new(sqlite3), security, logging);
        Ok(AppState::new(
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
        let configuration =
            Configuration::try_new(Persistence::try_new(sqlite3), security, logging);
        Ok(AppState::new(
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
        let configuration =
            Configuration::try_new(Persistence::try_new(sqlite3), security, logging);
        Ok(AppState::new(
            Arc::new(ArcSwap::from_pointee(configuration)),
            Arc::new(MockIdentityUseCaseFactory::new()),
            Arc::new(JwtTokenProvider::new("tmptmp".to_owned(), 3600)),
            user_use_case_factory,
        ))
    }
}
