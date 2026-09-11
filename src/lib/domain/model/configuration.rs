use crate::domain::model::persistence::Persistence;
use crate::domain::model::security::Security;

/// The application configuration.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[non_exhaustive]
pub struct Configuration {
    /// Persistence-related settings.
    persistence: Persistence,
    /// Security-related settings.
    security: Security,
}

impl Configuration {
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

    /// Initialise a new `Configuration`.
    #[must_use]
    pub fn try_new(persistence: Persistence, security: Security) -> Self {
        Self {
            persistence,
            security,
        }
    }
}
