pub mod application;
pub mod domain;
pub mod infrastructure;

/// Fixtures shared by the in-crate test modules.
#[cfg(test)]
mod test_helpers {
    use std::future::Future;
    use std::future::ready;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;

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
}
