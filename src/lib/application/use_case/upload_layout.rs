//! Library layout: the fixed library root, the staging root, and the paths
//! composed from them, kept in the application layer so the storage adapter
//! stays a pure filesystem-I/O boundary.

use std::path::Path;
use std::path::PathBuf;

use crate::domain::alias::NumericID;

/// Root directory of the library, relative to the working directory.
///
/// Every registered file is addressed relative to this root, whoever wrote its
/// bytes. The operator mounts it; the server never creates it.
pub(crate) const MEDIA_ROOT: &str = "media";

/// Root directory holding the server-owned staging area, relative to the
/// working directory.
const DATA_ROOT: &str = "data";

/// Name of the subfolder holding the files being uploaded.
const UPLOADS_FOLDER: &str = "uploads";

/// Return the path of the uploads staging folder under the fixed data root.
pub(crate) fn uploads_path() -> PathBuf {
    Path::new(DATA_ROOT).join(UPLOADS_FOLDER)
}

/// Return the path of the staging file of `upload_id` under the fixed data
/// root.
pub(crate) fn staged_path(upload_id: NumericID) -> PathBuf {
    uploads_path().join(upload_id.to_string())
}

/// Return the fixed library root.
pub(crate) fn media_root() -> PathBuf {
    PathBuf::from(MEDIA_ROOT)
}

/// Return the path of the file at `relative` under the library root.
///
/// `relative` is the root-relative path persisted in `file.path`, so the root
/// itself never reaches the database.
pub(crate) fn media_path(relative: &str) -> PathBuf {
    media_root().join(relative)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::media_path;
    use super::staged_path;
    use super::uploads_path;

    #[tokio::test]
    async fn a_stored_file_is_addressed_relative_to_the_media_root() {
        // Act
        let stored = media_path("12_clip.mp4");

        // Assert: no `data/files` prefix survives; the recorded path resolves
        // under the library root.
        assert_eq!(stored, PathBuf::from("media").join("12_clip.mp4"));
    }

    #[tokio::test]
    async fn a_staged_file_is_addressed_under_data_uploads() {
        // Act
        let staged = staged_path(7);

        // Assert
        assert_eq!(staged, PathBuf::from("data").join("uploads").join("7"));
        assert_eq!(uploads_path(), PathBuf::from("data").join("uploads"));
    }
}
