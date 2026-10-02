use chrono::NaiveDate;

use crate::domain::alias::NumericID;
use crate::domain::model::integrity_hash::IntegrityHash;
use crate::domain::model::integrity_hash::IntegrityHashError;
use crate::domain::model::integrity_hash::SHA256_HEX_LENGTH;

/// Error returned when initialising or updating a `File`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FileError {
    /// The integrity hash is malformed.
    #[error(transparent)]
    IntegrityHash(#[from] IntegrityHashError),
    /// The path is empty.
    #[error("path must not be empty")]
    PathEmpty,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// A file stored on the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    /// Unique identifier of the file.
    id: NumericID,
    /// Integrity hash of the file content.
    integrity_hash: IntegrityHash<SHA256_HEX_LENGTH>,
    /// Whether the file is publicly accessible.
    is_public: bool,
    /// Identifier of the mime type of the file.
    mime_type_id: String,
    /// Path of the file on the storage backend.
    path: String,
    /// Date at which the file was uploaded.
    uploaded_at: NaiveDate,
    /// Identifier of the user who owns the file.
    user_id: NumericID,
}

impl File {
    /// Return the unique identifier.
    #[must_use]
    pub fn id(&self) -> NumericID {
        self.id
    }

    /// Return the integrity hash of the file content.
    #[must_use]
    pub fn integrity_hash(&self) -> &IntegrityHash<SHA256_HEX_LENGTH> {
        &self.integrity_hash
    }

    /// Return whether the file is publicly accessible.
    #[must_use]
    pub fn is_public(&self) -> bool {
        self.is_public
    }

    /// Return the mime type identifier.
    #[must_use]
    pub fn mime_type_id(&self) -> &str {
        &self.mime_type_id
    }

    /// Return the path on the storage backend.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Update the `integrity_hash`.
    pub fn set_integrity_hash(&mut self, integrity_hash: IntegrityHash<SHA256_HEX_LENGTH>) {
        self.integrity_hash = integrity_hash;
    }

    /// Update whether the file is publicly accessible.
    pub fn set_is_public(&mut self, is_public: bool) {
        self.is_public = is_public;
    }

    /// Update the path.
    ///
    /// # Errors
    ///
    /// Returns [`FileError::PathEmpty`] when `path` is empty.
    pub fn set_path(&mut self, path: String) -> Result<(), FileError> {
        self.path = Self::validate_path(path)?;
        Ok(())
    }

    /// Initialise a new `File`, validating `path`.
    ///
    /// # Errors
    ///
    /// Returns [`FileError::PathEmpty`] when `path` is empty.
    pub fn try_new(
        id: NumericID,
        path: String,
        user_id: NumericID,
        is_public: bool,
        mime_type_id: String,
        uploaded_at: NaiveDate,
        integrity_hash: IntegrityHash<SHA256_HEX_LENGTH>,
    ) -> Result<Self, FileError> {
        let validated_path = Self::validate_path(path)?;
        Ok(Self {
            id,
            integrity_hash,
            is_public,
            mime_type_id,
            path: validated_path,
            uploaded_at,
            user_id,
        })
    }

    /// Return the date at which the file was uploaded.
    #[must_use]
    pub fn uploaded_at(&self) -> NaiveDate {
        self.uploaded_at
    }

    /// Return the identifier of the owning user.
    #[must_use]
    pub fn user_id(&self) -> NumericID {
        self.user_id
    }

    /// Validate `path`.
    ///
    /// # Errors
    ///
    /// Returns [`FileError::PathEmpty`] when `path` is empty.
    fn validate_path(path: String) -> Result<String, FileError> {
        if path.is_empty() {
            Err(FileError::PathEmpty)
        } else {
            Ok(path)
        }
    }
}
