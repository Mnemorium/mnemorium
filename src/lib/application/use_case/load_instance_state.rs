use std::fmt::Write as _;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use crate::application::port::load_instance_state::JWT_SECRET_VARIABLE;
use crate::application::port::load_instance_state::LoadInstanceStateCommand;
use crate::application::port::load_instance_state::LoadInstanceStateError;
use crate::application::port::load_instance_state::LoadInstanceStateResponse;
use crate::application::port::load_instance_state::LoadInstanceStateUseCase;
use crate::application::port::load_instance_state::PEPPER_VARIABLE;
use crate::application::port::load_instance_state::SECRET_HEX_LENGTH;
use crate::application::port::load_instance_state::SECRET_LENGTH;
use crate::domain::model::instance_state::InstanceState;
use crate::domain::port::error::InstanceStateStoreError;
use crate::domain::port::instance_state_store::InstanceStateStore;
use crate::domain::port::secret_generator::SecretGenerator;

/// Use case implementation for loading or creating the instance identity.
pub struct LoadInstanceState<S, G> {
    /// Store holding the persisted identity.
    instance_state_store: Arc<S>,
    /// Generator producing random secrets when no seed is supplied.
    secret_generator: Arc<G>,
}

impl<S, G> LoadInstanceState<S, G>
where
    S: InstanceStateStore,
    G: SecretGenerator,
{
    /// Create and persist the identity for a genuinely new instance.
    ///
    /// # Errors
    ///
    /// Returns an error when a secret cannot be generated, when the generated
    /// value fails validation, or when the store cannot persist it.
    async fn create(
        &self,
        command: &LoadInstanceStateCommand,
    ) -> Result<InstanceState, LoadInstanceStateError> {
        let pepper = self.resolve_secret(command.seeds().pepper()).await?;
        let jwt_secret = self.resolve_secret(command.seeds().jwt_secret()).await?;
        let state = InstanceState::try_new(Utc::now().timestamp(), pepper, jwt_secret)
            .map_err(|error| LoadInstanceStateError::Unknown(error.into()))?;
        self.instance_state_store
            .save(&state)
            .await
            .map_err(map_store_error)?;
        Ok(state)
    }

    /// Create a new use case.
    #[must_use]
    pub fn new(instance_state_store: Arc<S>, secret_generator: Arc<G>) -> Self {
        Self {
            instance_state_store,
            secret_generator,
        }
    }

    /// Resolve one secret: the environment seed when present, a fresh random
    /// value otherwise.
    ///
    /// # Errors
    ///
    /// Returns [`LoadInstanceStateError::Unknown`] when the generator fails.
    async fn resolve_secret(&self, seed: Option<&str>) -> Result<String, LoadInstanceStateError> {
        if let Some(value) = seed {
            return Ok(value.to_owned());
        }
        let bytes = self
            .secret_generator
            .generate(SECRET_LENGTH)
            .await
            .map_err(|error| LoadInstanceStateError::Unknown(error.into()))?;
        let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
        for byte in &bytes {
            let _result = write!(encoded, "{byte:02x}");
        }
        Ok(encoded)
    }

    /// Verify a supplied seed equals the persisted secret.
    ///
    /// The rejection names the variable, never either value (`OBS-004`).
    ///
    /// # Errors
    ///
    /// Returns [`LoadInstanceStateError::SeedMismatch`] when the two differ.
    fn verify_seed(
        seed: Option<&str>,
        persisted: &str,
        variable: &'static str,
    ) -> Result<(), LoadInstanceStateError> {
        match seed {
            Some(value) if value != persisted => {
                Err(LoadInstanceStateError::SeedMismatch { variable })
            }
            _ => Ok(()),
        }
    }
}

impl<S, G> LoadInstanceStateUseCase for LoadInstanceState<S, G>
where
    S: InstanceStateStore,
    G: SecretGenerator,
{
    fn execute<'future>(
        &'future self,
        command: LoadInstanceStateCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<LoadInstanceStateResponse, LoadInstanceStateError>>
                + Send
                + 'future,
        >,
    > {
        Box::pin(async move {
            validate_seed(command.seeds().pepper(), PEPPER_VARIABLE)?;
            validate_seed(command.seeds().jwt_secret(), JWT_SECRET_VARIABLE)?;

            match self
                .instance_state_store
                .load()
                .await
                .map_err(map_store_error)?
            {
                Some(state) => {
                    Self::verify_seed(command.seeds().pepper(), state.pepper(), PEPPER_VARIABLE)?;
                    Self::verify_seed(
                        command.seeds().jwt_secret(),
                        state.jwt_secret(),
                        JWT_SECRET_VARIABLE,
                    )?;
                    Ok(LoadInstanceStateResponse::new(state, false))
                }
                // A datastore without identity means the identity was lost; a
                // replacement would silently invalidate every password hash.
                None if command.datastore_exists() => Err(LoadInstanceStateError::IdentityMissing),
                None => {
                    let state = self.create(&command).await?;
                    Ok(LoadInstanceStateResponse::new(state, true))
                }
            }
        })
    }
}

/// Map an adapter failure onto the use-case error.
fn map_store_error(error: InstanceStateStoreError) -> LoadInstanceStateError {
    match error {
        InstanceStateStoreError::Conflict
        | InstanceStateStoreError::OperationFailed(_)
        | InstanceStateStoreError::Unknown(_) => {
            LoadInstanceStateError::StoreFailed(anyhow::Error::new(error))
        }
        InstanceStateStoreError::Corrupt(source) => LoadInstanceStateError::IdentityCorrupt(source),
        InstanceStateStoreError::UnsupportedVersion(version) => {
            LoadInstanceStateError::UnsupportedVersion(version)
        }
    }
}

/// Validate an optional seed is a 64-character hexadecimal string.
///
/// # Errors
///
/// Returns [`LoadInstanceStateError::InvalidSeed`] naming the variable when the
/// seed is present but malformed. The value itself never reaches the error.
fn validate_seed(seed: Option<&str>, variable: &'static str) -> Result<(), LoadInstanceStateError> {
    let is_hex = seed.is_some_and(|value| {
        value.len() == SECRET_HEX_LENGTH
            && value.chars().all(|character| character.is_ascii_hexdigit())
    });
    let is_malformed = seed.is_some() && !is_hex;
    if is_malformed {
        return Err(LoadInstanceStateError::InvalidSeed { variable });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::iter::repeat_n;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::PoisonError;

    use crate::application::port::load_instance_state::IdentitySeeds;
    use crate::application::port::load_instance_state::LoadInstanceStateCommand;
    use crate::application::port::load_instance_state::LoadInstanceStateError;
    use crate::application::port::load_instance_state::LoadInstanceStateUseCase as _;
    use crate::domain::model::instance_state::InstanceState;
    use crate::domain::model::instance_state::InstanceStateError;
    use crate::domain::port::error::InstanceStateStoreError;
    use crate::domain::port::error::SecretGeneratorError;
    use crate::domain::port::instance_state_store::MockInstanceStateStore;
    use crate::domain::port::secret_generator::MockSecretGenerator;

    use super::LoadInstanceState;

    type UseCase = LoadInstanceState<MockInstanceStateStore, MockSecretGenerator>;

    /// Record what the use case hands to `save`.
    type Saved = Arc<Mutex<Option<InstanceState>>>;

    /// Build a use case whose outbound dependencies are mocked; `setup` defines
    /// the expectations before the mocks are handed over.
    fn use_case_with(
        setup: impl FnOnce(&mut MockInstanceStateStore, &mut MockSecretGenerator),
    ) -> UseCase {
        let mut store = MockInstanceStateStore::new();
        let mut generator = MockSecretGenerator::new();
        setup(&mut store, &mut generator);
        LoadInstanceState::new(Arc::new(store), Arc::new(generator))
    }

    /// A 64-character hexadecimal string.
    fn hex64(character: char) -> String {
        repeat_n(character, 64).collect()
    }

    /// A persisted identity.
    fn state() -> Result<InstanceState, Box<dyn Error>> {
        Ok(InstanceState::try_new(
            1_700_000_000,
            hex64('a'),
            hex64('b'),
        )?)
    }

    #[tokio::test]
    async fn instance_state_valid_secrets_returns_the_value() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pepper = hex64('a');
        let jwt_secret = hex64('b');

        // Act
        let state = InstanceState::try_new(1_700_000_000, pepper.clone(), jwt_secret.clone())?;

        // Assert
        assert_eq!(
            state.created_at(),
            1_700_000_000,
            "the constructor must keep the creation instant"
        );
        assert_eq!(
            state.pepper(),
            pepper,
            "the constructor must keep the pepper"
        );
        assert_eq!(
            state.jwt_secret(),
            jwt_secret,
            "the constructor must keep the signing secret"
        );
        Ok(())
    }

    #[tokio::test]
    async fn instance_state_non_hex_pepper_is_rejected() {
        // Act
        let result = InstanceState::try_new(0, "not-a-pepper".to_owned(), hex64('b'));

        // Assert
        assert!(matches!(result, Err(InstanceStateError::InvalidPepper)));
    }

    #[tokio::test]
    async fn instance_state_short_pepper_is_rejected() {
        // Arrange
        let short = "a".repeat(63);

        // Act
        let result = InstanceState::try_new(0, short, hex64('b'));

        // Assert
        assert!(matches!(result, Err(InstanceStateError::InvalidPepper)));
    }

    #[tokio::test]
    async fn instance_state_non_hex_jwt_secret_is_rejected() {
        // Act
        let result = InstanceState::try_new(0, hex64('a'), "not-a-secret".to_owned());

        // Assert
        assert!(matches!(result, Err(InstanceStateError::InvalidJwtSecret)));
    }

    #[tokio::test]
    async fn instance_state_long_jwt_secret_is_rejected() {
        // Arrange
        let oversized = format!("{}0", hex64('a'));

        // Act
        let result = InstanceState::try_new(0, hex64('b'), oversized);

        // Assert
        assert!(matches!(result, Err(InstanceStateError::InvalidJwtSecret)));
    }

    #[tokio::test]
    async fn instance_state_debug_redacts_both_secrets() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pepper = hex64('a');
        let jwt_secret = hex64('b');
        let state = InstanceState::try_new(42, pepper.clone(), jwt_secret.clone())?;

        // Act
        let debug = format!("{state:?}");

        // Assert
        assert!(
            !debug.contains(pepper.as_str()),
            "Debug output must not contain the pepper, got: {debug:?}"
        );
        assert!(
            !debug.contains(jwt_secret.as_str()),
            "Debug output must not contain the signing secret, got: {debug:?}"
        );
        assert!(
            debug.contains("42"),
            "Debug output must still contain the creation instant, got: {debug:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn identity_seeds_debug_redacts_both_seeds() {
        // Arrange
        let pepper = hex64('a');
        let jwt_secret = hex64('b');
        let seeds = IdentitySeeds::new(Some(pepper.clone()), Some(jwt_secret.clone()));

        // Act
        let debug = format!("{seeds:?}");

        // Assert
        assert!(
            !debug.contains(pepper.as_str()),
            "Debug output must not contain the pepper, got: {debug:?}"
        );
        assert!(
            !debug.contains(jwt_secret.as_str()),
            "Debug output must not contain the signing secret, got: {debug:?}"
        );
        assert!(
            debug.contains("**"),
            "Debug output must mark a supplied seed as redacted, got: {debug:?}"
        );
    }

    #[tokio::test]
    async fn load_instance_state_fresh_instance_creates_identity() -> Result<(), Box<dyn Error>> {
        // Arrange
        let saved: Saved = Arc::new(Mutex::new(None));
        let recorder = Arc::clone(&saved);
        let use_case = use_case_with(|store, generator| {
            store
                .expect_load()
                .times(1)
                .returning(|| Box::pin(async { Ok(None) }));
            generator
                .expect_generate()
                .times(2)
                .returning(|_| Box::pin(async { Ok(vec![0xAA; 32]) }));
            store.expect_save().times(1).returning(move |state| {
                *recorder.lock().unwrap_or_else(PoisonError::into_inner) = Some(state.clone());
                Box::pin(async { Ok(()) })
            });
        });

        // Act
        let response = use_case
            .execute(LoadInstanceStateCommand::new(
                false,
                IdentitySeeds::new(None, None),
            ))
            .await?;

        // Assert
        assert!(response.created(), "a fresh instance must report creation");
        assert_eq!(response.state().pepper(), "aa".repeat(32));
        assert_eq!(response.state().jwt_secret(), "aa".repeat(32));
        assert!(
            response.state().created_at() > 0,
            "creation must record the instant identity was established"
        );
        assert_eq!(
            saved
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_ref(),
            Some(response.state()),
            "the created identity must be persisted"
        );
        Ok(())
    }

    #[tokio::test]
    async fn load_instance_state_existing_identity_is_reused() -> Result<(), Box<dyn Error>> {
        // Arrange
        let stored = state()?;
        let use_case = use_case_with(|store, generator| {
            let persisted = stored.clone();
            store.expect_load().times(1).returning(move || {
                let value = persisted.clone();
                Box::pin(async move { Ok(Some(value)) })
            });
            generator.expect_generate().times(0);
            store.expect_save().times(0);
        });

        // Act
        let response = use_case
            .execute(LoadInstanceStateCommand::new(
                true,
                IdentitySeeds::new(None, None),
            ))
            .await?;

        // Assert
        assert!(
            !response.created(),
            "a persisted identity must not be reported as created"
        );
        assert_eq!(response.state(), &state()?);
        Ok(())
    }

    #[tokio::test]
    async fn load_instance_state_missing_identity_for_existing_datastore_fails()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|store, generator| {
            store
                .expect_load()
                .times(1)
                .returning(|| Box::pin(async { Ok(None) }));
            generator.expect_generate().times(0);
            store.expect_save().times(0);
        });

        // Act
        let result = use_case
            .execute(LoadInstanceStateCommand::new(
                true,
                IdentitySeeds::new(None, None),
            ))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(LoadInstanceStateError::IdentityMissing)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn load_instance_state_seed_is_used_at_creation() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pepper_seed = hex64('c');
        let use_case = use_case_with(|store, generator| {
            store
                .expect_load()
                .times(1)
                .returning(|| Box::pin(async { Ok(None) }));
            generator
                .expect_generate()
                .times(1)
                .returning(|_| Box::pin(async { Ok(vec![0xBB; 32]) }));
            store
                .expect_save()
                .times(1)
                .returning(|_| Box::pin(async { Ok(()) }));
        });

        // Act
        let response = use_case
            .execute(LoadInstanceStateCommand::new(
                false,
                IdentitySeeds::new(Some(pepper_seed.clone()), None),
            ))
            .await?;

        // Assert
        assert_eq!(
            response.state().pepper(),
            pepper_seed,
            "the supplied pepper must be persisted"
        );
        assert_eq!(
            response.state().jwt_secret(),
            "bb".repeat(32),
            "an absent seed must be generated"
        );
        Ok(())
    }

    #[tokio::test]
    async fn load_instance_state_matching_seed_is_accepted() -> Result<(), Box<dyn Error>> {
        // Arrange
        let stored = state()?;
        let use_case = use_case_with(|store, generator| {
            let persisted = stored.clone();
            store.expect_load().times(1).returning(move || {
                let value = persisted.clone();
                Box::pin(async move { Ok(Some(value)) })
            });
            generator.expect_generate().times(0);
            store.expect_save().times(0);
        });

        // Act
        let response = use_case
            .execute(LoadInstanceStateCommand::new(
                true,
                IdentitySeeds::new(Some(hex64('a')), Some(hex64('b'))),
            ))
            .await?;

        // Assert
        assert!(!response.created());
        assert_eq!(response.state(), &state()?);
        Ok(())
    }

    #[tokio::test]
    async fn load_instance_state_differing_seed_fails_without_touching_identity()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let stored = state()?;
        let use_case = use_case_with(|store, generator| {
            let persisted = stored.clone();
            store.expect_load().times(1).returning(move || {
                let value = persisted.clone();
                Box::pin(async move { Ok(Some(value)) })
            });
            generator.expect_generate().times(0);
            store.expect_save().times(0);
        });

        // Act
        let result = use_case
            .execute(LoadInstanceStateCommand::new(
                true,
                IdentitySeeds::new(Some(hex64('c')), None),
            ))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(LoadInstanceStateError::SeedMismatch { .. })
        ));
        Ok(())
    }

    #[tokio::test]
    async fn load_instance_state_invalid_seed_fails() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|store, generator| {
            store.expect_load().times(0);
            generator.expect_generate().times(0);
            store.expect_save().times(0);
        });

        // Act
        let result = use_case
            .execute(LoadInstanceStateCommand::new(
                false,
                IdentitySeeds::new(Some("not-a-pepper".to_owned()), None),
            ))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(LoadInstanceStateError::InvalidSeed { .. })
        ));
        Ok(())
    }

    #[tokio::test]
    async fn load_instance_state_generator_failure_is_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|store, generator| {
            store
                .expect_load()
                .times(1)
                .returning(|| Box::pin(async { Ok(None) }));
            generator
                .expect_generate()
                .times(1)
                .returning(|_| Box::pin(async { Err(SecretGeneratorError::OperationFailed) }));
            store.expect_save().times(0);
        });

        // Act
        let result = use_case
            .execute(LoadInstanceStateCommand::new(
                false,
                IdentitySeeds::new(None, None),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(LoadInstanceStateError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    async fn load_instance_state_store_failure_is_reported() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|store, generator| {
            store.expect_load().times(1).returning(|| {
                Box::pin(async {
                    Err(InstanceStateStoreError::OperationFailed(anyhow::anyhow!(
                        "disk unavailable"
                    )))
                })
            });
            generator.expect_generate().times(0);
            store.expect_save().times(0);
        });

        // Act
        let result = use_case
            .execute(LoadInstanceStateCommand::new(
                false,
                IdentitySeeds::new(None, None),
            ))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(LoadInstanceStateError::StoreFailed(_))
        ));
        Ok(())
    }

    #[tokio::test]
    async fn load_instance_state_unsupported_version_is_reported() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|store, generator| {
            store.expect_load().times(1).returning(|| {
                Box::pin(async { Err(InstanceStateStoreError::UnsupportedVersion(9)) })
            });
            generator.expect_generate().times(0);
            store.expect_save().times(0);
        });

        // Act
        let result = use_case
            .execute(LoadInstanceStateCommand::new(
                false,
                IdentitySeeds::new(None, None),
            ))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(LoadInstanceStateError::UnsupportedVersion(9))
        ));
        Ok(())
    }

    #[tokio::test]
    async fn load_instance_state_errors_never_carry_a_secret() -> Result<(), Box<dyn Error>> {
        // Arrange
        let stored = state()?;
        let use_case = use_case_with(|store, generator| {
            let persisted = stored.clone();
            store.expect_load().times(1).returning(move || {
                let value = persisted.clone();
                Box::pin(async move { Ok(Some(value)) })
            });
            generator.expect_generate().times(0);
            store.expect_save().times(0);
        });
        let supplied = hex64('a');
        let differing = hex64('c');

        // Act
        let result = use_case
            .execute(LoadInstanceStateCommand::new(
                true,
                IdentitySeeds::new(Some(differing.clone()), Some(supplied.clone())),
            ))
            .await;

        // Assert
        let message = result
            .map_err(|error| error.to_string())
            .err()
            .unwrap_or_default();
        assert!(
            !message.contains(differing.as_str()),
            "an error must not carry the supplied secret, got: {message:?}"
        );
        assert!(
            !message.contains(stored.pepper()),
            "an error must not carry the persisted secret, got: {message:?}"
        );
        Ok(())
    }
}
