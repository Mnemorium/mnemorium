use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use crate::application::port::initialize_root_admin::InitializeRootAdminError;
use crate::application::port::initialize_root_admin::InitializeRootAdminResponse;
use crate::application::port::initialize_root_admin::InitializeRootAdminUseCase;
use crate::domain::model::credential::Credential;
use crate::domain::model::user::Role;
use crate::domain::model::user::User;
use crate::domain::port::credential_repository::CredentialRepository;
use crate::domain::port::password_generator::PasswordGenerator;
use crate::domain::port::password_hasher::PasswordHasher;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository;

/// Username granted to the root admin.
const ROOT_ADMIN_USERNAME: &str = "root";

/// Use case implementation for initializing the root admin.
pub struct InitializeRootAdmin<R, C, H, G> {
    /// Repository persisting credentials.
    credential_repository: Arc<C>,
    /// Generator producing random passwords.
    password_generator: Arc<G>,
    /// Hasher for user passwords.
    password_hasher: Arc<H>,
    /// Repository persisting users.
    user_repository: Arc<R>,
}

impl<R: UserRepository, C: CredentialRepository, H: PasswordHasher, G: PasswordGenerator>
    InitializeRootAdmin<R, C, H, G>
{
    /// Create a new use case.
    #[must_use]
    pub fn new(
        user_repository: Arc<R>,
        credential_repository: Arc<C>,
        password_hasher: Arc<H>,
        password_generator: Arc<G>,
    ) -> Self {
        Self {
            credential_repository,
            password_generator,
            password_hasher,
            user_repository,
        }
    }
}

impl<R: UserRepository, C: CredentialRepository, H: PasswordHasher, G: PasswordGenerator>
    InitializeRootAdminUseCase for InitializeRootAdmin<R, C, H, G>
{
    fn execute<'future>(
        &'future self,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<Option<InitializeRootAdminResponse>, InitializeRootAdminError>,
                > + Send
                + 'future,
        >,
    > {
        let credential_repository = Arc::clone(&self.credential_repository);
        let password_generator = Arc::clone(&self.password_generator);
        let password_hasher = Arc::clone(&self.password_hasher);
        let user_repository = Arc::clone(&self.user_repository);

        Box::pin(async move {
            let root_admin_exists = !user_repository
                .search(&UserFilter {
                    id: Some(0),
                    ..UserFilter::default()
                })
                .await
                .map_err(|error| InitializeRootAdminError::Unknown(error.into()))?
                .is_empty();
            if root_admin_exists {
                return Ok(None);
            }

            let default_password = password_generator
                .generate()
                .await
                .map_err(|error| InitializeRootAdminError::Unknown(error.into()))?;
            let password_hash = password_hasher
                .hash_password(&default_password)
                .await
                .map_err(|error| InitializeRootAdminError::Unknown(error.into()))?;

            let pending_credential = Credential::try_new(0, password_hash, Utc::now().naive_utc())
                .map_err(|error| InitializeRootAdminError::Unknown(error.into()))?;
            let credential = credential_repository
                .save(pending_credential)
                .await
                .map_err(|error| InitializeRootAdminError::Unknown(error.into()))?;

            let pending_user = match User::try_new(
                0,
                ROOT_ADMIN_USERNAME.to_owned(),
                None,
                credential.id(),
                Role::Admin,
            ) {
                Ok(user) => user,
                Err(error) => {
                    drop(credential_repository.delete(credential.id()).await);
                    return Err(InitializeRootAdminError::Unknown(error.into()));
                }
            };

            if let Err(error) = user_repository.save(pending_user).await {
                drop(credential_repository.delete(credential.id()).await);
                return Err(InitializeRootAdminError::Unknown(error.into()));
            }

            Ok(Some(InitializeRootAdminResponse::new(default_password)))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use crate::application::port::initialize_root_admin::InitializeRootAdminError;
    use crate::application::port::initialize_root_admin::InitializeRootAdminUseCase as _;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::port::credential_repository::MockCredentialRepository;
    use crate::domain::port::error::PasswordGeneratorError;
    use crate::domain::port::error::PasswordHasherError;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::password_generator::MockPasswordGenerator;
    use crate::domain::port::password_hasher::MockPasswordHasher;
    use crate::domain::port::user_repository::MockUserRepository;

    use super::InitializeRootAdmin;

    type UseCase = InitializeRootAdmin<
        MockUserRepository,
        MockCredentialRepository,
        MockPasswordHasher,
        MockPasswordGenerator,
    >;

    /// Build a use case whose outbound dependencies are mocked; `setup` defines
    /// the mock expectations before the mocks are handed over.
    fn use_case_with(
        setup: impl FnOnce(
            &mut MockUserRepository,
            &mut MockCredentialRepository,
            &mut MockPasswordHasher,
            &mut MockPasswordGenerator,
        ) -> Result<(), Box<dyn Error>>,
    ) -> Result<UseCase, Box<dyn Error>> {
        let mut user_repository = MockUserRepository::new();
        let mut credential_repository = MockCredentialRepository::new();
        let mut password_hasher = MockPasswordHasher::new();
        let mut password_generator = MockPasswordGenerator::new();

        setup(
            &mut user_repository,
            &mut credential_repository,
            &mut password_hasher,
            &mut password_generator,
        )?;

        Ok(InitializeRootAdmin::new(
            Arc::new(user_repository),
            Arc::new(credential_repository),
            Arc::new(password_hasher),
            Arc::new(password_generator),
        ))
    }

    fn expect_no_root_admin(user_repository: &mut MockUserRepository) {
        user_repository
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
    }

    fn expect_generated_password(password_generator: &mut MockPasswordGenerator) {
        password_generator
            .expect_generate()
            .times(1)
            .returning(|| Box::pin(async { Ok("generated-password!".to_owned()) }));
    }

    #[tokio::test]
    async fn initialize_root_admin_missing_creates_and_returns_password()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(
            |user_repository, credential_repository, password_hasher, password_generator| {
                expect_no_root_admin(user_repository);
                expect_generated_password(password_generator);
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
                Ok(())
            },
        )?;

        // Act
        let result = use_case.execute().await?;

        // Assert
        let response = result.ok_or_else(|| anyhow::anyhow!("expected a response"))?;
        assert_eq!(response.default_password(), "generated-password!");
        Ok(())
    }

    #[tokio::test]
    async fn initialize_root_admin_existing_returns_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository, _, _, _| {
            let root = User::try_new(0, "root".to_owned(), None, 7, Role::Admin)?;
            user_repository
                .expect_search()
                .times(1)
                .returning(move |_| {
                    let own_root = root.clone();
                    Box::pin(async move { Ok(vec![own_root]) })
                });
            Ok(())
        })?;

        // Act
        let response = use_case.execute().await?;

        // Assert
        assert!(response.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn initialize_root_admin_search_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository, _, _, _| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;

        // Act
        let result = use_case.execute().await;

        // Assert
        assert!(matches!(result, Err(InitializeRootAdminError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    async fn initialize_root_admin_generation_failure_returns_unknown() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = use_case_with(|user_repository, _, _, password_generator| {
            expect_no_root_admin(user_repository);
            password_generator
                .expect_generate()
                .times(1)
                .returning(|| Box::pin(async { Err(PasswordGeneratorError::OperationFailed) }));
            Ok(())
        })?;

        // Act
        let result = use_case.execute().await;

        // Assert
        assert!(matches!(result, Err(InitializeRootAdminError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    async fn initialize_root_admin_hash_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository, _, password_hasher, password_generator| {
            expect_no_root_admin(user_repository);
            expect_generated_password(password_generator);
            password_hasher
                .expect_hash_password()
                .times(1)
                .returning(|_| Box::pin(async { Err(PasswordHasherError::OperationFailed) }));
            Ok(())
        })?;

        // Act
        let result = use_case.execute().await;

        // Assert
        assert!(matches!(result, Err(InitializeRootAdminError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    async fn initialize_root_admin_save_credential_failure_returns_unknown()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(
            |user_repository, credential_repository, password_hasher, password_generator| {
                expect_no_root_admin(user_repository);
                expect_generated_password(password_generator);
                password_hasher
                    .expect_hash_password()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok("hashed-password".to_owned()) }));
                credential_repository
                    .expect_save()
                    .times(1)
                    .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
                Ok(())
            },
        )?;

        // Act
        let result = use_case.execute().await;

        // Assert
        assert!(matches!(result, Err(InitializeRootAdminError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    async fn initialize_root_admin_save_user_failure_returns_unknown_and_deletes_credential()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(
            |user_repository, credential_repository, password_hasher, password_generator| {
                expect_no_root_admin(user_repository);
                expect_generated_password(password_generator);
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
            },
        )?;

        // Act
        let result = use_case.execute().await;

        // Assert
        assert!(matches!(result, Err(InitializeRootAdminError::Unknown(_))));
        Ok(())
    }
}
