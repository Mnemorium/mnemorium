use crate::domain::model::sqlite3::Sqlite3;

/// Persistence-related settings.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[non_exhaustive]
pub struct Persistence {
    /// `SQLite3` datastore settings.
    sqlite3: Sqlite3,
}

impl Persistence {
    /// Return the `SQLite3` datastore settings.
    #[must_use]
    pub fn sqlite3(&self) -> &Sqlite3 {
        &self.sqlite3
    }

    /// Return a mutable reference to the `SQLite3` datastore settings.
    #[must_use]
    pub fn sqlite3_mut(&mut self) -> &mut Sqlite3 {
        &mut self.sqlite3
    }

    /// Initialise a new `Persistence`.
    #[must_use]
    pub fn try_new(sqlite3: Sqlite3) -> Self {
        Self { sqlite3 }
    }
}
