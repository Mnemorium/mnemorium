use crate::domain::model::configuration::ProviderId;

/// Opaque reference to one item at a provider.
///
/// The identifier is opaque: no layer outside the provider's adapter interprets
/// it. The reference stays provider-tagged because the future manual flow sends
/// a selection back to the provider that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct ExternalRef {
    /// Identifier the provider issued for the item.
    id: String,
    /// Provider that issued the identifier.
    provider: ProviderId,
}

impl ExternalRef {
    /// Return the identifier the provider issued.
    #[must_use]
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    /// Create a new external reference.
    #[must_use]
    pub(crate) fn new(provider: ProviderId, id: String) -> Self {
        Self { id, provider }
    }

    /// Return the provider that issued the identifier.
    #[must_use]
    pub(crate) fn provider(&self) -> ProviderId {
        self.provider
    }
}
