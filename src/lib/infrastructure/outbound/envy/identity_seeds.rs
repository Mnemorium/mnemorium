use std::env::vars;

use crate::application::port::load_instance_state::IdentitySeeds;

/// Prefix every server environment variable carries.
const ENVIRONMENT_PREFIX: &str = "MNEMORIUM_";

/// Error returned when the identity seeds cannot be read.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum IdentitySeedsError {
    /// The environment cannot be mapped onto the seed fields.
    #[error("the instance identity seeds could not be read: {0}")]
    Unreadable(#[source] envy::Error),
}

/// Adapter reading the identity seeds from the environment.
///
/// `envy` strips the shared prefix, lowercases the remainder and matches it
/// against a field of [`IdentitySeeds`], so a new seed is a struct field rather
/// than another environment read in the composition root (`D14`). The
/// variables are not configuration, so they never go through the configuration
/// table.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnvIdentitySeeds;

impl EnvIdentitySeeds {
    /// Create a new adapter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Read the seeds from the process environment.
    ///
    /// # Errors
    ///
    /// Returns an error when the environment cannot be mapped onto the seed
    /// fields.
    pub fn read(&self) -> Result<IdentitySeeds, IdentitySeedsError> {
        self.read_from(vars())
    }

    /// Read the seeds from `variables`.
    ///
    /// The iterator is the seam the adapter tests use, so they never mutate the
    /// process environment.
    ///
    /// # Errors
    ///
    /// Returns an error when `variables` cannot be mapped onto the seed fields.
    pub fn read_from<I>(&self, variables: I) -> Result<IdentitySeeds, IdentitySeedsError>
    where
        I: IntoIterator<Item = (String, String)>,
    {
        envy::prefixed(ENVIRONMENT_PREFIX)
            .from_iter(variables)
            .map_err(IdentitySeedsError::Unreadable)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::iter::empty;

    use super::EnvIdentitySeeds;

    /// Build a (name, value) pair the way the process environment yields it.
    fn variable(name: &str, value: &str) -> (String, String) {
        (name.to_owned(), value.to_owned())
    }

    #[test]
    fn identity_seeds_present_seed_is_read() -> Result<(), Box<dyn Error>> {
        // Arrange
        let adapter = EnvIdentitySeeds::new();
        let pepper = "a".repeat(64);

        // Act
        let seeds = adapter.read_from([variable("MNEMORIUM_PEPPER", pepper.as_str())])?;

        // Assert
        assert_eq!(
            seeds.pepper(),
            Some(pepper.as_str()),
            "a prefixed variable must map onto its field"
        );
        assert_eq!(seeds.jwt_secret(), None, "an absent seed must stay absent");
        Ok(())
    }

    #[test]
    fn identity_seeds_absent_seeds_are_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let adapter = EnvIdentitySeeds::new();

        // Act
        let seeds = adapter.read_from(empty())?;

        // Assert
        assert_eq!(seeds.pepper(), None);
        assert_eq!(seeds.jwt_secret(), None);
        Ok(())
    }

    #[test]
    fn identity_seeds_unknown_name_is_ignored() -> Result<(), Box<dyn Error>> {
        // Arrange
        let adapter = EnvIdentitySeeds::new();
        let jwt_secret = "b".repeat(64);

        // Act
        let seeds = adapter.read_from([
            variable("MNEMORIUM_LOG_LEVEL", "warn"),
            variable("MNEMORIUM_JWT_SECRET", jwt_secret.as_str()),
        ])?;

        // Assert
        assert_eq!(
            seeds.jwt_secret(),
            Some(jwt_secret.as_str()),
            "a configuration variable must not stop the seed read"
        );
        assert_eq!(
            seeds.pepper(),
            None,
            "a configuration variable must not become a seed"
        );
        Ok(())
    }
}
