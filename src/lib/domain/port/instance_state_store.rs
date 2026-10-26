use std::future::Future;

use crate::domain::model::instance_state::InstanceState;
use crate::domain::port::error::InstanceStateStoreError;

/// Port for reading and persisting the instance identity.
///
/// The store is existence-aware: `load` reports a missing identity as `None`,
/// which is a valid outcome rather than an error (`STY-RUST-016`). `save`
/// creates the identity and never overwrites one: a concurrent creator makes
/// the loser fail with [`InstanceStateStoreError::Conflict`] instead of
/// silently replacing key material another process already persisted.
#[cfg_attr(test, mockall::automock)]
pub trait InstanceStateStore: Send + Sync {
    /// Load the persisted instance identity.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read, cannot be decoded, fails
    /// validation, or declares a format version this build does not support.
    /// A missing file is `Ok(None)`, never an error.
    fn load(
        &self,
    ) -> impl Future<Output = Result<Option<InstanceState>, InstanceStateStoreError>> + Send;

    /// Persist the instance identity, creating it only when none exists.
    ///
    /// # Errors
    ///
    /// Returns [`InstanceStateStoreError::Conflict`] when an identity already
    /// exists, and an error when the file cannot be written.
    fn save(
        &self,
        state: &InstanceState,
    ) -> impl Future<Output = Result<(), InstanceStateStoreError>> + Send;
}
