use crate::domain::model::logging::Logging;
use crate::domain::model::persistence::Persistence;
use crate::domain::model::security::Security;

/// The application configuration.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[non_exhaustive]
pub struct Configuration {
    /// Logging-related settings.
    logging: Logging,
    /// Persistence-related settings.
    persistence: Persistence,
    /// Security-related settings.
    security: Security,
}

impl Configuration {
    /// Return the logging-related settings.
    #[must_use]
    pub fn logging(&self) -> &Logging {
        &self.logging
    }

    /// Initialise a new `Configuration`.
    #[must_use]
    pub fn new(persistence: Persistence, security: Security, logging: Logging) -> Self {
        Self {
            logging,
            persistence,
            security,
        }
    }

    /// Return the persistence-related settings.
    #[must_use]
    pub fn persistence(&self) -> &Persistence {
        &self.persistence
    }

    /// Return a mutable reference to the persistence-related settings.
    #[must_use]
    pub fn persistence_mut(&mut self) -> &mut Persistence {
        &mut self.persistence
    }

    /// Return the security-related settings.
    #[must_use]
    pub fn security(&self) -> &Security {
        &self.security
    }

    /// Return a mutable reference to the security-related settings.
    #[must_use]
    pub fn security_mut(&mut self) -> &mut Security {
        &mut self.security
    }
}
