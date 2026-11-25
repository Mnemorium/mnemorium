use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tracing::{error, info, warn};

use crate::application::port::{LoginUserCommand, LoginUserError, LoginUserUseCase};
use crate::domain::port::{
    CredentialFilter, CredentialRepository as _, IdentityUnitOfWork, IssuedToken, LoginAttemptKey,
    LoginAttemptLimiter, PasswordHasher, TokenProvider, UnitOfWork as _, UnitOfWorkFactory,
    UserFilter, UserRepository as _, UserUnitOfWork,
};

/// Use case implementation for authenticating a user.
pub(crate) struct LoginUser<F, H, P, L: ?Sized> {
    /// Limiter counting attempts per account.
    attempt_limiter: Arc<L>,
    /// Hasher for user passwords.
    password_hasher: Arc<H>,
    /// Provider issuing and validating tokens.
    token_provider: Arc<P>,
    /// Factory opening the unit of work wrapping the authentication.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, H: PasswordHasher, P: TokenProvider, L: LoginAttemptLimiter + ?Sized>
    LoginUser<F, H, P, L>
{
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(
        unit_of_work_factory: Arc<F>,
        password_hasher: Arc<H>,
        token_provider: Arc<P>,
        attempt_limiter: Arc<L>,
    ) -> Self {
        Self {
            attempt_limiter,
            password_hasher,
            token_provider,
            unit_of_work_factory,
        }
    }
}

impl<F, H, P, L> LoginUserUseCase for LoginUser<F, H, P, L>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + UserUnitOfWork,
    H: PasswordHasher,
    P: TokenProvider,
    L: LoginAttemptLimiter + ?Sized,
{
    fn execute<'future>(
        &'future self,
        command: LoginUserCommand,
    ) -> Pin<Box<dyn Future<Output = Result<IssuedToken, LoginUserError>> + Send + 'future>> {
        let password_hasher = Arc::clone(&self.password_hasher);
        let token_provider = Arc::clone(&self.token_provider);
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);
        let attempt_limiter = Arc::clone(&self.attempt_limiter);

        Box::pin(async move {
            let identifier = command.identifier().to_owned();
            let password = command.password().to_owned();

            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| LoginUserError::Unknown(error.into()))?;

            let result = async {
                // The submitted value is a username or an email address. A
                // value that matches a username resolves as that username even
                // when another account's email address matches it too; only
                // when no username matches is it looked up as an email address.
                let mut found = unit_of_work
                    .users()
                    .search(&UserFilter {
                        username: Some(identifier.clone()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| LoginUserError::Unknown(error.into()))?;
                if found.is_empty() {
                    found = unit_of_work
                        .users()
                        .search(&UserFilter {
                            email: Some(identifier.clone()),
                            ..UserFilter::default()
                        })
                        .await
                        .map_err(|error| LoginUserError::Unknown(error.into()))?;
                }
                let Some(user) = found.into_iter().next() else {
                    // An identifier that resolves to no account is throttled
                    // under a derived key, so a `429` cannot reveal whether it
                    // exists.
                    if let Some(wait) =
                        attempt_limiter.check(LoginAttemptKey::Identifier(identifier))
                    {
                        return Err(LoginUserError::RateLimited(wait));
                    }
                    // Spend the same password work as a real verification on
                    // the absent-user branch, then report an unknown identifier.
                    // A backend fault surfaces as an internal error, exactly as
                    // it does on the known-user branch.
                    password_hasher
                        .hash_password(&password)
                        .await
                        .map_err(|error| LoginUserError::Unknown(error.into()))?;
                    return Err(LoginUserError::InvalidIdentifier);
                };

                // The account dimension is checked before any password work, so
                // attempts against one account are slowed across client addresses.
                if let Some(wait) = attempt_limiter.check(LoginAttemptKey::User(user.id())) {
                    return Err(LoginUserError::RateLimited(wait));
                }

                let credential = unit_of_work
                    .credentials()
                    .search(&CredentialFilter {
                        id: Some(user.credential_id()),
                        ..CredentialFilter::default()
                    })
                    .await
                    .map_err(|error| LoginUserError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or_else(|| {
                        error!(
                            target: "security",
                            event = "application_error",
                            operation = %"login_user",
                            "a use case originated an internal error"
                        );
                        LoginUserError::Unknown(anyhow::anyhow!(
                            "user {} has no credential",
                            user.id()
                        ))
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

                info!(
                    target: "security",
                    event = "authn_succeeded",
                    actor = user.id(),
                    "authenticated a user"
                );

                Ok(token)
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| LoginUserError::Unknown(error.into()))?;
                    Ok(value)
                }
                Err(error) => {
                    if matches!(
                        error,
                        LoginUserError::InvalidIdentifier | LoginUserError::InvalidPassword
                    ) {
                        // The event names its reason and nothing the client sent:
                        // no username, email address or derived value reaches a
                        // sink, and the rejection is uniform across the two
                        // branches.
                        warn!(
                            target: "security",
                            event = "authn_failed",
                            reason = "invalid_credentials",
                            "rejected an authentication attempt"
                        );
                    }
                    if let Err(_rollback_error) = unit_of_work.rollback().await {
                        // The unit-of-work adapter owns the rollback-failure
                        // log (`OBS-002`); the use case returns the original
                        // business error (`STY-RUST-031`).
                    }
                    Err(error)
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::{Credential, Role, User, UserError};

    use crate::domain::port::{
        IssuedToken, MockCredentialRepository, MockLoginAttemptLimiter, MockPasswordHasher,
        MockTokenProvider, MockUserRepository, PasswordHasherError, RepositoryError,
        TokenProviderError,
    };

    use crate::test_helpers::*;
    use chrono::NaiveDateTime;
    use std::error::Error;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;
    use tracing_test::traced_test;

    type UseCase =
        LoginUser<TestFactory, MockPasswordHasher, MockTokenProvider, MockLoginAttemptLimiter>;

    /// A use case under test together with its transaction-lifecycle flags.
    struct Harness {
        /// Set when the unit of work is committed.
        committed: Arc<AtomicBool>,
        /// Set when the unit of work is rolled back.
        rolled_back: Arc<AtomicBool>,
        /// The use case under test.
        use_case: UseCase,
    }

    fn use_case_with_limiter(
        setup: impl FnOnce(
            &mut MockUserRepository,
            &mut MockCredentialRepository,
            &mut MockPasswordHasher,
            &mut MockTokenProvider,
        ) -> Result<(), Box<dyn Error>>,
        attempt_limiter: MockLoginAttemptLimiter,
    ) -> Result<Harness, Box<dyn Error>> {
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

        let unit_of_work = TestUnitOfWork {
            users: user_repository,
            credentials: credential_repository,
            ..TestUnitOfWork::default()
        };
        let committed = Arc::clone(&unit_of_work.committed);
        let rolled_back = Arc::clone(&unit_of_work.rolled_back);
        let factory = TestFactory::new(unit_of_work);

        Ok(Harness {
            use_case: LoginUser::new(
                Arc::new(factory),
                Arc::new(password_hasher),
                Arc::new(token_provider),
                Arc::new(attempt_limiter),
            ),
            committed,
            rolled_back,
        })
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

    #[tokio::test]
    async fn login_user_valid_credentials_returns_issued_token() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                    &mut MockTokenProvider,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(
                |user_repository, credential_repository, password_hasher, token_provider| {
                    (move |user_repository: &mut MockUserRepository, user: User| {
                        user_repository
                            .expect_search()
                            .times(1)
                            .returning(move |_| {
                                let own_user = user.clone();
                                Box::pin(async move { Ok(vec![own_user]) })
                            });
                    })(user_repository, user(1, "alice")?);
                    (move |credential_repository: &mut MockCredentialRepository,
                           credential: Credential| {
                        credential_repository
                            .expect_search()
                            .times(1)
                            .returning(move |_| {
                                let own_credential = credential.clone();
                                Box::pin(async move { Ok(vec![own_credential]) })
                            });
                    })(credential_repository, credential(1)?);
                    (move |password_hasher: &mut MockPasswordHasher| {
                        password_hasher
                            .expect_verify_password()
                            .times(1)
                            .returning(|_, _| Box::pin(async { Ok(true) }));
                    })(password_hasher);
                    token_provider.expect_issue().times(1).returning(|_| {
                        Box::pin(async { Ok(IssuedToken::new("token".to_owned(), 3600)) })
                    });
                    Ok(())
                },
            );
            (move || -> Result<Harness, Box<dyn Error>> {
                use_case_with_limiter(
                    setup,
                    (move || -> MockLoginAttemptLimiter {
                        let mut limiter = MockLoginAttemptLimiter::new();
                        limiter.expect_check().returning(|_| None);
                        limiter
                    })(),
                )
            })()
        }?;
        let command = command("alice", SECRET_PASSWORD);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.value(), "token");
        assert_eq!(response.expires_in(), 3600);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn login_user_unknown_identifier_returns_invalid_identifier() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                    &mut MockTokenProvider,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|user_repository, _, password_hasher, _| {
                (move |user_repository: &mut MockUserRepository| {
                    user_repository
                        .expect_search()
                        .times(2)
                        .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                })(user_repository);
                password_hasher
                    .expect_hash_password()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok("decoy-hash".to_owned()) }));
                Ok(())
            });
            (move || -> Result<Harness, Box<dyn Error>> {
                use_case_with_limiter(
                    setup,
                    (move || -> MockLoginAttemptLimiter {
                        let mut limiter = MockLoginAttemptLimiter::new();
                        limiter.expect_check().returning(|_| None);
                        limiter
                    })(),
                )
            })()
        }?;
        let command = command("ghost", SECRET_PASSWORD);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(LoginUserError::InvalidIdentifier)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn login_user_unknown_identifier_with_failing_hasher_returns_unknown()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                    &mut MockTokenProvider,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|user_repository, _, password_hasher, _| {
                (move |user_repository: &mut MockUserRepository| {
                    user_repository
                        .expect_search()
                        .times(2)
                        .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                })(user_repository);
                password_hasher
                    .expect_hash_password()
                    .times(1)
                    .returning(|_| Box::pin(async { Err(PasswordHasherError::OperationFailed) }));
                Ok(())
            });
            (move || -> Result<Harness, Box<dyn Error>> {
                use_case_with_limiter(
                    setup,
                    (move || -> MockLoginAttemptLimiter {
                        let mut limiter = MockLoginAttemptLimiter::new();
                        limiter.expect_check().returning(|_| None);
                        limiter
                    })(),
                )
            })()
        }?;
        let command = command("ghost", SECRET_PASSWORD);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(LoginUserError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn login_user_search_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                    &mut MockTokenProvider,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|user_repository, _, _, _| {
                user_repository
                    .expect_search()
                    .times(1)
                    .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
                Ok(())
            });
            (move || -> Result<Harness, Box<dyn Error>> {
                use_case_with_limiter(
                    setup,
                    (move || -> MockLoginAttemptLimiter {
                        let mut limiter = MockLoginAttemptLimiter::new();
                        limiter.expect_check().returning(|_| None);
                        limiter
                    })(),
                )
            })()
        }?;
        let command = command("alice", SECRET_PASSWORD);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(LoginUserError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn login_user_missing_credential_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                    &mut MockTokenProvider,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|user_repository, credential_repository, _, _| {
                (move |user_repository: &mut MockUserRepository, user: User| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(move |_| {
                            let own_user = user.clone();
                            Box::pin(async move { Ok(vec![own_user]) })
                        });
                })(user_repository, user(1, "alice")?);
                credential_repository
                    .expect_search()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                Ok(())
            });
            (move || -> Result<Harness, Box<dyn Error>> {
                use_case_with_limiter(
                    setup,
                    (move || -> MockLoginAttemptLimiter {
                        let mut limiter = MockLoginAttemptLimiter::new();
                        limiter.expect_check().returning(|_| None);
                        limiter
                    })(),
                )
            })()
        }?;
        let command = command("alice", SECRET_PASSWORD);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(LoginUserError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn login_user_wrong_password_returns_invalid_password() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                    &mut MockTokenProvider,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(
                |user_repository, credential_repository, password_hasher, _| {
                    (move |user_repository: &mut MockUserRepository, user: User| {
                        user_repository
                            .expect_search()
                            .times(1)
                            .returning(move |_| {
                                let own_user = user.clone();
                                Box::pin(async move { Ok(vec![own_user]) })
                            });
                    })(user_repository, user(1, "alice")?);
                    (move |credential_repository: &mut MockCredentialRepository,
                           credential: Credential| {
                        credential_repository
                            .expect_search()
                            .times(1)
                            .returning(move |_| {
                                let own_credential = credential.clone();
                                Box::pin(async move { Ok(vec![own_credential]) })
                            });
                    })(credential_repository, credential(1)?);
                    password_hasher
                        .expect_verify_password()
                        .times(1)
                        .returning(|_, _| Box::pin(async { Ok(false) }));
                    Ok(())
                },
            );
            (move || -> Result<Harness, Box<dyn Error>> {
                use_case_with_limiter(
                    setup,
                    (move || -> MockLoginAttemptLimiter {
                        let mut limiter = MockLoginAttemptLimiter::new();
                        limiter.expect_check().returning(|_| None);
                        limiter
                    })(),
                )
            })()
        }?;
        let command = command("alice", "wrong-password");

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(LoginUserError::InvalidPassword)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn login_user_password_verification_failure_returns_unknown() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                    &mut MockTokenProvider,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(
                |user_repository, credential_repository, password_hasher, _| {
                    (move |user_repository: &mut MockUserRepository, user: User| {
                        user_repository
                            .expect_search()
                            .times(1)
                            .returning(move |_| {
                                let own_user = user.clone();
                                Box::pin(async move { Ok(vec![own_user]) })
                            });
                    })(user_repository, user(1, "alice")?);
                    (move |credential_repository: &mut MockCredentialRepository,
                           credential: Credential| {
                        credential_repository
                            .expect_search()
                            .times(1)
                            .returning(move |_| {
                                let own_credential = credential.clone();
                                Box::pin(async move { Ok(vec![own_credential]) })
                            });
                    })(credential_repository, credential(1)?);
                    password_hasher
                        .expect_verify_password()
                        .times(1)
                        .returning(|_, _| {
                            Box::pin(async { Err(PasswordHasherError::OperationFailed) })
                        });
                    Ok(())
                },
            );
            (move || -> Result<Harness, Box<dyn Error>> {
                use_case_with_limiter(
                    setup,
                    (move || -> MockLoginAttemptLimiter {
                        let mut limiter = MockLoginAttemptLimiter::new();
                        limiter.expect_check().returning(|_| None);
                        limiter
                    })(),
                )
            })()
        }?;
        let command = command("alice", SECRET_PASSWORD);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(LoginUserError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn login_user_token_issuance_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                    &mut MockTokenProvider,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(
                |user_repository, credential_repository, password_hasher, token_provider| {
                    (move |user_repository: &mut MockUserRepository, user: User| {
                        user_repository
                            .expect_search()
                            .times(1)
                            .returning(move |_| {
                                let own_user = user.clone();
                                Box::pin(async move { Ok(vec![own_user]) })
                            });
                    })(user_repository, user(1, "alice")?);
                    (move |credential_repository: &mut MockCredentialRepository,
                           credential: Credential| {
                        credential_repository
                            .expect_search()
                            .times(1)
                            .returning(move |_| {
                                let own_credential = credential.clone();
                                Box::pin(async move { Ok(vec![own_credential]) })
                            });
                    })(credential_repository, credential(1)?);
                    (move |password_hasher: &mut MockPasswordHasher| {
                        password_hasher
                            .expect_verify_password()
                            .times(1)
                            .returning(|_, _| Box::pin(async { Ok(true) }));
                    })(password_hasher);
                    token_provider.expect_issue().times(1).returning(|_| {
                        Box::pin(async { Err(TokenProviderError::OperationFailed) })
                    });
                    Ok(())
                },
            );
            (move || -> Result<Harness, Box<dyn Error>> {
                use_case_with_limiter(
                    setup,
                    (move || -> MockLoginAttemptLimiter {
                        let mut limiter = MockLoginAttemptLimiter::new();
                        limiter.expect_check().returning(|_| None);
                        limiter
                    })(),
                )
            })()
        }?;
        let command = command("alice", SECRET_PASSWORD);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(LoginUserError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn login_user_valid_credentials_emits_authn_succeeded() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                    &mut MockTokenProvider,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(
                |user_repository, credential_repository, password_hasher, token_provider| {
                    (move |user_repository: &mut MockUserRepository, user: User| {
                        user_repository
                            .expect_search()
                            .times(1)
                            .returning(move |_| {
                                let own_user = user.clone();
                                Box::pin(async move { Ok(vec![own_user]) })
                            });
                    })(user_repository, user(1, "alice")?);
                    (move |credential_repository: &mut MockCredentialRepository,
                           credential: Credential| {
                        credential_repository
                            .expect_search()
                            .times(1)
                            .returning(move |_| {
                                let own_credential = credential.clone();
                                Box::pin(async move { Ok(vec![own_credential]) })
                            });
                    })(credential_repository, credential(1)?);
                    (move |password_hasher: &mut MockPasswordHasher| {
                        password_hasher
                            .expect_verify_password()
                            .times(1)
                            .returning(|_, _| Box::pin(async { Ok(true) }));
                    })(password_hasher);
                    token_provider.expect_issue().times(1).returning(|_| {
                        Box::pin(async { Ok(IssuedToken::new("token".to_owned(), 3600)) })
                    });
                    Ok(())
                },
            );
            (move || -> Result<Harness, Box<dyn Error>> {
                use_case_with_limiter(
                    setup,
                    (move || -> MockLoginAttemptLimiter {
                        let mut limiter = MockLoginAttemptLimiter::new();
                        limiter.expect_check().returning(|_| None);
                        limiter
                    })(),
                )
            })()
        }?;
        let command = command("alice", SECRET_PASSWORD);

        // Act
        assert!(harness.use_case.execute(command).await.is_ok());

        // Assert
        assert!(logs_contain("INFO"), "expected an info event");
        assert!(
            logs_contain("security:"),
            "the event must name the security target"
        );
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("event=\"authn_succeeded\""))
                .count();
            (count == 1).then_some(()).ok_or_else(|| {
                format!("expected exactly one authn_succeeded event (found {count})")
            })
        });
        logs_assert(|lines| {
            let count = lines.iter().filter(|line| line.contains("actor=1")).count();
            (count == 1)
                .then_some(())
                .ok_or_else(|| format!("expected the actor field (found {count})"))
        });
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn login_user_wrong_password_emits_authn_failed() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                    &mut MockTokenProvider,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(
                |user_repository, credential_repository, password_hasher, _| {
                    (move |user_repository: &mut MockUserRepository, user: User| {
                        user_repository
                            .expect_search()
                            .times(1)
                            .returning(move |_| {
                                let own_user = user.clone();
                                Box::pin(async move { Ok(vec![own_user]) })
                            });
                    })(user_repository, user(1, "alice")?);
                    (move |credential_repository: &mut MockCredentialRepository,
                           credential: Credential| {
                        credential_repository
                            .expect_search()
                            .times(1)
                            .returning(move |_| {
                                let own_credential = credential.clone();
                                Box::pin(async move { Ok(vec![own_credential]) })
                            });
                    })(credential_repository, credential(1)?);
                    password_hasher
                        .expect_verify_password()
                        .times(1)
                        .returning(|_, _| Box::pin(async { Ok(false) }));
                    Ok(())
                },
            );
            (move || -> Result<Harness, Box<dyn Error>> {
                use_case_with_limiter(
                    setup,
                    (move || -> MockLoginAttemptLimiter {
                        let mut limiter = MockLoginAttemptLimiter::new();
                        limiter.expect_check().returning(|_| None);
                        limiter
                    })(),
                )
            })()
        }?;
        let command = command("alice", "wrong-password");

        // Act
        assert!(harness.use_case.execute(command).await.is_err());

        // Assert
        assert!(logs_contain("WARN"), "expected a warn event");
        assert!(
            logs_contain("security:"),
            "the event must name the security target"
        );
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("event=\"authn_failed\""))
                .count();
            (count == 1)
                .then_some(())
                .ok_or_else(|| format!("expected exactly one authn_failed event (found {count})"))
        });
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("reason=\"invalid_credentials\""))
                .count();
            (count == 1)
                .then_some(())
                .ok_or_else(|| format!("expected the uniform reason (found {count})"))
        });
        assert!(
            !logs_contain("alice"),
            "the submitted identifier must never reach the sink"
        );
        assert!(
            !logs_contain("claimed_identity"),
            "the event carries no client-supplied identity"
        );
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn login_user_unknown_identifier_emits_uniform_authn_failed() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                    &mut MockTokenProvider,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|user_repository, _, password_hasher, _| {
                (move |user_repository: &mut MockUserRepository| {
                    user_repository
                        .expect_search()
                        .times(2)
                        .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                })(user_repository);
                password_hasher
                    .expect_hash_password()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok("decoy-hash".to_owned()) }));
                Ok(())
            });
            (move || -> Result<Harness, Box<dyn Error>> {
                use_case_with_limiter(
                    setup,
                    (move || -> MockLoginAttemptLimiter {
                        let mut limiter = MockLoginAttemptLimiter::new();
                        limiter.expect_check().returning(|_| None);
                        limiter
                    })(),
                )
            })()
        }?;
        let command = command("ghost", SECRET_PASSWORD);

        // Act
        assert!(harness.use_case.execute(command).await.is_err());

        // Assert
        assert!(logs_contain("WARN"), "expected a warn event");
        assert!(
            logs_contain("security:"),
            "the event must name the security target"
        );
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("event=\"authn_failed\""))
                .count();
            (count == 1)
                .then_some(())
                .ok_or_else(|| format!("expected exactly one authn_failed event (found {count})"))
        });
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("reason=\"invalid_credentials\""))
                .count();
            (count == 1)
                .then_some(())
                .ok_or_else(|| format!("expected the uniform reason (found {count})"))
        });
        assert!(
            !logs_contain("ghost"),
            "the submitted identifier must never reach the sink"
        );
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn login_user_unknown_identifier_never_reaches_a_sink() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                    &mut MockTokenProvider,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|user_repository, _, password_hasher, _| {
                (move |user_repository: &mut MockUserRepository| {
                    user_repository
                        .expect_search()
                        .times(2)
                        .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                })(user_repository);
                password_hasher
                    .expect_hash_password()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok("decoy-hash".to_owned()) }));
                Ok(())
            });
            (move || -> Result<Harness, Box<dyn Error>> {
                use_case_with_limiter(
                    setup,
                    (move || -> MockLoginAttemptLimiter {
                        let mut limiter = MockLoginAttemptLimiter::new();
                        limiter.expect_check().returning(|_| None);
                        limiter
                    })(),
                )
            })()
        }?;
        let command = command("bad\nname", SECRET_PASSWORD);

        // Act
        assert!(harness.use_case.execute(command).await.is_err());

        // Assert
        assert!(
            !logs_contain("bad"),
            "the submitted identifier must never reach the sink"
        );
        assert!(
            !logs_contain("claimed_identity"),
            "the event carries no client-supplied identity"
        );
        logs_assert(|lines| {
            (lines.len() == 1).then_some(()).ok_or_else(|| {
                format!(
                    "a client value must not forge a second log record (found {})",
                    lines.len()
                )
            })
        });
        Ok(())
    }

    // proves: SCN-LOGGING-014-C — Error handling
    #[tokio::test]
    #[traced_test]
    async fn login_user_missing_credential_emits_application_error() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                    &mut MockTokenProvider,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|user_repository, credential_repository, _, _| {
                (move |user_repository: &mut MockUserRepository, user: User| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(move |_| {
                            let own_user = user.clone();
                            Box::pin(async move { Ok(vec![own_user]) })
                        });
                })(user_repository, user(1, "alice")?);
                credential_repository
                    .expect_search()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                Ok(())
            });
            (move || -> Result<Harness, Box<dyn Error>> {
                use_case_with_limiter(
                    setup,
                    (move || -> MockLoginAttemptLimiter {
                        let mut limiter = MockLoginAttemptLimiter::new();
                        limiter.expect_check().returning(|_| None);
                        limiter
                    })(),
                )
            })()
        }?;
        let command = command("alice", SECRET_PASSWORD);

        // Act
        assert!(harness.use_case.execute(command).await.is_err());

        // Assert
        assert!(logs_contain("ERROR"), "expected an error event");
        assert!(
            logs_contain("security:"),
            "the event must name the security target"
        );
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("event=\"application_error\""))
                .count();
            (count == 1).then_some(()).ok_or_else(|| {
                format!("expected exactly one application_error event (found {count})")
            })
        });
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("operation=login_user"))
                .count();
            (count == 1)
                .then_some(())
                .ok_or_else(|| format!("expected the operation field (found {count})"))
        });
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn login_user_port_failure_emits_no_authn_event() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                    &mut MockTokenProvider,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(|user_repository, _, _, _| {
                user_repository
                    .expect_search()
                    .times(1)
                    .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
                Ok(())
            });
            (move || -> Result<Harness, Box<dyn Error>> {
                use_case_with_limiter(
                    setup,
                    (move || -> MockLoginAttemptLimiter {
                        let mut limiter = MockLoginAttemptLimiter::new();
                        limiter.expect_check().returning(|_| None);
                        limiter
                    })(),
                )
            })()
        }?;
        let command = command("alice", SECRET_PASSWORD);

        // Act
        assert!(harness.use_case.execute(command).await.is_err());

        // Assert
        assert!(
            !logs_contain("event="),
            "a port failure is owned by the adapter"
        );
        Ok(())
    }

    #[tokio::test]
    async fn login_user_exhausted_account_quota_returns_rate_limited() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the account's quota is spent before any password work runs.
        let mut attempt_limiter = MockLoginAttemptLimiter::new();
        attempt_limiter.expect_check().times(1).returning(|key| {
            assert_eq!(key, LoginAttemptKey::User(1));
            Some(Duration::from_secs(5))
        });
        let harness = use_case_with_limiter(
            |user_repository, _, _, _| {
                (move |user_repository: &mut MockUserRepository, user: User| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .returning(move |_| {
                            let own_user = user.clone();
                            Box::pin(async move { Ok(vec![own_user]) })
                        });
                })(user_repository, user(1, "alice")?);
                Ok(())
            },
            attempt_limiter,
        )?;
        let command = command("alice", SECRET_PASSWORD);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(LoginUserError::RateLimited(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn login_user_by_email_address_succeeds() -> Result<(), Box<dyn Error>> {
        // Arrange: the username lookup finds nothing, so the email lookup runs.
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                    &mut MockTokenProvider,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(
                |user_repository, credential_repository, password_hasher, token_provider| {
                    user_repository
                        .expect_search()
                        .times(1)
                        .withf(|filter| filter.username.is_some() && filter.email.is_none())
                        .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                    (move |user_repository: &mut MockUserRepository, found: Vec<User>| {
                        user_repository
                            .expect_search()
                            .times(1)
                            .withf(|filter| filter.username.is_none() && filter.email.is_some())
                            .returning(move |_| {
                                let own = found.clone();
                                Box::pin(async move { Ok(own) })
                            });
                    })(user_repository, vec![user(1, "alice")?]);
                    (move |credential_repository: &mut MockCredentialRepository,
                           credential: Credential| {
                        credential_repository
                            .expect_search()
                            .times(1)
                            .returning(move |_| {
                                let own_credential = credential.clone();
                                Box::pin(async move { Ok(vec![own_credential]) })
                            });
                    })(credential_repository, credential(1)?);
                    (move |password_hasher: &mut MockPasswordHasher| {
                        password_hasher
                            .expect_verify_password()
                            .times(1)
                            .returning(|_, _| Box::pin(async { Ok(true) }));
                    })(password_hasher);
                    token_provider.expect_issue().times(1).returning(|_| {
                        Box::pin(async { Ok(IssuedToken::new("token".to_owned(), 3600)) })
                    });
                    Ok(())
                },
            );
            (move || -> Result<Harness, Box<dyn Error>> {
                use_case_with_limiter(
                    setup,
                    (move || -> MockLoginAttemptLimiter {
                        let mut limiter = MockLoginAttemptLimiter::new();
                        limiter.expect_check().returning(|_| None);
                        limiter
                    })(),
                )
            })()
        }?;
        let command = command("alice@example.com", SECRET_PASSWORD);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.value(), "token");
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn login_user_identifier_matching_a_username_wins_over_an_email()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the username lookup returns an account, so the email lookup
        // must not run even though another account's address might match.
        let harness = {
            let setup: Box<
                dyn FnOnce(
                    &mut MockUserRepository,
                    &mut MockCredentialRepository,
                    &mut MockPasswordHasher,
                    &mut MockTokenProvider,
                ) -> Result<(), Box<dyn Error>>,
            > = Box::new(
                |user_repository, credential_repository, password_hasher, token_provider| {
                    let found = vec![user(1, "alice")?];
                    user_repository
                        .expect_search()
                        .times(1)
                        .withf(|filter| filter.username.is_some() && filter.email.is_none())
                        .returning(move |_| {
                            let existing = found.clone();
                            Box::pin(async move { Ok(existing) })
                        });
                    (move |credential_repository: &mut MockCredentialRepository,
                           credential: Credential| {
                        credential_repository
                            .expect_search()
                            .times(1)
                            .returning(move |_| {
                                let own_credential = credential.clone();
                                Box::pin(async move { Ok(vec![own_credential]) })
                            });
                    })(credential_repository, credential(1)?);
                    (move |password_hasher: &mut MockPasswordHasher| {
                        password_hasher
                            .expect_verify_password()
                            .times(1)
                            .returning(|_, _| Box::pin(async { Ok(true) }));
                    })(password_hasher);
                    token_provider.expect_issue().times(1).returning(|_| {
                        Box::pin(async { Ok(IssuedToken::new("token".to_owned(), 3600)) })
                    });
                    Ok(())
                },
            );
            (move || -> Result<Harness, Box<dyn Error>> {
                use_case_with_limiter(
                    setup,
                    (move || -> MockLoginAttemptLimiter {
                        let mut limiter = MockLoginAttemptLimiter::new();
                        limiter.expect_check().returning(|_| None);
                        limiter
                    })(),
                )
            })()
        }?;
        let command = command("alice", SECRET_PASSWORD);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.value(), "token");
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn login_user_exhausted_unknown_identifier_quota_returns_rate_limited()
    -> Result<(), Box<dyn Error>> {
        // Arrange: an unknown identifier is counted under its own derived key.
        let mut attempt_limiter = MockLoginAttemptLimiter::new();
        attempt_limiter.expect_check().times(1).returning(|key| {
            assert!(matches!(key, LoginAttemptKey::Identifier(_)));
            Some(Duration::from_secs(5))
        });
        let harness = use_case_with_limiter(
            |user_repository, _, _, _| {
                (move |user_repository: &mut MockUserRepository| {
                    user_repository
                        .expect_search()
                        .times(2)
                        .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                })(user_repository);
                Ok(())
            },
            attempt_limiter,
        )?;
        let command = command("ghost", SECRET_PASSWORD);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(LoginUserError::RateLimited(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
