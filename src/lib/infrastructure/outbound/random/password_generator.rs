use std::future::{Future, ready};

use rand::RngExt as _;
use rand::distr::{Alphanumeric, SampleString as _};
use rand::seq::IndexedRandom as _;

use crate::domain::port::{PasswordGenerator, PasswordGeneratorError};

/// Length of generated passwords.
const PASSWORD_LENGTH: usize = 20;

/// Symbols guaranteed to be present in generated passwords.
const SYMBOLS: &[u8] = b"!@#$%^&*()-_=+";

/// Password generator producing random, policy-compliant passwords.
pub(crate) struct RandomPasswordGenerator;

impl RandomPasswordGenerator {
    /// Create a new password generator.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Default for RandomPasswordGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl PasswordGenerator for RandomPasswordGenerator {
    fn generate(&self) -> impl Future<Output = Result<String, PasswordGeneratorError>> + Send {
        let mut rng = rand::rng();
        let mut password = Alphanumeric
            .sample_string(&mut rng, PASSWORD_LENGTH - 1)
            .into_bytes();
        let Some(symbol_ref) = SYMBOLS.choose(&mut rng) else {
            return ready(Err(PasswordGeneratorError::OperationFailed));
        };
        let symbol = *symbol_ref;
        let index = rng.random_range(0..=password.len());
        password.insert(index, symbol);
        ready(String::from_utf8(password).map_err(|_| PasswordGeneratorError::OperationFailed))
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use allure_cargotest::allure_test;

    use super::*;

    #[allure_test]
    #[tokio::test]
    async fn random_password_generator_samples_follow_declared_shape() -> Result<(), Box<dyn Error>>
    {
        allure.epic("Authentication");
        allure.feature("REQ-AUTHENTICATION-002");
        allure.story("SCN-AUTHENTICATION-002-A");

        // Act & Assert
        assert_samples_follow_declared_shape(&RandomPasswordGenerator::new(), 1_000).await
    }

    async fn assert_samples_follow_declared_shape(
        generator: &RandomPasswordGenerator,
        samples: usize,
    ) -> Result<(), Box<dyn Error>> {
        for _ in 0..samples {
            let password = generator.generate().await?;
            assert_eq!(password.chars().count(), PASSWORD_LENGTH);
            assert_eq!(
                password.chars().filter(char::is_ascii_alphanumeric).count(),
                PASSWORD_LENGTH - 1
            );
            assert_eq!(
                password
                    .chars()
                    .filter(|character| SYMBOLS.contains(&(*character as u8)))
                    .count(),
                1
            );
        }
        Ok(())
    }
}
