use chrono::NaiveDateTime;

use crate::domain::alias::NumericID;

/// Required length of `Upload::md5_integrity`, mirroring the
/// `chk_upload_md5_integrity` check constraint.
pub const MD5_INTEGRITY_LENGTH: usize = 32;

/// Hard upper bound on the number of chunks an upload may be split into.
///
/// Bounds the `chunk_bitmap` blob to 8 MiB (`MAX_TOTAL_CHUNKS / 8`) regardless
/// of configuration, as a backstop against an unbounded allocation.
pub const MAX_TOTAL_CHUNKS: usize = 67_108_864;

/// Error returned when initialising or updating a `ChunkBitmap`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ChunkBitmapError {
    /// The chunk number is outside the bitmap range.
    #[error("chunk number must be lower than the total number of chunks")]
    ChunkNumberOutOfRange,
    /// The byte representation is not long enough for the total number of
    /// chunks.
    #[error("the byte representation is too short for the total number of chunks")]
    InvalidLength,
    /// The total number of chunks is zero.
    #[error("the total number of chunks must be greater than zero")]
    TotalChunksZero,
}

/// Bitmap tracking which chunks of an upload have been received.
///
/// Bit `n` of the underlying bytes (least-significant bit first) is set when
/// chunk `n` has been received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkBitmap {
    /// Raw bitmap bytes.
    bytes: Vec<u8>,
    /// Number of chunks the bitmap tracks.
    total_chunks: usize,
}

impl ChunkBitmap {
    /// Return the raw bitmap bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Return the byte index and bit mask of `chunk_number`.
    ///
    /// # Errors
    ///
    /// Returns [`ChunkBitmapError::ChunkNumberOutOfRange`] when `chunk_number`
    /// is not lower than [`ChunkBitmap::total_chunks`].
    fn bit_position(&self, chunk_number: usize) -> Result<(usize, u8), ChunkBitmapError> {
        if chunk_number >= self.total_chunks {
            return Err(ChunkBitmapError::ChunkNumberOutOfRange);
        }
        #[expect(
            clippy::integer_division,
            reason = "the byte index of a bit is its chunk number divided by eight"
        )]
        let byte_index = chunk_number / 8;
        let bit_index = chunk_number % 8;
        let mask = 1u8
            .checked_shl(u32::try_from(bit_index).unwrap_or(0))
            .unwrap_or(0);
        Ok((byte_index, mask))
    }

    /// Initialise a bitmap from its raw byte representation.
    ///
    /// # Errors
    ///
    /// Returns [`ChunkBitmapError::InvalidLength`] when `bytes` is shorter than
    /// the representation of `total_chunks` chunks, and
    /// [`ChunkBitmapError::TotalChunksZero`] when `total_chunks` is zero.
    pub fn from_bytes(bytes: Vec<u8>, total_chunks: usize) -> Result<Self, ChunkBitmapError> {
        if total_chunks == 0 {
            return Err(ChunkBitmapError::TotalChunksZero);
        }
        if bytes.len() < byte_count(total_chunks) {
            return Err(ChunkBitmapError::InvalidLength);
        }
        Ok(Self {
            bytes,
            total_chunks,
        })
    }

    /// Return whether every tracked chunk has been received.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        (0..self.total_chunks).all(|chunk_number| self.is_received(chunk_number).unwrap_or(false))
    }

    /// Return whether chunk `chunk_number` has been received.
    ///
    /// # Errors
    ///
    /// Returns [`ChunkBitmapError::ChunkNumberOutOfRange`] when `chunk_number`
    /// is not lower than [`ChunkBitmap::total_chunks`].
    pub fn is_received(&self, chunk_number: usize) -> Result<bool, ChunkBitmapError> {
        let (byte_index, mask) = self.bit_position(chunk_number)?;
        Ok(self
            .bytes
            .get(byte_index)
            .is_some_and(|byte| byte & mask != 0))
    }

    /// Mark chunk `chunk_number` as received.
    ///
    /// # Errors
    ///
    /// Returns [`ChunkBitmapError::ChunkNumberOutOfRange`] when `chunk_number`
    /// is not lower than [`ChunkBitmap::total_chunks`].
    pub fn mark_received(&mut self, chunk_number: usize) -> Result<(), ChunkBitmapError> {
        let (byte_index, mask) = self.bit_position(chunk_number)?;
        if let Some(byte) = self.bytes.get_mut(byte_index) {
            *byte |= mask;
        }
        Ok(())
    }

    /// Return the number of chunks the bitmap tracks.
    #[must_use]
    pub fn total_chunks(&self) -> usize {
        self.total_chunks
    }

    /// Initialise a bitmap tracking `total_chunks` chunks, none of them
    /// received.
    ///
    /// # Errors
    ///
    /// Returns [`ChunkBitmapError::TotalChunksZero`] when `total_chunks` is
    /// zero.
    pub fn try_new(total_chunks: usize) -> Result<Self, ChunkBitmapError> {
        if total_chunks == 0 {
            return Err(ChunkBitmapError::TotalChunksZero);
        }
        Ok(Self {
            bytes: vec![0; byte_count(total_chunks)],
            total_chunks,
        })
    }
}

/// Error returned when initialising or updating an `Upload`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum UploadError {
    /// The chunk number is outside the upload range.
    #[error("chunk number must be lower than the total number of chunks")]
    ChunkNumberOutOfRange,
    /// The chunk size is zero.
    #[error("chunk size must be greater than zero")]
    InvalidChunkSize,
    /// The file name is empty or unsafe.
    #[error(
        "file name must be non-empty, must not start with a dot, and must not contain a path separator"
    )]
    InvalidFileName,
    /// The file size is zero.
    #[error("file size must be greater than zero")]
    InvalidFileSize,
    /// The `md5_integrity` is not exactly [`MD5_INTEGRITY_LENGTH`] characters
    /// long.
    #[error("md5_integrity must be exactly {MD5_INTEGRITY_LENGTH} characters long")]
    Md5IntegrityInvalidLength,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
    /// The upload is not complete and cannot be finished.
    #[error("the upload is not complete and cannot be finished")]
    UploadIncomplete,
}

/// An in-progress or finished chunked upload session.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::struct_field_names,
    reason = "`upload_id` is the primary key name mandated by the SQL section and the domain field naming"
)]
pub struct Upload {
    /// Bitmap of the chunks received so far.
    chunk_bitmap: ChunkBitmap,
    /// Size of every chunk except the last, in bytes.
    chunk_size: u64,
    /// Date and time at which the upload was created.
    created_at: NaiveDateTime,
    /// Original name of the file being uploaded.
    file_name: String,
    /// Total size of the file being uploaded, in bytes.
    file_size: u64,
    /// Whether the upload has been finished.
    is_finished: bool,
    /// MD5 digest of the complete file, supplied by the client.
    md5_integrity: String,
    /// Identifier of the mime type of the file being uploaded.
    mime_type_id: String,
    /// Unique identifier of the upload.
    upload_id: NumericID,
    /// Identifier of the user owning the upload.
    user_id: NumericID,
    /// Optimistic-concurrency version, bumped on every persisted change.
    version: i64,
}

impl Upload {
    /// Return whether the upload can be finished, i.e. every chunk has been
    /// received.
    #[must_use]
    pub fn can_finish(&self) -> bool {
        self.chunk_bitmap.is_complete()
    }

    /// Return the raw bitmap of the received chunks.
    #[must_use]
    pub fn chunk_bitmap(&self) -> &ChunkBitmap {
        &self.chunk_bitmap
    }

    /// Return the size of every chunk except the last, in bytes.
    #[must_use]
    pub fn chunk_size(&self) -> u64 {
        self.chunk_size
    }

    /// Return the date and time at which the upload was created.
    #[must_use]
    pub fn created_at(&self) -> NaiveDateTime {
        self.created_at
    }

    /// Return the original name of the file being uploaded.
    #[must_use]
    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    /// Return the total size of the file being uploaded, in bytes.
    #[must_use]
    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    /// Mark the upload as finished.
    pub fn finish(&mut self) {
        self.is_finished = true;
    }

    /// Return whether the upload has been finished.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.is_finished
    }

    /// Mark chunk `chunk_number` as received.
    ///
    /// # Errors
    ///
    /// Returns [`UploadError::ChunkNumberOutOfRange`] when `chunk_number` is
    /// not lower than [`Upload::total_chunks`].
    pub fn mark_chunk_received(&mut self, chunk_number: usize) -> Result<(), UploadError> {
        self.chunk_bitmap
            .mark_received(chunk_number)
            .map_err(|error| match error {
                ChunkBitmapError::ChunkNumberOutOfRange => UploadError::ChunkNumberOutOfRange,
                other @ (ChunkBitmapError::InvalidLength | ChunkBitmapError::TotalChunksZero) => {
                    UploadError::Unknown(anyhow::Error::new(other))
                }
            })
    }

    /// Return the MD5 digest of the complete file.
    #[must_use]
    pub fn md5_integrity(&self) -> &str {
        &self.md5_integrity
    }

    /// Return the identifier of the mime type of the file being uploaded.
    #[must_use]
    pub fn mime_type_id(&self) -> &str {
        &self.mime_type_id
    }

    /// Update the persisted chunk bitmap.
    pub fn set_chunk_bitmap(&mut self, chunk_bitmap: ChunkBitmap) {
        self.chunk_bitmap = chunk_bitmap;
    }

    /// Update whether the upload has been finished.
    pub fn set_is_finished(&mut self, is_finished: bool) {
        self.is_finished = is_finished;
    }

    /// Return the total number of chunks the upload is split into.
    ///
    /// The file size is always greater than zero and the chunk size always
    /// positive, so the result is at least one.
    #[must_use]
    pub fn total_chunks(&self) -> usize {
        let total = self.file_size.div_ceil(self.chunk_size);
        usize::try_from(total).unwrap_or(usize::MAX)
    }

    /// Initialise a new `Upload`, validating every field.
    ///
    /// # Errors
    ///
    /// Returns [`UploadError::InvalidFileName`] when `file_name` is unsafe,
    /// [`UploadError::InvalidFileSize`] when `file_size` is zero,
    /// [`UploadError::InvalidChunkSize`] when `chunk_size` is zero, and
    /// [`UploadError::Md5IntegrityInvalidLength`] when `md5_integrity` is not
    /// exactly [`MD5_INTEGRITY_LENGTH`] characters long.
    #[expect(
        clippy::too_many_arguments,
        reason = "a persisted aggregate needs every column; the constructor mirrors the row"
    )]
    pub fn try_new(
        upload_id: NumericID,
        user_id: NumericID,
        file_name: String,
        file_size: u64,
        mime_type_id: String,
        chunk_size: u64,
        md5_integrity: String,
        chunk_bitmap: ChunkBitmap,
        is_finished: bool,
        version: i64,
        created_at: NaiveDateTime,
    ) -> Result<Self, UploadError> {
        let validated_file_name = Self::validate_file_name(file_name)?;
        let validated_file_size = Self::validate_file_size(file_size)?;
        let validated_chunk_size = Self::validate_chunk_size(chunk_size)?;
        let validated_md5_integrity = Self::validate_md5_integrity(md5_integrity)?;
        Ok(Self {
            chunk_bitmap,
            chunk_size: validated_chunk_size,
            created_at,
            file_name: validated_file_name,
            file_size: validated_file_size,
            is_finished,
            md5_integrity: validated_md5_integrity,
            mime_type_id,
            upload_id,
            user_id,
            version,
        })
    }

    /// Return the unique identifier of the upload.
    #[must_use]
    pub fn upload_id(&self) -> NumericID {
        self.upload_id
    }

    /// Return the identifier of the user owning the upload.
    #[must_use]
    pub fn user_id(&self) -> NumericID {
        self.user_id
    }

    /// Validate `chunk_size`.
    ///
    /// # Errors
    ///
    /// Returns [`UploadError::InvalidChunkSize`] when `chunk_size` is zero.
    #[expect(
        clippy::single_call_fn,
        reason = "the validation is named after the field it guards for readability"
    )]
    fn validate_chunk_size(chunk_size: u64) -> Result<u64, UploadError> {
        if chunk_size == 0 {
            return Err(UploadError::InvalidChunkSize);
        }
        Ok(chunk_size)
    }

    /// Validate `file_name`.
    ///
    /// # Errors
    ///
    /// Returns [`UploadError::InvalidFileName`] when `file_name` is empty,
    /// starts with a dot, contains a path separator, or contains `..`.
    #[expect(
        clippy::single_call_fn,
        reason = "the validation is named after the field it guards for readability"
    )]
    fn validate_file_name(file_name: String) -> Result<String, UploadError> {
        let is_safe = !file_name.is_empty()
            && !file_name.starts_with('.')
            && !file_name.contains(['/', '\\'])
            && !file_name.contains("..");
        if is_safe {
            Ok(file_name)
        } else {
            Err(UploadError::InvalidFileName)
        }
    }

    /// Validate `file_size`.
    ///
    /// # Errors
    ///
    /// Returns [`UploadError::InvalidFileSize`] when `file_size` is zero.
    #[expect(
        clippy::single_call_fn,
        reason = "the validation is named after the field it guards for readability"
    )]
    fn validate_file_size(file_size: u64) -> Result<u64, UploadError> {
        if file_size == 0 {
            return Err(UploadError::InvalidFileSize);
        }
        Ok(file_size)
    }

    /// Validate `md5_integrity`.
    ///
    /// # Errors
    ///
    /// Returns [`UploadError::Md5IntegrityInvalidLength`] when `md5_integrity`
    /// is not exactly [`MD5_INTEGRITY_LENGTH`] characters long.
    #[expect(
        clippy::single_call_fn,
        reason = "the validation is named after the field it guards for readability"
    )]
    fn validate_md5_integrity(md5_integrity: String) -> Result<String, UploadError> {
        if md5_integrity.chars().count() == MD5_INTEGRITY_LENGTH {
            Ok(md5_integrity)
        } else {
            Err(UploadError::Md5IntegrityInvalidLength)
        }
    }

    /// Return the optimistic-concurrency version.
    #[must_use]
    pub fn version(&self) -> i64 {
        self.version
    }
}

/// Return the number of bytes needed to track `total_chunks` chunks.
fn byte_count(total_chunks: usize) -> usize {
    total_chunks.div_ceil(8)
}
