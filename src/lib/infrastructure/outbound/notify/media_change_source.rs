//! Change-source adapter observing the library root with `notify`.
//!
//! The watcher thread only translates a raw event and hands the result to an
//! unbounded channel, so it never blocks on the runtime. Content modifications
//! are dropped here: acting on one is the deferred `changed` decision, and the
//! startup scan remains the mechanism of record. Repeated events for one path
//! are coalesced by the consumer's stability gate before dispatch.

use std::future::Future;
use std::path::Component;
use std::path::Path;
use std::pin::Pin;
use std::sync::Mutex as StdMutex;

use notify::EventKind;
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify::Watcher as _;
use notify::event::ModifyKind;
use notify::event::RenameMode;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::application::port::media_change_source::MediaChange;
use crate::application::port::media_change_source::MediaChangeSource;
use crate::application::port::media_change_source::MediaChangeSourceError;
use crate::application::security_event;

/// Change source backed by the local filesystem watcher.
pub struct NotifyMediaChangeSource {
    /// Watcher kept alive for the source's lifetime; dropping it stops the
    /// watch.
    _watcher: RecommendedWatcher,
    /// Queue the watcher thread fills and the runtime drains.
    receiver: Mutex<UnboundedReceiver<Result<MediaChange, MediaChangeSourceError>>>,
}

impl NotifyMediaChangeSource {
    /// Watch `root` recursively, delivering its changes over an internal queue.
    ///
    /// # Errors
    ///
    /// Returns [`MediaChangeSourceError::WatchFailed`] when the watch cannot be
    /// registered. The caller logs one warning and keeps running: the startup
    /// scan remains authoritative.
    pub fn new(root: &Path) -> Result<Self, MediaChangeSourceError> {
        // `notify` reports event paths absolute, so the root everything is made
        // relative to must be absolute too; the caller passes the fixed
        // `media/` path relative to the working directory.
        let canonical_root = root.canonicalize().map_err(|error| {
            security_event::media_ingest_fault("watch_registration_failed");
            MediaChangeSourceError::Unknown(error.into())
        })?;
        let (sender, receiver) = mpsc::unbounded_channel();
        let root_for_events = canonical_root.clone();
        let pending_rename = StdMutex::new(None);
        let mut watcher =
            notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
                let changes = match result {
                    // A watcher error is not an overflow: the queue did not
                    // drop an event, the observation itself failed.
                    Err(_error) => vec![Err(MediaChangeSourceError::WatchError)],
                    Ok(event) => {
                        if event.need_rescan() {
                            vec![Err(MediaChangeSourceError::Overflow)]
                        } else {
                            let paths: Vec<String> = event
                                .paths
                                .iter()
                                .filter_map(|path| {
                                    let relative =
                                        path.strip_prefix(root_for_events.as_path()).ok()?;
                                    let mut segments = Vec::new();
                                    for component in relative.components() {
                                        let Component::Normal(part) = component else {
                                            return None;
                                        };
                                        let text = part.to_string_lossy();
                                        if text.starts_with('.') {
                                            return None;
                                        }
                                        segments.push(text.into_owned());
                                    }
                                    (!segments.is_empty()).then(|| segments.join("/"))
                                })
                                .collect();
                            match event.kind {
                                EventKind::Create(_) => paths
                                    .into_iter()
                                    .map(|path| Ok(MediaChange::Created(path)))
                                    .collect(),
                                EventKind::Modify(ModifyKind::Name(RenameMode::Both))
                                    if paths.len() >= 2 =>
                                {
                                    if let Ok(mut pending) = pending_rename.lock() {
                                        *pending = None;
                                    }
                                    vec![Ok(MediaChange::Renamed {
                                        from: paths.first().cloned().unwrap_or_default(),
                                        to: paths.get(1).cloned().unwrap_or_default(),
                                    })]
                                }
                                // `Both` is the correlated pair; `inotify` reports
                                // the two halves separately, so a `From` is
                                // remembered until its `To` arrives.
                                EventKind::Modify(ModifyKind::Name(
                                    RenameMode::To | RenameMode::Any | RenameMode::Other,
                                )) => {
                                    let pending = pending_rename
                                        .lock()
                                        .ok()
                                        .and_then(|mut pending| pending.take());
                                    paths
                                        .into_iter()
                                        .map(|to| match pending.clone() {
                                            Some(from) => Ok(MediaChange::Renamed { from, to }),
                                            None => Ok(MediaChange::Created(to)),
                                        })
                                        .collect()
                                }
                                EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
                                    if let Some(from) = paths.first()
                                        && let Ok(mut pending) = pending_rename.lock()
                                    {
                                        *pending = Some(from.clone());
                                    }
                                    Vec::new()
                                }
                                // A content or metadata modification is ignored:
                                // acting on it is the deferred `changed`
                                // decision, and no row is re-registered. Every
                                // other kind is likewise outside the ingest's
                                // scope.
                                EventKind::Remove(_) => {
                                    if let Ok(mut pending) = pending_rename.lock() {
                                        *pending = None;
                                    }
                                    paths
                                        .into_iter()
                                        .map(|path| Ok(MediaChange::Removed(path)))
                                        .collect()
                                }
                                EventKind::Any
                                | EventKind::Access(_)
                                | EventKind::Other
                                | EventKind::Modify(_) => Vec::new(),
                            }
                        }
                    }
                };
                for change in changes {
                    let _sent = sender.send(change);
                }
            })
            .map_err(|error| {
                security_event::media_ingest_fault("watch_registration_failed");
                MediaChangeSourceError::Unknown(error.into())
            })?;
        watcher
            .watch(canonical_root.as_path(), RecursiveMode::Recursive)
            .map_err(|_error| {
                security_event::media_ingest_fault("watch_registration_failed");
                MediaChangeSourceError::WatchFailed
            })?;
        Ok(Self {
            _watcher: watcher,
            receiver: Mutex::new(receiver),
        })
    }
}

impl MediaChangeSource for NotifyMediaChangeSource {
    fn changes(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<MediaChange, MediaChangeSourceError>> + Send + '_>>
    {
        Box::pin(async move {
            let mut receiver = self.receiver.lock().await;
            match receiver.recv().await {
                Some(change) => change,
                None => Err(MediaChangeSourceError::Unknown(anyhow::anyhow!(
                    "the change source stopped"
                ))),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::fs;
    use std::time::Duration;

    use tempfile::tempdir;
    use tokio::time::timeout;

    use crate::application::port::media_change_source::MediaChange;
    use crate::application::port::media_change_source::MediaChangeSource as _;
    use crate::application::port::media_change_source::MediaChangeSourceError;
    use tracing_test::traced_test;

    use super::NotifyMediaChangeSource;

    /// Wait up to five seconds for the next change.
    async fn next_change(source: &NotifyMediaChangeSource) -> Result<MediaChange, Box<dyn Error>> {
        Ok(timeout(Duration::from_secs(5), source.changes()).await??)
    }

    /// Wait until `expected` arrives, ignoring a platform's duplicate events.
    async fn wait_for(
        source: &NotifyMediaChangeSource,
        expected: MediaChange,
    ) -> Result<(), Box<dyn Error>> {
        for _attempt in 0u8..16u8 {
            if next_change(source).await? == expected {
                return Ok(());
            }
        }
        Err("the expected change was not delivered".into())
    }

    #[tokio::test]
    async fn observes_a_created_renamed_and_removed_file() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let root = tmp.path().to_path_buf();
        let source = NotifyMediaChangeSource::new(&root)?;
        let original = root.join("clip.mp4");

        // Act & Assert: create
        fs::write(&original, b"abc")?;
        wait_for(&source, MediaChange::Created("clip.mp4".to_owned())).await?;

        // Act & Assert: rename
        let renamed = root.join("moved.mp4");
        fs::rename(&original, &renamed)?;
        wait_for(
            &source,
            MediaChange::Renamed {
                from: "clip.mp4".to_owned(),
                to: "moved.mp4".to_owned(),
            },
        )
        .await?;

        // Act & Assert: remove
        fs::remove_file(&renamed)?;
        wait_for(&source, MediaChange::Removed("moved.mp4".to_owned())).await?;
        Ok(())
    }

    #[tokio::test]
    async fn ignores_a_content_modification() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let root = tmp.path().to_path_buf();
        let path = root.join("clip.mp4");
        fs::write(&path, b"abc")?;
        let source = NotifyMediaChangeSource::new(&root)?;
        // Drain the creation the watcher may have observed before the write.
        let _created = timeout(Duration::from_millis(300), source.changes()).await;

        // Act
        fs::write(&path, b"abcdef")?;
        let result = timeout(Duration::from_millis(500), source.changes()).await;

        // Assert: a content modification re-registers nothing.
        assert!(result.is_err(), "a modification must not be delivered");
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn watching_a_missing_root_fails() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;

        // Act
        let result = NotifyMediaChangeSource::new(&tmp.path().join("not-mounted"));

        // Assert: the failure is reported once and does not abort startup.
        assert!(matches!(
            result,
            Err(MediaChangeSourceError::WatchFailed | MediaChangeSourceError::Unknown(_))
        ));
        assert!(logs_contain("WARN"), "expected a warning");
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("kind=watch_registration_failed"))
                .count();
            (count == 1)
                .then_some(())
                .ok_or_else(|| format!("expected one warning (found {count})"))
        });
        Ok(())
    }
}
