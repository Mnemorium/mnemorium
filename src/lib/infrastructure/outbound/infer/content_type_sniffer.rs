//! Content-type sniffer over the `infer` crate.
//!
//! The adapter reads a bounded header from the file and maps the recognised
//! signature onto its media type. It owns no accept-list: a type the caller does
//! not support is still reported, and the caller decides what to do with it.

use std::future::Future;
use std::io::ErrorKind;
use std::path::Path;

use tokio::task::spawn_blocking;

use crate::domain::port::content_type_sniffer::ContentTypeSniffer;
use crate::domain::port::error::ContentTypeSnifferError;

/// Content-type sniffer backed by `infer`'s magic-number signatures.
pub struct InferContentTypeSniffer;

impl InferContentTypeSniffer {
    /// Create a new sniffer.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for InferContentTypeSniffer {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentTypeSniffer for InferContentTypeSniffer {
    fn sniff(
        &self,
        path: &Path,
    ) -> impl Future<Output = Result<Option<String>, ContentTypeSnifferError>> + Send {
        let owned = path.to_path_buf();
        async move {
            // Reading the header is blocking I/O. It runs on the blocking pool
            // like the media-source walk, so a slow mount cannot occupy a
            // runtime worker while a request is in flight.
            let read = spawn_blocking(move || infer::get_from_path(&owned))
                .await
                .map_err(|error| ContentTypeSnifferError::Unknown(error.into()))?;
            match read {
                Ok(found) => Ok(found.map(|kind| kind.mime_type().to_owned())),
                // A file that vanished, cannot be read, or is not a regular
                // file is unknown content rather than a dependency fault: the
                // caller skips it and keeps walking.
                Err(error)
                    if matches!(
                        error.kind(),
                        ErrorKind::NotFound | ErrorKind::PermissionDenied | ErrorKind::IsADirectory
                    ) =>
                {
                    Ok(None)
                }
                Err(error) => Err(ContentTypeSnifferError::Unknown(error.into())),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;

    use tempfile::tempdir;

    use crate::domain::port::content_type_sniffer::ContentTypeSniffer as _;

    use super::InferContentTypeSniffer;

    /// Write `bytes` into `dir/name` and return the path.
    fn fixture(dir: &Path, name: &str, bytes: &[u8]) -> Result<PathBuf, Box<dyn Error>> {
        let path = dir.join(name);
        fs::write(&path, bytes)?;
        Ok(path)
    }

    #[tokio::test]
    async fn sniff_reports_an_image() -> Result<(), Box<dyn Error>> {
        // Arrange: a minimal PNG signature.
        let tmp = tempdir()?;
        let path = fixture(tmp.path(), "picture.bin", b"\x89PNG\r\n\x1a\n\x00\x00")?;

        // Act
        let found = InferContentTypeSniffer::new().sniff(&path).await?;

        // Assert
        assert_eq!(found.as_deref(), Some("image/png"));
        Ok(())
    }

    #[tokio::test]
    async fn sniff_reports_a_video() -> Result<(), Box<dyn Error>> {
        // Arrange: an ISO base-media `ftyp` box.
        let tmp = tempdir()?;
        let path = fixture(
            tmp.path(),
            "clip.bin",
            b"\x00\x00\x00\x18ftypmp42\x00\x00\x00\x00mp42isom",
        )?;

        // Act
        let found = InferContentTypeSniffer::new().sniff(&path).await?;

        // Assert
        assert_eq!(found.as_deref(), Some("video/mp4"));
        Ok(())
    }

    #[tokio::test]
    async fn sniff_reports_audio() -> Result<(), Box<dyn Error>> {
        // Arrange: an ID3-tagged MP3.
        let tmp = tempdir()?;
        let path = fixture(tmp.path(), "track.bin", b"ID3\x04\x00\x00\x00\x00\x00\x00")?;

        // Act
        let found = InferContentTypeSniffer::new().sniff(&path).await?;

        // Assert
        assert_eq!(found.as_deref(), Some("audio/mpeg"));
        Ok(())
    }

    #[tokio::test]
    async fn sniff_reports_nothing_for_text() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let path = fixture(tmp.path(), "notes.txt", b"just some plain text")?;

        // Act
        let found = InferContentTypeSniffer::new().sniff(&path).await?;

        // Assert: the caller owns the supported set; unrecognised is nothing.
        assert!(found.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn sniff_reports_nothing_for_a_truncated_header() -> Result<(), Box<dyn Error>> {
        // Arrange: two bytes of a PNG signature are not enough to classify.
        let tmp = tempdir()?;
        let path = fixture(tmp.path(), "short.bin", b"\x89P")?;

        // Act
        let found = InferContentTypeSniffer::new().sniff(&path).await?;

        // Assert
        assert!(found.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn sniff_reports_nothing_for_a_missing_file() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;

        // Act
        let found = InferContentTypeSniffer::new()
            .sniff(&tmp.path().join("absent.png"))
            .await?;

        // Assert
        assert!(found.is_none());
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn sniff_reports_nothing_for_an_unreadable_file() -> Result<(), Box<dyn Error>> {
        // Act & Assert
        unreadable_file_is_unknown_content().await
    }

    #[cfg(unix)]
    async fn unreadable_file_is_unknown_content() -> Result<(), Box<dyn Error>> {
        use std::os::unix::fs::PermissionsExt as _;

        // Arrange
        let tmp = tempdir()?;
        let path = fixture(tmp.path(), "private.png", b"\x89PNG\r\n\x1a\n\x00\x00")?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000))?;
        // A privileged test process can still read the file, so there is no
        // permission boundary to assert.
        if fs::read(&path).is_ok() {
            return Ok(());
        }

        // Act
        let found = InferContentTypeSniffer::new().sniff(&path).await?;

        // Assert: unreadable content is an outcome, not an error.
        assert!(found.is_none());
        Ok(())
    }
}
