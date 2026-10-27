//! Media-library layout: the fixed storage root, and the folder names and the
//! paths composed from it, kept in the application layer so the storage adapter
//! stays a pure filesystem-I/O boundary.

use std::path::Path;
use std::path::PathBuf;

use crate::domain::alias::NumericID;

/// Root directory holding the staged and final files, relative to the working
/// directory.
const STORAGE_ROOT: &str = "data";

/// Name of the subfolder holding the files being uploaded.
const UPLOADS_FOLDER: &str = "uploads";

/// Name of the subfolder holding the finished files.
pub(crate) const FILES_FOLDER: &str = "files";

/// Return the path of the uploads staging folder under the fixed storage root.
pub(crate) fn uploads_path() -> PathBuf {
    Path::new(STORAGE_ROOT).join(UPLOADS_FOLDER)
}

/// Return the path of the finished files folder under the fixed storage root.
pub(crate) fn files_path() -> PathBuf {
    Path::new(STORAGE_ROOT).join(FILES_FOLDER)
}

/// Return the path of the staging file of `upload_id` under the fixed storage
/// root.
pub(crate) fn staged_path(upload_id: NumericID) -> PathBuf {
    uploads_path().join(upload_id.to_string())
}

/// Return the path of the stored file at `relative` under the fixed storage
/// root.
///
/// `relative` is the root-relative path persisted in `file.path`, so the root
/// itself never reaches the database.
pub(crate) fn stored_file_path(relative: &Path) -> PathBuf {
    Path::new(STORAGE_ROOT).join(relative)
}
