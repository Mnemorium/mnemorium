use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use crate::application::port::register_user::RegisterUserCommand;
use crate::application::port::register_user::RegisterUserError;
use crate::application::port::register_user::RegisterUserResponse;
use crate::application::port::register_user::RegisterUserUseCase;
use crate::domain::model::credential::Credential;
use crate::domain::model::user::Role;
use crate::domain::model::user::User;
use crate::domain::model::user::UserError;
use crate::domain::port::credential_repository::CredentialRepository;
use crate::domain::port::password_hasher::PasswordHasher;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository;
use crate::domain::service::password_policy::PasswordPolicy;
use crate::domain::service::password_policy::PasswordPolicyError;

/// Use case implementation for registering a new user.
pub struct RegisterUser<R, C, P> {
    /// Repository persisting credentials.
    credential_repository: Arc<C>,
    /// Hasher for user passwords.
    password_hasher: Arc<P>,
    /// Repository persisting users.
    user_repository: Arc<R>,
}

impl<R: UserRepository, C: CredentialRepository, P: PasswordHasher> RegisterUser<R, C, P> {
    /// Create a new use case.
    #[must_use]
    pub fn new(
        user_repository: Arc<R>,
        credential_repository: Arc<C>,
        password_hasher: Arc<P>,
    ) -> Self {
        Self {
            credential_repository,
            password_hasher,
            user_repository,
        }
    }
}

impl<R: UserRepository, C: CredentialRepository, P: PasswordHasher> RegisterUserUseCase
    for RegisterUser<R, C, P>
{
    fn execute<'future>(
        &'future self,
        command: RegisterUserCommand,
    ) -> Pin<
        Box<dyn Future<Output = Result<RegisterUserResponse, RegisterUserError>> + Send + 'future>,
    > {
        let credential_repository = Arc::clone(&self.credential_repository);
        let password_hasher = Arc::clone(&self.password_hasher);
        let user_repository = Arc::clone(&self.user_repository);
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

            let caller = user_repository
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

            let username_taken = !user_repository
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
                let email_taken = !user_repository
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

            let password_hash = password_hasher
                .hash_password(command.password())
                .await
                .map_err(|error| RegisterUserError::Unknown(error.into()))?;
            let pending_credential = Credential::try_new(0, password_hash, Utc::now().naive_utc())
                .map_err(|error| RegisterUserError::Unknown(error.into()))?;
            let credential = credential_repository
                .save(pending_credential)
                .await
                .map_err(|error| RegisterUserError::Unknown(error.into()))?;

            let pending_user =
                match User::try_new(0, username, email, credential.id(), command.role()) {
                    Ok(user) => user,
                    Err(error) => {
                        drop(credential_repository.delete(credential.id()).await);
                        return Err(RegisterUserError::Unknown(error.into()));
                    }
                };

            let user = match user_repository.save(pending_user).await {
                Ok(user) => user,
                Err(error) => {
                    drop(credential_repository.delete(credential.id()).await);
                    return Err(RegisterUserError::Unknown(error.into()));
                }
            };

            Ok(RegisterUserResponse::new(
                user.id(),
                user.username().to_owned(),
                user.email().map(str::to_owned),
                user.role(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use crate::application::port::register_user::RegisterUserCommand;
    use crate::application::port::register_user::RegisterUserError;
    use crate::application::port::register_user::RegisterUserUseCase as _;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::model::user::UserError;
    use crate::domain::port::credential_repository::MockCredentialRepository;
    use crate::domain::port::error::PasswordHasherError;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::password_hasher::MockPasswordHasher;
    use crate::domain::port::user_repository::MockUserRepository;

    use super::RegisterUser;

    type UseCase = RegisterUser<MockUserRepository, MockCredentialRepository, MockPasswordHasher>;

    /// Build a use case whose outbound dependencies are mocked; `setup` defines
    /// the mock expectations before the mocks are handed over.
    fn use_case_with(
        setup: impl FnOnce(
            &mut MockUserRepository,
            &mut MockCredentialRepository,
            &mut MockPasswordHasher,
        ) -> Result<(), Box<dyn Error>>,
    ) -> Result<UseCase, Box<dyn Error>> {
        let mut user_repository = MockUserRepository::new();
        let mut credential_repository = MockCredentialRepository::new();
        let mut password_hasher = MockPasswordHasher::new();

        setup(
            &mut user_repository,
            &mut credential_repository,
            &mut password_hasher,
        )?;

        Ok(RegisterUser::new(
            Arc::new(user_repository),
            Arc::new(credential_repository),
            Arc::new(password_hasher),
        ))
    }

    fn user(id: i64, username: &str, role: Role) -> Result<User, UserError> {
        User::try_new(id, username.to_owned(), None, 7, role)
    }

    fn command(caller_id: i64, username: &str, role: Role) -> RegisterUserCommand {
        RegisterUserCommand::new(
            caller_id,
            username.to_owned(),
            None,
            "super-secret!".to_owned(),
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

    fn existing_users(id: i64, username: &str, role: Role) -> Result<Vec<User>, RepositoryError> {
        user(id, username, role)
            .map(|user| vec![user])
            .map_err(|_| RepositoryError::OperationFailed)
    }

    fn expect_unique_username(user_repository: &mut MockUserRepository) {
        user_repository
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
    }

    fn expect_successful_persistence(
        credential_repository: &mut MockCredentialRepository,
        password_hasher: &mut MockPasswordHasher,
        user_repository: &mut MockUserRepository,
    ) {
        password_hasher
            .expect_hash_password()
            .times(1)
            .returning(|_| Box::pin(async { Ok("hashed-password".to_owned()) }));
        credential_repository
            .expect_save()
            .times(1)
            .returning(|credential| Box::pin(async { Ok(credential) }));
        user_repository
            .expect_save()
            .times(1)
            .returning(|user| Box::pin(async { Ok(user) }));
    }

    #[tokio::test]
    async fn register_user_root_admin_register_standard_user_succeeds() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = use_case_with(|user_repository, credential_repository, password_hasher| {
            expect_caller(user_repository, user(0, "root", Role::Admin)?);
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            expect_successful_persistence(credential_repository, password_hasher, user_repository);
            Ok(())
        })?;
        let command = RegisterUserCommand::new(
            0,
            "alice".to_owned(),
            Some("alice@example.com".to_owned()),
            "super-secret!".to_owned(),
            Role::Standard,
        );

        // Act
        let result = use_case.execute(command).await;

        // Assert
        let response = result?;
        assert_eq!(response.username(), "alice");
        assert_eq!(response.email(), Some("alice@example.com"));
        assert_eq!(response.role(), Role::Standard);
        Ok(())
    }

    #[tokio::test]
    async fn register_user_root_admin_register_admin_user_succeeds() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository, credential_repository, password_hasher| {
            expect_caller(user_repository, user(0, "root", Role::Admin)?);
            expect_unique_username(user_repository);
            expect_successful_persistence(credential_repository, password_hasher, user_repository);
            Ok(())
        })?;
        let command = command(0, "carol", Role::Admin);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(result.is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn register_user_admin_register_standard_user_succeeds() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository, credential_repository, password_hasher| {
            expect_caller(user_repository, user(5, "admin", Role::Admin)?);
            expect_unique_username(user_repository);
            expect_successful_persistence(credential_repository, password_hasher, user_repository);
            Ok(())
        })?;
        let command = command(5, "dave", Role::Standard);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(result.is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn register_user_standard_caller_register_user_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository, _, _| {
            expect_caller(user_repository, user(3, "bobby", Role::Standard)?);
            Ok(())
        })?;
        let command = command(3, "evelyn", Role::Standard);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::Forbidden)));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_admin_caller_register_admin_user_forbidden() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = use_case_with(|user_repository, _, _| {
            expect_caller(user_repository, user(5, "admin", Role::Admin)?);
            Ok(())
        })?;
        let command = command(5, "frank", Role::Admin);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::Forbidden)));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_unknown_caller_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository, _, _| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            Ok(())
        })?;
        let command = command(999, "grace", Role::Standard);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::Forbidden)));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_username_too_short_returns_invalid_username()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|_, _, _| Ok(()))?;
        let command = RegisterUserCommand::new(
            0,
            "ab".to_owned(),
            None,
            "super-secret!".to_owned(),
            Role::Standard,
        );

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::InvalidUsername)));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_invalid_email_returns_invalid_email() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|_, _, _| Ok(()))?;
        let command = RegisterUserCommand::new(
            0,
            "heidi".to_owned(),
            Some("not-an-email".to_owned()),
            "super-secret!".to_owned(),
            Role::Standard,
        );

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::InvalidEmail)));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_short_password_returns_invalid_password() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|_, _, _| Ok(()))?;
        let command = RegisterUserCommand::new(
            0,
            "ivan".to_owned(),
            None,
            "secret".to_owned(),
            Role::Standard,
        );

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::InvalidPassword)));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_password_without_symbol_returns_invalid_password()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|_, _, _| Ok(()))?;
        let command = RegisterUserCommand::new(
            0,
            "judy".to_owned(),
            None,
            "password123".to_owned(),
            Role::Standard,
        );

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::InvalidPassword)));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_existing_username_returns_user_already_exists()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository, _, _| {
            expect_caller(user_repository, user(0, "root", Role::Admin)?);
            user_repository.expect_search().times(1).returning(|_| {
                let users = existing_users(1, "mallory", Role::Standard);
                Box::pin(async move { users })
            });
            Ok(())
        })?;
        let command = command(0, "mallory", Role::Standard);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::UserAlreadyExists)));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_existing_email_returns_user_already_exists() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = use_case_with(|user_repository, _, _| {
            expect_caller(user_repository, user(0, "root", Role::Admin)?);
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
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
            "super-secret!".to_owned(),
            Role::Standard,
        );

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::UserAlreadyExists)));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_search_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository, _, _| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;
        let command = command(0, "patrick", Role::Standard);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_password_hash_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository, _, password_hasher| {
            expect_caller(user_repository, user(0, "root", Role::Admin)?);
            expect_unique_username(user_repository);
            password_hasher
                .expect_hash_password()
                .times(1)
                .returning(|_| Box::pin(async { Err(PasswordHasherError::OperationFailed) }));
            Ok(())
        })?;
        let command = command(0, "quinn", Role::Standard);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_save_credential_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository, credential_repository, password_hasher| {
            expect_caller(user_repository, user(0, "root", Role::Admin)?);
            expect_unique_username(user_repository);
            password_hasher
                .expect_hash_password()
                .times(1)
                .returning(|_| Box::pin(async { Ok("hashed-password".to_owned()) }));
            credential_repository
                .expect_save()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;
        let command = command(0, "rupert", Role::Standard);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_save_user_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository, credential_repository, password_hasher| {
            expect_caller(user_repository, user(0, "root", Role::Admin)?);
            expect_unique_username(user_repository);
            password_hasher
                .expect_hash_password()
                .times(1)
                .returning(|_| Box::pin(async { Ok("hashed-password".to_owned()) }));
            credential_repository
                .expect_save()
                .times(1)
                .returning(|credential| Box::pin(async { Ok(credential) }));
            credential_repository
                .expect_delete()
                .times(1)
                .returning(|_| Box::pin(async { Ok(true) }));
            user_repository
                .expect_save()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;
        let command = command(0, "trent", Role::Standard);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::Unknown(_))));
        Ok(())
    }
}
