use std::future::Future;

use crate::domain::port::error::SecretGeneratorError;

/// Port for generating a site-wide pepper for password hashing.
#[cfg_attr(test, mockall::automock)]
pub trait SecretGenerator: Send + Sync {
    /// Generate a random secret.
    fn generate(
        &self,
        length: u32,
    ) -> impl Future<Output = Result<Vec<u8>, SecretGeneratorError>> + Send;
}
