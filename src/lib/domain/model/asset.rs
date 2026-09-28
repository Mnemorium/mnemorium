/// Default root directory holding the staged and final files.
pub const DEFAULT_STORAGE_ROOT: &str = "data";

/// Default size of every chunk except the last, in bytes.
pub const DEFAULT_CHUNK_SIZE_BYTES: u64 = 5_242_880;

/// Default lifetime of an upload session, in seconds.
pub const DEFAULT_EXPIRY_SECONDS: u64 = 86_400;

/// Default maximum size of a single uploaded file, in bytes (100 GiB).
pub const DEFAULT_MAX_FILE_SIZE_BYTES: u64 = 107_374_182_400;

/// Error returned when initialising or updating an asset value object.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AssetError {
    /// The chunk size is zero.
    #[error("asset upload chunk_size_bytes must be greater than zero")]
    ChunkSizeZero,
    /// The upload expiry is zero.
    #[error("asset upload expiry_seconds must be greater than zero")]
    ExpiryZero,
    /// The maximum file size does not fit in a signed 64-bit integer.
    #[error("asset upload max_file_size_bytes must fit in a signed 64-bit integer")]
    MaxFileSizeTooLarge,
    /// The maximum file size is zero.
    #[error("asset upload max_file_size_bytes must be greater than zero")]
    MaxFileSizeZero,
    /// The storage root is empty.
    #[error("asset storage root must not be empty")]
    RootEmpty,
}

/// Storage-related settings of the Asset bounded context.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[non_exhaustive]
pub struct AssetStorage {
    /// Root directory holding the staged and final files.
    root: String,
}

impl AssetStorage {
    /// Return the root directory holding the staged and final files.
    #[must_use]
    pub fn root(&self) -> &str {
        &self.root
    }

    /// Update the root directory holding the staged and final files.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::RootEmpty`] when `root` is empty.
    pub fn set_root(&mut self, root: String) -> Result<(), AssetError> {
        self.root = Self::validate_root(root)?;
        Ok(())
    }

    /// Initialise a new `AssetStorage`, validating `root` is non-empty.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::RootEmpty`] when `root` is empty.
    pub fn try_new(root: String) -> Result<Self, AssetError> {
        let validated_root = Self::validate_root(root)?;
        Ok(Self {
            root: validated_root,
        })
    }

    /// Validate `root`.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::RootEmpty`] when `root` is empty.
    fn validate_root(root: String) -> Result<String, AssetError> {
        if root.is_empty() {
            return Err(AssetError::RootEmpty);
        }
        Ok(root)
    }
}

impl Default for AssetStorage {
    fn default() -> Self {
        Self {
            root: DEFAULT_STORAGE_ROOT.to_owned(),
        }
    }
}

/// Upload-related settings of the Asset bounded context.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[non_exhaustive]
pub struct AssetUpload {
    /// Size of every chunk except the last, in bytes.
    chunk_size_bytes: u64,
    /// Lifetime of an upload session, in seconds.
    expiry_seconds: u64,
    /// Maximum size of a single uploaded file, in bytes.
    max_file_size_bytes: u64,
}

impl AssetUpload {
    /// Return the size of every chunk except the last, in bytes.
    #[must_use]
    pub fn chunk_size_bytes(&self) -> u64 {
        self.chunk_size_bytes
    }

    /// Return the lifetime of an upload session, in seconds.
    #[must_use]
    pub fn expiry_seconds(&self) -> u64 {
        self.expiry_seconds
    }

    /// Return the maximum size of a single uploaded file, in bytes.
    #[must_use]
    pub fn max_file_size_bytes(&self) -> u64 {
        self.max_file_size_bytes
    }

    /// Update the size of every chunk except the last, in bytes.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::ChunkSizeZero`] when `chunk_size_bytes` is zero.
    pub fn set_chunk_size_bytes(&mut self, chunk_size_bytes: u64) -> Result<(), AssetError> {
        self.chunk_size_bytes = Self::validate_chunk_size_bytes(chunk_size_bytes)?;
        Ok(())
    }

    /// Update the lifetime of an upload session, in seconds.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::ExpiryZero`] when `expiry_seconds` is zero.
    pub fn set_expiry_seconds(&mut self, expiry_seconds: u64) -> Result<(), AssetError> {
        self.expiry_seconds = Self::validate_expiry_seconds(expiry_seconds)?;
        Ok(())
    }

    /// Update the maximum size of a single uploaded file, in bytes.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::MaxFileSizeZero`] when `max_file_size_bytes` is
    /// zero, and [`AssetError::MaxFileSizeTooLarge`] when it exceeds
    /// [`i64::MAX`].
    pub fn set_max_file_size_bytes(&mut self, max_file_size_bytes: u64) -> Result<(), AssetError> {
        self.max_file_size_bytes = Self::validate_max_file_size_bytes(max_file_size_bytes)?;
        Ok(())
    }

    /// Initialise a new `AssetUpload`, validating every setting.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::ChunkSizeZero`] when `chunk_size_bytes` is zero,
    /// [`AssetError::ExpiryZero`] when `expiry_seconds` is zero,
    /// [`AssetError::MaxFileSizeZero`] when `max_file_size_bytes` is zero, and
    /// [`AssetError::MaxFileSizeTooLarge`] when it exceeds [`i64::MAX`].
    pub fn try_new(
        chunk_size_bytes: u64,
        expiry_seconds: u64,
        max_file_size_bytes: u64,
    ) -> Result<Self, AssetError> {
        let validated_chunk_size = Self::validate_chunk_size_bytes(chunk_size_bytes)?;
        let validated_expiry = Self::validate_expiry_seconds(expiry_seconds)?;
        let validated_max_file_size = Self::validate_max_file_size_bytes(max_file_size_bytes)?;
        Ok(Self {
            chunk_size_bytes: validated_chunk_size,
            expiry_seconds: validated_expiry,
            max_file_size_bytes: validated_max_file_size,
        })
    }

    /// Validate `chunk_size_bytes`.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::ChunkSizeZero`] when `chunk_size_bytes` is zero.
    fn validate_chunk_size_bytes(chunk_size_bytes: u64) -> Result<u64, AssetError> {
        if chunk_size_bytes == 0 {
            return Err(AssetError::ChunkSizeZero);
        }
        Ok(chunk_size_bytes)
    }

    /// Validate `expiry_seconds`.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::ExpiryZero`] when `expiry_seconds` is zero.
    fn validate_expiry_seconds(expiry_seconds: u64) -> Result<u64, AssetError> {
        if expiry_seconds == 0 {
            return Err(AssetError::ExpiryZero);
        }
        Ok(expiry_seconds)
    }

    /// Validate `max_file_size_bytes`.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::MaxFileSizeZero`] when `max_file_size_bytes` is
    /// zero, and [`AssetError::MaxFileSizeTooLarge`] when it exceeds
    /// [`i64::MAX`].
    fn validate_max_file_size_bytes(max_file_size_bytes: u64) -> Result<u64, AssetError> {
        if max_file_size_bytes == 0 {
            return Err(AssetError::MaxFileSizeZero);
        }
        if max_file_size_bytes > i64::MAX as u64 {
            return Err(AssetError::MaxFileSizeTooLarge);
        }
        Ok(max_file_size_bytes)
    }
}

impl Default for AssetUpload {
    fn default() -> Self {
        Self {
            chunk_size_bytes: DEFAULT_CHUNK_SIZE_BYTES,
            expiry_seconds: DEFAULT_EXPIRY_SECONDS,
            max_file_size_bytes: DEFAULT_MAX_FILE_SIZE_BYTES,
        }
    }
}

/// Asset-related settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[non_exhaustive]
pub struct Asset {
    /// Storage settings.
    storage: AssetStorage,
    /// Upload settings.
    upload: AssetUpload,
}

impl Asset {
    /// Initialise a new `Asset`.
    #[must_use]
    pub fn new(storage: AssetStorage, upload: AssetUpload) -> Self {
        Self { storage, upload }
    }

    /// Return the storage settings.
    #[must_use]
    pub fn storage(&self) -> &AssetStorage {
        &self.storage
    }

    /// Return the upload settings.
    #[must_use]
    pub fn upload(&self) -> &AssetUpload {
        &self.upload
    }
}
