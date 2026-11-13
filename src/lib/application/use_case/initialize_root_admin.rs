use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use tracing::info;

use crate::application::port::{
    InitializeRootAdminCommand, InitializeRootAdminError, InitializeRootAdminResponse,
    InitializeRootAdminUseCase,
};
use crate::domain::model::{Credential, ResourceKind, ResourceRef, Role, User, Visibility};
use crate::domain::port::{
    CredentialRepository as _, IdentityUnitOfWork, NewResource, PasswordGenerator, PasswordHasher,
    ResourceRepository as _, UnitOfWork as _, UnitOfWorkFactory, UserFilter, UserRepository as _,
    UserUnitOfWork,
};

/// Username granted to the root admin.
const ROOT_ADMIN_USERNAME: &str = "root";

/// Use case implementation for initializing the root admin.
pub(crate) struct InitializeRootAdmin<F, H, G> {
    /// Generator producing random passwords.
    password_generator: Arc<G>,
    /// Hasher for user passwords.
    password_hasher: Arc<H>,
    /// Factory opening the unit of work wrapping the initialization.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, H: PasswordHasher, G: PasswordGenerator> InitializeRootAdmin<F, H, G> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(
        unit_of_work_factory: Arc<F>,
        password_hasher: Arc<H>,
        password_generator: Arc<G>,
    ) -> Self {
        Self {
            password_generator,
            password_hasher,
            unit_of_work_factory,
        }
    }
}

impl<F, H, G> InitializeRootAdminUseCase for InitializeRootAdmin<F, H, G>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + UserUnitOfWork,
    H: PasswordHasher,
    G: PasswordGenerator,
{
    fn execute<'future>(
        &'future self,
        command: InitializeRootAdminCommand,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<Option<InitializeRootAdminResponse>, InitializeRootAdminError>,
                > + Send
                + 'future,
        >,
    > {
        let password_generator = Arc::clone(&self.password_generator);
        let password_hasher = Arc::clone(&self.password_hasher);
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| InitializeRootAdminError::Unknown(error.into()))?;

            let result = async {
                let root_admin_exists = !unit_of_work
                    .users()
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
                if !command.reveal_password() {
                    return Err(InitializeRootAdminError::RevealDisabled);
                }

                let default_password = password_generator
                    .generate()
                    .await
                    .map_err(|error| InitializeRootAdminError::Unknown(error.into()))?;
                let password_hash = password_hasher
                    .hash_password(&default_password)
                    .await
                    .map_err(|error| InitializeRootAdminError::Unknown(error.into()))?;

                let pending_credential =
                    Credential::try_new(0, password_hash, Utc::now().naive_utc())
                        .map_err(|error| InitializeRootAdminError::Unknown(error.into()))?;
                let credential = unit_of_work
                    .credentials()
                    .create(pending_credential)
                    .await
                    .map_err(|error| InitializeRootAdminError::Unknown(error.into()))?;

                let pending_user = User::try_new(
                    0,
                    ROOT_ADMIN_USERNAME.to_owned(),
                    None,
                    credential.id(),
                    Role::Admin,
                )
                .map_err(|error| InitializeRootAdminError::Unknown(error.into()))?;
                unit_of_work
                    .users()
                    .save(pending_user)
                    .await
                    .map_err(|error| InitializeRootAdminError::Unknown(error.into()))?;
                // The root is a resource like every other account, so a caller
                // reads it through the decision engine (`REQ-AUTHORIZATION-021`).
                unit_of_work
                    .resources()
                    .create(NewResource {
                        auth_parent: Some(ResourceRef::new(ResourceKind::Instance, 0)),
                        entity_id: 0,
                        kind: ResourceKind::User,
                        owner_id: None,
                        visibility: Visibility::Private,
                    })
                    .await
                    .map_err(|error| InitializeRootAdminError::Unknown(error.into()))?;

                Ok(Some(InitializeRootAdminResponse::new(default_password)))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| InitializeRootAdminError::Unknown(error.into()))?;
                    if value.is_some() {
                        info!(
                            target: "security",
                            event = "system_object",
                            object = %"root_admin",
                            action = %"create",
                            "created a system-level object"
                        );
                    }
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
    use super::*;
    use crate::domain::port::{
        MockCredentialRepository, MockPasswordGenerator, MockPasswordHasher, MockUserRepository,
        PasswordGeneratorError, PasswordHasherError, RepositoryError,
    };

    use crate::test_helpers::*;
    use allure_cargotest::allure_test;
    use std::error::Error;
    use std::sync::atomic::{AtomicBool, Ordering};

    type UseCase = InitializeRootAdmin<TestFactory, MockPasswordHasher, MockPasswordGenerator>;

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
            &mut MockPasswordGenerator,
        ) -> Result<(), Box<dyn Error>>,
    ) -> Result<Harness, Box<dyn Error>> {
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

        let unit_of_work = TestUnitOfWork {
            users: user_repository,
            credentials: credential_repository,
            ..TestUnitOfWork::default()
        };
        let committed = Arc::clone(&unit_of_work.committed);
        let rolled_back = Arc::clone(&unit_of_work.rolled_back);
        let factory = TestFactory::new(unit_of_work);

        Ok(Harness {
            use_case: InitializeRootAdmin::new(
                Arc::new(factory),
                Arc::new(password_hasher),
                Arc::new(password_generator),
            ),
            committed,
            rolled_back,
        })
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
        let harness = use_case_with(
            |user_repository, credential_repository, password_hasher, password_generator| {
                expect_no_root_admin(user_repository);
                expect_generated_password(password_generator);
                password_hasher
                    .expect_hash_password()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok("hashed-password".to_owned()) }));
                credential_repository
                    .expect_create()
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
        let result = harness
            .use_case
            .execute(InitializeRootAdminCommand::new(true))
            .await?;

        // Assert
        let response = result.ok_or_else(|| anyhow::anyhow!("expected a response"))?;
        assert_eq!(response.default_password(), "generated-password!");
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn initialize_root_admin_existing_returns_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, _, _, _| {
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
        let response = harness
            .use_case
            .execute(InitializeRootAdminCommand::new(true))
            .await?;

        // Assert
        assert!(response.is_none());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn initialize_root_admin_search_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, _, _, _| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;

        // Act
        let result = harness
            .use_case
            .execute(InitializeRootAdminCommand::new(true))
            .await;

        // Assert
        assert!(matches!(result, Err(InitializeRootAdminError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn initialize_root_admin_generation_failure_returns_unknown() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let harness = use_case_with(|user_repository, _, _, password_generator| {
            expect_no_root_admin(user_repository);
            password_generator
                .expect_generate()
                .times(1)
                .returning(|| Box::pin(async { Err(PasswordGeneratorError::OperationFailed) }));
            Ok(())
        })?;

        // Act
        let result = harness
            .use_case
            .execute(InitializeRootAdminCommand::new(true))
            .await;

        // Assert
        assert!(matches!(result, Err(InitializeRootAdminError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn initialize_root_admin_hash_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, _, password_hasher, password_generator| {
            expect_no_root_admin(user_repository);
            expect_generated_password(password_generator);
            password_hasher
                .expect_hash_password()
                .times(1)
                .returning(|_| Box::pin(async { Err(PasswordHasherError::OperationFailed) }));
            Ok(())
        })?;

        // Act
        let result = harness
            .use_case
            .execute(InitializeRootAdminCommand::new(true))
            .await;

        // Assert
        assert!(matches!(result, Err(InitializeRootAdminError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn initialize_root_admin_save_credential_failure_returns_unknown()
    -> Result<(), Box<dyn Error>> {
        allure.epic("Authentication");
        allure.feature("REQ-AUTHENTICATION-001");
        allure.story("SCN-AUTHENTICATION-001-C");

        // Arrange
        let harness = use_case_with(
            |user_repository, credential_repository, password_hasher, password_generator| {
                expect_no_root_admin(user_repository);
                expect_generated_password(password_generator);
                password_hasher
                    .expect_hash_password()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok("hashed-password".to_owned()) }));
                credential_repository
                    .expect_create()
                    .times(1)
                    .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
                Ok(())
            },
        )?;

        // Act
        let result = harness
            .use_case
            .execute(InitializeRootAdminCommand::new(true))
            .await;

        // Assert
        assert!(matches!(result, Err(InitializeRootAdminError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn initialize_root_admin_save_user_failure_returns_unknown() -> Result<(), Box<dyn Error>>
    {
        allure.epic("Authentication");
        allure.feature("REQ-AUTHENTICATION-001");
        allure.story("SCN-AUTHENTICATION-001-D");

        // Arrange
        let harness = use_case_with(
            |user_repository, credential_repository, password_hasher, password_generator| {
                expect_no_root_admin(user_repository);
                expect_generated_password(password_generator);
                password_hasher
                    .expect_hash_password()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok("hashed-password".to_owned()) }));
                credential_repository
                    .expect_create()
                    .times(1)
                    .returning(|credential| Box::pin(async { Ok(credential) }));
                user_repository
                    .expect_save()
                    .times(1)
                    .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
                Ok(())
            },
        )?;

        // Act
        let result = harness
            .use_case
            .execute(InitializeRootAdminCommand::new(true))
            .await;

        // Assert
        assert!(matches!(result, Err(InitializeRootAdminError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
