use chrono::NaiveDate;
use chrono::NaiveDateTime;

use crate::domain::alias::NumericID;
use crate::domain::model::integrity_hash::DIGEST_HEX_LENGTH;
use crate::domain::model::integrity_hash::IntegrityHash;
use crate::domain::model::integrity_hash::IntegrityHashError;
use crate::domain::model::permission::Protected;
use crate::domain::model::permission::ResourceKind;
use crate::domain::model::permission::ResourceRef;

/// Error returned when initialising or updating a `File`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FileError {
    /// The integrity hash is malformed.
    #[error(transparent)]
    IntegrityHash(#[from] IntegrityHashError),
    /// The recorded size is negative.
    #[error("size_bytes must not be negative")]
    InvalidSizeBytes,
    /// The path is empty.
    #[error("path must not be empty")]
    PathEmpty,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Whether the server stored the file's bytes.
///
/// The default is [`Origin::Scanned`], the fail-safe direction: a row created by
/// a path that forgets to declare its origin is treated as content the server
/// did not create, which a future deletion policy must never remove.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Origin {
    /// The bytes were found under the library root and were not written by the
    /// server.
    #[default]
    Scanned,
    /// The bytes were received through the upload flow and promoted by the
    /// server.
    Uploaded,
}

impl Origin {
    /// Return the persisted representation.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Scanned => "SCANNED",
            Self::Uploaded => "UPLOADED",
        }
    }

    /// Parse the persisted representation, or `None` when it is not known.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "SCANNED" => Some(Self::Scanned),
            "UPLOADED" => Some(Self::Uploaded),
            _ => None,
        }
    }
}

/// A file stored on the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    /// Unique identifier of the file.
    id: NumericID,
    /// Integrity hash of the file content, absent for a scanned file.
    integrity_hash: Option<IntegrityHash<DIGEST_HEX_LENGTH>>,
    /// Whether the file is publicly accessible.
    is_public: bool,
    /// Identifier of the mime type of the file.
    mime_type_id: String,
    /// Instant at which the content was last modified.
    modified_at: NaiveDateTime,
    /// Origin of the file's bytes.
    origin: Origin,
    /// Path of the file relative to the library root.
    path: String,
    /// Size of the file content, in bytes.
    size_bytes: i64,
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

    /// Return the integrity hash of the file content, if it carries one.
    #[must_use]
    pub fn integrity_hash(&self) -> Option<&IntegrityHash<DIGEST_HEX_LENGTH>> {
        self.integrity_hash.as_ref()
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

    /// Return the instant at which the content was last modified.
    #[must_use]
    pub fn modified_at(&self) -> NaiveDateTime {
        self.modified_at
    }

    /// Return the origin of the file's bytes.
    #[must_use]
    pub fn origin(&self) -> Origin {
        self.origin
    }

    /// Return the path relative to the library root.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Update the `integrity_hash`.
    pub fn set_integrity_hash(&mut self, integrity_hash: IntegrityHash<DIGEST_HEX_LENGTH>) {
        self.integrity_hash = Some(integrity_hash);
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

    /// Return the size of the content, in bytes.
    #[must_use]
    pub fn size_bytes(&self) -> i64 {
        self.size_bytes
    }

    /// Initialise a new `File`, validating `path` and `size_bytes`.
    ///
    /// An absent `origin` defaults to [`Origin::Scanned`].
    ///
    /// # Errors
    ///
    /// Returns [`FileError::PathEmpty`] when `path` is empty, and
    /// [`FileError::InvalidSizeBytes`] when `size_bytes` is negative.
    #[expect(
        clippy::too_many_arguments,
        reason = "try_new mirrors all file columns"
    )]
    pub fn try_new(
        id: NumericID,
        path: String,
        user_id: NumericID,
        is_public: bool,
        mime_type_id: String,
        uploaded_at: NaiveDate,
        size_bytes: i64,
        modified_at: NaiveDateTime,
        integrity_hash: Option<IntegrityHash<DIGEST_HEX_LENGTH>>,
        origin: Option<Origin>,
    ) -> Result<Self, FileError> {
        let validated_path = Self::validate_path(path)?;
        if size_bytes < 0 {
            return Err(FileError::InvalidSizeBytes);
        }
        Ok(Self {
            id,
            integrity_hash,
            is_public,
            mime_type_id,
            modified_at,
            origin: origin.unwrap_or_default(),
            path: validated_path,
            size_bytes,
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

impl Protected for File {
    fn is_privileged_account(&self) -> bool {
        false
    }

    fn is_publicly_visible(&self) -> bool {
        self.is_public()
    }

    fn owner_id(&self) -> Option<NumericID> {
        Some(self.user_id)
    }

    fn resource_ref(&self) -> Option<ResourceRef> {
        Some(ResourceRef::new(ResourceKind::File, self.id))
    }
}
