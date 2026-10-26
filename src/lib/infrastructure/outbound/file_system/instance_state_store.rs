use std::ffi::OsStr;
use std::ffi::OsString;
use std::fs::File;
use std::fs::OpenOptions;
use std::fs::Permissions;
use std::fs::rename;
use std::future::Future;
use std::future::ready;
use std::io::Error;
use std::io::ErrorKind;
use std::io::Read as _;
use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::path::PathBuf;

use tracing::error;
use tracing::warn;

use crate::domain::model::instance_state::InstanceState;
use crate::domain::port::error::InstanceStateStoreError;
use crate::domain::port::instance_state_store::InstanceStateStore;

/// Format version this build writes and understands.
const STATE_FORMAT_VERSION: u32 = 1;

/// Largest identity document this build will decode, in bytes.
const MAX_STATE_FILE_BYTES: usize = 4096;

/// Read limit handed to the reader: one byte past the cap, so an oversized
/// document is detected before it is decoded.
const READ_LIMIT_BYTES: u64 = 4097;

/// Mode of the identity file and its lock file.
///
/// The identity holds the key material every password hash and every token
/// depends on, so only the account the server runs as may read or write it.
const STATE_FILE_MODE: u32 = 0o600;

/// File-backed instance identity store.
///
/// The artifact is a small versioned YAML document. The adapter owns the file
/// struct, so the format version never reaches the domain model. Writes are
/// atomic (temp file, `sync_all`, rename, directory sync), and creation is
/// guarded by an exclusive lock on a sibling `.lock` file so two processes
/// starting against the same fresh instance cannot each persist a different
/// identity.
#[derive(Debug)]
pub struct FileInstanceStateStore {
    /// Path of the identity document.
    path: PathBuf,
}

impl FileInstanceStateStore {
    /// Return the directory holding the identity file.
    ///
    /// The parent of a bare file name is the empty path, which does not open:
    /// a relative `state.yaml` therefore resolves to the working directory.
    fn directory(&self) -> &Path {
        self.path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
    }

    /// Map an I/O failure onto the port error, logging the catalogued
    /// `port_fault` with the classification only (`OBS-002`, `OBS-003`).
    fn map_io_error(failure: Error, operation: &str) -> InstanceStateStoreError {
        let kind = failure.kind();
        error!(
            target: "security",
            event = "port_fault",
            kind = ?kind,
            operation = operation,
            "an outbound dependency failed"
        );
        InstanceStateStoreError::OperationFailed(anyhow::Error::new(failure))
    }

    /// Create a new store persisting the identity at `path`.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Read the identity document, reporting a missing file as `None` and
    /// rejecting a document larger than the cap before decoding it.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read or exceeds the cap.
    fn read(&self) -> Result<Option<String>, InstanceStateStoreError> {
        let file = match File::open(&self.path) {
            Ok(file) => file,
            Err(failure) if failure.kind() == ErrorKind::NotFound => return Ok(None),
            Err(failure) => return Err(Self::map_io_error(failure, "load_instance_state")),
        };

        let mut bytes = Vec::new();
        file.take(READ_LIMIT_BYTES)
            .read_to_end(&mut bytes)
            .map_err(|failure| Self::map_io_error(failure, "load_instance_state"))?;
        if bytes.len() > MAX_STATE_FILE_BYTES {
            warn!(
                target: "security",
                event = "output_validation_failed",
                reason = "instance_identity_oversized",
                "the persisted instance identity failed validation"
            );
            return Err(InstanceStateStoreError::Corrupt(anyhow::anyhow!(
                "the instance identity exceeds {MAX_STATE_FILE_BYTES} bytes"
            )));
        }

        String::from_utf8(bytes).map_or_else(
            |failure| {
                warn!(
                    target: "security",
                    event = "output_validation_failed",
                    reason = "instance_identity_unreadable",
                    "the persisted instance identity failed validation"
                );
                Err(InstanceStateStoreError::Corrupt(anyhow::Error::new(
                    failure,
                )))
            },
            |contents| Ok(Some(contents)),
        )
    }

    /// Persist `state` under an exclusive lock, refusing to replace an identity
    /// another process created first.
    ///
    /// # Errors
    ///
    /// Returns [`InstanceStateStoreError::Conflict`] when an identity already
    /// exists, and an error when the lock cannot be taken or the document
    /// cannot be serialized or written.
    fn save_locked(&self, state: &InstanceState) -> Result<(), InstanceStateStoreError> {
        let lock_path = self.sibling(".lock");
        let lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .mode(STATE_FILE_MODE)
            .open(&lock_path)
            .map_err(|failure| Self::map_io_error(failure, "save_instance_state"))?;
        lock.lock()
            .map_err(|failure| Self::map_io_error(failure, "save_instance_state"))?;

        if self.path.exists() {
            return Err(InstanceStateStoreError::Conflict);
        }

        let document = StateFile::from(state);
        let contents = yaml_serde::to_string(&document).map_err(|failure| {
            InstanceStateStoreError::OperationFailed(anyhow::Error::new(failure))
        })?;
        self.write_atomic(&contents)
    }

    /// Build the sibling path carrying `suffix` after the file name.
    fn sibling(&self, suffix: &str) -> PathBuf {
        let mut name = self
            .path
            .file_name()
            .map_or_else(OsString::new, OsStr::to_os_string);
        name.push(suffix);
        self.path.with_file_name(name)
    }

    /// Write `contents` to a sibling temp file, sync it, then rename it into
    /// place and sync the directory so a crash never leaves a partial document.
    ///
    /// # Errors
    ///
    /// Returns an error when the temp file cannot be written, synced or
    /// renamed.
    fn write_atomic(&self, contents: &str) -> Result<(), InstanceStateStoreError> {
        let temporary = self.sibling(".tmp");
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(STATE_FILE_MODE)
            .open(&temporary)
            .map_err(|failure| Self::map_io_error(failure, "save_instance_state"))?;
        // `mode` only applies on creation, so state the permission explicitly
        // for a temp file a previous run may have left behind.
        file.set_permissions(Permissions::from_mode(STATE_FILE_MODE))
            .map_err(|failure| Self::map_io_error(failure, "save_instance_state"))?;
        file.write_all(contents.as_bytes())
            .map_err(|failure| Self::map_io_error(failure, "save_instance_state"))?;
        file.sync_all()
            .map_err(|failure| Self::map_io_error(failure, "save_instance_state"))?;
        drop(file);

        rename(&temporary, &self.path)
            .map_err(|failure| Self::map_io_error(failure, "save_instance_state"))?;

        File::open(self.directory())
            .and_then(|handle| handle.sync_all())
            .map_err(|failure| Self::map_io_error(failure, "save_instance_state"))
    }
}

impl InstanceStateStore for FileInstanceStateStore {
    fn load(
        &self,
    ) -> impl Future<Output = Result<Option<InstanceState>, InstanceStateStoreError>> + Send {
        let contents = self.read();
        ready(contents.and_then(|bytes| {
            bytes
                .map(
                    |document| -> Result<InstanceState, InstanceStateStoreError> {
                        let probe: VersionProbe =
                            yaml_serde::from_str(&document).map_err(|failure| {
                                warn!(
                                    target: "security",
                                    event = "output_validation_failed",
                                    reason = "instance_identity_unreadable",
                                    "the persisted instance identity failed validation"
                                );
                                InstanceStateStoreError::Corrupt(anyhow::Error::new(failure))
                            })?;
                        if probe.version != STATE_FORMAT_VERSION {
                            return Err(InstanceStateStoreError::UnsupportedVersion(probe.version));
                        }
                        let file: StateFile =
                            yaml_serde::from_str(&document).map_err(|failure| {
                                warn!(
                                    target: "security",
                                    event = "output_validation_failed",
                                    reason = "instance_identity_unreadable",
                                    "the persisted instance identity failed validation"
                                );
                                InstanceStateStoreError::Corrupt(anyhow::Error::new(failure))
                            })?;
                        InstanceState::try_new(file.created_at, file.pepper, file.jwt_secret)
                            .map_err(|rejection| {
                                warn!(
                                    target: "security",
                                    event = "output_validation_failed",
                                    reason = "instance_identity_invalid",
                                    "the persisted instance identity failed validation"
                                );
                                InstanceStateStoreError::Corrupt(anyhow::Error::new(rejection))
                            })
                    },
                )
                .transpose()
        }))
    }

    fn save(
        &self,
        state: &InstanceState,
    ) -> impl Future<Output = Result<(), InstanceStateStoreError>> + Send {
        ready(self.save_locked(state))
    }
}

/// Version field, decoded alone so an unknown version is named before the body
/// is validated.
#[derive(serde::Deserialize)]
struct VersionProbe {
    /// Format version of the document.
    version: u32,
}

/// Persisted identity document.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct StateFile {
    /// Instant the identity was established, as Unix seconds.
    created_at: i64,
    /// Token signing secret, as 64 hexadecimal characters.
    jwt_secret: String,
    /// Site-wide pepper, as 64 hexadecimal characters.
    pepper: String,
    /// Format version of the document.
    version: u32,
}

impl From<&InstanceState> for StateFile {
    fn from(state: &InstanceState) -> Self {
        Self {
            created_at: state.created_at(),
            jwt_secret: state.jwt_secret().to_owned(),
            pepper: state.pepper().to_owned(),
            version: STATE_FORMAT_VERSION,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::fs;
    use std::iter::repeat_n;
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::Barrier;
    use std::thread;

    use tempfile::tempdir;

    use crate::domain::model::instance_state::InstanceState;
    use crate::domain::port::error::InstanceStateStoreError;
    use crate::domain::port::instance_state_store::InstanceStateStore as _;

    use super::FileInstanceStateStore;

    /// A 64-character hexadecimal string.
    fn hex64(character: char) -> String {
        repeat_n(character, 64).collect()
    }

    /// An identity valid for every adapter test.
    fn state() -> Result<InstanceState, Box<dyn Error>> {
        Ok(InstanceState::try_new(
            1_700_000_000,
            hex64('a'),
            hex64('b'),
        )?)
    }

    /// Write a raw document at `path`.
    fn write_document(path: &Path, contents: &str) -> Result<(), Box<dyn Error>> {
        fs::write(path, contents)?;
        Ok(())
    }

    #[tokio::test]
    async fn file_instance_state_store_round_trips_a_state() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let store = FileInstanceStateStore::new(directory.path().join("state.yaml"));
        let expected = state()?;

        // Act
        store.save(&expected).await?;
        let actual = store.load().await?;

        // Assert
        assert_eq!(actual, Some(expected));
        Ok(())
    }

    #[test]
    fn file_instance_state_store_bare_name_resolves_to_the_working_directory() {
        // Arrange
        let store = FileInstanceStateStore::new(PathBuf::from("state.yaml"));

        // Act
        let directory = store.directory();

        // Assert
        assert_eq!(
            directory,
            Path::new("."),
            "a bare identity file name must resolve to the working directory"
        );
    }

    #[tokio::test]
    async fn file_instance_state_store_missing_file_loads_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let store = FileInstanceStateStore::new(directory.path().join("state.yaml"));

        // Act
        let actual = store.load().await?;

        // Assert
        assert_eq!(actual, None, "a missing identity file is not an error");
        Ok(())
    }

    #[tokio::test]
    async fn file_instance_state_store_unsupported_version_is_named() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("state.yaml");
        write_document(
            &path,
            &format!(
                "version: 2\ncreated_at: 1\npepper: \"{}\"\njwt_secret: \"{}\"\n",
                hex64('a'),
                hex64('b')
            ),
        )?;
        let store = FileInstanceStateStore::new(path.clone());

        // Act
        let result = store.load().await;

        // Assert
        assert!(
            matches!(result, Err(InstanceStateStoreError::UnsupportedVersion(2))),
            "an unknown format version must be named, got: {result:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn file_instance_state_store_unknown_field_is_rejected() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("state.yaml");
        write_document(
            &path,
            &format!(
                "version: 1\ncreated_at: 1\npepper: \"{}\"\njwt_secret: \"{}\"\ninstance_id: 7\n",
                hex64('a'),
                hex64('b')
            ),
        )?;
        let store = FileInstanceStateStore::new(path.clone());

        // Act
        let result = store.load().await;

        // Assert
        assert!(
            matches!(result, Err(InstanceStateStoreError::Corrupt(_))),
            "an unknown field must fail instead of being ignored, got: {result:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn file_instance_state_store_non_hex_secret_is_rejected() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("state.yaml");
        write_document(
            &path,
            "version: 1\ncreated_at: 1\npepper: \"not-a-pepper\"\njwt_secret: \"also-not\"\n",
        )?;
        let store = FileInstanceStateStore::new(path.clone());

        // Act
        let result = store.load().await;

        // Assert
        assert!(
            matches!(result, Err(InstanceStateStoreError::Corrupt(_))),
            "a stored secret that is not 64 hex characters must be corrupt, got: {result:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn file_instance_state_store_truncated_document_is_rejected() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("state.yaml");
        write_document(&path, "version: 1\ncreated_at: 1\npepper: \"aaaa")?;
        let store = FileInstanceStateStore::new(path.clone());

        // Act
        let result = store.load().await;

        // Assert
        assert!(
            matches!(result, Err(InstanceStateStoreError::Corrupt(_))),
            "a truncated document must fail instead of being treated as absent, got: {result:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn file_instance_state_store_oversized_document_is_rejected() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("state.yaml");
        write_document(&path, &"a".repeat(5000))?;
        let store = FileInstanceStateStore::new(path.clone());

        // Act
        let result = store.load().await;

        // Assert
        assert!(
            matches!(result, Err(InstanceStateStoreError::Corrupt(_))),
            "an oversized document must be rejected before decoding, got: {result:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn file_instance_state_store_writes_mode_0600() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("state.yaml");
        let store = FileInstanceStateStore::new(path.clone());

        // Act
        store.save(&state()?).await?;

        // Assert
        let mode = fs::metadata(&path)?.permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "only the server account may read or write the identity"
        );
        Ok(())
    }

    #[tokio::test]
    async fn file_instance_state_store_never_replaces_an_existing_identity()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("state.yaml");
        let store = FileInstanceStateStore::new(path.clone());
        store.save(&state()?).await?;
        let replacement = InstanceState::try_new(1, hex64('c'), hex64('d'))?;

        // Act
        let result = store.save(&replacement).await;

        // Assert
        assert!(
            matches!(result, Err(InstanceStateStoreError::Conflict)),
            "an existing identity must never be replaced, got: {result:?}"
        );
        assert_eq!(
            store.load().await?,
            Some(state()?),
            "the persisted identity must be left unchanged"
        );
        Ok(())
    }

    #[test]
    fn file_instance_state_store_concurrent_creation_persists_once() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("state.yaml");
        let barrier = Arc::new(Barrier::new(4));

        // Act
        let handles = (0u8..4)
            .map(|offset| {
                let store = FileInstanceStateStore::new(path.clone());
                let shared_barrier = Arc::clone(&barrier);
                let character = char::from(b'a' + offset);
                thread::spawn(move || {
                    let state = InstanceState::try_new(
                        1,
                        repeat_n(character, 64).collect(),
                        repeat_n(character, 64).collect(),
                    )
                    .map_err(|error| InstanceStateStoreError::Corrupt(anyhow::Error::new(error)))?;
                    // Every writer waits so the load-or-create sections
                    // genuinely interleave instead of running one after another.
                    shared_barrier.wait();
                    store.save_locked(&state)
                })
            })
            .collect::<Vec<_>>();
        let outcomes: Vec<Result<(), InstanceStateStoreError>> = handles
            .into_iter()
            .map(|handle| handle.join().map_err(|_| "a writer thread panicked"))
            .collect::<Result<Vec<_>, _>>()?;

        // Assert
        assert_eq!(
            outcomes.iter().filter(|outcome| outcome.is_ok()).count(),
            1,
            "exactly one process may persist an identity, got: {outcomes:?}"
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| matches!(outcome, Err(InstanceStateStoreError::Conflict)))
                .count(),
            3,
            "every other creator must fail, not overwrite"
        );
        assert!(
            path.exists(),
            "the winning identity must be persisted for the next start"
        );
        Ok(())
    }
}
