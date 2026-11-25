/// The one application-side authorizer every use case calls.
mod authorization;
/// Keeps a backing file addressable when its reference is removed.
mod backing_file;
/// The cover-art readability rule every use case that writes a cover calls.
mod cover;
/// The tri-state field a partial-update command leaves alone, clears or replaces.
mod field_update;
/// The application use-case ports.
pub(crate) mod port;
/// The application use-case implementations.
pub(crate) mod use_case;

pub(crate) use self::authorization::{allows, authorize};
pub(crate) use self::backing_file::detach_backing_file;
pub(crate) use self::cover::is_viewable as cover_is_viewable;
pub(crate) use self::field_update::FieldUpdate;
