use crate::domain::NumericID;
use crate::domain::model::{Protected, ResourceRef};

/// The resource that owns a stored asset, as far as authorization is concerned.
///
/// Reading an asset delegates to the read action of the resource that owns it,
/// so the asset is exactly as readable as the resource it belongs to. The owner
/// travels with the stored file, so the decision needs no further lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum AssetOwner {
    /// Shared catalogue art, owned by the library rather than one resource:
    /// `library.read` decides it against [`crate::domain::model::Global`].
    Library,
    /// The one resource that owns the asset; its read action decides it.
    Resource(ProtectedResource),
}

/// The part of an owning resource an authorization decision needs.
///
/// A decision reads a resource only through [`Protected`], so the three facts
/// that trait exposes are enough to decide without loading the aggregate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProtectedResource {
    /// Whether every authenticated caller may read the resource.
    is_public: bool,
    /// Identifier of the owning user, absent for the catalogue.
    owner_id: Option<NumericID>,
    /// Kind and identifier of the resource, matched by a `LISTED` scope.
    reference: ResourceRef,
}

impl ProtectedResource {
    /// Return the resource named by `reference`, owned and visible as given.
    #[must_use]
    pub(crate) fn new(
        reference: ResourceRef,
        owner_id: Option<NumericID>,
        is_public: bool,
    ) -> Self {
        Self {
            is_public,
            owner_id,
            reference,
        }
    }

    /// Return the reference of the owning resource.
    #[must_use]
    pub(crate) fn reference(self) -> ResourceRef {
        self.reference
    }
}

impl Protected for ProtectedResource {
    fn is_privileged_account(&self) -> bool {
        false
    }

    fn is_publicly_visible(&self) -> bool {
        self.is_public
    }

    fn owner_id(&self) -> Option<NumericID> {
        self.owner_id
    }

    fn resource_ref(&self) -> Option<ResourceRef> {
        Some(self.reference)
    }
}
