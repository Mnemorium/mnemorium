use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::login_user::LoginUserCommand;
use crate::application::port::login_user::LoginUserError;
use crate::application::port::login_user::LoginUserResponse;
use crate::application::port::login_user::LoginUserUseCase;
use crate::domain::port::credential_repository::CredentialRepository;
use crate::domain::port::password_hasher::PasswordHasher;
use crate::domain::port::token_provider::TokenProvider;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository;

/// Use case implementation for authenticating a user.
pub struct LoginUser<U, C, H, P> {
    /// Repository persisting credentials.
    credential_repository: Arc<C>,
    /// Hasher for user passwords.
    password_hasher: Arc<H>,
    /// Provider issuing and validating tokens.
    token_provider: Arc<P>,
    /// Repository persisting users.
    user_repository: Arc<U>,
}

impl<U: UserRepository, C: CredentialRepository, H: PasswordHasher, P: TokenProvider>
    LoginUser<U, C, H, P>
{
    /// Create a new use case.
    #[must_use]
    pub fn new(
        user_repository: Arc<U>,
        credential_repository: Arc<C>,
        password_hasher: Arc<H>,
        token_provider: Arc<P>,
    ) -> Self {
        Self {
            credential_repository,
            password_hasher,
            token_provider,
            user_repository,
        }
    }
}

impl<U: UserRepository, C: CredentialRepository, H: PasswordHasher, P: TokenProvider>
    LoginUserUseCase for LoginUser<U, C, H, P>
{
    fn execute<'future>(
        &'future self,
        command: LoginUserCommand,
    ) -> Pin<Box<dyn Future<Output = Result<LoginUserResponse, LoginUserError>> + Send + 'future>>
    {
        let credential_repository = Arc::clone(&self.credential_repository);
        let password_hasher = Arc::clone(&self.password_hasher);
        let token_provider = Arc::clone(&self.token_provider);
        let user_repository = Arc::clone(&self.user_repository);

        Box::pin(async move {
            let username = command.username().to_owned();
            let password = command.password().to_owned();

            let user = user_repository
                .search(&UserFilter {
                    username: Some(username),
                    ..UserFilter::default()
                })
                .await
                .map_err(|error| LoginUserError::Unknown(error.into()))?
                .into_iter()
                .next()
                .ok_or(LoginUserError::InvalidUsername)?;

            let credential = credential_repository
                .find(user.credential_id())
                .await
                .map_err(|error| LoginUserError::Unknown(error.into()))?
                .ok_or_else(|| {
                    LoginUserError::Unknown(anyhow::anyhow!("user {} has no credential", user.id()))
                })?;

            let verified = password_hasher
                .verify_password(&password, credential.password_hash())
                .await
                .map_err(|error| LoginUserError::Unknown(error.into()))?;
            if !verified {
                return Err(LoginUserError::InvalidPassword);
            }

            let token = token_provider
                .issue(user.id())
                .await
                .map_err(|error| LoginUserError::Unknown(error.into()))?;

            Ok(LoginUserResponse::new(
                token.value().to_owned(),
                token.expires_in(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use chrono::NaiveDateTime;

    use crate::application::port::login_user::LoginUserCommand;
    use crate::application::port::login_user::LoginUserError;
    use crate::application::port::login_user::LoginUserUseCase as _;
    use crate::domain::model::credential::Credential;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::model::user::UserError;
    use crate::domain::port::credential_repository::MockCredentialRepository;
    use crate::domain::port::error::PasswordHasherError;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::error::TokenProviderError;
    use crate::domain::port::password_hasher::MockPasswordHasher;
    use crate::domain::port::token_provider::IssuedToken;
    use crate::domain::port::token_provider::MockTokenProvider;
    use crate::domain::port::user_repository::MockUserRepository;

    use super::LoginUser;

    type UseCase = LoginUser<
        MockUserRepository,
        MockCredentialRepository,
        MockPasswordHasher,
        MockTokenProvider,
    >;

    fn use_case_with(
        setup: impl FnOnce(
            &mut MockUserRepository,
            &mut MockCredentialRepository,
            &mut MockPasswordHasher,
            &mut MockTokenProvider,
        ) -> Result<(), Box<dyn Error>>,
    ) -> Result<UseCase, Box<dyn Error>> {
        let mut user_repository = MockUserRepository::new();
        let mut credential_repository = MockCredentialRepository::new();
        let mut password_hasher = MockPasswordHasher::new();
        let mut token_provider = MockTokenProvider::new();

        setup(
            &mut user_repository,
            &mut credential_repository,
            &mut password_hasher,
            &mut token_provider,
        )?;

        Ok(LoginUser::new(
            Arc::new(user_repository),
            Arc::new(credential_repository),
            Arc::new(password_hasher),
            Arc::new(token_provider),
        ))
    }

    fn user(id: i64, username: &str) -> Result<User, UserError> {
        User::try_new(id, username.to_owned(), None, id, Role::Standard)
    }

    fn credential(id: i64) -> Result<Credential, Box<dyn Error>> {
        let updated_at = NaiveDateTime::parse_from_str("2026-01-01 12:00:00", "%F %T")?;
        Ok(Credential::try_new(id, "hash".to_owned(), updated_at)?)
    }

    fn command(username: &str, password: &str) -> LoginUserCommand {
        LoginUserCommand::new(username.to_owned(), password.to_owned())
    }

    fn expect_user(user_repository: &mut MockUserRepository, user: User) {
        user_repository
            .expect_search()
            .times(1)
            .returning(move |_| {
                let own_user = user.clone();
                Box::pin(async move { Ok(vec![own_user]) })
            });
    }

    fn expect_credential(
        credential_repository: &mut MockCredentialRepository,
        credential: Credential,
    ) {
        credential_repository
            .expect_find()
            .times(1)
            .returning(move |_| {
                let own_credential = credential.clone();
                Box::pin(async move { Ok(Some(own_credential)) })
            });
    }

    fn expect_valid_password(password_hasher: &mut MockPasswordHasher) {
        password_hasher
            .expect_verify_password()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(true) }));
    }

    #[tokio::test]
    async fn login_user_valid_credentials_returns_issued_token() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(
            |user_repository, credential_repository, password_hasher, token_provider| {
                expect_user(user_repository, user(1, "alice")?);
                expect_credential(credential_repository, credential(1)?);
                expect_valid_password(password_hasher);
                token_provider.expect_issue().times(1).returning(|_| {
                    Box::pin(async { Ok(IssuedToken::new("token".to_owned(), 3600)) })
                });
                Ok(())
            },
        )?;
        let command = command("alice", "super-secret");

        // Act
        let response = use_case.execute(command).await?;

        // Assert
        assert_eq!(response.access_token(), "token");
        assert_eq!(response.expires_in(), 3600);
        Ok(())
    }

    #[tokio::test]
    async fn login_user_unknown_username_returns_invalid_username() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository, _, _, _| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            Ok(())
        })?;
        let command = command("ghost", "super-secret");

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(LoginUserError::InvalidUsername)));
        Ok(())
    }

    #[tokio::test]
    async fn login_user_search_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository, _, _, _| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;
        let command = command("alice", "super-secret");

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(LoginUserError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    async fn login_user_missing_credential_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository, credential_repository, _, _| {
            expect_user(user_repository, user(1, "alice")?);
            credential_repository
                .expect_find()
                .times(1)
                .returning(|_| Box::pin(async { Ok(None) }));
            Ok(())
        })?;
        let command = command("alice", "super-secret");

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(LoginUserError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    async fn login_user_wrong_password_returns_invalid_password() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(
            |user_repository, credential_repository, password_hasher, _| {
                expect_user(user_repository, user(1, "alice")?);
                expect_credential(credential_repository, credential(1)?);
                password_hasher
                    .expect_verify_password()
                    .times(1)
                    .returning(|_, _| Box::pin(async { Ok(false) }));
                Ok(())
            },
        )?;
        let command = command("alice", "wrong-password");

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(LoginUserError::InvalidPassword)));
        Ok(())
    }

    #[tokio::test]
    async fn login_user_password_verification_failure_returns_unknown() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = use_case_with(
            |user_repository, credential_repository, password_hasher, _| {
                expect_user(user_repository, user(1, "alice")?);
                expect_credential(credential_repository, credential(1)?);
                password_hasher
                    .expect_verify_password()
                    .times(1)
                    .returning(|_, _| {
                        Box::pin(async { Err(PasswordHasherError::OperationFailed) })
                    });
                Ok(())
            },
        )?;
        let command = command("alice", "super-secret");

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(LoginUserError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    async fn login_user_token_issuance_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(
            |user_repository, credential_repository, password_hasher, token_provider| {
                expect_user(user_repository, user(1, "alice")?);
                expect_credential(credential_repository, credential(1)?);
                expect_valid_password(password_hasher);
                token_provider
                    .expect_issue()
                    .times(1)
                    .returning(|_| Box::pin(async { Err(TokenProviderError::OperationFailed) }));
                Ok(())
            },
        )?;
        let command = command("alice", "super-secret");

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(LoginUserError::Unknown(_))));
        Ok(())
    }
}
