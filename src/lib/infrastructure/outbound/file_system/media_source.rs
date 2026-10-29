//! Media-source adapter walking the library root.
//!
//! The walk is read-only, never follows a symbolic link and skips hidden
//! entries, so it cannot escape the root it was built for. An absent root is
//! reported as an outcome (`Ok(None)`), because it usually means the volume did
//! not mount rather than that the walk failed.

use std::fs;
use std::fs::Metadata;
use std::future::Future;
use std::io;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use chrono::DateTime;
use chrono::Utc;
use tokio::task::spawn_blocking;

use crate::domain::port::error::MediaSourceError;
use crate::domain::port::media_source::MediaEntry;
use crate::domain::port::media_source::MediaSource;

/// Media source backed by the local filesystem under a fixed root.
pub struct LocalMediaSource {
    /// Fixed root every reported path is relative to.
    root: PathBuf,
}

impl LocalMediaSource {
    /// Create a source rooted at `root`.
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl MediaSource for LocalMediaSource {
    fn stat(
        &self,
        relative_path: &str,
    ) -> impl Future<Output = Result<Option<MediaEntry>, MediaSourceError>> + Send {
        let root = self.root.clone();
        let relative = relative_path.to_owned();
        async move {
            spawn_blocking(move || {
                let path = Path::new(&relative);
                if path.as_os_str().is_empty()
                    || !path
                        .components()
                        .all(|component| matches!(component, Component::Normal(_)))
                {
                    return Ok(None);
                }
                let absolute = root.join(&relative);
                match fs::symlink_metadata(&absolute) {
                    Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
                        entry_of(relative, &metadata).map(Some)
                    }
                    Ok(_) => Ok(None),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
                    Err(error) => Err(error),
                }
            })
            .await
            .map_err(|error| MediaSourceError::Unknown(error.into()))?
            .map_err(|error| MediaSourceError::Unknown(error.into()))
        }
    }

    fn walk(
        &self,
    ) -> impl Future<Output = Result<Option<Vec<MediaEntry>>, MediaSourceError>> + Send {
        let root = self.root.clone();
        async move {
            spawn_blocking(move || match fs::symlink_metadata(&root) {
                Ok(metadata) if metadata.is_dir() => {
                    let mut entries = Vec::new();
                    collect(&root, "", &mut entries)?;
                    Ok(Some(entries))
                }
                // A root that is a symlink or a file is not the mount we expect:
                // the adapter never follows it.
                Ok(_) => Ok(Some(Vec::new())),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(error),
            })
            .await
            .map_err(|error| MediaSourceError::Unknown(error.into()))?
            .map_err(|error| MediaSourceError::Unknown(error.into()))
        }
    }
}

/// Recursively collect the regular files under `dir`.
///
/// Hidden entries and symbolic links, whether they name a file or a directory,
/// are skipped, which keeps every reported path inside `root`.
fn collect(dir: &Path, relative_dir: &str, entries: &mut Vec<MediaEntry>) -> io::Result<()> {
    let directory = match fs::read_dir(dir) {
        Ok(directory) => directory,
        // A nested directory the process cannot read is skipped: one
        // unreadable subtree must not stop the whole walk. The root itself is
        // different: if it cannot be read, the walk has no truth to report.
        Err(_error) if !relative_dir.is_empty() => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry_result in directory {
        let Ok(entry) = entry_result else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        let relative = if relative_dir.is_empty() {
            name
        } else {
            format!("{relative_dir}/{name}")
        };
        if metadata.is_dir() {
            // A nested walk handles its own unreadable entries.
            let _skipped = collect(&entry.path(), &relative, entries);
        } else if metadata.is_file() {
            if let Ok(media_entry) = entry_of(relative, &metadata) {
                entries.push(media_entry);
            }
        } else {
            // A socket, FIFO or device is not content the ingest registers.
        }
    }
    Ok(())
}

/// Build a [`MediaEntry`] from `relative` and its file metadata.
fn entry_of(relative: String, metadata: &Metadata) -> io::Result<MediaEntry> {
    let size_bytes = i64::try_from(metadata.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "the file is larger than i64"))?;
    let modified_at = metadata
        .modified()
        .map(|instant| DateTime::<Utc>::from(instant).naive_utc())?;
    Ok(MediaEntry {
        modified_at,
        relative_path: relative,
        size_bytes,
    })
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::fs;
    use std::io;
    use std::path::Path;

    use tempfile::tempdir;

    use super::LocalMediaSource;
    use crate::domain::port::media_source::MediaSource as _;

    /// Create a symbolic link, available only on a Unix test host.
    #[cfg(unix)]
    fn unix_symlink(original: &Path, link: &Path) -> io::Result<()> {
        use std::os::unix::fs::symlink;

        symlink(original, link)
    }

    #[tokio::test]
    async fn walk_reports_a_nested_tree_relative_to_the_root() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        fs::create_dir_all(tmp.path().join("films/2026"))?;
        fs::write(tmp.path().join("poster.png"), b"abc")?;
        fs::write(tmp.path().join("films/2026/clip.mp4"), b"abcdef")?;
        let source = LocalMediaSource::new(tmp.path().to_path_buf());

        // Act
        let mut entries = source.walk().await?.ok_or("the root should exist")?;
        entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

        // Assert
        let paths: Vec<&str> = entries
            .iter()
            .map(|entry| entry.relative_path.as_str())
            .collect();
        assert_eq!(paths, vec!["films/2026/clip.mp4", "poster.png"]);
        assert_eq!(entries.first().map(|entry| entry.size_bytes), Some(6));
        Ok(())
    }

    #[tokio::test]
    async fn walk_skips_a_hidden_file_and_a_hidden_directory() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        fs::create_dir_all(tmp.path().join(".cache"))?;
        fs::write(tmp.path().join(".hidden.png"), b"abc")?;
        fs::write(tmp.path().join(".cache/inside.png"), b"abc")?;
        fs::write(tmp.path().join("visible.png"), b"abc")?;
        let source = LocalMediaSource::new(tmp.path().to_path_buf());

        // Act
        let entries = source.walk().await?.ok_or("the root should exist")?;

        // Assert
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries.first().map(|entry| entry.relative_path.as_str()),
            Some("visible.png")
        );
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn walk_does_not_follow_a_symlink_outside_the_root() -> Result<(), Box<dyn Error>> {
        // Arrange: a real file outside the root, named from inside it.
        let outside = tempdir()?;
        let secret = outside.path().join("secret.png");
        fs::write(&secret, b"secret")?;
        let tmp = tempdir()?;
        unix_symlink(&secret, &tmp.path().join("link.png"))?;
        unix_symlink(outside.path(), &tmp.path().join("linked-dir"))?;
        fs::write(tmp.path().join("real.png"), b"abc")?;
        let source = LocalMediaSource::new(tmp.path().to_path_buf());

        // Act
        let entries = source.walk().await?.ok_or("the root should exist")?;

        // Assert
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries.first().map(|entry| entry.relative_path.as_str()),
            Some("real.png")
        );
        Ok(())
    }

    #[tokio::test]
    async fn walk_reports_an_absent_root_as_an_outcome() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let source = LocalMediaSource::new(tmp.path().join("not-mounted"));

        // Act
        let entries = source.walk().await?;

        // Assert: absence is a value, not an error.
        assert!(entries.is_none());
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn walk_skips_an_unreadable_nested_directory() -> Result<(), Box<dyn Error>> {
        // Act & Assert
        unreadable_nested_directory_is_skipped().await
    }

    #[cfg(unix)]
    async fn unreadable_nested_directory_is_skipped() -> Result<(), Box<dyn Error>> {
        use std::os::unix::fs::PermissionsExt as _;

        // Arrange
        let tmp = tempdir()?;
        let private = tmp.path().join("private");
        fs::create_dir_all(&private)?;
        fs::write(private.join("secret.png"), b"abc")?;
        fs::write(tmp.path().join("visible.png"), b"abc")?;
        fs::set_permissions(&private, fs::Permissions::from_mode(0o000))?;
        // A privileged test process can still read the directory, so there is
        // no permission boundary to assert.
        if fs::read_dir(&private).is_ok() {
            fs::set_permissions(&private, fs::Permissions::from_mode(0o700))?;
            return Ok(());
        }
        let source = LocalMediaSource::new(tmp.path().to_path_buf());

        // Act
        let entries = source.walk().await?.ok_or("the root should exist")?;

        // Assert: the unreadable subtree is skipped, the sibling survives.
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries.first().map(|entry| entry.relative_path.as_str()),
            Some("visible.png")
        );
        fs::set_permissions(&private, fs::Permissions::from_mode(0o700))?;
        Ok(())
    }

    #[tokio::test]
    async fn stat_reports_a_file_and_refuses_an_escaping_path() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        fs::write(tmp.path().join("clip.mp4"), b"abcdef")?;
        let source = LocalMediaSource::new(tmp.path().to_path_buf());

        // Act
        let found = source.stat("clip.mp4").await?;
        let escaping = source.stat("../clip.mp4").await?;

        // Assert
        assert_eq!(found.as_ref().map(|entry| entry.size_bytes), Some(6));
        assert!(escaping.is_none());
        Ok(())
    }
}
