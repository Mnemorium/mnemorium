use std::env::vars;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::future::Future;
use std::future::ready;

use crate::application::port::load_state::StateSeedSource;
use crate::application::port::load_state::StateSeeds as StateSeedsPort;
use crate::domain::port::error::StateSeedSourceError;

/// Prefix every server environment variable carries.
const ENVIRONMENT_PREFIX: &str = "MNEMORIUM_";

/// The creation seeds the environment supplies, before validation.
///
/// The field name is the variable name below the shared prefix in `snake_case`,
/// which is what `envy` matches, so `MNEMORIUM_PEPPER` lands on `pepper` and
/// `MNEMORIUM_JWT_SECRET` on `jwt_secret`. The fields stay strings so a value
/// never reaches an error chain (`OBS-004`); the shape is checked by the use
/// case, whose errors name the variable and never the value. A prefixed name no
/// field matches is ignored, never rejected.
#[derive(Default, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct EnvStateSeeds {
    /// Signing secret supplied by the environment.
    jwt_secret: Option<String>,
    /// Pepper supplied by the environment.
    pepper: Option<String>,
}

impl Debug for EnvStateSeeds {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("EnvStateSeeds")
            .field("jwt_secret", &self.jwt_secret.as_ref().map(|_| "**"))
            .field("pepper", &self.pepper.as_ref().map(|_| "**"))
            .finish_non_exhaustive()
    }
}

impl EnvStateSeeds {
    /// Create a seed set from explicit values.
    #[must_use]
    pub fn new(pepper: Option<String>, jwt_secret: Option<String>) -> Self {
        Self { jwt_secret, pepper }
    }
}

impl StateSeedsPort for EnvStateSeeds {
    fn jwt_secret(&self) -> Option<String> {
        self.jwt_secret.clone()
    }

    fn pepper(&self) -> Option<String> {
        self.pepper.clone()
    }
}

/// Adapter reading the creation seeds from the environment.
///
/// `envy` strips the shared prefix, lowercases the remainder and matches it
/// against a field of [`EnvStateSeeds`], so a new seed is a struct field rather
/// than another environment read in the composition root (`D14`). The
/// environment also carries the configuration overrides, so a prefix-carrying
/// name that is neither a seed nor an override is discarded: the server does
/// not reject variables it does not know.
#[derive(Debug, Default)]
pub struct EnvStateSeedSource;

impl EnvStateSeedSource {
    /// Create a new adapter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Read the seeds from `variables`.
    ///
    /// The iterator is the seam the adapter tests use, so they never mutate the
    /// process environment.
    ///
    /// # Errors
    ///
    /// Returns an error when `variables` cannot be mapped onto the seed fields.
    pub fn read_from<I>(&self, variables: I) -> Result<EnvStateSeeds, StateSeedSourceError>
    where
        I: IntoIterator<Item = (String, String)>,
    {
        envy::prefixed(ENVIRONMENT_PREFIX)
            .from_iter(variables)
            .map_err(StateSeedSourceError::Unreadable)
    }
}

impl StateSeedSource for EnvStateSeedSource {
    fn read(
        &self,
    ) -> impl Future<Output = Result<Box<dyn StateSeedsPort>, StateSeedSourceError>> + Send {
        ready(
            self.read_from(vars())
                .map(|seeds| Box::new(seeds) as Box<dyn StateSeedsPort>),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::iter::empty;

    use crate::application::port::load_state::StateSeeds as StateSeedsPort;

    use super::EnvStateSeedSource;

    /// Build a (name, value) pair the way the process environment yields it.
    fn variable(name: &str, value: &str) -> (String, String) {
        (name.to_owned(), value.to_owned())
    }

    #[test]
    fn state_seeds_present_seed_is_read() -> Result<(), Box<dyn Error>> {
        // Arrange
        let adapter = EnvStateSeedSource::new();
        let pepper = "a".repeat(64);

        // Act
        let seeds = adapter.read_from([variable("MNEMORIUM_PEPPER", pepper.as_str())])?;

        // Assert
        assert_eq!(
            seeds.pepper(),
            Some(pepper.clone()),
            "a prefixed variable must map onto its field"
        );
        assert_eq!(seeds.jwt_secret(), None, "an absent seed must stay absent");
        Ok(())
    }

    #[test]
    fn state_seeds_absent_seeds_are_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let adapter = EnvStateSeedSource::new();

        // Act
        let seeds = adapter.read_from(empty())?;

        // Assert
        assert_eq!(seeds.pepper(), None);
        assert_eq!(seeds.jwt_secret(), None);
        Ok(())
    }

    #[test]
    fn state_seeds_unprefixed_variable_is_ignored() -> Result<(), Box<dyn Error>> {
        // Arrange
        let adapter = EnvStateSeedSource::new();

        // Act
        let seeds = adapter.read_from([variable("PATH", "/usr/bin")])?;

        // Assert
        assert_eq!(
            seeds.pepper(),
            None,
            "a variable without the server prefix is the host's, not a seed"
        );
        assert_eq!(seeds.jwt_secret(), None);
        Ok(())
    }

    #[test]
    fn state_seeds_configuration_variable_is_discarded() -> Result<(), Box<dyn Error>> {
        // Arrange
        let adapter = EnvStateSeedSource::new();
        let jwt_secret = "b".repeat(64);

        // Act
        let seeds = adapter.read_from([
            variable("MNEMORIUM_LOGGING_LEVEL", "warn"),
            variable("MNEMORIUM_JWT_SECRET", &jwt_secret),
        ])?;

        // Assert
        assert_eq!(
            seeds.jwt_secret(),
            Some(jwt_secret.clone()),
            "a configuration variable must be discarded, not rejected"
        );
        assert_eq!(seeds.pepper(), None);
        Ok(())
    }

    #[test]
    fn state_seeds_debug_redacts_both_seeds() {
        // Arrange
        let pepper = "a".repeat(64);
        let jwt_secret = "b".repeat(64);
        let seeds = super::EnvStateSeeds::new(Some(pepper.clone()), Some(jwt_secret.clone()));

        // Act
        let debug = format!("{seeds:?}");

        // Assert
        assert!(
            !debug.contains(pepper.as_str()),
            "Debug output must not contain the pepper, got: {debug:?}"
        );
        assert!(
            !debug.contains(jwt_secret.as_str()),
            "Debug output must not contain the signing secret, got: {debug:?}"
        );
        assert!(
            debug.contains("**"),
            "Debug output must mark a supplied seed as redacted, got: {debug:?}"
        );
    }

    #[test]
    fn state_seeds_port_exposes_the_supplied_seeds() {
        // Arrange
        let pepper = "c".repeat(64);
        let seeds = super::EnvStateSeeds::new(Some(pepper.clone()), None);
        let port: &dyn StateSeedsPort = &seeds;

        // Act
        let reported = (port.pepper(), port.jwt_secret());

        // Assert
        assert_eq!(reported, (Some(pepper), None));
    }
}
