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

/// Return whether `file_name` is a safe single path segment.
#[expect(
    clippy::single_call_fn,
    reason = "the safety rule is named for readability and mirrors the domain model"
)]
pub(crate) fn is_path_safe(file_name: &str) -> bool {
    !file_name.is_empty()
        && !file_name.starts_with('.')
        && !file_name.contains(['/', '\\'])
        && !file_name.contains("..")
}

/// Return the path of a finished file relative to the storage root.
///
/// This is the path persisted on the file row.
#[expect(
    clippy::single_call_fn,
    reason = "the relative-path composition is named after the layout rule it encodes"
)]
pub(crate) fn relative_final(upload_id: NumericID, file_name: &str) -> PathBuf {
    PathBuf::from(FILES_FOLDER).join(format!("{upload_id}_{file_name}"))
}

/// Return the absolute path of the staging file of `upload_id` under `root`.
pub(crate) fn staged_path(root: &Path, upload_id: NumericID) -> PathBuf {
    uploads_path(root).join(upload_id.to_string())
}

/// Return the absolute path of the finished file of `upload_id` under `root`.
#[expect(
    clippy::single_call_fn,
    reason = "the final-path composition is named after the layout rule it encodes"
)]
pub(crate) fn final_path(root: &Path, upload_id: NumericID, file_name: &str) -> PathBuf {
    files_path(root).join(format!("{upload_id}_{file_name}"))
}
