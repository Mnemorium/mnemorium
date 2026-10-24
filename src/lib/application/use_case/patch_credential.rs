use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use crate::application::port::patch_credential::PatchCredentialCommand;
use crate::application::port::patch_credential::PatchCredentialError;
use crate::application::port::patch_credential::PatchCredentialUseCase;
use crate::application::security_event;
use crate::domain::model::credential::Credential;
use crate::domain::model::user::Role;
use crate::domain::port::credential_repository::CredentialFilter;
use crate::domain::port::credential_repository::CredentialRepository as _;
use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
use crate::domain::port::password_hasher::PasswordHasher;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::service::password_policy::PasswordPolicy;
use crate::domain::service::password_policy::PasswordPolicyError;

/// Use case implementation for changing the password behind a credential.
pub struct PatchCredential<F, H> {
    /// Hasher for user passwords.
    password_hasher: Arc<H>,
    /// Factory opening the unit of work wrapping the change.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, H: PasswordHasher> PatchCredential<F, H> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>, password_hasher: Arc<H>) -> Self {
        Self {
            password_hasher,
            unit_of_work_factory,
        }
    }
}

impl<F, H> PatchCredentialUseCase for PatchCredential<F, H>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + UserUnitOfWork,
    H: PasswordHasher,
{
    fn execute<'future>(
        &'future self,
        command: PatchCredentialCommand,
    ) -> Pin<Box<dyn Future<Output = Result<(), PatchCredentialError>> + Send + 'future>> {
        let password_hasher = Arc::clone(&self.password_hasher);
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);
        let password_policy = PasswordPolicy::new();

        Box::pin(async move {
            password_policy
                .validate(command.password())
                .map_err(|error| match error {
                    PasswordPolicyError::PasswordMissingSymbol
                    | PasswordPolicyError::PasswordTooShort => {
                        PatchCredentialError::InvalidPassword
                    }
                })?;

            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| PatchCredentialError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| PatchCredentialError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or_else(|| {
                        security_event::authorization_failed(
                            command.caller_id(),
                            "patch",
                            "credential",
                        );
                        PatchCredentialError::Forbidden
                    })?;

                if !(caller.role() == Role::Admin && caller.id() == 0) {
                    security_event::authorization_failed(caller.id(), "patch", "credential");
                    return Err(PatchCredentialError::Forbidden);
                }

                let credential = unit_of_work
                    .credentials()
                    .search(&CredentialFilter {
                        id: Some(command.credential_id()),
                        ..CredentialFilter::default()
                    })
                    .await
                    .map_err(|error| PatchCredentialError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(PatchCredentialError::UnknownCredential)?;

                let password_hash = password_hasher
                    .hash_password(command.password())
                    .await
                    .map_err(|error| PatchCredentialError::Unknown(error.into()))?;
                let updated_credential =
                    Credential::try_new(credential.id(), password_hash, Utc::now().naive_utc())
                        .map_err(|error| PatchCredentialError::Unknown(error.into()))?;

                unit_of_work
                    .credentials()
                    .save(updated_credential)
                    .await
                    .map_err(|error| PatchCredentialError::Unknown(error.into()))?;

                Ok(())
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| PatchCredentialError::Unknown(error.into()))?;
                    security_event::credential_rotated(command.caller_id());
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
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;

    use chrono::NaiveDateTime;

    use crate::application::port::patch_credential::PatchCredentialCommand;
    use crate::application::port::patch_credential::PatchCredentialError;
    use crate::application::port::patch_credential::PatchCredentialUseCase as _;
    use crate::domain::model::credential::Credential;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::model::user::UserError;
    use crate::domain::port::credential_repository::MockCredentialRepository;
    use crate::domain::port::error::PasswordHasherError;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::password_hasher::MockPasswordHasher;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::SECRET_PASSWORD;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::TestUnitOfWork;

    use super::PatchCredential;

    type UseCase = PatchCredential<TestFactory, MockPasswordHasher>;

    /// A use case under test together with its transaction-lifecycle flags.
    struct Harness {
        /// Set when the unit of work is committed.
        committed: Arc<AtomicBool>,
        /// Set when the unit of work is rolled back.
        rolled_back: Arc<AtomicBool>,
        /// The use case under test.
        use_case: UseCase,
    }

    /// Build a use case whose outbound dependencies are mocked; `setup` defines
    /// the mock expectations before the mocks are handed over.
    fn use_case_with(
        setup: impl FnOnce(
            &mut MockUserRepository,
            &mut MockCredentialRepository,
            &mut MockPasswordHasher,
        ) -> Result<(), Box<dyn Error>>,
    ) -> Result<Harness, Box<dyn Error>> {
        let mut user_repository = MockUserRepository::new();
        let mut credential_repository = MockCredentialRepository::new();
        let mut password_hasher = MockPasswordHasher::new();

        setup(
            &mut user_repository,
            &mut credential_repository,
            &mut password_hasher,
        )?;

        let unit_of_work = TestUnitOfWork {
            users: user_repository,
            credentials: credential_repository,
            ..TestUnitOfWork::default()
        };
        let committed = Arc::clone(&unit_of_work.committed);
        let rolled_back = Arc::clone(&unit_of_work.rolled_back);
        let factory = TestFactory::new(unit_of_work);

        Ok(Harness {
            use_case: PatchCredential::new(Arc::new(factory), Arc::new(password_hasher)),
            committed,
            rolled_back,
        })
    }

    fn user(id: i64, username: &str, role: Role) -> Result<User, UserError> {
        User::try_new(id, username.to_owned(), None, id, role)
    }

    fn credential(id: i64) -> Result<Credential, Box<dyn Error>> {
        let updated_at = NaiveDateTime::parse_from_str("2026-01-01 12:00:00", "%F %T")?;
        Ok(Credential::try_new(id, "old-hash".to_owned(), updated_at)?)
    }

    fn command(caller_id: i64, credential_id: i64, password: &str) -> PatchCredentialCommand {
        PatchCredentialCommand::new(caller_id, credential_id, password.to_owned())
    }

    fn expect_caller(user_repository: &mut MockUserRepository, caller: User) {
        user_repository
            .expect_search()
            .times(1)
            .returning(move |_| {
                let own_caller = caller.clone();
                Box::pin(async move { Ok(vec![own_caller]) })
            });
    }

    fn expect_credential(
        credential_repository: &mut MockCredentialRepository,
        credential: Credential,
    ) {
        credential_repository
            .expect_search()
            .times(1)
            .returning(move |_| {
                let own_credential = credential.clone();
                Box::pin(async move { Ok(vec![own_credential]) })
            });
    }

    fn expect_hashed_password(password_hasher: &mut MockPasswordHasher) {
        password_hasher
            .expect_hash_password()
            .times(1)
            .returning(|_| Box::pin(async { Ok("new-hash".to_owned()) }));
    }

    fn expect_saved_credential(credential_repository: &mut MockCredentialRepository) {
        credential_repository
            .expect_save()
            .times(1)
            .returning(|credential| Box::pin(async { Ok(credential) }));
    }

    #[tokio::test]
    async fn patch_credential_root_admin_changes_own_credential_succeeds()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, credential_repository, password_hasher| {
            expect_caller(user_repository, user(0, "root", Role::Admin)?);
            expect_credential(credential_repository, credential(0)?);
            expect_hashed_password(password_hasher);
            expect_saved_credential(credential_repository);
            Ok(())
        })?;
        let command = command(0, 0, SECRET_PASSWORD);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_root_admin_changes_other_user_credential_succeeds()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, credential_repository, password_hasher| {
            expect_caller(user_repository, user(0, "root", Role::Admin)?);
            expect_credential(credential_repository, credential(4)?);
            expect_hashed_password(password_hasher);
            expect_saved_credential(credential_repository);
            Ok(())
        })?;
        let command = command(0, 4, SECRET_PASSWORD);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_standard_caller_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, _, _| {
            expect_caller(user_repository, user(3, "bobby", Role::Standard)?);
            Ok(())
        })?;
        let command = command(3, 1, SECRET_PASSWORD);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(PatchCredentialError::Forbidden)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_non_root_admin_caller_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, _, _| {
            expect_caller(user_repository, user(5, "admin", Role::Admin)?);
            Ok(())
        })?;
        let command = command(5, 1, SECRET_PASSWORD);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(PatchCredentialError::Forbidden)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_unknown_caller_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, _, _| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            Ok(())
        })?;
        let command = command(999, 1, SECRET_PASSWORD);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(PatchCredentialError::Forbidden)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_short_password_returns_invalid_password() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let harness = use_case_with(|_, _, _| Ok(()))?;
        let command = command(0, 0, "xY3!z9w");

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(PatchCredentialError::InvalidPassword)));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_password_without_symbol_returns_invalid_password()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|_, _, _| Ok(()))?;
        let command = command(0, 0, "password123");

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(PatchCredentialError::InvalidPassword)));
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_unknown_credential_returns_unknown_credential()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, credential_repository, _| {
            expect_caller(user_repository, user(0, "root", Role::Admin)?);
            credential_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            Ok(())
        })?;
        let command = command(0, 42, SECRET_PASSWORD);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(
            result,
            Err(PatchCredentialError::UnknownCredential)
        ));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_user_search_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, _, _| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;
        let command = command(0, 1, SECRET_PASSWORD);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(PatchCredentialError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_credential_search_failure_returns_unknown()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, credential_repository, _| {
            expect_caller(user_repository, user(0, "root", Role::Admin)?);
            credential_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;
        let command = command(0, 1, SECRET_PASSWORD);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(PatchCredentialError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_hash_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, credential_repository, password_hasher| {
            expect_caller(user_repository, user(0, "root", Role::Admin)?);
            expect_credential(credential_repository, credential(1)?);
            password_hasher
                .expect_hash_password()
                .times(1)
                .returning(|_| Box::pin(async { Err(PasswordHasherError::OperationFailed) }));
            Ok(())
        })?;
        let command = command(0, 1, SECRET_PASSWORD);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(PatchCredentialError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_credential_save_failure_returns_unknown() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let harness = use_case_with(|user_repository, credential_repository, password_hasher| {
            expect_caller(user_repository, user(0, "root", Role::Admin)?);
            expect_credential(credential_repository, credential(1)?);
            expect_hashed_password(password_hasher);
            credential_repository
                .expect_save()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;
        let command = command(0, 1, SECRET_PASSWORD);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(PatchCredentialError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn credential_debug_redacts_password_hash_but_keeps_id() -> Result<(), Box<dyn Error>> {
        // Arrange
        let password_hash = "hashed-password".to_owned();
        let updated_at = NaiveDateTime::parse_from_str("2026-01-01 12:00:00", "%F %T")?;
        let credential = Credential::try_new(7, password_hash.clone(), updated_at)?;

        // Act
        let debug = format!("{credential:?}");

        // Assert
        assert!(
            !debug.contains(password_hash.as_str()),
            "Debug output must not contain the password hash, got: {debug:?}"
        );
        assert!(
            debug.contains("id"),
            "Debug output must still contain the credential id, got: {debug:?}"
        );
        Ok(())
    }
}
