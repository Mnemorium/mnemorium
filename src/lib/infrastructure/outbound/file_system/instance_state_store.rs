use std::ffi::{OsStr, OsString};
use std::fs::{File, Metadata, OpenOptions, Permissions, metadata, rename};
use std::future::{Future, ready};
use std::io::{Error, ErrorKind, Read as _, Write as _};
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use tracing::{error, warn};

use crate::domain::model::InstanceState;
use crate::domain::port::{InstanceStateStore, InstanceStateStoreError};

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
pub(crate) struct FileInstanceStateStore {
    /// Path of the identity document.
    path: PathBuf,
}

/// How a persisted identity's privacy is judged before it is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Privacy {
    /// The file is regular, owned by the server account and owner-only.
    Private,
    /// The file is not a regular file, or another account owns it.
    Refuse,
    /// The file is regular and owned by the server account, but its mode is wider.
    Repair,
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

    /// Require the opened identity file to be private to the account that owns
    /// the working directory, tightening a wider mode to `0600` before reading.
    ///
    /// The check runs on the opened descriptor, so a symlink or a file swap
    /// between the check and the read cannot bypass it.
    ///
    /// # Errors
    ///
    /// Returns [`InstanceStateStoreError::Insecure`] when the file is not a
    /// regular file, is owned by another account, or its mode cannot be changed.
    fn ensure_private(&self, file: &File) -> Result<(), InstanceStateStoreError> {
        self.ensure_private_with(file, |opened| {
            opened.set_permissions(Permissions::from_mode(STATE_FILE_MODE))
        })
    }

    /// Apply the privacy rule, delegating the mode repair to `repair`.
    ///
    /// The seam lets the adapter test prove that a mode the server cannot
    /// repair fails startup, without a file system the test cannot create.
    ///
    /// # Errors
    ///
    /// Returns [`InstanceStateStoreError::Insecure`] when the file is not a
    /// regular file, is owned by another account, or `repair` fails.
    fn ensure_private_with<R>(&self, file: &File, repair: R) -> Result<(), InstanceStateStoreError>
    where
        R: FnOnce(&File) -> Result<(), Error>,
    {
        let directory_owner = metadata(self.directory())
            .map_err(|failure| Self::map_io_error(failure, "load_instance_state"))?
            .uid();
        let metadata = file
            .metadata()
            .map_err(|failure| Self::map_io_error(failure, "load_instance_state"))?;

        match assess_privacy(&metadata, directory_owner) {
            Privacy::Private => Ok(()),
            Privacy::Repair => repair(file).map_err(|_failure| {
                Self::warn_insecure();
                InstanceStateStoreError::Insecure
            }),
            Privacy::Refuse => {
                Self::warn_insecure();
                Err(InstanceStateStoreError::Insecure)
            }
        }
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
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Read the identity document, reporting a missing file as `None` and
    /// rejecting a document larger than the cap before decoding it.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read or exceeds the cap, and
    /// when the opened file is not private to the server account.
    fn read(&self) -> Result<Option<String>, InstanceStateStoreError> {
        let file = match File::open(&self.path) {
            Ok(file) => file,
            Err(failure) if failure.kind() == ErrorKind::NotFound => return Ok(None),
            Err(failure) => return Err(Self::map_io_error(failure, "load_instance_state")),
        };
        self.ensure_private(&file)?;

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

    /// Record the catalogued validation event for an identity that is not
    /// private, naming the classification and never a secret (`OBS-002`,
    /// `OBS-004`).
    fn warn_insecure() {
        warn!(
            target: "security",
            event = "output_validation_failed",
            reason = "instance_identity_insecure",
            "the persisted instance identity failed validation"
        );
    }

    /// Write `contents` to a sibling temp file, sync it, then rename it into
    /// place and sync the directory so a crash never leaves a partial document.
    ///
    /// # Errors
    ///
    /// Returns an error when the temp file cannot be written, synced or
    /// renamed.
    fn write_atomic(&self, contents: &str) -> Result<(), InstanceStateStoreError> {
        self.write_atomic_with(contents, |from, to| rename(from, to))
    }

    /// Write `contents` atomically, delegating the final rename to
    /// `rename_into_place`.
    ///
    /// The seam lets the adapter test prove that a failed rename leaves the
    /// previous identity in place, without a static or a process-level race.
    ///
    /// # Errors
    ///
    /// Returns an error when the temp file cannot be written or synced, when
    /// `rename_into_place` fails, or when the directory cannot be synced.
    fn write_atomic_with<R>(
        &self,
        contents: &str,
        rename_into_place: R,
    ) -> Result<(), InstanceStateStoreError>
    where
        R: FnOnce(&Path, &Path) -> Result<(), Error>,
    {
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

        rename_into_place(&temporary, &self.path)
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

/// Judge a persisted identity's privacy from its metadata and the working
/// directory's owner.
///
/// The owner check compares the file with the directory the state is written
/// into: the image chowns `/app` to the server account, so the working
/// directory's owner is the server account.
fn assess_privacy(metadata: &Metadata, directory_owner: u32) -> Privacy {
    if !metadata.is_file() || metadata.uid() != directory_owner {
        return Privacy::Refuse;
    }
    if metadata.mode() & 0o077 != 0 {
        Privacy::Repair
    } else {
        Privacy::Private
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::error::Error;
    use std::fs;
    use std::io::Error as IoError;
    use std::iter::repeat_n;
    use tempfile::tempdir;

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

    #[rstest]
    #[case::version_zero(0u32)]
    #[case::version_two(2u32)]
    #[tokio::test]
    async fn file_instance_state_store_unsupported_version_is_named(
        #[case] version: u32,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("state.yaml");
        let store = FileInstanceStateStore::new(path.clone());
        let document = format!(
            "version: {version}\ncreated_at: 1\npepper: \"{}\"\njwt_secret: \"{}\"\n",
            hex64('a'),
            hex64('b')
        );
        write_document(&path, &document)?;

        // Act
        let result = store.load().await;

        // Assert
        assert!(
            matches!(result, Err(InstanceStateStoreError::UnsupportedVersion(declared)) if declared == version),
            "an unsupported format version must be named, got: {result:?}"
        );
        assert_eq!(
            fs::read_to_string(&path)?,
            document,
            "the document must be left unchanged"
        );
        Ok(())
    }

    #[tokio::test]
    async fn file_instance_state_store_unknown_field_is_rejected() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("state.yaml");
        let document = format!(
            "version: 1\ncreated_at: 1\npepper: \"{}\"\njwt_secret: \"{}\"\ninstance_id: 7\n",
            hex64('a'),
            hex64('b')
        );
        write_document(&path, &document)?;
        let store = FileInstanceStateStore::new(path.clone());

        // Act
        let result = store.load().await;

        // Assert
        assert!(
            matches!(result, Err(InstanceStateStoreError::Corrupt(_))),
            "an unknown field must fail instead of being ignored, got: {result:?}"
        );
        assert_eq!(
            fs::read_to_string(&path)?,
            document,
            "the document must be left unchanged"
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
        let document = "version: 1\ncreated_at: 1\npepper: \"aaaa";
        write_document(&path, document)?;
        let store = FileInstanceStateStore::new(path.clone());

        // Act
        let result = store.load().await;

        // Assert
        assert!(
            matches!(result, Err(InstanceStateStoreError::Corrupt(_))),
            "a truncated document must fail instead of being treated as absent, got: {result:?}"
        );
        assert_eq!(
            fs::read_to_string(&path)?,
            document,
            "the document must be left unchanged"
        );
        Ok(())
    }

    #[tokio::test]
    async fn file_instance_state_store_oversized_document_is_rejected() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("state.yaml");
        let document = "a".repeat(5000);
        write_document(&path, &document)?;
        let store = FileInstanceStateStore::new(path.clone());

        // Act
        let result = store.load().await;

        // Assert
        assert!(
            matches!(result, Err(InstanceStateStoreError::Corrupt(_))),
            "an oversized document must be rejected before decoding, got: {result:?}"
        );
        assert_eq!(
            fs::read_to_string(&path)?,
            document,
            "the document must be left unchanged"
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

    #[test]
    fn file_instance_state_store_privacy_predicate_classifies_each_case()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let private_path = directory.path().join("private.yaml");
        fs::write(&private_path, "x")?;
        fs::set_permissions(&private_path, Permissions::from_mode(0o600))?;
        let wide_path = directory.path().join("wide.yaml");
        fs::write(&wide_path, "x")?;
        fs::set_permissions(&wide_path, Permissions::from_mode(0o644))?;
        let owner = fs::metadata(directory.path())?.uid();

        // Act
        let private = assess_privacy(&fs::metadata(&private_path)?, owner);
        let wide = assess_privacy(&fs::metadata(&wide_path)?, owner);
        let non_regular = assess_privacy(&fs::metadata(directory.path())?, owner);
        let foreign = assess_privacy(
            &fs::metadata(directory.path().join("private.yaml"))?,
            owner + 1,
        );

        // Assert
        assert_eq!(
            private,
            Privacy::Private,
            "an owner-only mode needs no repair"
        );
        assert_eq!(
            wide,
            Privacy::Repair,
            "a wider mode is tightened before reading"
        );
        assert_eq!(
            non_regular,
            Privacy::Refuse,
            "a state path that is not a regular file is refused"
        );
        assert_eq!(
            foreign,
            Privacy::Refuse,
            "a file another account owns is refused, never repaired"
        );
        Ok(())
    }

    #[tokio::test]
    async fn file_instance_state_store_non_regular_file_is_insecure() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("state.yaml");
        fs::create_dir_all(&path)?;
        let store = FileInstanceStateStore::new(path);

        // Act
        let result = store.load().await;

        // Assert
        assert!(
            matches!(result, Err(InstanceStateStoreError::Insecure)),
            "a state path that is not a regular file must fail startup, got: {result:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn file_instance_state_store_unrepairable_mode_is_insecure() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("state.yaml");
        fs::write(&path, "x")?;
        fs::set_permissions(&path, Permissions::from_mode(0o644))?;
        let store = FileInstanceStateStore::new(path.clone());
        let file = File::open(&path)?;

        // Act
        let result = store.ensure_private_with(&file, |_| {
            Err(IoError::new(ErrorKind::PermissionDenied, "cannot chmod"))
        });

        // Assert
        assert!(
            matches!(result, Err(InstanceStateStoreError::Insecure)),
            "a mode the server cannot repair must fail startup, got: {result:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn file_instance_state_store_failed_rename_leaves_the_previous_state()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let store = FileInstanceStateStore::new(directory.path().join("state.yaml"));
        store.save(&state()?).await?;
        let previous = store.load().await?;
        let replacement = InstanceState::try_new(2, hex64('c'), hex64('d'))?;
        let document = yaml_serde::to_string(&StateFile::from(&replacement))?;

        // Act
        let result =
            store.write_atomic_with(&document, |_, _| Err(IoError::other("rename failed")));

        // Assert
        assert!(
            matches!(result, Err(InstanceStateStoreError::OperationFailed(_))),
            "a failed rename must be reported, got: {result:?}"
        );
        assert_eq!(
            store.load().await?,
            previous,
            "the previous identity must stay loadable"
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
}
