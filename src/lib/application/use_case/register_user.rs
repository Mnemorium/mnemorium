use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;
use tracing::error;

use crate::application::port::register_user::RegisterUserCommand;
use crate::application::port::register_user::RegisterUserError;
use crate::application::port::register_user::RegisterUserResponse;
use crate::application::port::register_user::RegisterUserUseCase;
use crate::domain::model::credential::Credential;
use crate::domain::model::user::Role;
use crate::domain::model::user::User;
use crate::domain::model::user::UserError;
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

/// Use case implementation for registering a new user.
pub struct RegisterUser<F, P> {
    /// Hasher for user passwords.
    password_hasher: Arc<P>,
    /// Factory opening the unit of work wrapping the registration.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, P: PasswordHasher> RegisterUser<F, P> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>, password_hasher: Arc<P>) -> Self {
        Self {
            password_hasher,
            unit_of_work_factory,
        }
    }
}

impl<F, P> RegisterUserUseCase for RegisterUser<F, P>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + UserUnitOfWork,
    P: PasswordHasher,
{
    fn execute<'future>(
        &'future self,
        command: RegisterUserCommand,
    ) -> Pin<
        Box<dyn Future<Output = Result<RegisterUserResponse, RegisterUserError>> + Send + 'future>,
    > {
        let password_hasher = Arc::clone(&self.password_hasher);
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);
        let password_policy = PasswordPolicy::new();

        Box::pin(async move {
            let username = command.username().to_owned();
            let email = command.email().map(str::to_owned);

            User::try_new(0, username.clone(), email.clone(), 0, command.role()).map_err(
                |error| match error {
                    UserError::InvalidEmail(_) => RegisterUserError::InvalidEmail,
                    UserError::UsernameTooShort => RegisterUserError::InvalidUsername,
                    UserError::Unknown(source) => RegisterUserError::Unknown(source),
                },
            )?;

            password_policy
                .validate(command.password())
                .map_err(|error| match error {
                    PasswordPolicyError::PasswordMissingSymbol
                    | PasswordPolicyError::PasswordTooShort => RegisterUserError::InvalidPassword,
                })?;

            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| RegisterUserError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| RegisterUserError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(RegisterUserError::Forbidden)?;

                let authorized = match command.role() {
                    Role::Admin => caller.role() == Role::Admin && caller.id() == 0,
                    Role::Standard => caller.role() == Role::Admin,
                };
                if !authorized {
                    return Err(RegisterUserError::Forbidden);
                }

                let username_taken = !unit_of_work
                    .users()
                    .search(&UserFilter {
                        username: Some(username.clone()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| RegisterUserError::Unknown(error.into()))?
                    .is_empty();
                if username_taken {
                    return Err(RegisterUserError::UserAlreadyExists);
                }
                if let Some(email_address) = email.as_deref() {
                    let email_taken = !unit_of_work
                        .users()
                        .search(&UserFilter {
                            email: Some(email_address.to_owned()),
                            ..UserFilter::default()
                        })
                        .await
                        .map_err(|error| RegisterUserError::Unknown(error.into()))?
                        .is_empty();
                    if email_taken {
                        return Err(RegisterUserError::UserAlreadyExists);
                    }
                }

                // TODO: the Argon2 hash is computed while the unit of work holds the single
                // pooled connection (default `max_connections = 1`), blocking other requests for
                // its duration. Raise `max_connections` if this becomes a bottleneck; do not move
                // the hash off the transaction without authorizing the caller first.
                let password_hash = password_hasher
                    .hash_password(command.password())
                    .await
                    .map_err(|error| RegisterUserError::Unknown(error.into()))?;
                let pending_credential =
                    Credential::try_new(0, password_hash, Utc::now().naive_utc())
                        .map_err(|error| RegisterUserError::Unknown(error.into()))?;
                let credential = unit_of_work
                    .credentials()
                    .create(pending_credential)
                    .await
                    .map_err(|error| RegisterUserError::Unknown(error.into()))?;

                let pending_user =
                    User::try_new(0, username, email, credential.id(), command.role())
                        .map_err(|error| RegisterUserError::Unknown(error.into()))?;
                let user = unit_of_work
                    .users()
                    .create(pending_user)
                    .await
                    .map_err(|error| RegisterUserError::Unknown(error.into()))?;

                Ok(RegisterUserResponse::new(
                    user.id(),
                    user.username().to_owned(),
                    user.email().map(str::to_owned),
                    user.role(),
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| RegisterUserError::Unknown(error.into()))?;
                    Ok(value)
                }
                Err(error) => {
                    if let Err(rollback_error) = unit_of_work.rollback().await {
                        error!(
                            error = ?rollback_error,
                            "failed to roll back the register user unit of work"
                        );
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
    use std::sync::Mutex;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;

    use crate::application::port::register_user::RegisterUserCommand;
    use crate::application::port::register_user::RegisterUserError;
    use crate::application::port::register_user::RegisterUserUseCase as _;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::model::user::UserError;
    use crate::domain::port::configuration_repository::MockConfigurationRepository;
    use crate::domain::port::credential_repository::MockCredentialRepository;
    use crate::domain::port::error::PasswordHasherError;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::password_hasher::MockPasswordHasher;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::SECRET_PASSWORD;
    use crate::test_helpers::TestUnitOfWork;
    use crate::test_helpers::TestUnitOfWorkFactory;

    use super::RegisterUser;

    type UseCase = RegisterUser<
        TestUnitOfWorkFactory<
            MockUserRepository,
            MockCredentialRepository,
            MockConfigurationRepository,
        >,
        MockPasswordHasher,
    >;

    /// A use case under test together with the transaction-lifecycle flags of
    /// its fake unit of work.
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

        let committed = Arc::new(AtomicBool::new(false));
        let rolled_back = Arc::new(AtomicBool::new(false));
        let factory = TestUnitOfWorkFactory {
            unit_of_work: Mutex::new(Some(TestUnitOfWork {
                committed: Arc::clone(&committed),
                configuration: MockConfigurationRepository::new(),
                credentials: credential_repository,
                rolled_back: Arc::clone(&rolled_back),
                users: user_repository,
            })),
        };

        Ok(Harness {
            use_case: RegisterUser::new(Arc::new(factory), Arc::new(password_hasher)),
            committed,
            rolled_back,
        })
    }

    fn user(id: i64, username: &str, role: Role) -> Result<User, UserError> {
        User::try_new(id, username.to_owned(), None, 7, role)
    }

    fn command(caller_id: i64, username: &str, role: Role) -> RegisterUserCommand {
        RegisterUserCommand::new(
            caller_id,
            username.to_owned(),
            None,
            SECRET_PASSWORD.to_owned(),
            role,
        )
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

    fn expect_unique_search(user_repository: &mut MockUserRepository) {
        user_repository
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
    }

    fn existing_users(id: i64, username: &str, role: Role) -> Result<Vec<User>, RepositoryError> {
        user(id, username, role)
            .map(|user| vec![user])
            .map_err(|_| RepositoryError::OperationFailed)
    }

    fn expect_hashed_password(password_hasher: &mut MockPasswordHasher) {
        password_hasher
            .expect_hash_password()
            .times(1)
            .returning(|_| Box::pin(async { Ok("hashed-password".to_owned()) }));
    }

    fn expect_successful_persistence(
        credential_repository: &mut MockCredentialRepository,
        password_hasher: &mut MockPasswordHasher,
        user_repository: &mut MockUserRepository,
    ) {
        expect_hashed_password(password_hasher);
        credential_repository
            .expect_create()
            .times(1)
            .returning(|credential| Box::pin(async { Ok(credential) }));
        user_repository
            .expect_create()
            .times(1)
            .returning(|user| Box::pin(async { Ok(user) }));
    }

    #[tokio::test]
    async fn register_user_root_admin_register_standard_user_succeeds() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let harness = use_case_with(|user_repository, credential_repository, password_hasher| {
            expect_caller(user_repository, user(0, "root", Role::Admin)?);
            expect_unique_search(user_repository);
            expect_unique_search(user_repository);
            expect_successful_persistence(credential_repository, password_hasher, user_repository);
            Ok(())
        })?;
        let command = RegisterUserCommand::new(
            0,
            "alice".to_owned(),
            Some("alice@example.com".to_owned()),
            SECRET_PASSWORD.to_owned(),
            Role::Standard,
        );

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        let response = result?;
        assert_eq!(response.username(), "alice");
        assert_eq!(response.email(), Some("alice@example.com"));
        assert_eq!(response.role(), Role::Standard);
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_root_admin_register_admin_user_succeeds() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, credential_repository, password_hasher| {
            expect_caller(user_repository, user(0, "root", Role::Admin)?);
            expect_unique_search(user_repository);
            expect_successful_persistence(credential_repository, password_hasher, user_repository);
            Ok(())
        })?;
        let command = command(0, "carol", Role::Admin);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_admin_register_standard_user_succeeds() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, credential_repository, password_hasher| {
            expect_caller(user_repository, user(5, "admin", Role::Admin)?);
            expect_unique_search(user_repository);
            expect_successful_persistence(credential_repository, password_hasher, user_repository);
            Ok(())
        })?;
        let command = command(5, "dave", Role::Standard);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_standard_caller_register_user_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, _, _| {
            expect_caller(user_repository, user(3, "bobby", Role::Standard)?);
            Ok(())
        })?;
        let command = command(3, "evelyn", Role::Standard);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::Forbidden)));
        assert!(!harness.committed.load(Ordering::SeqCst));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_admin_caller_register_admin_user_forbidden() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let harness = use_case_with(|user_repository, _, _| {
            expect_caller(user_repository, user(5, "admin", Role::Admin)?);
            Ok(())
        })?;
        let command = command(5, "frank", Role::Admin);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::Forbidden)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_unknown_caller_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, _, _| {
            expect_unique_search(user_repository);
            Ok(())
        })?;
        let command = command(999, "grace", Role::Standard);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::Forbidden)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_username_too_short_returns_invalid_username()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|_, _, _| Ok(()))?;
        let command = RegisterUserCommand::new(
            0,
            "ab".to_owned(),
            None,
            SECRET_PASSWORD.to_owned(),
            Role::Standard,
        );

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::InvalidUsername)));
        assert!(!harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_invalid_email_returns_invalid_email() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|_, _, _| Ok(()))?;
        let command = RegisterUserCommand::new(
            0,
            "heidi".to_owned(),
            Some("not-an-email".to_owned()),
            SECRET_PASSWORD.to_owned(),
            Role::Standard,
        );

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::InvalidEmail)));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_short_password_returns_invalid_password() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|_, _, _| Ok(()))?;
        let command = RegisterUserCommand::new(
            0,
            "ivan".to_owned(),
            None,
            "secret".to_owned(),
            Role::Standard,
        );

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::InvalidPassword)));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_password_without_symbol_returns_invalid_password()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|_, _, _| Ok(()))?;
        let command = RegisterUserCommand::new(
            0,
            "judy".to_owned(),
            None,
            "password123".to_owned(),
            Role::Standard,
        );

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::InvalidPassword)));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_existing_username_returns_user_already_exists()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, _, _| {
            expect_caller(user_repository, user(0, "root", Role::Admin)?);
            user_repository.expect_search().times(1).returning(|_| {
                let users = existing_users(1, "mallory", Role::Standard);
                Box::pin(async move { users })
            });
            Ok(())
        })?;
        let command = command(0, "mallory", Role::Standard);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::UserAlreadyExists)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_existing_email_returns_user_already_exists() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let harness = use_case_with(|user_repository, _, _| {
            expect_caller(user_repository, user(0, "root", Role::Admin)?);
            expect_unique_search(user_repository);
            user_repository.expect_search().times(1).returning(|_| {
                let users = existing_users(1, "nancy", Role::Standard);
                Box::pin(async move { users })
            });
            Ok(())
        })?;
        let command = RegisterUserCommand::new(
            0,
            "oscar".to_owned(),
            Some("nancy@example.com".to_owned()),
            SECRET_PASSWORD.to_owned(),
            Role::Standard,
        );

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::UserAlreadyExists)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_search_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, _, _| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;
        let command = command(0, "patrick", Role::Standard);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_password_hash_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, _, password_hasher| {
            expect_caller(user_repository, user(0, "root", Role::Admin)?);
            expect_unique_search(user_repository);
            password_hasher
                .expect_hash_password()
                .times(1)
                .returning(|_| Box::pin(async { Err(PasswordHasherError::OperationFailed) }));
            Ok(())
        })?;
        let command = command(0, "quinn", Role::Standard);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_save_credential_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, credential_repository, password_hasher| {
            expect_caller(user_repository, user(0, "root", Role::Admin)?);
            expect_unique_search(user_repository);
            expect_hashed_password(password_hasher);
            credential_repository
                .expect_create()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;
        let command = command(0, "rupert", Role::Standard);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_save_user_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, credential_repository, password_hasher| {
            expect_caller(user_repository, user(0, "root", Role::Admin)?);
            expect_unique_search(user_repository);
            expect_hashed_password(password_hasher);
            credential_repository
                .expect_create()
                .times(1)
                .returning(|credential| Box::pin(async { Ok(credential) }));
            user_repository
                .expect_create()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;
        let command = command(0, "trent", Role::Standard);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
