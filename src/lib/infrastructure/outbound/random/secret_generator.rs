use std::future::{Future, ready};

use rand::Rng as _;
use rand::SeedableRng as _;
use rand::rngs::ChaCha20Rng;

use crate::domain::port::error::SecretGeneratorError;
use crate::domain::port::secret_generator::SecretGenerator;

/// Secret generator producing random bytes from a `ChaCha20` RNG.
pub struct ChaChaSecretGenerator;

impl ChaChaSecretGenerator {
    /// Create a new secret generator.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for ChaChaSecretGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretGenerator for ChaChaSecretGenerator {
    fn generate(
        &self,
        length: u32,
    ) -> impl Future<Output = Result<Vec<u8>, SecretGeneratorError>> + Send {
        let mut rng = ChaCha20Rng::from_rng(&mut rand::rng());
        let mut secret = vec![0u8; length as usize];
        rng.fill_bytes(&mut secret);
        ready(Ok(secret))
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::port::secret_generator::SecretGenerator as _;

    use super::ChaChaSecretGenerator;

    #[tokio::test]
    async fn generate_returns_secret_of_expected_length() -> Result<(), anyhow::Error> {
        // Arrange
        let generator = ChaChaSecretGenerator::new();

        // Act
        let secret = generator.generate(32).await?;

        // Assert
        assert_eq!(secret.len(), 32);
        Ok(())
    }

    #[tokio::test]
    async fn generate_returns_distinct_secret() -> Result<(), anyhow::Error> {
        // Arrange
        let generator = ChaChaSecretGenerator::new();

        // Act
        let first = generator.generate(32).await?;
        let second = generator.generate(32).await?;

        // Assert
        assert_ne!(first, second);
        Ok(())
    }
}
