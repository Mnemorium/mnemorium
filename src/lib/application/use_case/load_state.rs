use std::fmt::Write as _;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use crate::application::port::load_state::DATABASE_PATH;
use crate::application::port::load_state::JWT_SECRET_VARIABLE;
use crate::application::port::load_state::LoadStateError;
use crate::application::port::load_state::LoadStateResponse;
use crate::application::port::load_state::LoadStateUseCase;
use crate::application::port::load_state::PEPPER_VARIABLE;
use crate::application::port::load_state::SECRET_HEX_LENGTH;
use crate::application::port::load_state::SECRET_LENGTH;
use crate::application::port::load_state::StateSeedSource;
use crate::domain::model::instance_state::InstanceState;
use crate::domain::port::error::InstanceStateStoreError;
use crate::domain::port::instance_state_store::InstanceStateStore;
use crate::domain::port::secret_generator::SecretGenerator;

/// Use case implementation for loading or creating the instance state.
///
/// Mirrors `LoadConfiguration`: one persisted-source port, one environment port
/// and the composing logic in `execute`. The environment layer's rule is the
/// state's own — it verifies a persisted state instead of overriding it, and an
/// absent file is either creation or a hard failure, never "defaults".
pub struct LoadState<S, G, D> {
    /// Store holding the persisted state.
    instance_state_store: Arc<S>,
    /// Generator producing random secrets when no seed is supplied.
    secret_generator: Arc<G>,
    /// Source of the creation seeds supplied by the environment.
    seed_source: Arc<D>,
}

impl<S, G, D> LoadState<S, G, D>
where
    S: InstanceStateStore,
    G: SecretGenerator,
    D: StateSeedSource,
{
    /// Create and persist the state for a genuinely new instance.
    ///
    /// # Errors
    ///
    /// Returns an error when a secret cannot be generated, when the generated
    /// value fails validation, or when the store cannot persist it.
    async fn create(
        &self,
        pepper_seed: Option<&str>,
        jwt_secret_seed: Option<&str>,
    ) -> Result<(InstanceState, bool, bool), LoadStateError> {
        let (pepper, pepper_generated) = self.resolve_secret(pepper_seed).await?;
        let (jwt_secret, jwt_secret_generated) = self.resolve_secret(jwt_secret_seed).await?;
        let state = InstanceState::try_new(Utc::now().timestamp(), pepper, jwt_secret)
            .map_err(|error| LoadStateError::Unknown(error.into()))?;
        self.instance_state_store
            .save(&state)
            .await
            .map_err(map_store_error)?;
        Ok((state, pepper_generated, jwt_secret_generated))
    }

    /// Create a new use case.
    #[must_use]
    pub fn new(
        instance_state_store: Arc<S>,
        secret_generator: Arc<G>,
        seed_source: Arc<D>,
    ) -> Self {
        Self {
            instance_state_store,
            secret_generator,
            seed_source,
        }
    }

    /// Resolve one secret: the environment seed when present, a fresh random
    /// value otherwise.
    ///
    /// Returns the secret and whether it was generated.
    ///
    /// # Errors
    ///
    /// Returns [`LoadStateError::Unknown`] when the generator fails.
    async fn resolve_secret(&self, seed: Option<&str>) -> Result<(String, bool), LoadStateError> {
        if let Some(value) = seed {
            return Ok((value.to_owned(), false));
        }
        let bytes = self
            .secret_generator
            .generate(SECRET_LENGTH)
            .await
            .map_err(|error| LoadStateError::Unknown(error.into()))?;
        let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
        for byte in &bytes {
            let _result = write!(encoded, "{byte:02x}");
        }
        Ok((encoded, true))
    }

    /// Verify a supplied seed equals the persisted secret.
    ///
    /// The rejection names the variable, never either value (`OBS-004`).
    ///
    /// # Errors
    ///
    /// Returns [`LoadStateError::SeedMismatch`] when the two differ.
    fn verify_seed(
        seed: Option<&str>,
        persisted: &str,
        variable: &'static str,
    ) -> Result<(), LoadStateError> {
        match seed {
            Some(value) if value != persisted => Err(LoadStateError::SeedMismatch { variable }),
            _ => Ok(()),
        }
    }
}

impl<S, G, D> LoadStateUseCase for LoadState<S, G, D>
where
    S: InstanceStateStore,
    G: SecretGenerator,
    D: StateSeedSource,
{
    fn execute<'future>(
        &'future self,
    ) -> Pin<Box<dyn Future<Output = Result<LoadStateResponse, LoadStateError>> + Send + 'future>>
    {
        Box::pin(async move {
            // The check runs first: a failure to tell whether the datastore
            // exists must never reach creation, or a lost state file would be
            // replaced beside a live datastore. `try_exists`, not `exists`, so
            // an unreadable datastore fails startup instead of reading as a
            // fresh instance.
            let datastore_exists = Path::new(DATABASE_PATH).try_exists().map_err(|failure| {
                LoadStateError::DatastoreUnreadable(anyhow::anyhow!(
                    "cannot inspect {DATABASE_PATH}: {failure}"
                ))
            })?;
            let seeds = self
                .seed_source
                .read()
                .await
                .map_err(|error| LoadStateError::SeedsUnreadable(anyhow::Error::new(error)))?;
            let pepper = seeds.pepper();
            let jwt_secret = seeds.jwt_secret();
            validate_seed(pepper.as_deref(), PEPPER_VARIABLE)?;
            validate_seed(jwt_secret.as_deref(), JWT_SECRET_VARIABLE)?;

            match self
                .instance_state_store
                .load()
                .await
                .map_err(map_store_error)?
            {
                Some(state) => {
                    Self::verify_seed(pepper.as_deref(), state.pepper(), PEPPER_VARIABLE)?;
                    Self::verify_seed(
                        jwt_secret.as_deref(),
                        state.jwt_secret(),
                        JWT_SECRET_VARIABLE,
                    )?;
                    Ok(LoadStateResponse::new(state, false, false, false))
                }
                // A datastore without state means the state was lost; a
                // replacement would silently invalidate every password hash.
                None if datastore_exists => Err(LoadStateError::StateMissing),
                None => {
                    let (state, pepper_generated, jwt_secret_generated) = self
                        .create(pepper.as_deref(), jwt_secret.as_deref())
                        .await?;
                    Ok(LoadStateResponse::new(
                        state,
                        true,
                        pepper_generated,
                        jwt_secret_generated,
                    ))
                }
            }
        })
    }
}

/// Map an adapter failure onto the use-case error.
fn map_store_error(error: InstanceStateStoreError) -> LoadStateError {
    match error {
        InstanceStateStoreError::Conflict
        | InstanceStateStoreError::OperationFailed(_)
        | InstanceStateStoreError::Unknown(_) => {
            LoadStateError::StoreFailed(anyhow::Error::new(error))
        }
        InstanceStateStoreError::Corrupt(source) => LoadStateError::StateCorrupt(source),
        InstanceStateStoreError::UnsupportedVersion(version) => {
            LoadStateError::UnsupportedVersion(version)
        }
    }
}

/// Validate an optional seed is a 64-character hexadecimal string.
///
/// # Errors
///
/// Returns [`LoadStateError::InvalidSeed`] naming the variable when the seed is
/// present but malformed. The value itself never reaches the error.
fn validate_seed(seed: Option<&str>, variable: &'static str) -> Result<(), LoadStateError> {
    let is_hex = seed.is_some_and(|value| {
        value.len() == SECRET_HEX_LENGTH
            && value.chars().all(|character| character.is_ascii_hexdigit())
    });
    let is_malformed = seed.is_some() && !is_hex;
    if is_malformed {
        return Err(LoadStateError::InvalidSeed { variable });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::future::Future;
    use std::future::ready;
    use std::iter::repeat_n;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::PoisonError;

    use crate::application::port::load_state::LoadStateError;
    use crate::application::port::load_state::LoadStateUseCase as _;
    use crate::application::port::load_state::StateSeedSource;
    use crate::application::port::load_state::StateSeeds;
    use crate::domain::model::instance_state::InstanceState;
    use crate::domain::model::instance_state::InstanceStateError;
    use crate::domain::port::error::InstanceStateStoreError;
    use crate::domain::port::error::SecretGeneratorError;
    use crate::domain::port::error::StateSeedSourceError;
    use crate::domain::port::instance_state_store::MockInstanceStateStore;
    use crate::domain::port::secret_generator::MockSecretGenerator;

    use super::LoadState;

    type UseCase = LoadState<MockInstanceStateStore, MockSecretGenerator, TestSeedSource>;

    /// Record what the use case hands to `save`.
    type Saved = Arc<Mutex<Option<InstanceState>>>;

    /// The seeds a test supplies to the use case.
    #[derive(Default)]
    struct TestSeeds {
        /// Signing secret supplied by the test.
        jwt_secret: Option<String>,
        /// Pepper supplied by the test.
        pepper: Option<String>,
    }

    impl StateSeeds for TestSeeds {
        fn jwt_secret(&self) -> Option<String> {
            self.jwt_secret.clone()
        }

        fn pepper(&self) -> Option<String> {
            self.pepper.clone()
        }
    }

    /// Seed source returning the values a test supplied.
    struct TestSeedSource {
        /// Failure the source reports instead of the seeds.
        failure: bool,
        /// Seeds the source reports.
        seeds: TestSeeds,
    }

    impl StateSeedSource for TestSeedSource {
        fn read(
            &self,
        ) -> impl Future<Output = Result<Box<dyn StateSeeds>, StateSeedSourceError>> + Send
        {
            let failure = self.failure;
            let seeds = TestSeeds {
                jwt_secret: self.seeds.jwt_secret.clone(),
                pepper: self.seeds.pepper.clone(),
            };
            ready(if failure {
                Err(StateSeedSourceError::Unreadable(envy::Error::Custom(
                    "the environment is unreadable".to_owned(),
                )))
            } else {
                Ok(Box::new(seeds) as Box<dyn StateSeeds>)
            })
        }
    }

    /// Build a use case whose outbound dependencies are mocked; `setup` defines
    /// the expectations before the mocks are handed over.
    fn use_case_with(
        seeds: TestSeeds,
        setup: impl FnOnce(&mut MockInstanceStateStore, &mut MockSecretGenerator),
    ) -> UseCase {
        let mut store = MockInstanceStateStore::new();
        let mut generator = MockSecretGenerator::new();
        setup(&mut store, &mut generator);
        LoadState::new(
            Arc::new(store),
            Arc::new(generator),
            Arc::new(TestSeedSource {
                failure: false,
                seeds,
            }),
        )
    }

    /// A 64-character hexadecimal string.
    fn hex64(character: char) -> String {
        repeat_n(character, 64).collect()
    }

    /// A persisted state.
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
    async fn load_state_existing_state_is_reused() -> Result<(), Box<dyn Error>> {
        // Arrange
        let stored = state()?;
        let use_case = use_case_with(TestSeeds::default(), |store, generator| {
            let persisted = stored.clone();
            store.expect_load().times(1).returning(move || {
                let value = persisted.clone();
                Box::pin(async move { Ok(Some(value)) })
            });
            generator.expect_generate().times(0);
            store.expect_save().times(0);
        });

        // Act
        let response = use_case.execute().await?;

        // Assert
        assert!(
            !response.created(),
            "a persisted state must not be reported as created"
        );
        assert!(
            !response.pepper_generated() && !response.jwt_secret_generated(),
            "reuse generates nothing"
        );
        assert_eq!(response.state(), &state()?);
        Ok(())
    }

    #[tokio::test]
    async fn load_state_matching_seed_is_accepted() -> Result<(), Box<dyn Error>> {
        // Arrange
        let stored = state()?;
        let seeds = TestSeeds {
            jwt_secret: Some(hex64('b')),
            pepper: Some(hex64('a')),
        };
        let use_case = use_case_with(seeds, |store, generator| {
            let persisted = stored.clone();
            store.expect_load().times(1).returning(move || {
                let value = persisted.clone();
                Box::pin(async move { Ok(Some(value)) })
            });
            generator.expect_generate().times(0);
            store.expect_save().times(0);
        });

        // Act
        let response = use_case.execute().await?;

        // Assert
        assert!(!response.created());
        assert_eq!(response.state(), &state()?);
        Ok(())
    }

    #[tokio::test]
    async fn load_state_differing_seed_fails_without_touching_the_state()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let stored = state()?;
        let use_case = use_case_with(
            TestSeeds {
                jwt_secret: None,
                pepper: Some(hex64('c')),
            },
            |store, generator| {
                let persisted = stored.clone();
                store.expect_load().times(1).returning(move || {
                    let value = persisted.clone();
                    Box::pin(async move { Ok(Some(value)) })
                });
                generator.expect_generate().times(0);
                store.expect_save().times(0);
            },
        );

        // Act
        let result = use_case.execute().await;

        // Assert
        assert!(matches!(result, Err(LoadStateError::SeedMismatch { .. })));
        Ok(())
    }

    #[tokio::test]
    async fn load_state_invalid_seed_fails() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(
            TestSeeds {
                jwt_secret: None,
                pepper: Some("not-a-pepper".to_owned()),
            },
            |store, generator| {
                store.expect_load().times(0);
                generator.expect_generate().times(0);
                store.expect_save().times(0);
            },
        );

        // Act
        let result = use_case.execute().await;

        // Assert
        assert!(matches!(result, Err(LoadStateError::InvalidSeed { .. })));
        Ok(())
    }

    #[tokio::test]
    async fn load_state_store_failure_is_reported() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(TestSeeds::default(), |store, generator| {
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
        let result = use_case.execute().await;

        // Assert
        assert!(matches!(result, Err(LoadStateError::StoreFailed(_))));
        Ok(())
    }

    #[tokio::test]
    async fn load_state_unsupported_version_is_reported() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(TestSeeds::default(), |store, generator| {
            store.expect_load().times(1).returning(|| {
                Box::pin(async { Err(InstanceStateStoreError::UnsupportedVersion(9)) })
            });
            generator.expect_generate().times(0);
            store.expect_save().times(0);
        });

        // Act
        let result = use_case.execute().await;

        // Assert
        assert!(matches!(result, Err(LoadStateError::UnsupportedVersion(9))));
        Ok(())
    }

    #[tokio::test]
    async fn load_state_seed_source_failure_is_reported() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut store = MockInstanceStateStore::new();
        store.expect_load().times(0);
        let mut generator = MockSecretGenerator::new();
        generator.expect_generate().times(0);
        let use_case = LoadState::new(
            Arc::new(store),
            Arc::new(generator),
            Arc::new(TestSeedSource {
                failure: true,
                seeds: TestSeeds::default(),
            }),
        );

        // Act
        let result = use_case.execute().await;

        // Assert
        assert!(
            matches!(result, Err(LoadStateError::SeedsUnreadable(_))),
            "a seed-source failure must be reported as such, got: {result:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn load_state_errors_never_carry_a_secret() -> Result<(), Box<dyn Error>> {
        // Arrange
        let stored = state()?;
        let use_case = use_case_with(
            TestSeeds {
                jwt_secret: Some(hex64('b')),
                pepper: Some(hex64('c')),
            },
            |store, generator| {
                let persisted = stored.clone();
                store.expect_load().times(1).returning(move || {
                    let value = persisted.clone();
                    Box::pin(async move { Ok(Some(value)) })
                });
                generator.expect_generate().times(0);
                store.expect_save().times(0);
            },
        );
        let supplied = hex64('b');
        let differing = hex64('c');

        // Act
        let result = use_case.execute().await;

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
            !message.contains(supplied.as_str()),
            "an error must not carry the other supplied secret, got: {message:?}"
        );
        assert!(
            !message.contains(stored.pepper()),
            "an error must not carry the persisted secret, got: {message:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn load_state_creation_generates_both_secrets() -> Result<(), Box<dyn Error>> {
        // Arrange
        let saved: Saved = Arc::new(Mutex::new(None));
        let recorder = Arc::clone(&saved);
        let use_case = use_case_with(TestSeeds::default(), |store, generator| {
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
        let (state, pepper_generated, jwt_secret_generated) = use_case.create(None, None).await?;

        // Assert
        assert!(
            pepper_generated,
            "a pepper no seed supplied must be reported as generated"
        );
        assert!(
            jwt_secret_generated,
            "a signing secret no seed supplied must be reported as generated"
        );
        assert_eq!(state.pepper(), "aa".repeat(32));
        assert_eq!(state.jwt_secret(), "aa".repeat(32));
        assert!(
            state.created_at() > 0,
            "creation must record the instant the state was established"
        );
        assert_eq!(
            saved
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_ref(),
            Some(&state),
            "the created state must be persisted"
        );
        Ok(())
    }

    #[tokio::test]
    async fn load_state_creation_uses_a_supplied_seed() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pepper_seed = hex64('c');
        let use_case = use_case_with(TestSeeds::default(), |store, generator| {
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
        let (state, pepper_generated, jwt_secret_generated) =
            use_case.create(Some(pepper_seed.as_str()), None).await?;

        // Assert
        assert_eq!(
            state.pepper(),
            pepper_seed,
            "the supplied seed must be used"
        );
        assert!(!pepper_generated, "a supplied seed is not generated");
        assert_eq!(
            state.jwt_secret(),
            "bb".repeat(32),
            "an absent seed must be generated"
        );
        assert!(jwt_secret_generated);
        Ok(())
    }

    #[tokio::test]
    async fn load_state_creation_generator_failure_is_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(TestSeeds::default(), |store, generator| {
            generator
                .expect_generate()
                .times(1)
                .returning(|_| Box::pin(async { Err(SecretGeneratorError::OperationFailed) }));
            store.expect_save().times(0);
        });

        // Act
        let result = use_case.create(None, None).await;

        // Assert
        assert!(matches!(result, Err(LoadStateError::Unknown(_))));
        Ok(())
    }
}
