use std::future::Future;
use std::pin::Pin;

use crate::domain::model::instance_state::InstanceState;
use crate::domain::port::error::StateSeedSourceError;

/// Fixed path of the `SQLite3` datastore, relative to the working directory.
///
/// The path is not configurable: configuration resolves after the state, and
/// the container mounts `/app`, which already holds the datastore. The load
/// state use case inspects it to tell a genuinely new instance from one whose
/// state file was lost.
pub const DATABASE_PATH: &str = "mnemorium.db";

/// Environment variable seeding the pepper at creation.
pub const PEPPER_VARIABLE: &str = "MNEMORIUM_PEPPER";

/// Environment variable seeding the signing secret at creation.
pub const JWT_SECRET_VARIABLE: &str = "MNEMORIUM_JWT_SECRET";

/// Length of a hexadecimal-encoded secret, in characters.
pub const SECRET_HEX_LENGTH: usize = 64;

/// Length of each generated secret, in bytes.
pub const SECRET_LENGTH: u32 = 32;

/// Path of the instance state file, relative to the working directory.
pub const STATE_PATH: &str = "state.yaml";

/// Response of a successful state load.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct LoadStateResponse {
    /// Whether this call created the instance state.
    created: bool,
    /// Whether this call generated the signing secret rather than seeding it.
    jwt_secret_generated: bool,
    /// Whether this call generated the pepper rather than seeding it.
    pepper_generated: bool,
    /// The effective instance state.
    state: InstanceState,
}

impl LoadStateResponse {
    /// Return whether this call created the instance state.
    #[must_use]
    pub fn created(&self) -> bool {
        self.created
    }

    /// Return whether this call generated the signing secret.
    ///
    /// A created state reports `true` for a secret the environment did not
    /// seed; reuse reports `false` for both, because nothing was generated.
    #[must_use]
    pub fn jwt_secret_generated(&self) -> bool {
        self.jwt_secret_generated
    }

    /// Create a new load response.
    #[must_use]
    pub fn new(
        state: InstanceState,
        created: bool,
        pepper_generated: bool,
        jwt_secret_generated: bool,
    ) -> Self {
        Self {
            created,
            jwt_secret_generated,
            pepper_generated,
            state,
        }
    }

    /// Return whether this call generated the pepper.
    ///
    /// A created state reports `true` for a secret the environment did not
    /// seed; reuse reports `false` for both, because nothing was generated.
    #[must_use]
    pub fn pepper_generated(&self) -> bool {
        self.pepper_generated
    }

    /// Return the effective instance state.
    #[must_use]
    pub fn state(&self) -> &InstanceState {
        &self.state
    }
}

/// Error returned when loading or creating the instance state fails.
///
/// The messages name `state.yaml`, the environment variable and the recovery,
/// never a secret value (`OBS-004`).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LoadStateError {
    /// The presence of the datastore file could not be decided.
    #[error(
        "whether the datastore already exists could not be determined; check that mnemorium.db is readable and restart the server: {0}"
    )]
    DatastoreUnreadable(#[source] anyhow::Error),
    /// An environment seed is not a 64-character hexadecimal string.
    #[error("the environment variable {variable} must hold 64 hexadecimal characters")]
    InvalidSeed {
        /// Name of the environment variable carrying the rejected seed.
        variable: &'static str,
    },
    /// An environment seed differs from the persisted state.
    #[error(
        "the environment variable {variable} differs from the persisted instance state; remove it or restore the matching state.yaml"
    )]
    SeedMismatch {
        /// Name of the environment variable carrying the differing seed.
        variable: &'static str,
    },
    /// The creation seeds could not be read from the environment.
    #[error(
        "the instance state seeds could not be read from the environment; check the MNEMORIUM_* variables and restart the server: {0}"
    )]
    SeedsUnreadable(#[source] anyhow::Error),
    /// The persisted state cannot be decoded or fails validation.
    #[error(
        "the instance state in state.yaml is corrupt; restore state.yaml from a backup — deleting it does not regenerate credentials: {0}"
    )]
    StateCorrupt(#[source] anyhow::Error),
    /// The datastore exists but no state is persisted.
    #[error(
        "the instance state file state.yaml is missing while mnemorium.db exists; restore state.yaml from a backup — deleting the datastore does not regenerate credentials"
    )]
    StateMissing,
    /// The state could not be read or persisted.
    #[error("the instance state store failed: {0}")]
    StoreFailed(#[source] anyhow::Error),
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
    /// The persisted state declares a format version this build does not
    /// support.
    #[error("the instance state in state.yaml declares unsupported format version {0}")]
    UnsupportedVersion(u32),
}

/// The creation seeds, as the environment adapter supplies them.
///
/// The port's view of the seeds is a capability interface: the adapter owns the
/// struct that holds the values, so nothing in this layer names an adapter type
/// (`ARCH-001`). The accessors hand back owned strings so the trait object stays
/// simple to mock, and a value never reaches an error chain (`OBS-004`); the
/// shape is checked by the use case, whose errors name the variable and never
/// the value.
#[cfg_attr(test, mockall::automock)]
pub trait StateSeeds: Send + Sync {
    /// Return the signing secret supplied by the environment.
    fn jwt_secret(&self) -> Option<String>;

    /// Return the pepper supplied by the environment.
    fn pepper(&self) -> Option<String>;
}

/// Port for reading the creation seeds from the environment.
///
/// The seeds apply only when the instance state is created; when it already
/// exists, the use case compares them with the persisted secrets and fails on a
/// difference instead of overriding them.
#[cfg_attr(test, mockall::automock)]
pub trait StateSeedSource: Send + Sync {
    /// Read the seeds from the environment.
    ///
    /// # Errors
    ///
    /// Returns an error when the environment cannot be mapped onto the seed
    /// fields.
    fn read(
        &self,
    ) -> impl Future<Output = Result<Box<dyn StateSeeds>, StateSeedSourceError>> + Send;
}

/// Use case for loading or creating the instance state.
#[cfg_attr(test, mockall::automock)]
pub trait LoadStateUseCase: Send + Sync {
    /// Load the persisted state, or create it for a genuinely new instance.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as `Arc<dyn LoadStateUseCase>`
    /// (`STY-RUST-070`).
    fn execute<'future>(
        &'future self,
    ) -> Pin<Box<dyn Future<Output = Result<LoadStateResponse, LoadStateError>> + Send + 'future>>;
}
