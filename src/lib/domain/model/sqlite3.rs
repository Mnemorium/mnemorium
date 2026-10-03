/// Error returned when initialising or updating a `Sqlite3` value object.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Sqlite3Error {
    /// The maximum number of connections is zero.
    #[error("sqlite3 max_connections must be greater than zero")]
    InvalidMaxConnections,
    /// The path is empty.
    #[error("sqlite3 path must not be empty")]
    PathEmpty,
}

/// `SQLite3` datastore settings.
///
/// Deserialization is routed through `Sqlite3Config` and [`TryFrom`] so that the
/// layered configuration cannot bypass the validation performed by
/// [`Sqlite3::try_new`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(try_from = "Sqlite3Config")]
#[non_exhaustive]
pub struct Sqlite3 {
    /// Maximum number of connections to the database.
    max_connections: u32,
    /// Path to the `SQLite3` database file.
    path: String,
}

/// Unchecked deserialization mirror of [`Sqlite3`].
///
/// Serde builds this snapshot and hands it to the [`TryFrom`] implementation,
/// which validates it through [`Sqlite3::try_new`].
#[derive(serde::Deserialize)]
struct Sqlite3Config {
    /// Maximum number of connections to the database.
    max_connections: u32,
    /// Path to the `SQLite3` database file.
    path: String,
}

impl TryFrom<Sqlite3Config> for Sqlite3 {
    type Error = Sqlite3Error;

    fn try_from(config: Sqlite3Config) -> Result<Self, Self::Error> {
        Self::try_new(config.path, config.max_connections)
    }
}

impl Sqlite3 {
    /// Return the maximum number of connections to the database.
    #[must_use]
    pub fn max_connections(&self) -> u32 {
        self.max_connections
    }

    /// Return the path to the `SQLite3` database file.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Update the maximum number of connections to the database.
    ///
    /// # Errors
    ///
    /// Returns [`Sqlite3Error::InvalidMaxConnections`] when `max_connections`
    /// is zero.
    pub fn set_max_connections(&mut self, max_connections: u32) -> Result<(), Sqlite3Error> {
        self.max_connections = Self::validate_max_connections(max_connections)?;
        Ok(())
    }

    /// Update the path to the `SQLite3` database file.
    ///
    /// # Errors
    ///
    /// Returns [`Sqlite3Error::PathEmpty`] when `path` is empty.
    pub fn set_path(&mut self, path: String) -> Result<(), Sqlite3Error> {
        self.path = Self::validate_path(path)?;
        Ok(())
    }

    /// Initialise a new `Sqlite3`, validating `path` and `max_connections`.
    ///
    /// # Errors
    ///
    /// Returns [`Sqlite3Error::PathEmpty`] when `path` is empty, and
    /// [`Sqlite3Error::InvalidMaxConnections`] when `max_connections` is zero.
    pub fn try_new(path: String, max_connections: u32) -> Result<Self, Sqlite3Error> {
        let validated_path = Self::validate_path(path)?;
        let validated_max_connections = Self::validate_max_connections(max_connections)?;
        Ok(Self {
            max_connections: validated_max_connections,
            path: validated_path,
        })
    }

    /// Validate `max_connections`.
    ///
    /// # Errors
    ///
    /// Returns [`Sqlite3Error::InvalidMaxConnections`] when `max_connections`
    /// is zero.
    fn validate_max_connections(max_connections: u32) -> Result<u32, Sqlite3Error> {
        if max_connections == 0 {
            return Err(Sqlite3Error::InvalidMaxConnections);
        }
        Ok(max_connections)
    }

    /// Validate `path`.
    ///
    /// # Errors
    ///
    /// Returns [`Sqlite3Error::PathEmpty`] when `path` is empty.
    fn validate_path(path: String) -> Result<String, Sqlite3Error> {
        if path.is_empty() {
            return Err(Sqlite3Error::PathEmpty);
        }
        Ok(path)
    }
}
