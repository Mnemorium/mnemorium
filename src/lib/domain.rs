/// The crate's numeric identifier alias.
mod alias;
/// The domain models: aggregates, entities and value objects.
pub(crate) mod model;
/// The domain ports: repository, storage and provider traits.
pub(crate) mod port;
/// The domain services: pure policies over the models.
pub(crate) mod service;

pub(crate) use self::alias::{NonNegativeF64, NumericID, PositiveI64, ScalarError};
