use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::login_user::LoginUserCommand;
use crate::application::port::login_user::LoginUserError;
use crate::application::port::login_user::LoginUserResponse;
use crate::application::port::login_user::LoginUserUseCase;
use crate::application::security_event;
use crate::domain::port::credential_repository::CredentialFilter;
use crate::domain::port::credential_repository::CredentialRepository as _;
use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
use crate::domain::port::password_hasher::PasswordHasher;
use crate::domain::port::token_provider::TokenProvider;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;

/// Use case implementation for authenticating a user.
pub struct LoginUser<F, H, P> {
    /// Hasher for user passwords.
    password_hasher: Arc<H>,
    /// Provider issuing and validating tokens.
    token_provider: Arc<P>,
    /// Factory opening the unit of work wrapping the authentication.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, H: PasswordHasher, P: TokenProvider> LoginUser<F, H, P> {
    /// Create a new use case.
    #[must_use]
    pub fn new(
        unit_of_work_factory: Arc<F>,
        password_hasher: Arc<H>,
        token_provider: Arc<P>,
    ) -> Self {
        Self {
            password_hasher,
            token_provider,
            unit_of_work_factory,
        }
    }
}

impl<F, H, P> LoginUserUseCase for LoginUser<F, H, P>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + UserUnitOfWork,
    H: PasswordHasher,
    P: TokenProvider,
{
    fn execute<'future>(
        &'future self,
        command: LoginUserCommand,
    ) -> Pin<Box<dyn Future<Output = Result<LoginUserResponse, LoginUserError>> + Send + 'future>>
    {
        let password_hasher = Arc::clone(&self.password_hasher);
        let token_provider = Arc::clone(&self.token_provider);
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let username = command.username().to_owned();
            let password = command.password().to_owned();

            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| LoginUserError::Unknown(error.into()))?;

            let result = async {
                let Some(user) = unit_of_work
                    .users()
                    .search(&UserFilter {
                        username: Some(username),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| LoginUserError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                else {
                    // Spend the same password work as a real verification on
                    // the absent-user branch, then report an unknown username.
                    // A backend fault surfaces as an internal error, exactly as
                    // it does on the known-user branch.
                    password_hasher
                        .hash_password(&password)
                        .await
                        .map_err(|error| LoginUserError::Unknown(error.into()))?;
                    return Err(LoginUserError::InvalidUsername);
                };

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
                        security_event::application_error("login_user");
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

                security_event::authentication_succeeded(user.id());

                Ok(LoginUserResponse::new(
                    token.value().to_owned(),
                    token.expires_in(),
                ))
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
                        LoginUserError::InvalidUsername | LoginUserError::InvalidPassword
                    ) {
                        security_event::authentication_failed(command.username());
                    }
                    if let Err(_rollback_error) = unit_of_work.rollback().await {
                        // The unit-of-work adapter owns the rollback-failure
                        // log (`OBS-002`); the use case returns the original
                        // business error (`STY-RUST-038`).
                    }
                    Err(error)
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::error::Error;
    use std::fmt::Debug;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;

    use chrono::NaiveDateTime;
    use tracing::Event;
    use tracing::Level;
    use tracing::field::Field;
    use tracing::field::Visit;
    use tracing::subscriber::DefaultGuard;
    use tracing::subscriber::set_default;
    use tracing_subscriber::layer::Context;
    use tracing_subscriber::layer::Layer;
    use tracing_subscriber::layer::SubscriberExt as _;
    use tracing_subscriber::registry;

    use crate::application::port::login_user::LoginUserCommand;
    use crate::application::port::login_user::LoginUserError;
    use crate::application::port::login_user::LoginUserUseCase as _;
    use crate::domain::model::credential::Credential;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::model::user::UserError;
    use crate::domain::port::configuration_repository::MockConfigurationRepository;
    use crate::domain::port::credential_repository::MockCredentialRepository;
    use crate::domain::port::error::PasswordHasherError;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::error::TokenProviderError;
    use crate::domain::port::password_hasher::MockPasswordHasher;
    use crate::domain::port::token_provider::IssuedToken;
    use crate::domain::port::token_provider::MockTokenProvider;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::SECRET_PASSWORD;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::unit_of_work_factory;

    use super::LoginUser;

    type UseCase = LoginUser<TestFactory, MockPasswordHasher, MockTokenProvider>;

    /// A use case under test together with its transaction-lifecycle flags.
    struct Harness {
        /// Set when the unit of work is committed.
        committed: Arc<AtomicBool>,
        /// Set when the unit of work is rolled back.
        rolled_back: Arc<AtomicBool>,
        /// The use case under test.
        use_case: UseCase,
    }

    /// A captured `tracing` event: its level, target and fields.
    #[derive(Clone, Debug)]
    struct CapturedEvent {
        /// Recorded fields, keyed by name.
        fields: BTreeMap<String, String>,
        /// Severity level of the event.
        level: Level,
        /// Target of the event.
        target: String,
    }

    /// Collect the `tracing` events emitted while it is installed.
    #[derive(Clone, Default)]
    struct CaptureEvents {
        /// The events captured so far.
        events: Arc<Mutex<Vec<CapturedEvent>>>,
    }

    /// Collect the fields of one event into a map.
    #[derive(Default)]
    struct FieldVisitor {
        /// Recorded fields, keyed by name.
        fields: BTreeMap<String, String>,
    }

    impl CaptureEvents {
        /// Return the single captured event, or `None` when the count differs.
        fn single(&self) -> Option<CapturedEvent> {
            let events = self.events.lock().ok()?;
            if events.len() == 1 {
                events.first().cloned()
            } else {
                None
            }
        }

        /// Return a snapshot of the captured events.
        fn snapshot(&self) -> Vec<CapturedEvent> {
            self.events
                .lock()
                .map(|events| events.clone())
                .unwrap_or_default()
        }
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "only `on_event` is needed; every other `Layer` method keeps its default"
    )]
    impl<S: tracing::Subscriber> Layer<S> for CaptureEvents {
        /// Record every event the subscriber receives.
        fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
            let metadata = event.metadata();
            let mut visitor = FieldVisitor::default();
            event.record(&mut visitor);
            let captured = CapturedEvent {
                level: *metadata.level(),
                target: metadata.target().to_owned(),
                fields: visitor.fields,
            };
            if let Ok(mut events) = self.events.lock() {
                events.push(captured);
            }
        }
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "only the field types the catalog emits are recorded; the rest keep their defaults"
    )]
    impl Visit for FieldVisitor {
        /// Record a boolean field.
        fn record_bool(&mut self, field: &Field, value: bool) {
            self.fields
                .insert(field.name().to_owned(), value.to_string());
        }

        /// Record a `Debug`-formatted field.
        fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
            self.fields
                .insert(field.name().to_owned(), format!("{value:?}"));
        }

        /// Record a signed-integer field.
        fn record_i64(&mut self, field: &Field, value: i64) {
            self.fields
                .insert(field.name().to_owned(), value.to_string());
        }

        /// Record a string field.
        fn record_str(&mut self, field: &Field, value: &str) {
            self.fields
                .insert(field.name().to_owned(), value.to_owned());
        }

        /// Record an unsigned-integer field.
        fn record_u64(&mut self, field: &Field, value: u64) {
            self.fields
                .insert(field.name().to_owned(), value.to_string());
        }
    }

    /// Install a capturing subscriber for the current test thread.
    fn capture() -> (CaptureEvents, DefaultGuard) {
        let capture = CaptureEvents::default();
        let subscriber = registry().with(capture.clone());
        let guard = set_default(subscriber);
        (capture, guard)
    }

    fn use_case_with(
        setup: impl FnOnce(
            &mut MockUserRepository,
            &mut MockCredentialRepository,
            &mut MockPasswordHasher,
            &mut MockTokenProvider,
        ) -> Result<(), Box<dyn Error>>,
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

        let committed = Arc::new(AtomicBool::new(false));
        let rolled_back = Arc::new(AtomicBool::new(false));
        let factory = unit_of_work_factory(
            user_repository,
            credential_repository,
            MockConfigurationRepository::new(),
            Arc::clone(&committed),
            Arc::clone(&rolled_back),
        );

        Ok(Harness {
            use_case: LoginUser::new(
                Arc::new(factory),
                Arc::new(password_hasher),
                Arc::new(token_provider),
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
            .expect_search()
            .times(1)
            .returning(move |_| {
                let own_credential = credential.clone();
                Box::pin(async move { Ok(vec![own_credential]) })
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
        let harness = use_case_with(
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
        let command = command("alice", SECRET_PASSWORD);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.access_token(), "token");
        assert_eq!(response.expires_in(), 3600);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn login_user_unknown_username_returns_invalid_username() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, _, password_hasher, _| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            password_hasher
                .expect_hash_password()
                .times(1)
                .returning(|_| Box::pin(async { Ok("decoy-hash".to_owned()) }));
            Ok(())
        })?;
        let command = command("ghost", SECRET_PASSWORD);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(LoginUserError::InvalidUsername)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn login_user_unknown_username_with_failing_hasher_returns_unknown()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, _, password_hasher, _| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            password_hasher
                .expect_hash_password()
                .times(1)
                .returning(|_| Box::pin(async { Err(PasswordHasherError::OperationFailed) }));
            Ok(())
        })?;
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
        let harness = use_case_with(|user_repository, _, _, _| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;
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
        let harness = use_case_with(|user_repository, credential_repository, _, _| {
            expect_user(user_repository, user(1, "alice")?);
            credential_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            Ok(())
        })?;
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
        let harness = use_case_with(
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
        let harness = use_case_with(
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
        let harness = use_case_with(
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
        let command = command("alice", SECRET_PASSWORD);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(LoginUserError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn login_user_valid_credentials_emits_authn_succeeded() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(
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
        let command = command("alice", SECRET_PASSWORD);
        let (capture, _guard) = capture();

        // Act
        assert!(harness.use_case.execute(command).await.is_ok());

        // Assert
        let Some(event) = capture.single() else {
            return Err("expected exactly one security event".into());
        };
        assert_eq!(event.target, "security");
        assert_eq!(event.level, Level::INFO);
        assert_eq!(
            event.fields.get("event").map(String::as_str),
            Some("authn_succeeded")
        );
        assert_eq!(event.fields.get("actor").map(String::as_str), Some("1"));
        Ok(())
    }

    #[tokio::test]
    async fn login_user_wrong_password_emits_authn_failed() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(
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
        let (capture, _guard) = capture();

        // Act
        assert!(harness.use_case.execute(command).await.is_err());

        // Assert
        let Some(event) = capture.single() else {
            return Err("expected exactly one security event".into());
        };
        assert_eq!(event.target, "security");
        assert_eq!(event.level, Level::WARN);
        assert_eq!(
            event.fields.get("event").map(String::as_str),
            Some("authn_failed")
        );
        assert_eq!(
            event.fields.get("reason").map(String::as_str),
            Some("invalid_credentials")
        );
        assert_eq!(
            event.fields.get("claimed_identity").map(String::as_str),
            Some("alice")
        );
        Ok(())
    }

    #[tokio::test]
    async fn login_user_unknown_username_emits_uniform_authn_failed() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let harness = use_case_with(|user_repository, _, password_hasher, _| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            password_hasher
                .expect_hash_password()
                .times(1)
                .returning(|_| Box::pin(async { Ok("decoy-hash".to_owned()) }));
            Ok(())
        })?;
        let command = command("ghost", SECRET_PASSWORD);
        let (capture, _guard) = capture();

        // Act
        assert!(harness.use_case.execute(command).await.is_err());

        // Assert
        let Some(event) = capture.single() else {
            return Err("expected exactly one security event".into());
        };
        assert_eq!(event.target, "security");
        assert_eq!(event.level, Level::WARN);
        assert_eq!(
            event.fields.get("event").map(String::as_str),
            Some("authn_failed")
        );
        assert_eq!(
            event.fields.get("reason").map(String::as_str),
            Some("invalid_credentials")
        );
        assert_eq!(
            event.fields.get("claimed_identity").map(String::as_str),
            Some("ghost")
        );
        Ok(())
    }

    #[tokio::test]
    async fn login_user_missing_credential_emits_application_error() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, credential_repository, _, _| {
            expect_user(user_repository, user(1, "alice")?);
            credential_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            Ok(())
        })?;
        let command = command("alice", SECRET_PASSWORD);
        let (capture, _guard) = capture();

        // Act
        assert!(harness.use_case.execute(command).await.is_err());

        // Assert
        let Some(event) = capture.single() else {
            return Err("expected exactly one security event".into());
        };
        assert_eq!(event.target, "security");
        assert_eq!(event.level, Level::ERROR);
        assert_eq!(
            event.fields.get("event").map(String::as_str),
            Some("application_error")
        );
        assert_eq!(
            event.fields.get("operation").map(String::as_str),
            Some("login_user")
        );
        Ok(())
    }

    #[tokio::test]
    async fn login_user_port_failure_emits_no_authn_event() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, _, _, _| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;
        let command = command("alice", SECRET_PASSWORD);
        let (capture, _guard) = capture();

        // Act
        assert!(harness.use_case.execute(command).await.is_err());

        // Assert
        assert!(
            capture.snapshot().is_empty(),
            "a port failure is owned by the adapter"
        );
        Ok(())
    }
}
