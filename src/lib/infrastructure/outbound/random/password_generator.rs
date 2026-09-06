use std::future::{Future, ready};

use rand::RngExt as _;
use rand::distr::Alphanumeric;
use rand::distr::SampleString as _;
use rand::seq::IndexedRandom as _;

use crate::domain::port::error::PasswordGeneratorError;
use crate::domain::port::password_generator::PasswordGenerator;

/// Length of generated passwords.
const PASSWORD_LENGTH: usize = 20;

/// Symbols guaranteed to be present in generated passwords.
const SYMBOLS: &[u8] = b"!@#$%^&*()-_=+";

/// Password generator producing random, policy-compliant passwords.
pub struct RandomPasswordGenerator;

impl RandomPasswordGenerator {
    /// Create a new password generator.
    #[must_use]
    pub fn new() -> Self {
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
