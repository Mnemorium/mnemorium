use std::future::{Future, ready};

use argon2::Algorithm;
use argon2::Argon2;
use argon2::Params;
use argon2::Version;
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
    /// Site-wide secret mixed into every password hash as a pepper.
    pepper: Vec<u8>,
}

impl Argon2PasswordHasher {
    /// Build an Argon2 engine bound to this hasher's pepper, using the
    /// recommended default parameters.
    fn engine(&self) -> Result<Argon2<'_>, PasswordHasherError> {
        Argon2::new_with_secret(
            &self.pepper,
            Algorithm::default(),
            Version::default(),
            Params::default(),
        )
        .map_err(PasswordHashError::from)
        .map_err(PasswordHasherError::from)
    }

    /// Create a new hasher that mixes `pepper` into every password hash.
    #[must_use]
    pub fn new(pepper: Vec<u8>) -> Self {
        Self { pepper }
    }
}

impl PasswordHasher for Argon2PasswordHasher {
    fn hash_password(
        &self,
        password: &str,
    ) -> impl Future<Output = Result<String, PasswordHasherError>> + Send {
        let result = self.engine().and_then(|engine| {
            engine
                .hash_password(password.as_bytes())
                .map(|hash| hash.to_string())
                .map_err(PasswordHasherError::from)
        });
        ready(result)
    }

    fn verify_password(
        &self,
        password: &str,
        hash: &str,
    ) -> impl Future<Output = Result<bool, PasswordHasherError>> + Send {
        let result = self.engine().and_then(|engine| {
            match engine.verify_password(password.as_bytes(), hash) {
                Ok(()) => Ok(true),
                Err(PasswordHashError::PasswordInvalid) => Ok(false),
                Err(err) => Err(PasswordHasherError::from(err)),
            }
        });

        ready(result)
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::port::password_hasher::PasswordHasher as _;

    use super::Argon2PasswordHasher;

    fn pepper(value: u8) -> Vec<u8> {
        vec![value; 32]
    }

    #[tokio::test]
    async fn hash_then_verify_round_trips_password_with_same_pepper() -> Result<(), anyhow::Error> {
        // Arrange
        let hasher = Argon2PasswordHasher::new(pepper(7));

        // Act
        let hash = hasher.hash_password("s3cret").await?;

        // Assert
        assert!(hasher.verify_password("s3cret", &hash).await?);
        Ok(())
    }

    #[tokio::test]
    async fn verify_wrong_password_returns_false() -> Result<(), anyhow::Error> {
        // Arrange
        let hasher = Argon2PasswordHasher::new(pepper(7));
        let hash = hasher.hash_password("right-password").await?;

        // Act
        let result = hasher.verify_password("wrong-password", &hash).await?;

        // Assert
        assert!(!result);
        Ok(())
    }

    #[tokio::test]
    async fn hash_does_not_verify_under_different_pepper() -> Result<(), anyhow::Error> {
        // Arrange
        let hasher_a = Argon2PasswordHasher::new(pepper(1));
        let hasher_b = Argon2PasswordHasher::new(pepper(2));
        let hash = hasher_a.hash_password("s3cret").await?;

        // Act
        let result = hasher_b.verify_password("s3cret", &hash).await?;

        // Assert
        assert!(!result);
        Ok(())
    }
}
