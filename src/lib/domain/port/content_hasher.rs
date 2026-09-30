use crate::domain::model::integrity_hash::IntegrityHash;
use crate::domain::model::integrity_hash::SHA256_HEX_LENGTH;
use crate::domain::port::error::ContentHasherError;

/// Port for incrementally hashing byte content.
///
/// A caller starts a session with [`ContentHasher::hasher`], feeds it bytes as
/// they arrive, and finalizes it into an [`IntegrityHash`]. The primitive is
/// algorithm-agnostic: the adapter chooses the algorithm the domain pins by
/// digest length.
#[cfg_attr(test, mockall::automock)]
pub trait ContentHasher: Send + Sync {
    /// Start a new incremental hashing session.
    fn hasher(&self) -> Box<dyn ContentHasherSession>;
}

/// One incremental hashing session.
#[cfg_attr(test, mockall::automock)]
pub trait ContentHasherSession: Send {
    /// Consume the session, returning the digest of everything fed so far.
    ///
    /// # Errors
    ///
    /// Returns [`ContentHasherError`] when the digest cannot be finalized.
    fn finalize(self: Box<Self>) -> Result<IntegrityHash<SHA256_HEX_LENGTH>, ContentHasherError>;

    /// Feed `bytes` into the digest.
    ///
    /// The operation is infallible.
    fn update(&mut self, bytes: &[u8]);
}
