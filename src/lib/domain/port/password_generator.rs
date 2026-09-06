use std::future::Future;

use crate::domain::port::error::PasswordGeneratorError;

/// Port for generating random passwords.
#[cfg_attr(test, mockall::automock)]
pub trait PasswordGenerator: Send + Sync {
    /// Generate a random password.
    fn generate(&self) -> impl Future<Output = Result<String, PasswordGeneratorError>> + Send;
}
