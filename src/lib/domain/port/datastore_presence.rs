use crate::domain::port::DatastorePresenceError;

/// Port for deciding whether the datastore already exists.
///
/// A missing datastore is a value (`Ok(false)`), never an error (`STY-RUST-016`).
/// An unreadable path is an error, so a lost state file is never replaced beside
/// a live datastore.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait DatastorePresence: Send + Sync {
    /// Return whether the datastore file exists.
    ///
    /// # Errors
    ///
    /// Returns an error when the presence cannot be decided.
    fn exists(&self) -> Result<bool, DatastorePresenceError>;
}
