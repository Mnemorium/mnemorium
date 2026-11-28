use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use tracing::{info, warn};

use crate::application::port::{SetupRootAdminCommand, SetupRootAdminError, SetupRootAdminUseCase};
use crate::domain::model::{Credential, User};
use crate::domain::port::{
    CredentialRepository as _, IdentityUnitOfWork, PasswordHasher, RepositoryError,
    SetupAttemptLimiter, SetupGrantRepository as _, UnitOfWork as _, UnitOfWorkFactory, UserFilter,
    UserRepository as _, UserUnitOfWork,
};
use crate::domain::service::{PasswordPolicy, SetupTokenDeriver};

/// Use case implementation for exchanging the setup token for the Root Admin
/// credential.
pub(crate) struct SetupRootAdmin<F, H, L: ?Sized> {
    /// Hasher for user passwords.
    password_hasher: Arc<H>,
    /// Instance pepper the setup token is derived from.
    pepper: String,
    /// Process-wide setup-attempt limiter.
    setup_attempt_limiter: Arc<L>,
    /// Factory opening the unit of work wrapping the exchange.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, H: PasswordHasher, L: SetupAttemptLimiter + ?Sized>
    SetupRootAdmin<F, H, L>
{
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(
        unit_of_work_factory: Arc<F>,
        password_hasher: Arc<H>,
        setup_attempt_limiter: Arc<L>,
        pepper: String,
    ) -> Self {
        Self {
            password_hasher,
            pepper,
            setup_attempt_limiter,
            unit_of_work_factory,
        }
    }
}

impl<F, H, L> SetupRootAdminUseCase for SetupRootAdmin<F, H, L>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + UserUnitOfWork,
    H: PasswordHasher,
    L: SetupAttemptLimiter + ?Sized,
{
    fn execute<'future>(
        &'future self,
        command: SetupRootAdminCommand,
    ) -> Pin<Box<dyn Future<Output = Result<(), SetupRootAdminError>> + Send + 'future>> {
        let password_hasher = Arc::clone(&self.password_hasher);
        let pepper = self.pepper.clone();
        let setup_attempt_limiter = Arc::clone(&self.setup_attempt_limiter);
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| SetupRootAdminError::Unknown(error.into()))?;

            let result = async {
                if let Some(wait) = setup_attempt_limiter.check() {
                    return Err(SetupRootAdminError::RateLimited { wait });
                }

                let now = Utc::now().naive_utc();
                let grant = unit_of_work
                    .setup_grants()
                    .find()
                    .await
                    .map_err(|error| SetupRootAdminError::Unknown(error.into()))?
                    .ok_or(SetupRootAdminError::InvalidToken)?;
                if !grant.is_live(now)
                    || !SetupTokenDeriver::verify(&pepper, grant.nonce(), command.token())
                {
                    return Err(SetupRootAdminError::InvalidToken);
                }

                if PasswordPolicy::new().validate(command.password()).is_err() {
                    return Err(SetupRootAdminError::WeakPassword);
                }

                let email = command.email().map(str::trim).map(str::to_owned);
                if let Some(address) = email.as_deref() {
                    User::validate_email(Some(address.to_owned()))
                        .map_err(|_| SetupRootAdminError::InvalidEmail)?;
                    let users = unit_of_work
                        .users()
                        .search(&UserFilter::default())
                        .await
                        .map_err(|error| SetupRootAdminError::Unknown(error.into()))?;
                    if users.iter().any(|user| {
                        user.id() != 0
                            && user
                                .email()
                                .is_some_and(|held| held.eq_ignore_ascii_case(address))
                    }) {
                        return Err(SetupRootAdminError::EmailTaken);
                    }
                }

                let root = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(0),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| SetupRootAdminError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or_else(|| {
                        SetupRootAdminError::Unknown(anyhow::anyhow!(
                            "the Root Admin account does not exist"
                        ))
                    })?;

                let password_hash = password_hasher
                    .hash_password(command.password())
                    .await
                    .map_err(|error| SetupRootAdminError::Unknown(error.into()))?;
                let credential = Credential::try_new(root.credential_id(), password_hash, now)
                    .map_err(|error| SetupRootAdminError::Unknown(error.into()))?;
                unit_of_work
                    .credentials()
                    .save(credential)
                    .await
                    .map_err(|error| SetupRootAdminError::Unknown(error.into()))?;

                if let Some(address) = email {
                    let mut updated_root = root;
                    updated_root
                        .set_email(Some(address))
                        .map_err(|_| SetupRootAdminError::InvalidEmail)?;
                    unit_of_work
                        .users()
                        .save(updated_root)
                        .await
                        .map_err(|error| {
                            if matches!(&error, RepositoryError::AlreadyExist) {
                                SetupRootAdminError::EmailTaken
                            } else {
                                SetupRootAdminError::Unknown(error.into())
                            }
                        })?;
                }

                // The consume is last, so a failed write above leaves the grant
                // usable and a lost race cannot leave a live token.
                let consumed = unit_of_work
                    .setup_grants()
                    .consume(now)
                    .await
                    .map_err(|error| SetupRootAdminError::Unknown(error.into()))?;
                if !consumed {
                    return Err(SetupRootAdminError::InvalidToken);
                }

                Ok(())
            }
            .await;

            match result {
                Ok(()) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| SetupRootAdminError::Unknown(error.into()))?;
                    info!(
                        target: "security",
                        event = "credential_rotated",
                        actor = 0i64,
                        "replaced a credential"
                    );
                    Ok(())
                }
                Err(error) => {
                    if matches!(error, SetupRootAdminError::InvalidToken) {
                        warn!(
                            target: "security",
                            event = "authn_failed",
                            reason = "invalid_setup_token",
                            "rejected a setup attempt"
                        );
                    }
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

    use crate::domain::model::{Role, SetupGrant, UserError};
    use crate::domain::port::{
        MockCredentialRepository, MockPasswordHasher, MockSetupAttemptLimiter,
        MockSetupGrantRepository, MockUserRepository, PasswordHasherError,
    };

    use crate::test_helpers::*;
    use chrono::{Duration as ChronoDuration, NaiveDateTime};
    use std::error::Error;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;
    use tracing_test::traced_test;

    type UseCase = SetupRootAdmin<TestFactory, MockPasswordHasher, MockSetupAttemptLimiter>;

    /// A use case under test together with its transaction-lifecycle flags.
    struct Harness {
        /// Set when the unit of work is committed.
        committed: Arc<AtomicBool>,
        /// Set when the unit of work is rolled back.
        rolled_back: Arc<AtomicBool>,
        /// The use case under test.
        use_case: UseCase,
    }

    fn user(id: i64, username: &str, email: Option<&str>) -> Result<User, UserError> {
        User::try_new(
            id,
            username.to_owned(),
            email.map(str::to_owned),
            1,
            Role::Admin,
        )
    }

    fn grant(
        nonce: &str,
        expires_in: i64,
        consumed: Option<NaiveDateTime>,
    ) -> Result<SetupGrant, Box<dyn Error>> {
        let expires_at = Utc::now()
            .naive_utc()
            .checked_add_signed(ChronoDuration::seconds(expires_in))
            .ok_or("the test expiry is not representable")?;
        Ok(SetupGrant::from_stored(
            0,
            nonce.to_owned(),
            expires_at,
            consumed,
        )?)
    }

    fn token_for(nonce: &str) -> String {
        SetupTokenDeriver::display(&SetupTokenDeriver::derive(TEST_PEPPER, nonce))
    }

    fn use_case_with(
        setup: impl FnOnce(
            &mut MockUserRepository,
            &mut MockCredentialRepository,
            &mut MockSetupGrantRepository,
            &mut MockPasswordHasher,
        ) -> Result<(), Box<dyn Error>>,
        limiter: MockSetupAttemptLimiter,
    ) -> Result<Harness, Box<dyn Error>> {
        let mut user_repository = MockUserRepository::new();
        let mut credential_repository = MockCredentialRepository::new();
        let mut setup_grant_repository = MockSetupGrantRepository::new();
        let mut password_hasher = MockPasswordHasher::new();

        setup(
            &mut user_repository,
            &mut credential_repository,
            &mut setup_grant_repository,
            &mut password_hasher,
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
            use_case: SetupRootAdmin::new(
                Arc::new(factory),
                Arc::new(password_hasher),
                Arc::new(limiter),
                TEST_PEPPER.to_owned(),
            ),
            committed,
            rolled_back,
        })
    }

    fn command(token: &str, password: &str, email: Option<&str>) -> SetupRootAdminCommand {
        SetupRootAdminCommand::new(
            email.map(str::to_owned),
            password.to_owned(),
            token.to_owned(),
        )
    }

    #[tokio::test]
    async fn setup_root_admin_live_token_replaces_the_credential() -> Result<(), Box<dyn Error>> {
        // Arrange
        let nonce = "a".repeat(32);
        let token = token_for(&nonce);
        let harness = use_case_with(
            |user_repository, credential_repository, setup_grant_repository, password_hasher| {
                (move |repository: &mut MockSetupGrantRepository, nonce: &str| {
                    let pending = grant(nonce, 60, None).ok();
                    repository.expect_find().times(1).returning(move || {
                        let found = pending.clone();
                        Box::pin(async move { Ok(found) })
                    });
                })(setup_grant_repository, &nonce);
                (move |user_repository: &mut MockUserRepository| {
                    let root = user(0, "root", None).ok();
                    user_repository
                        .expect_search()
                        .times(1)
                        .withf(|filter| filter.id == Some(0))
                        .returning(move |_| {
                            let found = root.clone();
                            Box::pin(async move { Ok(found.into_iter().collect()) })
                        });
                })(user_repository);
                password_hasher
                    .expect_hash_password()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok("hashed".to_owned()) }));
                credential_repository
                    .expect_save()
                    .times(1)
                    .withf(|credential| credential.id() == 1)
                    .returning(|credential| Box::pin(async { Ok(credential) }));
                setup_grant_repository
                    .expect_consume()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok(true) }));
                Ok(())
            },
            (move || {
                let mut limiter = MockSetupAttemptLimiter::new();
                limiter.expect_check().returning(|| None);
                limiter
            })(),
        )?;

        // Act
        harness
            .use_case
            .execute(command(&token, SECRET_PASSWORD, None))
            .await?;

        // Assert
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn setup_root_admin_stores_the_optional_email() -> Result<(), Box<dyn Error>> {
        // Arrange
        let nonce = "b".repeat(32);
        let token = token_for(&nonce);
        let harness = use_case_with(
            |user_repository, credential_repository, setup_grant_repository, password_hasher| {
                (move |repository: &mut MockSetupGrantRepository, nonce: &str| {
                    let pending = grant(nonce, 60, None).ok();
                    repository.expect_find().times(1).returning(move || {
                        let found = pending.clone();
                        Box::pin(async move { Ok(found) })
                    });
                })(setup_grant_repository, &nonce);
                (move |user_repository: &mut MockUserRepository| {
                    let root = user(0, "root", None).ok();
                    user_repository
                        .expect_search()
                        .times(1)
                        .withf(|filter| filter.id == Some(0))
                        .returning(move |_| {
                            let found = root.clone();
                            Box::pin(async move { Ok(found.into_iter().collect()) })
                        });
                })(user_repository);
                user_repository
                    .expect_search()
                    .times(1)
                    .withf(|filter| {
                        filter.email.is_none()
                            && filter.id.is_none()
                            && filter.role.is_none()
                            && filter.username.is_none()
                    })
                    .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                user_repository
                    .expect_save()
                    .times(1)
                    .withf(|root| root.email() == Some("root@example.com"))
                    .returning(|root| Box::pin(async { Ok(root) }));
                password_hasher
                    .expect_hash_password()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok("hashed".to_owned()) }));
                credential_repository
                    .expect_save()
                    .times(1)
                    .returning(|credential| Box::pin(async { Ok(credential) }));
                setup_grant_repository
                    .expect_consume()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok(true) }));
                Ok(())
            },
            (move || {
                let mut limiter = MockSetupAttemptLimiter::new();
                limiter.expect_check().returning(|| None);
                limiter
            })(),
        )?;

        // Act
        harness
            .use_case
            .execute(command(&token, SECRET_PASSWORD, Some("root@example.com")))
            .await?;

        // Assert
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn setup_root_admin_wrong_token_is_refused_uniformly() -> Result<(), Box<dyn Error>> {
        // Arrange
        let nonce = "c".repeat(32);
        let harness = use_case_with(
            |_, _, setup_grant_repository, _| {
                (move |repository: &mut MockSetupGrantRepository, nonce: &str| {
                    let pending = grant(nonce, 60, None).ok();
                    repository.expect_find().times(1).returning(move || {
                        let found = pending.clone();
                        Box::pin(async move { Ok(found) })
                    });
                })(setup_grant_repository, &nonce);
                Ok(())
            },
            (move || {
                let mut limiter = MockSetupAttemptLimiter::new();
                limiter.expect_check().returning(|| None);
                limiter
            })(),
        )?;

        // Act
        let result = harness
            .use_case
            .execute(command("0000-0000", SECRET_PASSWORD, None))
            .await;

        // Assert
        assert!(matches!(result, Err(SetupRootAdminError::InvalidToken)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn setup_root_admin_expired_token_is_refused() -> Result<(), Box<dyn Error>> {
        // Arrange
        let nonce = "d".repeat(32);
        let token = token_for(&nonce);
        let expired = grant(&nonce, -60, None)?;
        let harness = use_case_with(
            |_, _, setup_grant_repository, _| {
                setup_grant_repository
                    .expect_find()
                    .times(1)
                    .returning(move || {
                        let found = expired.clone();
                        Box::pin(async move { Ok(Some(found)) })
                    });
                Ok(())
            },
            (move || {
                let mut limiter = MockSetupAttemptLimiter::new();
                limiter.expect_check().returning(|| None);
                limiter
            })(),
        )?;

        // Act
        let result = harness
            .use_case
            .execute(command(&token, SECRET_PASSWORD, None))
            .await;

        // Assert
        assert!(matches!(result, Err(SetupRootAdminError::InvalidToken)));
        Ok(())
    }

    #[tokio::test]
    async fn setup_root_admin_consumed_token_is_refused() -> Result<(), Box<dyn Error>> {
        // Arrange
        let nonce = "e".repeat(32);
        let token = token_for(&nonce);
        let consumed = grant(&nonce, 60, Some(Utc::now().naive_utc()))?;
        let harness = use_case_with(
            |_, _, setup_grant_repository, _| {
                setup_grant_repository
                    .expect_find()
                    .times(1)
                    .returning(move || {
                        let found = consumed.clone();
                        Box::pin(async move { Ok(Some(found)) })
                    });
                Ok(())
            },
            (move || {
                let mut limiter = MockSetupAttemptLimiter::new();
                limiter.expect_check().returning(|| None);
                limiter
            })(),
        )?;

        // Act
        let result = harness
            .use_case
            .execute(command(&token, SECRET_PASSWORD, None))
            .await;

        // Assert
        assert!(matches!(result, Err(SetupRootAdminError::InvalidToken)));
        Ok(())
    }

    #[tokio::test]
    async fn setup_root_admin_absent_grant_is_refused() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(
            |_, _, setup_grant_repository, _| {
                setup_grant_repository
                    .expect_find()
                    .times(1)
                    .returning(|| Box::pin(async { Ok(None) }));
                Ok(())
            },
            (move || {
                let mut limiter = MockSetupAttemptLimiter::new();
                limiter.expect_check().returning(|| None);
                limiter
            })(),
        )?;

        // Act
        let result = harness
            .use_case
            .execute(command("0000-0000", SECRET_PASSWORD, None))
            .await;

        // Assert
        assert!(matches!(result, Err(SetupRootAdminError::InvalidToken)));
        Ok(())
    }

    #[tokio::test]
    async fn setup_root_admin_weak_password_leaves_the_grant_usable() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let nonce = "f".repeat(32);
        let token = token_for(&nonce);
        let harness = use_case_with(
            |_, _, setup_grant_repository, _| {
                (move |repository: &mut MockSetupGrantRepository, nonce: &str| {
                    let pending = grant(nonce, 60, None).ok();
                    repository.expect_find().times(1).returning(move || {
                        let found = pending.clone();
                        Box::pin(async move { Ok(found) })
                    });
                })(setup_grant_repository, &nonce);
                Ok(())
            },
            (move || {
                let mut limiter = MockSetupAttemptLimiter::new();
                limiter.expect_check().returning(|| None);
                limiter
            })(),
        )?;

        // Act
        let result = harness
            .use_case
            .execute(command(&token, "short", None))
            .await;

        // Assert
        assert!(matches!(result, Err(SetupRootAdminError::WeakPassword)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn setup_root_admin_taken_email_changes_nothing() -> Result<(), Box<dyn Error>> {
        // Arrange
        let nonce = "g".repeat(32);
        let token = token_for(&nonce);
        let harness = use_case_with(
            |user_repository, _, setup_grant_repository, _| {
                (move |repository: &mut MockSetupGrantRepository, nonce: &str| {
                    let pending = grant(nonce, 60, None).ok();
                    repository.expect_find().times(1).returning(move || {
                        let found = pending.clone();
                        Box::pin(async move { Ok(found) })
                    });
                })(setup_grant_repository, &nonce);
                let holder = user(1, "holder", Some("taken@example.com")).ok();
                user_repository
                    .expect_search()
                    .times(1)
                    .returning(move |_| {
                        let found = holder.clone();
                        Box::pin(async move { Ok(found.into_iter().collect()) })
                    });
                Ok(())
            },
            (move || {
                let mut limiter = MockSetupAttemptLimiter::new();
                limiter.expect_check().returning(|| None);
                limiter
            })(),
        )?;

        // Act
        let result = harness
            .use_case
            .execute(command(&token, SECRET_PASSWORD, Some("TAKEN@example.com")))
            .await;

        // Assert
        assert!(matches!(result, Err(SetupRootAdminError::EmailTaken)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn setup_root_admin_invalid_email_is_refused() -> Result<(), Box<dyn Error>> {
        // Arrange
        let nonce = "h".repeat(32);
        let token = token_for(&nonce);
        let harness = use_case_with(
            |_, _, setup_grant_repository, _| {
                (move |repository: &mut MockSetupGrantRepository, nonce: &str| {
                    let pending = grant(nonce, 60, None).ok();
                    repository.expect_find().times(1).returning(move || {
                        let found = pending.clone();
                        Box::pin(async move { Ok(found) })
                    });
                })(setup_grant_repository, &nonce);
                Ok(())
            },
            (move || {
                let mut limiter = MockSetupAttemptLimiter::new();
                limiter.expect_check().returning(|| None);
                limiter
            })(),
        )?;

        // Act
        let result = harness
            .use_case
            .execute(command(&token, SECRET_PASSWORD, Some("not-an-email")))
            .await;

        // Assert
        assert!(matches!(result, Err(SetupRootAdminError::InvalidEmail)));
        Ok(())
    }

    #[tokio::test]
    async fn setup_root_admin_spent_budget_is_rate_limited() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(
            |_, _, _, _| Ok(()),
            (move || {
                let mut limiter = MockSetupAttemptLimiter::new();
                limiter
                    .expect_check()
                    .returning(|| Some(Duration::from_secs(5)));
                limiter
            })(),
        )?;

        // Act
        let result = harness
            .use_case
            .execute(command("0000-0000", SECRET_PASSWORD, None))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(SetupRootAdminError::RateLimited { .. })
        ));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    // proves: SCN-AUTHENTICATION-021-E — Recovery
    #[tokio::test]
    async fn setup_root_admin_failed_credential_write_applies_nothing() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let nonce = "i".repeat(32);
        let token = token_for(&nonce);
        let harness = use_case_with(
            |user_repository, credential_repository, setup_grant_repository, password_hasher| {
                (move |repository: &mut MockSetupGrantRepository, nonce: &str| {
                    let pending = grant(nonce, 60, None).ok();
                    repository.expect_find().times(1).returning(move || {
                        let found = pending.clone();
                        Box::pin(async move { Ok(found) })
                    });
                })(setup_grant_repository, &nonce);
                (move |user_repository: &mut MockUserRepository| {
                    let root = user(0, "root", None).ok();
                    user_repository
                        .expect_search()
                        .times(1)
                        .withf(|filter| filter.id == Some(0))
                        .returning(move |_| {
                            let found = root.clone();
                            Box::pin(async move { Ok(found.into_iter().collect()) })
                        });
                })(user_repository);
                user_repository
                    .expect_search()
                    .times(1)
                    .withf(|filter| {
                        filter.email.is_none()
                            && filter.id.is_none()
                            && filter.role.is_none()
                            && filter.username.is_none()
                    })
                    .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                password_hasher
                    .expect_hash_password()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok("hashed".to_owned()) }));
                credential_repository
                    .expect_save()
                    .times(1)
                    .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
                Ok(())
            },
            (move || {
                let mut limiter = MockSetupAttemptLimiter::new();
                limiter.expect_check().returning(|| None);
                limiter
            })(),
        )?;

        // Act
        let result = harness
            .use_case
            .execute(command(&token, SECRET_PASSWORD, Some("root@example.com")))
            .await;

        // Assert
        assert!(matches!(result, Err(SetupRootAdminError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn setup_root_admin_lost_consume_race_is_refused() -> Result<(), Box<dyn Error>> {
        // Arrange
        let nonce = "j".repeat(32);
        let token = token_for(&nonce);
        let harness = use_case_with(
            |user_repository, credential_repository, setup_grant_repository, password_hasher| {
                (move |repository: &mut MockSetupGrantRepository, nonce: &str| {
                    let pending = grant(nonce, 60, None).ok();
                    repository.expect_find().times(1).returning(move || {
                        let found = pending.clone();
                        Box::pin(async move { Ok(found) })
                    });
                })(setup_grant_repository, &nonce);
                (move |user_repository: &mut MockUserRepository| {
                    let root = user(0, "root", None).ok();
                    user_repository
                        .expect_search()
                        .times(1)
                        .withf(|filter| filter.id == Some(0))
                        .returning(move |_| {
                            let found = root.clone();
                            Box::pin(async move { Ok(found.into_iter().collect()) })
                        });
                })(user_repository);
                password_hasher
                    .expect_hash_password()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok("hashed".to_owned()) }));
                credential_repository
                    .expect_save()
                    .times(1)
                    .returning(|credential| Box::pin(async { Ok(credential) }));
                setup_grant_repository
                    .expect_consume()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok(false) }));
                Ok(())
            },
            (move || {
                let mut limiter = MockSetupAttemptLimiter::new();
                limiter.expect_check().returning(|| None);
                limiter
            })(),
        )?;

        // Act
        let result = harness
            .use_case
            .execute(command(&token, SECRET_PASSWORD, None))
            .await;

        // Assert
        assert!(matches!(result, Err(SetupRootAdminError::InvalidToken)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn setup_root_admin_hash_failure_is_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let nonce = "k".repeat(32);
        let token = token_for(&nonce);
        let harness = use_case_with(
            |user_repository, _, setup_grant_repository, password_hasher| {
                (move |repository: &mut MockSetupGrantRepository, nonce: &str| {
                    let pending = grant(nonce, 60, None).ok();
                    repository.expect_find().times(1).returning(move || {
                        let found = pending.clone();
                        Box::pin(async move { Ok(found) })
                    });
                })(setup_grant_repository, &nonce);
                (move |user_repository: &mut MockUserRepository| {
                    let root = user(0, "root", None).ok();
                    user_repository
                        .expect_search()
                        .times(1)
                        .withf(|filter| filter.id == Some(0))
                        .returning(move |_| {
                            let found = root.clone();
                            Box::pin(async move { Ok(found.into_iter().collect()) })
                        });
                })(user_repository);
                password_hasher
                    .expect_hash_password()
                    .times(1)
                    .returning(|_| Box::pin(async { Err(PasswordHasherError::OperationFailed) }));
                Ok(())
            },
            (move || {
                let mut limiter = MockSetupAttemptLimiter::new();
                limiter.expect_check().returning(|| None);
                limiter
            })(),
        )?;

        // Act
        let result = harness
            .use_case
            .execute(command(&token, SECRET_PASSWORD, None))
            .await;

        // Assert
        assert!(matches!(result, Err(SetupRootAdminError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn setup_root_admin_success_emits_one_credential_rotated() -> Result<(), Box<dyn Error>> {
        // Arrange
        let nonce = "l".repeat(32);
        let token = token_for(&nonce);
        let harness = use_case_with(
            |user_repository, credential_repository, setup_grant_repository, password_hasher| {
                (move |repository: &mut MockSetupGrantRepository, nonce: &str| {
                    let pending = grant(nonce, 60, None).ok();
                    repository.expect_find().times(1).returning(move || {
                        let found = pending.clone();
                        Box::pin(async move { Ok(found) })
                    });
                })(setup_grant_repository, &nonce);
                (move |user_repository: &mut MockUserRepository| {
                    let root = user(0, "root", None).ok();
                    user_repository
                        .expect_search()
                        .times(1)
                        .withf(|filter| filter.id == Some(0))
                        .returning(move |_| {
                            let found = root.clone();
                            Box::pin(async move { Ok(found.into_iter().collect()) })
                        });
                })(user_repository);
                password_hasher
                    .expect_hash_password()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok("hashed".to_owned()) }));
                credential_repository
                    .expect_save()
                    .times(1)
                    .returning(|credential| Box::pin(async { Ok(credential) }));
                setup_grant_repository
                    .expect_consume()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok(true) }));
                Ok(())
            },
            (move || {
                let mut limiter = MockSetupAttemptLimiter::new();
                limiter.expect_check().returning(|| None);
                limiter
            })(),
        )?;

        // Act
        harness
            .use_case
            .execute(command(&token, SECRET_PASSWORD, None))
            .await?;

        // Assert
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("event=\"credential_rotated\""))
                .count();
            (count == 1).then_some(()).ok_or_else(|| {
                format!("expected exactly one credential_rotated event (found {count})")
            })
        });
        assert!(logs_contain("actor=0"));
        assert!(!logs_contain(&token), "the event must not carry the token");
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn setup_root_admin_refusal_emits_one_authn_failed() -> Result<(), Box<dyn Error>> {
        // Arrange
        let nonce = "m".repeat(32);
        let harness = use_case_with(
            |_, _, setup_grant_repository, _| {
                (move |repository: &mut MockSetupGrantRepository, nonce: &str| {
                    let pending = grant(nonce, 60, None).ok();
                    repository.expect_find().times(1).returning(move || {
                        let found = pending.clone();
                        Box::pin(async move { Ok(found) })
                    });
                })(setup_grant_repository, &nonce);
                Ok(())
            },
            (move || {
                let mut limiter = MockSetupAttemptLimiter::new();
                limiter.expect_check().returning(|| None);
                limiter
            })(),
        )?;

        // Act
        let _result = harness
            .use_case
            .execute(command("0000-0000", SECRET_PASSWORD, None))
            .await;

        // Assert
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("event=\"authn_failed\""))
                .count();
            (count == 1)
                .then_some(())
                .ok_or_else(|| format!("expected exactly one authn_failed event (found {count})"))
        });
        assert!(logs_contain("reason=\"invalid_setup_token\""));
        assert!(!logs_contain("0000"));
        Ok(())
    }
}
