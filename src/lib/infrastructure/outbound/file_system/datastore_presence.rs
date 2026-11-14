use std::io::ErrorKind;
use std::path::PathBuf;

use crate::domain::port::{DatastorePresence, DatastorePresenceError};

/// File-backed probe for the datastore's presence.
///
/// The probe tells a genuinely new instance from one whose state file was lost,
/// so an unreadable path must fail rather than read as absent. `try_exists`
/// reports a missing path as `Ok(false)`; the `NotFound` arm keeps that
/// contract explicit (`STY-RUST-016`).
#[derive(Debug)]
pub(crate) struct FileDatastorePresence {
    /// Path of the datastore file.
    path: PathBuf,
}

impl FileDatastorePresence {
    /// Create a probe for the datastore at `path`.
    #[must_use]
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl DatastorePresence for FileDatastorePresence {
    fn exists(&self) -> Result<bool, DatastorePresenceError> {
        match self.path.try_exists() {
            Ok(exists) => Ok(exists),
            Err(failure) if failure.kind() == ErrorKind::NotFound => Ok(false),
            Err(failure) => Err(DatastorePresenceError::Unreadable(failure)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use std::fs;
    use std::os::unix::fs::PermissionsExt as _;
    use tempfile::tempdir;

    #[test]
    fn file_datastore_presence_reports_an_existing_file() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("mnemorium.db");
        fs::write(&path, "")?;
        let presence = FileDatastorePresence::new(path);

        // Act
        let exists = presence.exists()?;

        // Assert
        assert!(exists, "an existing datastore must be reported as present");
        Ok(())
    }

    #[test]
    fn file_datastore_presence_reports_a_missing_file() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let presence = FileDatastorePresence::new(directory.path().join("mnemorium.db"));

        // Act
        let exists = presence.exists()?;

        // Assert
        assert!(!exists, "a missing datastore is a value, never an error");
        Ok(())
    }

    #[test]
    fn file_datastore_presence_reports_an_undecidable_path() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let sealed = directory.path().join("sealed");
        fs::create_dir_all(&sealed)?;
        let path = sealed.join("mnemorium.db");
        fs::set_permissions(&sealed, fs::Permissions::from_mode(0o000))?;

        // Act
        let result = FileDatastorePresence::new(path).exists();

        // Assert
        fs::set_permissions(&sealed, fs::Permissions::from_mode(0o700))?;
        assert!(
            matches!(result, Err(DatastorePresenceError::Unreadable(_))),
            "an unreadable path must fail closed, got: {result:?}"
        );
        Ok(())
    }
}
