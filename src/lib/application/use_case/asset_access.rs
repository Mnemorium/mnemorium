//! Shared read path of the Asset bounded context: resolve a stored asset, decide
//! whether the caller may read it, and hand the use case what it needs.
//!
//! Every asset route goes through this module so the file, image and content
//! representations cannot drift on authorization (`API-044`): an unreadable
//! asset and an unknown asset are one outcome, and an unreadable asset's bytes
//! are never read.

/// The asset access path shared by every asset use case.
mod access;

pub(crate) use self::access::{
    AssetAccessError, access_audio, access_file, access_image, access_video,
};
