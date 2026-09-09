use chrono::NaiveDate;

use crate::domain::alias::NumericID;

/// Required length of `File::md5_integrity`, mirroring the
/// `chk_file_md5_integrity` check constraint.
pub const MD5_INTEGRITY_LENGTH: usize = 128;

/// Error returned when initialising or updating a `File`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FileError {
    /// The `md5_integrity` is not exactly [`MD5_INTEGRITY_LENGTH`] characters
    /// long.
    #[error("md5_integrity must be exactly {MD5_INTEGRITY_LENGTH} characters long")]
    Md5IntegrityInvalidLength,
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
    /// Whether the file is publicly accessible.
    is_public: bool,
    /// MD5 integrity digest of the file content.
    md5_integrity: String,
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

    /// Return whether the file is publicly accessible.
    #[must_use]
    pub fn is_public(&self) -> bool {
        self.is_public
    }

    /// Return the MD5 integrity digest.
    #[must_use]
    pub fn md5_integrity(&self) -> &str {
        &self.md5_integrity
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

    /// Update whether the file is publicly accessible.
    pub fn set_is_public(&mut self, is_public: bool) {
        self.is_public = is_public;
    }

    /// Update the `md5_integrity`.
    ///
    /// # Errors
    ///
    /// Returns [`FileError::Md5IntegrityInvalidLength`] when `md5_integrity`
    /// is not exactly [`MD5_INTEGRITY_LENGTH`] characters long.
    pub fn set_md5_integrity(&mut self, md5_integrity: String) -> Result<(), FileError> {
        self.md5_integrity = Self::validate_md5_integrity(md5_integrity)?;
        Ok(())
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

    /// Initialise a new `File`, validating `path` and `md5_integrity`.
    ///
    /// # Errors
    ///
    /// Returns [`FileError::PathEmpty`] when `path` is empty, and
    /// [`FileError::Md5IntegrityInvalidLength`] when `md5_integrity` is not
    /// exactly [`MD5_INTEGRITY_LENGTH`] characters long.
    pub fn try_new(
        id: NumericID,
        path: String,
        user_id: NumericID,
        is_public: bool,
        mime_type_id: String,
        uploaded_at: NaiveDate,
        md5_integrity: String,
    ) -> Result<Self, FileError> {
        let validated_path = Self::validate_path(path)?;
        let validated_md5_integrity = Self::validate_md5_integrity(md5_integrity)?;
        Ok(Self {
            id,
            is_public,
            md5_integrity: validated_md5_integrity,
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

    /// Validate and normalise `md5_integrity`.
    ///
    /// # Errors
    ///
    /// Returns [`FileError::Md5IntegrityInvalidLength`] when `md5_integrity`
    /// is not exactly [`MD5_INTEGRITY_LENGTH`] characters long.
    fn validate_md5_integrity(md5_integrity: String) -> Result<String, FileError> {
        if md5_integrity.chars().count() == MD5_INTEGRITY_LENGTH {
            Ok(md5_integrity)
        } else {
            Err(FileError::Md5IntegrityInvalidLength)
        }
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
