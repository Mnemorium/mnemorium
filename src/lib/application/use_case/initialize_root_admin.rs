use std::fmt::Write as _;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::{Duration as ChronoDuration, Utc};

use tracing::info;

use crate::application::port::{
    InitializeRootAdminCommand, InitializeRootAdminError, InitializeRootAdminResponse,
    InitializeRootAdminUseCase,
};
use crate::domain::model::{
    Credential, ResourceKind, ResourceRef, Role, SetupGrant, User, Visibility,
};
use crate::domain::port::{
    CredentialRepository as _, IdentityUnitOfWork, NewResource, PasswordGenerator, PasswordHasher,
    ResourceRepository as _, SecretGenerator, SetupGrantRepository as _, UnitOfWork as _,
    UnitOfWorkFactory, UserFilter, UserRepository as _, UserUnitOfWork,
};
use crate::domain::service::SetupTokenDeriver;

/// Username granted to the root admin.
const ROOT_ADMIN_USERNAME: &str = "root";

/// Number of random bytes a setup-grant nonce carries.
const SETUP_GRANT_NONCE_BYTES: u32 = 32;

/// Use case implementation for initializing the root admin.
pub(crate) struct InitializeRootAdmin<F, H, G, S> {
    /// Generator producing random passwords.
    password_generator: Arc<G>,
    /// Hasher for user passwords.
    password_hasher: Arc<H>,
    /// Instance pepper the setup token is derived from.
    pepper: String,
    /// Generator producing the setup-grant nonce.
    secret_generator: Arc<S>,
    /// Factory opening the unit of work wrapping the initialization.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, H: PasswordHasher, G: PasswordGenerator, S: SecretGenerator>
    InitializeRootAdmin<F, H, G, S>
{
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(
        unit_of_work_factory: Arc<F>,
        password_hasher: Arc<H>,
        password_generator: Arc<G>,
        secret_generator: Arc<S>,
        pepper: String,
    ) -> Self {
        Self {
            password_generator,
            password_hasher,
            pepper,
            secret_generator,
            unit_of_work_factory,
        }
    }
}

impl<F, H, G, S> InitializeRootAdminUseCase for InitializeRootAdmin<F, H, G, S>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + UserUnitOfWork,
    H: PasswordHasher,
    G: PasswordGenerator,
    S: SecretGenerator,
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
        let pepper = self.pepper.clone();
        let secret_generator = Arc::clone(&self.secret_generator);

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
                let now = Utc::now().naive_utc();
                let Some(expires_at) = now.checked_add_signed(ChronoDuration::seconds(
                    i64::try_from(command.ttl_seconds()).unwrap_or_default(),
                )) else {
                    return Err(InitializeRootAdminError::Unknown(anyhow::anyhow!(
                        "the setup-token lifetime is not representable"
                    )));
                };

                if root_admin_exists {
                    let Some(grant) = unit_of_work
                        .setup_grants()
                        .find()
                        .await
                        .map_err(|error| InitializeRootAdminError::Unknown(error.into()))?
                    else {
                        // An absent grant means setup already completed (or the
                        // instance predates the grant): issue nothing.
                        return Ok((false, None));
                    };
                    if grant.consumed_at().is_some() {
                        return Ok((false, None));
                    }
                    if !command.reveal() {
                        return Err(InitializeRootAdminError::RevealDisabled);
                    }

                    let nonce = generate_nonce(secret_generator.as_ref()).await?;
                    let renewed = grant
                        .renew(nonce.clone(), expires_at, now)
                        .map_err(|error| InitializeRootAdminError::Unknown(error.into()))?;
                    unit_of_work
                        .setup_grants()
                        .save(renewed)
                        .await
                        .map_err(|error| InitializeRootAdminError::Unknown(error.into()))?;

                    let token =
                        SetupTokenDeriver::display(&SetupTokenDeriver::derive(&pepper, &nonce));
                    return Ok((false, Some(InitializeRootAdminResponse::new(token))));
                }

                if !command.reveal() {
                    return Err(InitializeRootAdminError::RevealDisabled);
                }

                // The generated password is the unusable secret whose hash the
                // credential holds until setup replaces it; it is discarded
                // before anything can reveal it.
                let discarded_password = password_generator
                    .generate()
                    .await
                    .map_err(|error| InitializeRootAdminError::Unknown(error.into()))?;
                let password_hash = password_hasher
                    .hash_password(&discarded_password)
                    .await
                    .map_err(|error| InitializeRootAdminError::Unknown(error.into()))?;

                let pending_credential = Credential::try_new(0, password_hash, now)
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
                // reads it through the decision engine (`REQ-USER-023`).
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

                let nonce = generate_nonce(secret_generator.as_ref()).await?;
                let grant = SetupGrant::try_new(nonce.clone(), expires_at, now)
                    .map_err(|error| InitializeRootAdminError::Unknown(error.into()))?;
                unit_of_work
                    .setup_grants()
                    .create(grant)
                    .await
                    .map_err(|error| InitializeRootAdminError::Unknown(error.into()))?;

                Ok((
                    true,
                    Some(InitializeRootAdminResponse::new(
                        SetupTokenDeriver::display(&SetupTokenDeriver::derive(&pepper, &nonce)),
                    )),
                ))
            }
            .await;

            match result {
                Ok((created, value)) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| InitializeRootAdminError::Unknown(error.into()))?;
                    if created {
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

/// Generate the nonce a fresh grant carries.
async fn generate_nonce<S: SecretGenerator>(
    generator: &S,
) -> Result<String, InitializeRootAdminError> {
    let bytes = generator
        .generate(SETUP_GRANT_NONCE_BYTES)
        .await
        .map_err(|error| InitializeRootAdminError::Unknown(error.into()))?;
    let mut nonce = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in &bytes {
        write!(&mut nonce, "{byte:02x}")
            .map_err(|error| InitializeRootAdminError::Unknown(error.into()))?;
    }
    Ok(nonce)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::port::{
        MockCredentialRepository, MockPasswordGenerator, MockPasswordHasher,
        MockSetupGrantRepository, MockUserRepository, PasswordGeneratorError, PasswordHasherError,
        RepositoryError, SecretGeneratorError,
    };

    use crate::test_helpers::*;
    use std::error::Error;
    use std::sync::atomic::{AtomicBool, Ordering};

    type UseCase =
        InitializeRootAdmin<TestFactory, MockPasswordHasher, MockPasswordGenerator, MockSecret>;

    /// Secret generator the use case tests drive without randomness.
    struct MockSecret;

    impl SecretGenerator for MockSecret {
        fn generate(
            &self,
            length: u32,
        ) -> impl Future<Output = Result<Vec<u8>, SecretGeneratorError>> + Send {
            std::future::ready(Ok(vec![0xab; usize::try_from(length).unwrap_or_default()]))
        }
    }

    /// A use case under test together with its transaction-lifecycle flags.
    struct Harness {
        /// Set when the unit of work is committed.
        committed: Arc<AtomicBool>,
        /// Set when the unit of work is rolled back.
        rolled_back: Arc<AtomicBool>,
        /// The use case under test.
        use_case: UseCase,
    }

    /// Build a use case whose outbound dependencies are mocked.
    fn use_case_with(
        setup: impl FnOnce(
            &mut MockUserRepository,
            &mut MockCredentialRepository,
            &mut MockSetupGrantRepository,
            &mut MockPasswordHasher,
            &mut MockPasswordGenerator,
        ) -> Result<(), Box<dyn Error>>,
    ) -> Result<Harness, Box<dyn Error>> {
        let mut user_repository = MockUserRepository::new();
        let mut credential_repository = MockCredentialRepository::new();
        let mut setup_grant_repository = MockSetupGrantRepository::new();
        let mut password_hasher = MockPasswordHasher::new();
        let mut password_generator = MockPasswordGenerator::new();

        setup(
            &mut user_repository,
            &mut credential_repository,
            &mut setup_grant_repository,
            &mut password_hasher,
            &mut password_generator,
        )?;

        let unit_of_work = TestUnitOfWork {
            users: user_repository,
            credentials: credential_repository,
            setup_grants: setup_grant_repository,
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
                Arc::new(MockSecret),
                TEST_PEPPER.to_owned(),
            ),
            committed,
            rolled_back,
        })
    }

    #[tokio::test]
    async fn initialize_root_admin_missing_creates_and_returns_a_setup_token()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(
            |user_repository,
             credential_repository,
             setup_grant_repository,
             password_hasher,
             password_generator| {
                (move |user_repository: &mut MockUserRepository| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                })(user_repository);
                password_generator
                    .expect_generate()
                    .times(1)
                    .returning(|| Box::pin(async { Ok("discarded-password!".to_owned()) }));
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
                setup_grant_repository
                    .expect_create()
                    .times(1)
                    .returning(|grant| Box::pin(async { Ok(grant) }));
                Ok(())
            },
        )?;

        // Act
        let result = harness
            .use_case
            .execute(InitializeRootAdminCommand::new(true, 1800))
            .await?;

        // Assert
        let response = result.ok_or_else(|| anyhow::anyhow!("expected a response"))?;
        assert_eq!(response.setup_token().chars().count(), 9);
        assert_eq!(response.setup_token().chars().nth(4), Some('-'));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn initialize_root_admin_existing_without_grant_returns_none()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, _, setup_grant_repository, _, _| {
            let root = User::try_new(0, "root".to_owned(), None, 7, Role::Admin)?;
            user_repository
                .expect_search()
                .times(1)
                .returning(move |_| {
                    let own_root = root.clone();
                    Box::pin(async move { Ok(vec![own_root]) })
                });
            setup_grant_repository
                .expect_find()
                .times(1)
                .returning(|| Box::pin(async { Ok(None) }));
            Ok(())
        })?;

        // Act
        let response = harness
            .use_case
            .execute(InitializeRootAdminCommand::new(true, 1800))
            .await?;

        // Assert
        assert!(response.is_none());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn initialize_root_admin_pending_grant_issues_a_new_token() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let existing = SetupGrant::try_new(
            "a".repeat(32),
            Utc::now().naive_utc() + ChronoDuration::seconds(60),
            Utc::now().naive_utc(),
        )?;
        let harness = use_case_with(move |user_repository, _, setup_grant_repository, _, _| {
            let root = User::try_new(0, "root".to_owned(), None, 7, Role::Admin)?;
            user_repository
                .expect_search()
                .times(1)
                .returning(move |_| {
                    let own_root = root.clone();
                    Box::pin(async move { Ok(vec![own_root]) })
                });
            let found = existing.clone();
            setup_grant_repository
                .expect_find()
                .times(1)
                .returning(move || {
                    let grant = found.clone();
                    Box::pin(async move { Ok(Some(grant)) })
                });
            setup_grant_repository
                .expect_save()
                .times(1)
                .returning(|grant| Box::pin(async { Ok(grant) }));
            Ok(())
        })?;

        // Act
        let response = harness
            .use_case
            .execute(InitializeRootAdminCommand::new(true, 1800))
            .await?
            .ok_or_else(|| anyhow::anyhow!("expected a reissued token"))?;

        // Assert
        assert_eq!(response.setup_token().chars().count(), 9);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn initialize_root_admin_pending_grant_without_the_reveal_is_refused()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let existing = SetupGrant::try_new(
            "a".repeat(32),
            Utc::now().naive_utc() + ChronoDuration::seconds(60),
            Utc::now().naive_utc(),
        )?;
        let harness = use_case_with(move |user_repository, _, setup_grant_repository, _, _| {
            let root = User::try_new(0, "root".to_owned(), None, 7, Role::Admin)?;
            user_repository
                .expect_search()
                .times(1)
                .returning(move |_| {
                    let own_root = root.clone();
                    Box::pin(async move { Ok(vec![own_root]) })
                });
            let found = existing.clone();
            setup_grant_repository
                .expect_find()
                .times(1)
                .returning(move || {
                    let grant = found.clone();
                    Box::pin(async move { Ok(Some(grant)) })
                });
            Ok(())
        })?;

        // Act
        let result = harness
            .use_case
            .execute(InitializeRootAdminCommand::new(false, 1800))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(InitializeRootAdminError::RevealDisabled)
        ));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn initialize_root_admin_missing_without_the_reveal_is_refused()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, _, _, _, _| {
            (move |user_repository: &mut MockUserRepository| {
                user_repository
                    .expect_search()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            })(user_repository);
            Ok(())
        })?;

        // Act
        let result = harness
            .use_case
            .execute(InitializeRootAdminCommand::new(false, 1800))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(InitializeRootAdminError::RevealDisabled)
        ));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn initialize_root_admin_search_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, _, _, _, _| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;

        // Act
        let result = harness
            .use_case
            .execute(InitializeRootAdminCommand::new(true, 1800))
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
        let harness = use_case_with(|user_repository, _, _, _, password_generator| {
            (move |user_repository: &mut MockUserRepository| {
                user_repository
                    .expect_search()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            })(user_repository);
            password_generator
                .expect_generate()
                .times(1)
                .returning(|| Box::pin(async { Err(PasswordGeneratorError::OperationFailed) }));
            Ok(())
        })?;

        // Act
        let result = harness
            .use_case
            .execute(InitializeRootAdminCommand::new(true, 1800))
            .await;

        // Assert
        assert!(matches!(result, Err(InitializeRootAdminError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn initialize_root_admin_hash_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(
            |user_repository, _, _, password_hasher, password_generator| {
                (move |user_repository: &mut MockUserRepository| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                })(user_repository);
                password_generator
                    .expect_generate()
                    .times(1)
                    .returning(|| Box::pin(async { Ok("discarded-password!".to_owned()) }));
                password_hasher
                    .expect_hash_password()
                    .times(1)
                    .returning(|_| Box::pin(async { Err(PasswordHasherError::OperationFailed) }));
                Ok(())
            },
        )?;

        // Act
        let result = harness
            .use_case
            .execute(InitializeRootAdminCommand::new(true, 1800))
            .await;

        // Assert
        assert!(matches!(result, Err(InitializeRootAdminError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    // proves: SCN-AUTHENTICATION-001-C — Recovery
    #[tokio::test]
    async fn initialize_root_admin_credential_failure_returns_unknown() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let harness = use_case_with(
            |user_repository, credential_repository, _, password_hasher, password_generator| {
                (move |user_repository: &mut MockUserRepository| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                })(user_repository);
                password_generator
                    .expect_generate()
                    .times(1)
                    .returning(|| Box::pin(async { Ok("discarded-password!".to_owned()) }));
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
            .execute(InitializeRootAdminCommand::new(true, 1800))
            .await;

        // Assert
        assert!(matches!(result, Err(InitializeRootAdminError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    // proves: SCN-AUTHENTICATION-001-D — Recovery
    #[tokio::test]
    async fn initialize_root_admin_user_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(
            |user_repository, credential_repository, _, password_hasher, password_generator| {
                (move |user_repository: &mut MockUserRepository| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                })(user_repository);
                password_generator
                    .expect_generate()
                    .times(1)
                    .returning(|| Box::pin(async { Ok("discarded-password!".to_owned()) }));
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
            .execute(InitializeRootAdminCommand::new(true, 1800))
            .await;

        // Assert
        assert!(matches!(result, Err(InitializeRootAdminError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    // proves: SCN-AUTHENTICATION-001-E — Recovery
    #[tokio::test]
    async fn initialize_root_admin_grant_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(
            |user_repository,
             credential_repository,
             setup_grant_repository,
             password_hasher,
             password_generator| {
                (move |user_repository: &mut MockUserRepository| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                })(user_repository);
                password_generator
                    .expect_generate()
                    .times(1)
                    .returning(|| Box::pin(async { Ok("discarded-password!".to_owned()) }));
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
                setup_grant_repository
                    .expect_create()
                    .times(1)
                    .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
                Ok(())
            },
        )?;

        // Act
        let result = harness
            .use_case
            .execute(InitializeRootAdminCommand::new(true, 1800))
            .await;

        // Assert
        assert!(matches!(result, Err(InitializeRootAdminError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn setup_grant_rejects_a_short_nonce() {
        // Act
        let result = SetupGrant::try_new(
            "short".to_owned(),
            Utc::now()
                .naive_utc()
                .checked_add_signed(ChronoDuration::seconds(60))
                .unwrap_or_default(),
            Utc::now().naive_utc(),
        );

        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn setup_grant_rejects_a_past_expiry() {
        // Arrange
        let now = Utc::now().naive_utc();
        let past = now
            .checked_sub_signed(ChronoDuration::seconds(1))
            .unwrap_or(now);

        // Act
        let result = SetupGrant::try_new("a".repeat(32), past, now);

        // Assert
        assert!(result.is_err());
    }
}
