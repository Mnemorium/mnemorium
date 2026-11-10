use crate::domain::model::{DIGEST_HEX_LENGTH, IntegrityHash};
use crate::domain::port::ContentHasherError;

/// Port for incrementally hashing byte content.
///
/// A caller starts a session with [`ContentHasher::hasher`], feeds it bytes as
/// they arrive, and finalizes it into an [`IntegrityHash`]. The primitive is
/// algorithm-agnostic: the adapter chooses the algorithm the domain pins by
/// digest length.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait ContentHasher: Send + Sync {
    /// Start a new incremental hashing session.
    fn hasher(&self) -> Box<dyn ContentHasherSession>;
}

/// One incremental hashing session.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait ContentHasherSession: Send + Sync {
    /// Consume the session, returning the digest of everything fed so far.
    ///
    /// # Errors
    ///
    /// Returns [`ContentHasherError`] when the digest cannot be finalized.
    fn finalize(self: Box<Self>) -> Result<IntegrityHash<DIGEST_HEX_LENGTH>, ContentHasherError>;

    /// Feed `bytes` into the digest.
    ///
    /// The operation is infallible.
    fn update(&mut self, bytes: &[u8]);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Assert `T` implements `Send + Sync`.
    fn assert_send_sync<T: Send + Sync + ?Sized>() {}

    #[test]
    fn content_hasher_session_is_send_and_sync() {
        // Act & Assert
        assert_send_sync::<dyn ContentHasherSession>();
    }
}
