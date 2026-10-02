use std::fmt::{Display, Formatter, Result as FmtResult};

/// Length of a SHA-256 digest in lowercase hexadecimal characters.
///
/// The constant is algorithm-neutral in the type it parameterizes
/// ([`IntegrityHash`]) but fixes the length of the digest persisted for file
/// content.
pub const SHA256_HEX_LENGTH: usize = 64;

/// Error returned when initialising an [`IntegrityHash`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum IntegrityHashError {
    /// The value is not exactly `expected` characters long.
    #[error("the integrity hash must be exactly {expected} characters long")]
    InvalidLength {
        /// Required length of the hash, in characters.
        expected: usize,
    },
    /// The value contains a character that is not an ASCII hexadecimal digit.
    #[error("the integrity hash must be a hexadecimal string")]
    NonHexadecimal,
}

/// A content-integrity digest.
///
/// The value is a lowercase hexadecimal string exactly `LENGTH` characters
/// long. The type is parameterized by the digest length so a caller can pin the
/// algorithm's output width without the name naming the algorithm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrityHash<const LENGTH: usize> {
    /// Lowercase hexadecimal digest.
    value: String,
}

impl<const LENGTH: usize> IntegrityHash<LENGTH> {
    /// Return the digest as a lowercase hexadecimal string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// Initialise a hash, validating its length and hexadecimal alphabet.
    ///
    /// # Errors
    ///
    /// Returns [`IntegrityHashError::InvalidLength`] when `value` is not exactly
    /// `LENGTH` bytes long, and [`IntegrityHashError::NonHexadecimal`] when it
    /// contains a character that is not an ASCII hexadecimal digit. A
    /// hexadecimal value is lowercased on success.
    pub fn try_new(mut value: String) -> Result<Self, IntegrityHashError> {
        if value.len() != LENGTH {
            return Err(IntegrityHashError::InvalidLength { expected: LENGTH });
        }
        if !value.chars().all(|character| character.is_ascii_hexdigit()) {
            return Err(IntegrityHashError::NonHexadecimal);
        }
        value.make_ascii_lowercase();
        Ok(Self { value })
    }
}

impl<const LENGTH: usize> Display for IntegrityHash<LENGTH> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(&self.value)
    }
}
