use crate::domain::NumericID;
use crate::domain::model::ResourceRef;

/// The resource that owns a stored asset, as far as authorization is concerned.
///
/// Reading an asset delegates to the read action of the resource that owns it,
/// so the asset is exactly as readable as the resource it belongs to. The owner
/// travels with the stored file, so the decision needs no further lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum AssetOwner {
    /// Shared catalogue art, owned by the library rather than one resource.
    Library,
    /// The one resource that owns the asset; its read action decides it.
    Resource(ProtectedResource),
}

/// The part of an owning resource an authorization decision needs.
///
/// The three facts an asset decision needs from its owning resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProtectedResource {
    /// Whether every authenticated caller may read the resource.
    is_public: bool,
    /// Identifier of the owning user, absent for the catalogue.
    owner_id: Option<NumericID>,
    /// Kind and identifier of the resource, matched by a resource grant.
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
}
