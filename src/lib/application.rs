/// The tri-state field a partial-update command leaves alone, clears or replaces.
mod field_update;
/// The application use-case ports.
pub(crate) mod port;
/// The application use-case implementations.
pub(crate) mod use_case;

pub(crate) use self::field_update::FieldUpdate;
