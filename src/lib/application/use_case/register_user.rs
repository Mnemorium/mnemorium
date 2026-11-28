use crate::application::authorization::authorize;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use tracing::{error, info};

use crate::application::port::{
    RegisterUserCommand, RegisterUserError, RegisterUserResponse, RegisterUserUseCase,
};
use crate::domain::model::{
    Action, ChildKind, Credential, Grant, GrantEffect, Principal, ResourceKind, ResourceRef,
    ResourceRole, Role, User, Visibility,
};
use crate::domain::port::{
    CredentialRepository as _, GrantRepository as _, IdentityUnitOfWork, NewResource,
    PasswordHasher, RepositoryError, ResourceRepository as _, UnitOfWork as _, UnitOfWorkFactory,
    UserFilter, UserRepository as _, UserUnitOfWork,
};
use crate::domain::service::{
    PasswordPolicy, PasswordPolicyError, UsernamePolicy, UsernamePolicyError,
};

/// Use case implementation for registering a new user.
pub(crate) struct RegisterUser<F, P> {
    /// Hasher for user passwords.
    password_hasher: Arc<P>,
    /// Factory opening the unit of work wrapping the registration.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, P: PasswordHasher> RegisterUser<F, P> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>, password_hasher: Arc<P>) -> Self {
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

            UsernamePolicy::validate(command.username()).map_err(|error| match error {
                UsernamePolicyError::TooShort
                | UsernamePolicyError::TooLong
                | UsernamePolicyError::ContainsNul
                | UsernamePolicyError::ContainsWhitespace => RegisterUserError::InvalidUsername,
            })?;

            User::try_new(0, username.clone(), email.clone(), 0, command.role())
                .map_err(|_| RegisterUserError::InvalidEmail)?;

            password_policy
                .validate(command.password())
                .map_err(|error| match error {
                    PasswordPolicyError::PasswordMissingSymbol
                    | PasswordPolicyError::PasswordTooShort => RegisterUserError::InvalidPassword,
                })?;

            let mut unit_of_work = unit_of_work_factory
                .begin_write()
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
                    .ok_or(RegisterUserError::NoSuchCaller)?;
                let child_kind = match command.role() {
                    Role::Admin => ChildKind::Admin,
                    Role::Standard => ChildKind::User,
                };
                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::UserCollection, 0),
                    Action::CreateChild,
                    Some(child_kind),
                )
                .await
                .map_err(|_| RegisterUserError::Forbidden)?;

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
                    return Err(RegisterUserError::UsernameAlreadyExists);
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
                        return Err(RegisterUserError::EmailAlreadyExists);
                    }
                }

                // The Argon2 hash is computed while the unit of work holds one pooled
                // connection, blocking that connection for its duration. The pool defaults
                // above one connection and SQLite runs in write-ahead logging mode, so
                // other requests keep being served; do not move the hash off the
                // transaction without authorizing the caller first.
                let password_hash = password_hasher
                    .hash_password(command.password())
                    .await
                    .map_err(|error| RegisterUserError::Unknown(error.into()))?;
                let pending_credential =
                    Credential::try_new(0, password_hash, Utc::now().naive_utc()).map_err(
                        |error| {
                            error!(
                                target: "security",
                                event = "application_error",
                                operation = %"register_user",
                                "a use case originated an internal error"
                            );
                            RegisterUserError::Unknown(error.into())
                        },
                    )?;
                let credential = unit_of_work
                    .credentials()
                    .create(pending_credential)
                    .await
                    .map_err(|error| RegisterUserError::Unknown(error.into()))?;

                let pending_user =
                    User::try_new(0, username, email, credential.id(), command.role()).map_err(
                        |error| {
                            error!(
                                target: "security",
                                event = "application_error",
                                operation = %"register_user",
                                "a use case originated an internal error"
                            );
                            RegisterUserError::Unknown(error.into())
                        },
                    )?;
                let user = unit_of_work
                    .users()
                    .create(pending_user)
                    .await
                    .map_err(|error| match error {
                        RepositoryError::AlreadyExist => RegisterUserError::UserAlreadyExists,
                        // Every other variant maps to `Unknown`; naming them
                        // keeps a new port variant a compile error here.
                        RepositoryError::Conflict
                        | RepositoryError::DataIntegrityViolation
                        | RepositoryError::ConcurrencyConflict
                        | RepositoryError::OperationFailed
                        | RepositoryError::Timeout
                        | RepositoryError::Unavailable
                        | RepositoryError::Unknown(_) => RegisterUserError::Unknown(error.into()),
                    })?;

                let user_resource = unit_of_work
                    .resources()
                    .create(NewResource {
                        kind: ResourceKind::User,
                        entity_id: user.id(),
                        owner_id: None,
                        auth_parent: Some(ResourceRef::new(ResourceKind::Instance, 0)),
                        visibility: Visibility::Private,
                    })
                    .await
                    .map_err(|error| RegisterUserError::Unknown(error.into()))?;
                let admin_owner = Grant::try_new(
                    Principal::Role(Role::Admin),
                    user_resource.resource_id(),
                    ResourceRole::Owner,
                    ChildKind::All,
                    GrantEffect::Allow,
                )
                .map_err(|error| RegisterUserError::Unknown(error.into()))?;
                unit_of_work
                    .grants()
                    .save_grant(admin_owner)
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
                    info!(
                        target: "security",
                        event = "user_admin",
                        actor = command.caller_id(),
                        action = %"create",
                        target = %"user",
                        "changed a user-administration object"
                    );
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

    use crate::domain::model::UserError;

    use crate::domain::port::{
        MockCredentialRepository, MockPasswordHasher, MockResourceRepository, MockUserRepository,
        PasswordHasherError,
    };

    use crate::domain::model::{Resource, ResourceAccess};
    use crate::test_helpers::*;
    use std::error::Error;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};

    type UseCase = RegisterUser<TestFactory, MockPasswordHasher>;

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

    fn existing_users(id: i64, username: &str, role: Role) -> Result<Vec<User>, RepositoryError> {
        user(id, username, role)
            .map(|user| vec![user])
            .map_err(|_| RepositoryError::OperationFailed)
    }

    #[tokio::test]
    async fn register_user_root_admin_register_standard_user_succeeds() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|user_repository, credential_repository, password_hasher| {
                (move |user_repository: &mut MockUserRepository, caller: User| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(move |_| {
                            let own_caller = caller.clone();
                            Box::pin(async move { Ok(vec![own_caller]) })
                        });
                })(user_repository, user(0, "root", Role::Admin)?);
                (move |user_repository: &mut MockUserRepository| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                })(user_repository);
                (move |user_repository: &mut MockUserRepository| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                })(user_repository);
                (move |credential_repository: &mut MockCredentialRepository,
                       password_hasher: &mut MockPasswordHasher,
                       user_repository: &mut MockUserRepository| {
                    (move |password_hasher: &mut MockPasswordHasher| {
                        password_hasher
                            .expect_hash_password()
                            .times(1)
                            .returning(|_| Box::pin(async { Ok("hashed-password".to_owned()) }));
                    })(password_hasher);
                    credential_repository
                        .expect_create()
                        .times(1)
                        .returning(|credential| Box::pin(async { Ok(credential) }));
                    user_repository
                        .expect_create()
                        .times(1)
                        .returning(|user| Box::pin(async { Ok(user) }));
                })(credential_repository, password_hasher, user_repository);
                Ok(())
            });
            (move || -> Result<Harness, Box<dyn Error>> {
                let mut user_repository = MockUserRepository::new();
                let mut credential_repository = MockCredentialRepository::new();
                let mut password_hasher = MockPasswordHasher::new();

                setup(
                    &mut user_repository,
                    &mut credential_repository,
                    &mut password_hasher,
                )?;

                let unit_of_work = TestUnitOfWork {
                    credentials: credential_repository,
                    users: user_repository,
                    ..TestUnitOfWork::default()
                };
                let committed = Arc::clone(&unit_of_work.committed);
                let rolled_back = Arc::clone(&unit_of_work.rolled_back);
                let factory = TestFactory::new(unit_of_work);

                Ok(Harness {
                    use_case: RegisterUser::new(Arc::new(factory), Arc::new(password_hasher)),
                    committed,
                    rolled_back,
                })
            })()
        }?;
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

    // proves: SCN-USER-025-A — Business rule
    #[tokio::test]
    async fn register_user_created_account_has_instance_root_parent() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let recorded = Arc::new(Mutex::new(None));
        let capture = Arc::clone(&recorded);
        let caller = user(0, "root", Role::Admin)?;
        let mut user_repository = MockUserRepository::new();
        user_repository
            .expect_search()
            .times(1)
            .returning(move |_| {
                let own_caller = caller.clone();
                Box::pin(async move { Ok(vec![own_caller]) })
            });
        user_repository
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        user_repository
            .expect_create()
            .times(1)
            .returning(|user| Box::pin(async { Ok(user) }));
        let mut credential_repository = MockCredentialRepository::new();
        credential_repository
            .expect_create()
            .times(1)
            .returning(|credential| Box::pin(async { Ok(credential) }));
        let mut password_hasher = MockPasswordHasher::new();
        password_hasher
            .expect_hash_password()
            .times(1)
            .returning(|_| Box::pin(async { Ok("hashed-password".to_owned()) }));
        let mut resources = MockResourceRepository::new();
        resources
            .expect_search()
            .times(1)
            .returning(|_filter, caller| {
                let user_id = caller.id();
                let resource = Resource::from_stored(
                    1,
                    ResourceKind::UserCollection,
                    0,
                    Some(user_id),
                    None,
                    Visibility::Private,
                );
                let grant = Grant::from_stored(
                    1,
                    Principal::User(user_id),
                    1,
                    ResourceRole::Downloader,
                    ChildKind::All,
                    GrantEffect::Allow,
                );
                Box::pin(async move {
                    Ok(vec![ResourceAccess::new(
                        resource,
                        Vec::new(),
                        vec![grant],
                        Vec::new(),
                    )])
                })
            });
        resources
            .expect_create()
            .times(1)
            .returning(move |new_resource| {
                let resource = Resource::from_stored(
                    1,
                    new_resource.kind,
                    new_resource.entity_id,
                    new_resource.owner_id,
                    None,
                    new_resource.visibility,
                );
                if let Ok(mut slot) = capture.lock() {
                    *slot = Some(new_resource);
                }
                Box::pin(async move { Ok(resource) })
            });
        let unit_of_work = TestUnitOfWork {
            credentials: credential_repository,
            resources,
            users: user_repository,
            ..TestUnitOfWork::default()
        };
        let committed = Arc::clone(&unit_of_work.committed);
        let factory = TestFactory::new(unit_of_work);
        let use_case = RegisterUser::new(Arc::new(factory), Arc::new(password_hasher));

        // Act
        let result = use_case.execute(command(0, "alice", Role::Standard)).await;

        // Assert
        assert!(result.is_ok());
        assert!(committed.load(Ordering::SeqCst));
        let created = recorded.lock().ok().and_then(|slot| slot.as_ref().cloned());
        assert_eq!(
            created.as_ref().map(|resource| resource.kind),
            Some(ResourceKind::User)
        );
        assert_eq!(
            created.and_then(|resource| resource.auth_parent),
            Some(ResourceRef::new(ResourceKind::Instance, 0))
        );
        Ok(())
    }

    #[tokio::test]
    async fn register_user_root_admin_register_admin_user_succeeds() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|user_repository, credential_repository, password_hasher| {
                (move |user_repository: &mut MockUserRepository, caller: User| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(move |_| {
                            let own_caller = caller.clone();
                            Box::pin(async move { Ok(vec![own_caller]) })
                        });
                })(user_repository, user(0, "root", Role::Admin)?);
                (move |user_repository: &mut MockUserRepository| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                })(user_repository);
                (move |credential_repository: &mut MockCredentialRepository,
                       password_hasher: &mut MockPasswordHasher,
                       user_repository: &mut MockUserRepository| {
                    (move |password_hasher: &mut MockPasswordHasher| {
                        password_hasher
                            .expect_hash_password()
                            .times(1)
                            .returning(|_| Box::pin(async { Ok("hashed-password".to_owned()) }));
                    })(password_hasher);
                    credential_repository
                        .expect_create()
                        .times(1)
                        .returning(|credential| Box::pin(async { Ok(credential) }));
                    user_repository
                        .expect_create()
                        .times(1)
                        .returning(|user| Box::pin(async { Ok(user) }));
                })(credential_repository, password_hasher, user_repository);
                Ok(())
            });
            (move || -> Result<Harness, Box<dyn Error>> {
                let mut user_repository = MockUserRepository::new();
                let mut credential_repository = MockCredentialRepository::new();
                let mut password_hasher = MockPasswordHasher::new();

                setup(
                    &mut user_repository,
                    &mut credential_repository,
                    &mut password_hasher,
                )?;

                let unit_of_work = TestUnitOfWork {
                    credentials: credential_repository,
                    users: user_repository,
                    ..TestUnitOfWork::default()
                };
                let committed = Arc::clone(&unit_of_work.committed);
                let rolled_back = Arc::clone(&unit_of_work.rolled_back);
                let factory = TestFactory::new(unit_of_work);

                Ok(Harness {
                    use_case: RegisterUser::new(Arc::new(factory), Arc::new(password_hasher)),
                    committed,
                    rolled_back,
                })
            })()
        }?;
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
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|user_repository, credential_repository, password_hasher| {
                (move |user_repository: &mut MockUserRepository, caller: User| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(move |_| {
                            let own_caller = caller.clone();
                            Box::pin(async move { Ok(vec![own_caller]) })
                        });
                })(user_repository, user(5, "admin", Role::Admin)?);
                (move |user_repository: &mut MockUserRepository| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                })(user_repository);
                (move |credential_repository: &mut MockCredentialRepository,
                       password_hasher: &mut MockPasswordHasher,
                       user_repository: &mut MockUserRepository| {
                    (move |password_hasher: &mut MockPasswordHasher| {
                        password_hasher
                            .expect_hash_password()
                            .times(1)
                            .returning(|_| Box::pin(async { Ok("hashed-password".to_owned()) }));
                    })(password_hasher);
                    credential_repository
                        .expect_create()
                        .times(1)
                        .returning(|credential| Box::pin(async { Ok(credential) }));
                    user_repository
                        .expect_create()
                        .times(1)
                        .returning(|user| Box::pin(async { Ok(user) }));
                })(credential_repository, password_hasher, user_repository);
                Ok(())
            });
            (move || -> Result<Harness, Box<dyn Error>> {
                let mut user_repository = MockUserRepository::new();
                let mut credential_repository = MockCredentialRepository::new();
                let mut password_hasher = MockPasswordHasher::new();

                setup(
                    &mut user_repository,
                    &mut credential_repository,
                    &mut password_hasher,
                )?;

                let unit_of_work = TestUnitOfWork {
                    credentials: credential_repository,
                    users: user_repository,
                    ..TestUnitOfWork::default()
                };
                let committed = Arc::clone(&unit_of_work.committed);
                let rolled_back = Arc::clone(&unit_of_work.rolled_back);
                let factory = TestFactory::new(unit_of_work);

                Ok(Harness {
                    use_case: RegisterUser::new(Arc::new(factory), Arc::new(password_hasher)),
                    committed,
                    rolled_back,
                })
            })()
        }?;
        let command = command(5, "dave", Role::Standard);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }
    #[tokio::test]
    async fn register_user_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|user_repository, _, _| {
                (move |user_repository: &mut MockUserRepository| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                })(user_repository);
                Ok(())
            });
            (move || -> Result<Harness, Box<dyn Error>> {
                let mut user_repository = MockUserRepository::new();
                let mut credential_repository = MockCredentialRepository::new();
                let mut password_hasher = MockPasswordHasher::new();

                setup(
                    &mut user_repository,
                    &mut credential_repository,
                    &mut password_hasher,
                )?;

                let unit_of_work = TestUnitOfWork {
                    credentials: credential_repository,
                    users: user_repository,
                    ..TestUnitOfWork::default()
                };
                let committed = Arc::clone(&unit_of_work.committed);
                let rolled_back = Arc::clone(&unit_of_work.rolled_back);
                let factory = TestFactory::new(unit_of_work);

                Ok(Harness {
                    use_case: RegisterUser::new(Arc::new(factory), Arc::new(password_hasher)),
                    committed,
                    rolled_back,
                })
            })()
        }?;
        let command = command(999, "grace", Role::Standard);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_username_too_short_returns_invalid_username()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|_, _, _| Ok(()));
            (move || -> Result<Harness, Box<dyn Error>> {
                let mut user_repository = MockUserRepository::new();
                let mut credential_repository = MockCredentialRepository::new();
                let mut password_hasher = MockPasswordHasher::new();

                setup(
                    &mut user_repository,
                    &mut credential_repository,
                    &mut password_hasher,
                )?;

                let unit_of_work = TestUnitOfWork {
                    credentials: credential_repository,
                    users: user_repository,
                    ..TestUnitOfWork::default()
                };
                let committed = Arc::clone(&unit_of_work.committed);
                let rolled_back = Arc::clone(&unit_of_work.rolled_back);
                let factory = TestFactory::new(unit_of_work);

                Ok(Harness {
                    use_case: RegisterUser::new(Arc::new(factory), Arc::new(password_hasher)),
                    committed,
                    rolled_back,
                })
            })()
        }?;
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
    async fn register_user_uppercase_username_succeeds() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|user_repository, credential_repository, password_hasher| {
                (move |user_repository: &mut MockUserRepository, caller: User| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(move |_| {
                            let own_caller = caller.clone();
                            Box::pin(async move { Ok(vec![own_caller]) })
                        });
                })(user_repository, user(0, "root", Role::Admin)?);
                (move |user_repository: &mut MockUserRepository| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                })(user_repository);
                (move |credential_repository: &mut MockCredentialRepository,
                       password_hasher: &mut MockPasswordHasher,
                       user_repository: &mut MockUserRepository| {
                    (move |password_hasher: &mut MockPasswordHasher| {
                        password_hasher
                            .expect_hash_password()
                            .times(1)
                            .returning(|_| Box::pin(async { Ok("hashed-password".to_owned()) }));
                    })(password_hasher);
                    credential_repository
                        .expect_create()
                        .times(1)
                        .returning(|credential| Box::pin(async { Ok(credential) }));
                    user_repository
                        .expect_create()
                        .times(1)
                        .returning(|user| Box::pin(async { Ok(user) }));
                })(credential_repository, password_hasher, user_repository);
                Ok(())
            });
            (move || -> Result<Harness, Box<dyn Error>> {
                let mut user_repository = MockUserRepository::new();
                let mut credential_repository = MockCredentialRepository::new();
                let mut password_hasher = MockPasswordHasher::new();

                setup(
                    &mut user_repository,
                    &mut credential_repository,
                    &mut password_hasher,
                )?;

                let unit_of_work = TestUnitOfWork {
                    credentials: credential_repository,
                    users: user_repository,
                    ..TestUnitOfWork::default()
                };
                let committed = Arc::clone(&unit_of_work.committed);
                let rolled_back = Arc::clone(&unit_of_work.rolled_back);
                let factory = TestFactory::new(unit_of_work);

                Ok(Harness {
                    use_case: RegisterUser::new(Arc::new(factory), Arc::new(password_hasher)),
                    committed,
                    rolled_back,
                })
            })()
        }?;
        let command = command(0, "Alice", Role::Standard);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert_eq!(result?.username(), "Alice");
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_surrounding_whitespace_username_returns_invalid_username()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|_, _, _| Ok(()));
            (move || -> Result<Harness, Box<dyn Error>> {
                let mut user_repository = MockUserRepository::new();
                let mut credential_repository = MockCredentialRepository::new();
                let mut password_hasher = MockPasswordHasher::new();

                setup(
                    &mut user_repository,
                    &mut credential_repository,
                    &mut password_hasher,
                )?;

                let unit_of_work = TestUnitOfWork {
                    credentials: credential_repository,
                    users: user_repository,
                    ..TestUnitOfWork::default()
                };
                let committed = Arc::clone(&unit_of_work.committed);
                let rolled_back = Arc::clone(&unit_of_work.rolled_back);
                let factory = TestFactory::new(unit_of_work);

                Ok(Harness {
                    use_case: RegisterUser::new(Arc::new(factory), Arc::new(password_hasher)),
                    committed,
                    rolled_back,
                })
            })()
        }?;
        let command = command(0, " alice", Role::Standard);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::InvalidUsername)));
        assert!(!harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_too_long_username_returns_invalid_username() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|_, _, _| Ok(()));
            (move || -> Result<Harness, Box<dyn Error>> {
                let mut user_repository = MockUserRepository::new();
                let mut credential_repository = MockCredentialRepository::new();
                let mut password_hasher = MockPasswordHasher::new();

                setup(
                    &mut user_repository,
                    &mut credential_repository,
                    &mut password_hasher,
                )?;

                let unit_of_work = TestUnitOfWork {
                    credentials: credential_repository,
                    users: user_repository,
                    ..TestUnitOfWork::default()
                };
                let committed = Arc::clone(&unit_of_work.committed);
                let rolled_back = Arc::clone(&unit_of_work.rolled_back);
                let factory = TestFactory::new(unit_of_work);

                Ok(Harness {
                    use_case: RegisterUser::new(Arc::new(factory), Arc::new(password_hasher)),
                    committed,
                    rolled_back,
                })
            })()
        }?;
        let username = "a".repeat(101);
        let command = command(0, &username, Role::Standard);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::InvalidUsername)));
        assert!(!harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_internal_space_username_returns_invalid_username()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|_, _, _| Ok(()));
            (move || -> Result<Harness, Box<dyn Error>> {
                let mut user_repository = MockUserRepository::new();
                let mut credential_repository = MockCredentialRepository::new();
                let mut password_hasher = MockPasswordHasher::new();

                setup(
                    &mut user_repository,
                    &mut credential_repository,
                    &mut password_hasher,
                )?;

                let unit_of_work = TestUnitOfWork {
                    credentials: credential_repository,
                    users: user_repository,
                    ..TestUnitOfWork::default()
                };
                let committed = Arc::clone(&unit_of_work.committed);
                let rolled_back = Arc::clone(&unit_of_work.rolled_back);
                let factory = TestFactory::new(unit_of_work);

                Ok(Harness {
                    use_case: RegisterUser::new(Arc::new(factory), Arc::new(password_hasher)),
                    committed,
                    rolled_back,
                })
            })()
        }?;
        let command = command(0, "al ice", Role::Standard);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::InvalidUsername)));
        assert!(!harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_invalid_email_returns_invalid_email() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|_, _, _| Ok(()));
            (move || -> Result<Harness, Box<dyn Error>> {
                let mut user_repository = MockUserRepository::new();
                let mut credential_repository = MockCredentialRepository::new();
                let mut password_hasher = MockPasswordHasher::new();

                setup(
                    &mut user_repository,
                    &mut credential_repository,
                    &mut password_hasher,
                )?;

                let unit_of_work = TestUnitOfWork {
                    credentials: credential_repository,
                    users: user_repository,
                    ..TestUnitOfWork::default()
                };
                let committed = Arc::clone(&unit_of_work.committed);
                let rolled_back = Arc::clone(&unit_of_work.rolled_back);
                let factory = TestFactory::new(unit_of_work);

                Ok(Harness {
                    use_case: RegisterUser::new(Arc::new(factory), Arc::new(password_hasher)),
                    committed,
                    rolled_back,
                })
            })()
        }?;
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
    async fn register_user_display_name_email_returns_invalid_email() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|_, _, _| Ok(()));
            (move || -> Result<Harness, Box<dyn Error>> {
                let mut user_repository = MockUserRepository::new();
                let mut credential_repository = MockCredentialRepository::new();
                let mut password_hasher = MockPasswordHasher::new();

                setup(
                    &mut user_repository,
                    &mut credential_repository,
                    &mut password_hasher,
                )?;

                let unit_of_work = TestUnitOfWork {
                    credentials: credential_repository,
                    users: user_repository,
                    ..TestUnitOfWork::default()
                };
                let committed = Arc::clone(&unit_of_work.committed);
                let rolled_back = Arc::clone(&unit_of_work.rolled_back);
                let factory = TestFactory::new(unit_of_work);

                Ok(Harness {
                    use_case: RegisterUser::new(Arc::new(factory), Arc::new(password_hasher)),
                    committed,
                    rolled_back,
                })
            })()
        }?;
        let command = RegisterUserCommand::new(
            0,
            "heidi".to_owned(),
            Some("Alice <alice@example.com>".to_owned()),
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
    async fn register_user_padded_email_returns_invalid_email() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|_, _, _| Ok(()));
            (move || -> Result<Harness, Box<dyn Error>> {
                let mut user_repository = MockUserRepository::new();
                let mut credential_repository = MockCredentialRepository::new();
                let mut password_hasher = MockPasswordHasher::new();

                setup(
                    &mut user_repository,
                    &mut credential_repository,
                    &mut password_hasher,
                )?;

                let unit_of_work = TestUnitOfWork {
                    credentials: credential_repository,
                    users: user_repository,
                    ..TestUnitOfWork::default()
                };
                let committed = Arc::clone(&unit_of_work.committed);
                let rolled_back = Arc::clone(&unit_of_work.rolled_back);
                let factory = TestFactory::new(unit_of_work);

                Ok(Harness {
                    use_case: RegisterUser::new(Arc::new(factory), Arc::new(password_hasher)),
                    committed,
                    rolled_back,
                })
            })()
        }?;
        let command = RegisterUserCommand::new(
            0,
            "heidi".to_owned(),
            Some(" alice@example.com ".to_owned()),
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
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|_, _, _| Ok(()));
            (move || -> Result<Harness, Box<dyn Error>> {
                let mut user_repository = MockUserRepository::new();
                let mut credential_repository = MockCredentialRepository::new();
                let mut password_hasher = MockPasswordHasher::new();

                setup(
                    &mut user_repository,
                    &mut credential_repository,
                    &mut password_hasher,
                )?;

                let unit_of_work = TestUnitOfWork {
                    credentials: credential_repository,
                    users: user_repository,
                    ..TestUnitOfWork::default()
                };
                let committed = Arc::clone(&unit_of_work.committed);
                let rolled_back = Arc::clone(&unit_of_work.rolled_back);
                let factory = TestFactory::new(unit_of_work);

                Ok(Harness {
                    use_case: RegisterUser::new(Arc::new(factory), Arc::new(password_hasher)),
                    committed,
                    rolled_back,
                })
            })()
        }?;
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
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|_, _, _| Ok(()));
            (move || -> Result<Harness, Box<dyn Error>> {
                let mut user_repository = MockUserRepository::new();
                let mut credential_repository = MockCredentialRepository::new();
                let mut password_hasher = MockPasswordHasher::new();

                setup(
                    &mut user_repository,
                    &mut credential_repository,
                    &mut password_hasher,
                )?;

                let unit_of_work = TestUnitOfWork {
                    credentials: credential_repository,
                    users: user_repository,
                    ..TestUnitOfWork::default()
                };
                let committed = Arc::clone(&unit_of_work.committed);
                let rolled_back = Arc::clone(&unit_of_work.rolled_back);
                let factory = TestFactory::new(unit_of_work);

                Ok(Harness {
                    use_case: RegisterUser::new(Arc::new(factory), Arc::new(password_hasher)),
                    committed,
                    rolled_back,
                })
            })()
        }?;
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
    async fn register_user_existing_username_returns_username_already_exists()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|user_repository, _, _| {
                (move |user_repository: &mut MockUserRepository, caller: User| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(move |_| {
                            let own_caller = caller.clone();
                            Box::pin(async move { Ok(vec![own_caller]) })
                        });
                })(user_repository, user(0, "root", Role::Admin)?);
                user_repository.expect_search().times(1).returning(|_| {
                    let users = existing_users(1, "mallory", Role::Standard);
                    Box::pin(async move { users })
                });
                Ok(())
            });
            (move || -> Result<Harness, Box<dyn Error>> {
                let mut user_repository = MockUserRepository::new();
                let mut credential_repository = MockCredentialRepository::new();
                let mut password_hasher = MockPasswordHasher::new();

                setup(
                    &mut user_repository,
                    &mut credential_repository,
                    &mut password_hasher,
                )?;

                let unit_of_work = TestUnitOfWork {
                    credentials: credential_repository,
                    users: user_repository,
                    ..TestUnitOfWork::default()
                };
                let committed = Arc::clone(&unit_of_work.committed);
                let rolled_back = Arc::clone(&unit_of_work.rolled_back);
                let factory = TestFactory::new(unit_of_work);

                Ok(Harness {
                    use_case: RegisterUser::new(Arc::new(factory), Arc::new(password_hasher)),
                    committed,
                    rolled_back,
                })
            })()
        }?;
        let command = command(0, "mallory", Role::Standard);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(
            result,
            Err(RegisterUserError::UsernameAlreadyExists)
        ));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_existing_email_returns_email_already_exists()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|user_repository, _, _| {
                (move |user_repository: &mut MockUserRepository, caller: User| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(move |_| {
                            let own_caller = caller.clone();
                            Box::pin(async move { Ok(vec![own_caller]) })
                        });
                })(user_repository, user(0, "root", Role::Admin)?);
                (move |user_repository: &mut MockUserRepository| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                })(user_repository);
                user_repository.expect_search().times(1).returning(|_| {
                    let users = existing_users(1, "nancy", Role::Standard);
                    Box::pin(async move { users })
                });
                Ok(())
            });
            (move || -> Result<Harness, Box<dyn Error>> {
                let mut user_repository = MockUserRepository::new();
                let mut credential_repository = MockCredentialRepository::new();
                let mut password_hasher = MockPasswordHasher::new();

                setup(
                    &mut user_repository,
                    &mut credential_repository,
                    &mut password_hasher,
                )?;

                let unit_of_work = TestUnitOfWork {
                    credentials: credential_repository,
                    users: user_repository,
                    ..TestUnitOfWork::default()
                };
                let committed = Arc::clone(&unit_of_work.committed);
                let rolled_back = Arc::clone(&unit_of_work.rolled_back);
                let factory = TestFactory::new(unit_of_work);

                Ok(Harness {
                    use_case: RegisterUser::new(Arc::new(factory), Arc::new(password_hasher)),
                    committed,
                    rolled_back,
                })
            })()
        }?;
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
        assert!(matches!(result, Err(RegisterUserError::EmailAlreadyExists)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_search_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|user_repository, _, _| {
                user_repository
                    .expect_search()
                    .times(1)
                    .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
                Ok(())
            });
            (move || -> Result<Harness, Box<dyn Error>> {
                let mut user_repository = MockUserRepository::new();
                let mut credential_repository = MockCredentialRepository::new();
                let mut password_hasher = MockPasswordHasher::new();

                setup(
                    &mut user_repository,
                    &mut credential_repository,
                    &mut password_hasher,
                )?;

                let unit_of_work = TestUnitOfWork {
                    credentials: credential_repository,
                    users: user_repository,
                    ..TestUnitOfWork::default()
                };
                let committed = Arc::clone(&unit_of_work.committed);
                let rolled_back = Arc::clone(&unit_of_work.rolled_back);
                let factory = TestFactory::new(unit_of_work);

                Ok(Harness {
                    use_case: RegisterUser::new(Arc::new(factory), Arc::new(password_hasher)),
                    committed,
                    rolled_back,
                })
            })()
        }?;
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
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|user_repository, _, password_hasher| {
                (move |user_repository: &mut MockUserRepository, caller: User| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(move |_| {
                            let own_caller = caller.clone();
                            Box::pin(async move { Ok(vec![own_caller]) })
                        });
                })(user_repository, user(0, "root", Role::Admin)?);
                (move |user_repository: &mut MockUserRepository| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                })(user_repository);
                password_hasher
                    .expect_hash_password()
                    .times(1)
                    .returning(|_| Box::pin(async { Err(PasswordHasherError::OperationFailed) }));
                Ok(())
            });
            (move || -> Result<Harness, Box<dyn Error>> {
                let mut user_repository = MockUserRepository::new();
                let mut credential_repository = MockCredentialRepository::new();
                let mut password_hasher = MockPasswordHasher::new();

                setup(
                    &mut user_repository,
                    &mut credential_repository,
                    &mut password_hasher,
                )?;

                let unit_of_work = TestUnitOfWork {
                    credentials: credential_repository,
                    users: user_repository,
                    ..TestUnitOfWork::default()
                };
                let committed = Arc::clone(&unit_of_work.committed);
                let rolled_back = Arc::clone(&unit_of_work.rolled_back);
                let factory = TestFactory::new(unit_of_work);

                Ok(Harness {
                    use_case: RegisterUser::new(Arc::new(factory), Arc::new(password_hasher)),
                    committed,
                    rolled_back,
                })
            })()
        }?;
        let command = command(0, "quinn", Role::Standard);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    // proves: SCN-USER-007-C — Recovery
    #[tokio::test]
    async fn register_user_save_credential_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|user_repository, credential_repository, password_hasher| {
                (move |user_repository: &mut MockUserRepository, caller: User| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(move |_| {
                            let own_caller = caller.clone();
                            Box::pin(async move { Ok(vec![own_caller]) })
                        });
                })(user_repository, user(0, "root", Role::Admin)?);
                (move |user_repository: &mut MockUserRepository| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                })(user_repository);
                (move |password_hasher: &mut MockPasswordHasher| {
                    password_hasher
                        .expect_hash_password()
                        .times(1)
                        .returning(|_| Box::pin(async { Ok("hashed-password".to_owned()) }));
                })(password_hasher);
                credential_repository
                    .expect_create()
                    .times(1)
                    .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
                Ok(())
            });
            (move || -> Result<Harness, Box<dyn Error>> {
                let mut user_repository = MockUserRepository::new();
                let mut credential_repository = MockCredentialRepository::new();
                let mut password_hasher = MockPasswordHasher::new();

                setup(
                    &mut user_repository,
                    &mut credential_repository,
                    &mut password_hasher,
                )?;

                let unit_of_work = TestUnitOfWork {
                    credentials: credential_repository,
                    users: user_repository,
                    ..TestUnitOfWork::default()
                };
                let committed = Arc::clone(&unit_of_work.committed);
                let rolled_back = Arc::clone(&unit_of_work.rolled_back);
                let factory = TestFactory::new(unit_of_work);

                Ok(Harness {
                    use_case: RegisterUser::new(Arc::new(factory), Arc::new(password_hasher)),
                    committed,
                    rolled_back,
                })
            })()
        }?;
        let command = command(0, "rupert", Role::Standard);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    // proves: SCN-USER-007-C — Recovery
    #[tokio::test]
    async fn register_user_save_user_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|user_repository, credential_repository, password_hasher| {
                (move |user_repository: &mut MockUserRepository, caller: User| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(move |_| {
                            let own_caller = caller.clone();
                            Box::pin(async move { Ok(vec![own_caller]) })
                        });
                })(user_repository, user(0, "root", Role::Admin)?);
                (move |user_repository: &mut MockUserRepository| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                })(user_repository);
                (move |password_hasher: &mut MockPasswordHasher| {
                    password_hasher
                        .expect_hash_password()
                        .times(1)
                        .returning(|_| Box::pin(async { Ok("hashed-password".to_owned()) }));
                })(password_hasher);
                credential_repository
                    .expect_create()
                    .times(1)
                    .returning(|credential| Box::pin(async { Ok(credential) }));
                user_repository
                    .expect_create()
                    .times(1)
                    .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
                Ok(())
            });
            (move || -> Result<Harness, Box<dyn Error>> {
                let mut user_repository = MockUserRepository::new();
                let mut credential_repository = MockCredentialRepository::new();
                let mut password_hasher = MockPasswordHasher::new();

                setup(
                    &mut user_repository,
                    &mut credential_repository,
                    &mut password_hasher,
                )?;

                let unit_of_work = TestUnitOfWork {
                    credentials: credential_repository,
                    users: user_repository,
                    ..TestUnitOfWork::default()
                };
                let committed = Arc::clone(&unit_of_work.committed);
                let rolled_back = Arc::clone(&unit_of_work.rolled_back);
                let factory = TestFactory::new(unit_of_work);

                Ok(Harness {
                    use_case: RegisterUser::new(Arc::new(factory), Arc::new(password_hasher)),
                    committed,
                    rolled_back,
                })
            })()
        }?;
        let command = command(0, "trent", Role::Standard);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn register_user_raced_duplicate_returns_user_already_exists()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|user_repository, credential_repository, password_hasher| {
                (move |user_repository: &mut MockUserRepository, caller: User| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(move |_| {
                            let own_caller = caller.clone();
                            Box::pin(async move { Ok(vec![own_caller]) })
                        });
                })(user_repository, user(0, "root", Role::Admin)?);
                (move |user_repository: &mut MockUserRepository| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                })(user_repository);
                (move |password_hasher: &mut MockPasswordHasher| {
                    password_hasher
                        .expect_hash_password()
                        .times(1)
                        .returning(|_| Box::pin(async { Ok("hashed-password".to_owned()) }));
                })(password_hasher);
                credential_repository
                    .expect_create()
                    .times(1)
                    .returning(|credential| Box::pin(async { Ok(credential) }));
                user_repository
                    .expect_create()
                    .times(1)
                    .returning(|_| Box::pin(async { Err(RepositoryError::AlreadyExist) }));
                Ok(())
            });
            (move || -> Result<Harness, Box<dyn Error>> {
                let mut user_repository = MockUserRepository::new();
                let mut credential_repository = MockCredentialRepository::new();
                let mut password_hasher = MockPasswordHasher::new();

                setup(
                    &mut user_repository,
                    &mut credential_repository,
                    &mut password_hasher,
                )?;

                let unit_of_work = TestUnitOfWork {
                    credentials: credential_repository,
                    users: user_repository,
                    ..TestUnitOfWork::default()
                };
                let committed = Arc::clone(&unit_of_work.committed);
                let rolled_back = Arc::clone(&unit_of_work.rolled_back);
                let factory = TestFactory::new(unit_of_work);

                Ok(Harness {
                    use_case: RegisterUser::new(Arc::new(factory), Arc::new(password_hasher)),
                    committed,
                    rolled_back,
                })
            })()
        }?;
        let command = command(0, "mallory", Role::Standard);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(RegisterUserError::UserAlreadyExists)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
