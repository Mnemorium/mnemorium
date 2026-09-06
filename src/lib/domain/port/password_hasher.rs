use std::future::Future;

use crate::domain::port::error::PasswordHasherError;

/// Port for hashing and verifying passwords.
#[cfg_attr(test, mockall::automock)]
pub trait PasswordHasher: Send + Sync {
    /// Hash `password`, returning an algorithm-agnostic encoded hash.
    fn hash_password(
        &self,
        password: &str,
    ) -> impl Future<Output = Result<String, PasswordHasherError>> + Send;

    /// Verify `password` against `hash`.
    ///
    /// Returns `Ok(true)` when `password` matches `hash`, and `Ok(false)` when
    /// it does not; a mismatch is a valid outcome, not an error.
    fn verify_password(
        &self,
        password: &str,
        hash: &str,
    ) -> impl Future<Output = Result<bool, PasswordHasherError>> + Send;
}
