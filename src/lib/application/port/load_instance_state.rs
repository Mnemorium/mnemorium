use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::future::Future;
use std::pin::Pin;

use crate::domain::model::instance_state::InstanceState;

/// Environment variable seeding the pepper at creation.
pub const PEPPER_VARIABLE: &str = "MNEMORIUM_PEPPER";

/// Environment variable seeding the signing secret at creation.
pub const JWT_SECRET_VARIABLE: &str = "MNEMORIUM_JWT_SECRET";

/// Length of a hexadecimal-encoded secret, in characters.
pub const SECRET_HEX_LENGTH: usize = 64;

/// Length of each generated secret, in bytes.
pub const SECRET_LENGTH: u32 = 32;

/// Path of the instance identity file, relative to the working directory.
pub const STATE_PATH: &str = "state.yaml";

/// Command carrying what the identity load needs from the boot sequence.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct LoadInstanceStateCommand {
    /// Whether the datastore file already exists.
    ///
    /// A datastore without an identity means the identity was lost: the use
    /// case refuses to create a replacement.
    datastore_exists: bool,
    /// Identity secrets supplied by the environment, applied only at creation.
    seeds: IdentitySeeds,
}

impl LoadInstanceStateCommand {
    /// Return whether the datastore file already exists.
    #[must_use]
    pub fn datastore_exists(&self) -> bool {
        self.datastore_exists
    }

    /// Create a new command.
    #[must_use]
    pub fn new(datastore_exists: bool, seeds: IdentitySeeds) -> Self {
        Self {
            datastore_exists,
            seeds,
        }
    }

    /// Return the identity secrets supplied by the environment.
    #[must_use]
    pub fn seeds(&self) -> &IdentitySeeds {
        &self.seeds
    }
}

/// Response of a successful identity load.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct LoadInstanceStateResponse {
    /// Whether this call created the identity.
    created: bool,
    /// The effective identity.
    state: InstanceState,
}

impl LoadInstanceStateResponse {
    /// Return whether this call created the identity.
    #[must_use]
    pub fn created(&self) -> bool {
        self.created
    }

    /// Create a new load response.
    #[must_use]
    pub fn new(state: InstanceState, created: bool) -> Self {
        Self { created, state }
    }

    /// Return the effective identity.
    #[must_use]
    pub fn state(&self) -> &InstanceState {
        &self.state
    }
}

/// Error returned when loading or creating the instance identity fails.
///
/// The messages name `state.yaml` and the recovery, never a secret value
/// (`OBS-004`).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LoadInstanceStateError {
    /// The persisted identity cannot be decoded or fails validation.
    #[error(
        "the instance identity in state.yaml is corrupt; restore state.yaml from a backup — deleting it does not regenerate credentials: {0}"
    )]
    IdentityCorrupt(#[source] anyhow::Error),
    /// The datastore exists but no identity is persisted.
    #[error(
        "the instance identity file state.yaml is missing while mnemorium.db exists; restore state.yaml from a backup — deleting the datastore does not regenerate credentials"
    )]
    IdentityMissing,
    /// An environment seed is not a 64-character hexadecimal string.
    #[error("the environment variable {variable} must hold 64 hexadecimal characters")]
    InvalidSeed {
        /// Name of the environment variable carrying the rejected seed.
        variable: &'static str,
    },
    /// An environment seed differs from the persisted identity.
    #[error(
        "the environment variable {variable} differs from the persisted instance identity; remove it or restore the matching state.yaml"
    )]
    SeedMismatch {
        /// Name of the environment variable carrying the differing seed.
        variable: &'static str,
    },
    /// The identity could not be read or persisted.
    #[error("the instance identity store failed: {0}")]
    StoreFailed(#[source] anyhow::Error),
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
    /// The persisted identity declares a format version this build does not
    /// support.
    #[error("the instance identity in state.yaml declares unsupported format version {0}")]
    UnsupportedVersion(u32),
}

/// Identity secrets supplied by the environment, applied only at creation.
///
/// The field names are the variable names below the shared prefix, which is
/// what `envy` maps, so adding a seed is adding a field rather than another
/// environment read in the composition root. The fields stay strings so a
/// value never reaches an error chain (`OBS-004`): the shape is checked by the
/// use case, whose errors name the variable and never the value. Unknown names
/// are not denied here because every configuration variable carries the same
/// prefix; the configuration resolver rejects them.
#[derive(Default, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct IdentitySeeds {
    /// Signing secret supplied by the environment.
    jwt_secret: Option<String>,
    /// Pepper supplied by the environment.
    pepper: Option<String>,
}

impl Debug for IdentitySeeds {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("IdentitySeeds")
            .field("jwt_secret", &self.jwt_secret.as_ref().map(|_| "**"))
            .field("pepper", &self.pepper.as_ref().map(|_| "**"))
            .finish_non_exhaustive()
    }
}

impl IdentitySeeds {
    /// Return the signing secret supplied by the environment.
    #[must_use]
    pub fn jwt_secret(&self) -> Option<&str> {
        self.jwt_secret.as_deref()
    }

    /// Create a seed set from explicit values.
    #[must_use]
    pub fn new(pepper: Option<String>, jwt_secret: Option<String>) -> Self {
        Self { jwt_secret, pepper }
    }

    /// Return the pepper supplied by the environment.
    #[must_use]
    pub fn pepper(&self) -> Option<&str> {
        self.pepper.as_deref()
    }
}

/// Use case for loading or creating the instance identity.
#[cfg_attr(test, mockall::automock)]
pub trait LoadInstanceStateUseCase: Send + Sync {
    /// Load the persisted identity, or create it for a genuinely new instance.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as
    /// `Arc<dyn LoadInstanceStateUseCase>` (`STY-RUST-070`).
    fn execute<'future>(
        &'future self,
        command: LoadInstanceStateCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<LoadInstanceStateResponse, LoadInstanceStateError>>
                + Send
                + 'future,
        >,
    >;
}
