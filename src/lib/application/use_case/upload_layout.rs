//! Media-library layout: the folder names and the paths composed from the
//! storage root, kept in the application layer so the storage adapter stays a
//! pure filesystem-I/O boundary.

use std::path::Path;
use std::path::PathBuf;

use crate::domain::alias::NumericID;

/// Name of the subfolder holding the files being uploaded.
pub(crate) const UPLOADS_FOLDER: &str = "uploads";

/// Name of the subfolder holding the finished files.
pub(crate) const FILES_FOLDER: &str = "files";

/// Return the absolute path of the uploads staging folder under `root`.
pub(crate) fn uploads_path(root: &Path) -> PathBuf {
    root.join(UPLOADS_FOLDER)
}

/// Return the absolute path of the finished files folder under `root`.
pub(crate) fn files_path(root: &Path) -> PathBuf {
    root.join(FILES_FOLDER)
}

/// Return the absolute path of the staging file of `upload_id` under `root`.
pub(crate) fn staged_path(root: &Path, upload_id: NumericID) -> PathBuf {
    uploads_path(root).join(upload_id.to_string())
}
