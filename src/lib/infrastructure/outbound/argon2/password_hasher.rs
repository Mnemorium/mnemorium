use std::future::{Future, ready};

use argon2::Argon2;
use argon2::password_hash::Error as PasswordHashError;
use argon2::password_hash::{PasswordHasher as _, PasswordVerifier as _};
use tracing::error;

use crate::domain::port::error::PasswordHasherError;
use crate::domain::port::password_hasher::PasswordHasher;

/// Map a `password_hash` engine error to the port error.
impl From<PasswordHashError> for PasswordHasherError {
    fn from(err: PasswordHashError) -> Self {
        error!(
            error = ?err,
            "an error occurred while hashing or verifying a password"
        );
        match err {
            PasswordHashError::Crypto => {
                Self::Unknown(anyhow::anyhow!("the crypto backend failed"))
            }
            PasswordHashError::Algorithm
            | PasswordHashError::EncodingInvalid
            | PasswordHashError::Internal
            | PasswordHashError::OutOfMemory
            | PasswordHashError::OutputSize
            | PasswordHashError::ParamInvalid { .. }
            | PasswordHashError::ParamsInvalid
            | PasswordHashError::PasswordInvalid
            | PasswordHashError::RngFailure
            | PasswordHashError::SaltInvalid
            | PasswordHashError::Version
            | _ => Self::OperationFailed,
        }
    }
}

/// Password hasher backed by the Argon2 algorithm.
pub struct Argon2PasswordHasher {
    /// The Argon2 engine configured with the recommended default parameters.
    inner: Argon2<'static>,
}

impl Default for Argon2PasswordHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl Argon2PasswordHasher {
    /// Create a new hasher using Argon2's recommended default parameters.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Argon2::default(),
        }
    }
}

impl PasswordHasher for Argon2PasswordHasher {
    fn hash_password(
        &self,
        password: &str,
    ) -> impl Future<Output = Result<String, PasswordHasherError>> + Send {
        let result = self.inner.hash_password(password.as_bytes());
        ready(
            result
                .map(|hash| hash.to_string())
                .map_err(PasswordHasherError::from),
        )
    }

    fn verify_password(
        &self,
        password: &str,
        hash: &str,
    ) -> impl Future<Output = Result<bool, PasswordHasherError>> + Send {
        let result = match self.inner.verify_password(password.as_bytes(), hash) {
            Ok(()) => Ok(true),
            Err(PasswordHashError::PasswordInvalid) => Ok(false),
            Err(err) => Err(PasswordHasherError::from(err)),
        };

        ready(result)
    }
}
