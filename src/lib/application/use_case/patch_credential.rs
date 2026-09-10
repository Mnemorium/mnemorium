use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use crate::application::port::patch_credential::PatchCredentialCommand;
use crate::application::port::patch_credential::PatchCredentialError;
use crate::application::port::patch_credential::PatchCredentialUseCase;
use crate::domain::model::credential::Credential;
use crate::domain::model::user::Role;
use crate::domain::port::credential_repository::CredentialFilter;
use crate::domain::port::credential_repository::CredentialRepository;
use crate::domain::port::password_hasher::PasswordHasher;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository;
use crate::domain::service::password_policy::PasswordPolicy;
use crate::domain::service::password_policy::PasswordPolicyError;

/// Use case implementation for changing the password behind a credential.
pub struct PatchCredential<R, C, H> {
    /// Repository persisting credentials.
    credential_repository: Arc<C>,
    /// Hasher for user passwords.
    password_hasher: Arc<H>,
    /// Repository persisting users.
    user_repository: Arc<R>,
}

impl<R: UserRepository, C: CredentialRepository, H: PasswordHasher> PatchCredential<R, C, H> {
    /// Create a new use case.
    #[must_use]
    pub fn new(
        user_repository: Arc<R>,
        credential_repository: Arc<C>,
        password_hasher: Arc<H>,
    ) -> Self {
        Self {
            credential_repository,
            password_hasher,
            user_repository,
        }
    }
}

impl<R: UserRepository, C: CredentialRepository, H: PasswordHasher> PatchCredentialUseCase
    for PatchCredential<R, C, H>
{
    fn execute<'future>(
        &'future self,
        command: PatchCredentialCommand,
    ) -> Pin<Box<dyn Future<Output = Result<(), PatchCredentialError>> + Send + 'future>> {
        let credential_repository = Arc::clone(&self.credential_repository);
        let password_hasher = Arc::clone(&self.password_hasher);
        let user_repository = Arc::clone(&self.user_repository);
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

            let caller = user_repository
                .search(&UserFilter {
                    id: Some(command.caller_id()),
                    ..UserFilter::default()
                })
                .await
                .map_err(|error| PatchCredentialError::Unknown(error.into()))?
                .into_iter()
                .next()
                .ok_or(PatchCredentialError::Forbidden)?;

            if !(caller.role() == Role::Admin && caller.id() == 0) {
                return Err(PatchCredentialError::Forbidden);
            }

            let credential = credential_repository
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

            credential_repository
                .save(updated_credential)
                .await
                .map_err(|error| PatchCredentialError::Unknown(error.into()))?;

            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

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

    use super::PatchCredential;

    type UseCase =
        PatchCredential<MockUserRepository, MockCredentialRepository, MockPasswordHasher>;

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

        Ok(PatchCredential::new(
            Arc::new(user_repository),
            Arc::new(credential_repository),
            Arc::new(password_hasher),
        ))
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
        let use_case = use_case_with(|user_repository, credential_repository, password_hasher| {
            expect_caller(user_repository, user(0, "root", Role::Admin)?);
            expect_credential(credential_repository, credential(0)?);
            expect_hashed_password(password_hasher);
            expect_saved_credential(credential_repository);
            Ok(())
        })?;
        let command = command(0, 0, "super-secret!");

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(result.is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_root_admin_changes_other_user_credential_succeeds()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository, credential_repository, password_hasher| {
            expect_caller(user_repository, user(0, "root", Role::Admin)?);
            expect_credential(credential_repository, credential(4)?);
            expect_hashed_password(password_hasher);
            expect_saved_credential(credential_repository);
            Ok(())
        })?;
        let command = command(0, 4, "other-user!");

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(result.is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_standard_caller_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository, _, _| {
            expect_caller(user_repository, user(3, "bobby", Role::Standard)?);
            Ok(())
        })?;
        let command = command(3, 1, "secret-one!");

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(PatchCredentialError::Forbidden)));
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_non_root_admin_caller_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository, _, _| {
            expect_caller(user_repository, user(5, "admin", Role::Admin)?);
            Ok(())
        })?;
        let command = command(5, 1, "secret-two!");

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(PatchCredentialError::Forbidden)));
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_unknown_caller_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository, _, _| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            Ok(())
        })?;
        let command = command(999, 1, "secret-three!");

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(PatchCredentialError::Forbidden)));
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_short_password_returns_invalid_password() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = use_case_with(|_, _, _| Ok(()))?;
        let command = command(0, 0, "xY3!z9w");

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(PatchCredentialError::InvalidPassword)));
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_password_without_symbol_returns_invalid_password()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|_, _, _| Ok(()))?;
        let command = command(0, 0, "password123");

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(PatchCredentialError::InvalidPassword)));
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_unknown_credential_returns_unknown_credential()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository, credential_repository, _| {
            expect_caller(user_repository, user(0, "root", Role::Admin)?);
            credential_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            Ok(())
        })?;
        let command = command(0, 42, "secret-four!");

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(
            result,
            Err(PatchCredentialError::UnknownCredential)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_user_search_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository, _, _| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;
        let command = command(0, 1, "secret-five!");

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(PatchCredentialError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_credential_search_failure_returns_unknown()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository, credential_repository, _| {
            expect_caller(user_repository, user(0, "root", Role::Admin)?);
            credential_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;
        let command = command(0, 1, "secret-six!");

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(PatchCredentialError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_hash_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository, credential_repository, password_hasher| {
            expect_caller(user_repository, user(0, "root", Role::Admin)?);
            expect_credential(credential_repository, credential(1)?);
            password_hasher
                .expect_hash_password()
                .times(1)
                .returning(|_| Box::pin(async { Err(PasswordHasherError::OperationFailed) }));
            Ok(())
        })?;
        let command = command(0, 1, "secret-seven!");

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(PatchCredentialError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_credential_save_failure_returns_unknown() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = use_case_with(|user_repository, credential_repository, password_hasher| {
            expect_caller(user_repository, user(0, "root", Role::Admin)?);
            expect_credential(credential_repository, credential(1)?);
            expect_hashed_password(password_hasher);
            credential_repository
                .expect_save()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;
        let command = command(0, 1, "secret-eight!");

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(PatchCredentialError::Unknown(_))));
        Ok(())
    }
}
