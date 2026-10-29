use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio::time::sleep;

use crate::application::port::ingest_media::IngestMediaCommand;
use crate::application::port::ingest_media::IngestRequest;
use crate::application::port::library_use_case_factory::LibraryUseCaseFactory;
use crate::application::port::media_change_source::MediaChange;
use crate::application::port::media_change_source::MediaChangeSource;
use crate::application::port::media_change_source::MediaChangeSourceError;
use crate::application::security_event;
use crate::application::use_case::upload_layout::MEDIA_ROOT;
use crate::application::use_case::upload_layout::media_root;
use crate::domain::port::media_source::MediaEntry;
use crate::domain::port::media_source::MediaSource;
use crate::infrastructure::outbound::file_system::media_source::LocalMediaSource;
use crate::infrastructure::outbound::notify::media_change_source::NotifyMediaChangeSource;
use tracing::warn;

/// How long a path must keep the same size and modification instant before it is
/// processed.
const DEFAULT_STABILITY_INTERVAL: Duration = Duration::from_millis(100);

/// How long a path may keep changing before the observer gives up on it.
const DEFAULT_STABILITY_TIMEOUT: Duration = Duration::from_secs(30);

/// Runtime driving the startup scan and the change observer.
///
/// Both drivers build the same ingest use case from the Library factory, so the
/// startup walk stays the authority and the observer is an accelerator. A path
/// observed as created is held until its size and modification instant stop
/// changing, because the kernel reports a creation long before the bytes are all
/// there.
pub struct MediaIngestRuntime<S> {
    /// Source delivering observed changes.
    changes: Arc<dyn MediaChangeSource>,
    /// Factory building the use case reconciling the library root.
    factory: Arc<dyn LibraryUseCaseFactory>,
    /// Source reading the metadata the stability gate compares.
    media_source: Arc<S>,
    /// Interval between two stability samples.
    stability_interval: Duration,
    /// Bound on how long a path may keep changing.
    stability_timeout: Duration,
}

impl<S> MediaIngestRuntime<S>
where
    S: MediaSource,
{
    /// Run one ingest request, reporting a failure without stopping the
    /// observer.
    async fn apply(&self, request: IngestRequest) {
        if self
            .factory
            .ingest_media()
            .execute(IngestMediaCommand::new(request))
            .await
            .is_err()
        {
            security_event::application_error("media_ingest_change");
        }
    }

    /// Wait until `path`'s size and modification instant stop changing.
    ///
    /// Returns the stable entry, or `None` when the file vanished or kept
    /// changing past [`MediaIngestRuntime::stability_timeout`].
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "a monotonic deadline built from two bounded durations cannot overflow"
    )]
    async fn await_stable(&self, path: &str) -> Option<MediaEntry> {
        let deadline = Instant::now() + self.stability_timeout;
        let mut previous: Option<MediaEntry> = None;
        loop {
            if Instant::now() >= deadline {
                return None;
            }
            sleep(self.stability_interval).await;
            let Ok(Some(current)) = self.media_source.stat(path).await else {
                return None;
            };
            if previous.as_ref() == Some(&current) {
                return Some(current);
            }
            previous = Some(current);
        }
    }

    /// Dispatch one observed change through the ingest use case.
    async fn dispatch(&self, change: MediaChange) {
        match change {
            MediaChange::Created(path) => {
                if self.await_stable(&path).await.is_some() {
                    self.apply(IngestRequest::Path(path)).await;
                }
            }
            MediaChange::Renamed { from, to } => {
                self.apply(IngestRequest::Rename { from, to }).await;
            }
            MediaChange::Removed(path) => {
                self.apply(IngestRequest::Remove(path)).await;
            }
        }
    }

    /// Create a runtime with the production stability windows.
    #[must_use]
    pub fn new(
        factory: Arc<dyn LibraryUseCaseFactory>,
        changes: Arc<dyn MediaChangeSource>,
        media_source: Arc<S>,
    ) -> Self {
        Self::with_windows(
            factory,
            changes,
            media_source,
            DEFAULT_STABILITY_INTERVAL,
            DEFAULT_STABILITY_TIMEOUT,
        )
    }

    /// Run the change observer until `shutdown` is signalled or the source
    /// cannot deliver any further change.
    pub async fn observe(&self, mut shutdown: watch::Receiver<bool>) {
        loop {
            tokio::select! {
                observed = self.changes.changes() => {
                    match observed {
                        Ok(change) => self.dispatch(change).await,
                        // One warning per degraded condition; the startup scan
                        // remains the mechanism of record.
                        Err(MediaChangeSourceError::Overflow) => {
                            security_event::media_ingest_fault("queue_overflow");
                        }
                        Err(MediaChangeSourceError::WatchError) => {
                            security_event::media_ingest_fault("watch_error");
                            return;
                        }
                        Err(MediaChangeSourceError::WatchFailed) => {
                            security_event::media_ingest_fault("watch_registration_failed");
                            return;
                        }
                        Err(MediaChangeSourceError::Unknown(_error)) => {
                            security_event::media_ingest_fault("change_source_failed");
                            return;
                        }
                    }
                }
                _changed = shutdown.changed() => {
                    if *shutdown.borrow() {
                        return;
                    }
                }
            }
        }
    }

    /// Run the startup scan until it finishes or `shutdown` is signalled.
    ///
    /// Cancelling the future drops the in-flight unit of work, which rolls it
    /// back; nothing is committed for the interrupted file.
    pub async fn scan(&self, mut shutdown: watch::Receiver<bool>) {
        let ingest = self.factory.ingest_media();
        tokio::select! {
            result = ingest.execute(IngestMediaCommand::new(IngestRequest::Scan)) => {
                if result.is_err() {
                    security_event::application_error("media_ingest_scan");
                }
            }
            _changed = shutdown.changed() => {}
        }
    }

    /// Create a runtime with explicit stability windows.
    #[must_use]
    pub fn with_windows(
        factory: Arc<dyn LibraryUseCaseFactory>,
        changes: Arc<dyn MediaChangeSource>,
        media_source: Arc<S>,
        stability_interval: Duration,
        stability_timeout: Duration,
    ) -> Self {
        Self {
            changes,
            factory,
            media_source,
            stability_interval,
            stability_timeout,
        }
    }
}

/// Long-lived driver of the library ingest.
///
/// The runner builds the filesystem adapters, warns once when the mount is
/// absent, and spawns the scan and the observer on background tasks.
pub struct MediaIngestRunner {
    /// Factory building the use case reconciling the library root.
    factory: Arc<dyn LibraryUseCaseFactory>,
    /// Whether the library root exists at startup.
    media_root_present: bool,
}

impl MediaIngestRunner {
    /// Create a runner over the Library use-case factory.
    #[must_use]
    pub fn new(factory: Arc<dyn LibraryUseCaseFactory>, media_root_present: bool) -> Self {
        Self {
            factory,
            media_root_present,
        }
    }

    /// Warn when the mount is absent, then start the scan and the observer.
    #[must_use]
    pub fn start(self) -> MediaIngestTasks {
        if !self.media_root_present {
            warn!(
                media_root = MEDIA_ROOT,
                "the library root is absent; it is not created and no file is registered"
            );
        }
        let media_source = Arc::new(LocalMediaSource::new(media_root()));
        let (changes, observer_available): (Arc<dyn MediaChangeSource>, bool) =
            if self.media_root_present {
                match NotifyMediaChangeSource::new(&media_root()) {
                    Ok(source) => (Arc::new(source), true),
                    // The adapter already reported the failed registration and
                    // the startup scan remains the mechanism of record.
                    Err(_error) => (Arc::new(StoppedMediaChangeSource), false),
                }
            } else {
                // There is nothing to watch. The runner already warned about the
                // absent root, so registering the watch would only duplicate
                // that warning and fail.
                (Arc::new(StoppedMediaChangeSource), false)
            };
        let runtime = Arc::new(MediaIngestRuntime::new(self.factory, changes, media_source));
        let (shutdown, _receiver) = watch::channel(false);
        let scan_runtime = Arc::clone(&runtime);
        let scan_shutdown = shutdown.subscribe();
        let scan = tokio::spawn(async move { scan_runtime.scan(scan_shutdown).await });
        let observer = observer_available.then(|| {
            let observer_shutdown = shutdown.subscribe();
            tokio::spawn(async move { runtime.observe(observer_shutdown).await })
        });
        MediaIngestTasks {
            observer: Mutex::new(observer),
            scan: Mutex::new(Some(scan)),
            shutdown,
        }
    }
}

/// Handles of the spawned ingest tasks.
pub struct MediaIngestTasks {
    /// Observer task, when the watch could be registered.
    observer: Mutex<Option<JoinHandle<()>>>,
    /// Startup scan task.
    scan: Mutex<Option<JoinHandle<()>>>,
    /// Cancellation signal shared by both tasks.
    shutdown: watch::Sender<bool>,
}

impl MediaIngestTasks {
    /// Signal both drivers to stop before axum is drained.
    pub fn shutdown(&self) {
        let _sent = self.shutdown.send(true);
    }

    /// Drain both drivers after the listener stops.
    pub async fn wait(&self) {
        if let Some(scan) = self.scan.lock().await.take() {
            let _joined = scan.await;
        }
        if let Some(observer) = self.observer.lock().await.take() {
            let _joined = observer.await;
        }
    }
}

/// Change source for a deployment whose watch could not be registered.
///
/// It is never polled: the runner does not spawn the observer when registration
/// failed, and the startup scan remains the mechanism of record.
struct StoppedMediaChangeSource;

impl MediaChangeSource for StoppedMediaChangeSource {
    fn changes<'future>(
        &'future self,
    ) -> Pin<Box<dyn Future<Output = Result<MediaChange, MediaChangeSourceError>> + Send + 'future>>
    {
        Box::pin(async { Err(MediaChangeSourceError::WatchFailed) })
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::future::Future;
    use std::future::pending;
    use std::sync::Arc;

    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    use chrono::NaiveDateTime;

    use crate::application::port::ingest_media::IngestMediaCommand;
    use crate::application::port::ingest_media::IngestMediaResponse;
    use crate::application::port::ingest_media::IngestMediaUseCase;
    use crate::application::port::ingest_media::IngestRequest;
    use crate::application::port::ingest_media::MockIngestMediaUseCase;
    use crate::application::port::library_use_case_factory::LibraryUseCaseFactory;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::media_change_source::MediaChange;
    use crate::application::port::media_change_source::MediaChangeSourceError;
    use crate::application::port::media_change_source::MockMediaChangeSource;
    use crate::domain::port::media_source::MediaEntry;
    use crate::domain::port::media_source::MockMediaSource;
    use tracing_test::traced_test;

    use tokio::sync::watch;
    use tokio::time::sleep;
    use tokio::time::timeout;

    use super::MediaIngestRuntime;

    /// Await `future` for at most `duration`, returning whether it completed.
    async fn completes_within<F: Future>(future: F, duration: Duration) -> bool {
        timeout(duration, future).await.is_ok()
    }

    /// Fixture modification instant.
    fn instant() -> NaiveDateTime {
        NaiveDateTime::parse_from_str("2026-10-28 10:00:00", "%Y-%m-%d %H:%M:%S")
            .unwrap_or_default()
    }

    /// Build an entry of `size_bytes`.
    fn entry(size_bytes: i64) -> MediaEntry {
        MediaEntry {
            modified_at: instant(),
            relative_path: "clip.mp4".to_owned(),
            size_bytes,
        }
    }

    /// Build a Library factory whose ingest accessor hands out `ingest`.
    fn factory(ingest: MockIngestMediaUseCase) -> Arc<dyn LibraryUseCaseFactory> {
        let use_case: Arc<dyn IngestMediaUseCase> = Arc::new(ingest);
        let mut library = MockLibraryUseCaseFactory::new();
        library
            .expect_ingest_media()
            .returning(move || Arc::clone(&use_case));
        Arc::new(library)
    }

    /// Build a runtime over the given mocks with short windows.
    fn runtime(
        ingest: MockIngestMediaUseCase,
        changes: MockMediaChangeSource,
        media_source: MockMediaSource,
    ) -> MediaIngestRuntime<MockMediaSource> {
        MediaIngestRuntime::with_windows(
            factory(ingest),
            Arc::new(changes),
            Arc::new(media_source),
            Duration::from_millis(10),
            Duration::from_secs(5),
        )
    }

    /// A source whose file keeps growing for two samples before settling.
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "the fixture size is a small bounded multiple"
    )]
    fn changing_sizes_source() -> MockMediaSource {
        let mut source = MockMediaSource::new();
        let mut samples = 0u32;
        source.expect_stat().returning(move |_| {
            samples = samples.saturating_add(1);
            let size = if samples < 3 {
                i64::from(samples) * 3
            } else {
                9
            };
            Box::pin(async move { Ok(Some(entry(size))) })
        });
        source
    }

    /// An ingest that records whether it was dispatched.
    fn dispatch_ingest() -> (MockIngestMediaUseCase, Arc<AtomicBool>) {
        let dispatched = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&dispatched);
        let mut ingest = MockIngestMediaUseCase::new();
        ingest
            .expect_execute()
            .times(1)
            .withf(|command: &IngestMediaCommand| {
                matches!(command.request(), IngestRequest::Path(path) if path == "clip.mp4")
            })
            .returning(move |_| {
                flag.store(true, Ordering::SeqCst);
                Box::pin(async { Ok(IngestMediaResponse::default()) })
            });
        (ingest, dispatched)
    }

    /// A source that delivers one creation and then stops.
    fn created_once_source() -> MockMediaChangeSource {
        let mut changes = MockMediaChangeSource::new();
        let mut sent = false;
        changes.expect_changes().returning(move || {
            if sent {
                return Box::pin(async {
                    Err(MediaChangeSourceError::Unknown(anyhow::anyhow!("stopped")))
                });
            }
            sent = true;
            Box::pin(async { Ok(MediaChange::Created("clip.mp4".to_owned())) })
        });
        changes
    }

    /// A source that delivers a rename, then a removal, then stops.
    fn rename_then_remove_source() -> MockMediaChangeSource {
        let mut changes = MockMediaChangeSource::new();
        let mut calls = 0u32;
        changes.expect_changes().returning(move || {
            calls = calls.saturating_add(1);
            let change = match calls {
                1 => Ok(MediaChange::Renamed {
                    from: "old.mp4".to_owned(),
                    to: "new.mp4".to_owned(),
                }),
                2 => Ok(MediaChange::Removed("new.mp4".to_owned())),
                _ => Err(MediaChangeSourceError::Unknown(anyhow::anyhow!("stopped"))),
            };
            Box::pin(async move { change })
        });
        changes
    }

    /// A source that overflows once and then stops.
    fn overflow_then_stop_source() -> MockMediaChangeSource {
        let mut changes = MockMediaChangeSource::new();
        let mut calls = 0u32;
        changes.expect_changes().returning(move || {
            calls = calls.saturating_add(1);
            let result = match calls {
                1 => Err(MediaChangeSourceError::Overflow),
                _ => Err(MediaChangeSourceError::Unknown(anyhow::anyhow!("stopped"))),
            };
            Box::pin(async move { result })
        });
        changes
    }

    /// Assert exactly one watcher-error warning was emitted.
    fn assert_one_watcher_error(lines: &[&str]) -> Result<(), String> {
        let count = lines
            .iter()
            .filter(|line| line.contains("kind=watch_error"))
            .count();
        if count == 1 {
            Ok(())
        } else {
            Err(format!("expected one watcher error (found {count})"))
        }
    }

    /// Assert exactly one warning names the absent library root.
    fn assert_one_absent_root_warning(lines: &[&str]) -> Result<(), String> {
        let warnings: Vec<&str> = lines
            .iter()
            .copied()
            .filter(|line| line.contains("WARN"))
            .collect();
        if warnings.len() == 1
            && warnings
                .first()
                .is_some_and(|line| line.contains("media_root=\"media\""))
        {
            Ok(())
        } else {
            Err(format!(
                "expected exactly one absent-root warning (found {})",
                warnings.len()
            ))
        }
    }

    /// Wait until `flag` is set, bounded so a stuck driver cannot hang the test.
    async fn wait_for_dispatch(flag: &AtomicBool) {
        for _attempt in 0u32..500u32 {
            if flag.load(Ordering::SeqCst) {
                return;
            }
            sleep(Duration::from_millis(10)).await;
        }
    }

    #[tokio::test]
    async fn a_created_path_is_processed_after_it_stops_changing() -> Result<(), Box<dyn Error>> {
        // Arrange: the file is written in two steps, so its size moves twice.
        let (ingest, dispatched) = dispatch_ingest();
        let source = changing_sizes_source();
        let changes = created_once_source();
        let (sender, receiver) = watch::channel(false);
        let runtime = Arc::new(runtime(ingest, changes, source));

        // Act
        let observer = Arc::clone(&runtime);
        let handle = tokio::spawn(async move { observer.observe(receiver).await });
        wait_for_dispatch(&dispatched).await;
        let _sent = sender.send(true);
        let stopped = completes_within(handle, Duration::from_secs(5)).await;

        // Assert: exactly one dispatch, and it happened only after the final
        // sample stopped moving.
        assert!(dispatched.load(Ordering::SeqCst));
        assert!(stopped, "the observer must stop on shutdown");
        Ok(())
    }

    #[tokio::test]
    async fn a_rename_and_a_removal_are_dispatched() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut ingest = MockIngestMediaUseCase::new();
        ingest
            .expect_execute()
            .times(2)
            .returning(|_| Box::pin(async { Ok(IngestMediaResponse::default()) }));
        let changes = rename_then_remove_source();
        let runtime = runtime(ingest, changes, MockMediaSource::new());
        let (sender, receiver) = watch::channel(false);

        // Act
        let handle = tokio::spawn(async move { runtime.observe(receiver).await });

        // The source's third call ends the loop; the shutdown completes it.
        let stopped = completes_within(handle, Duration::from_secs(5)).await;
        let _sent = sender.send(true);

        // Assert
        assert!(stopped, "the observer must stop when the source fails");
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn an_overflow_is_reported_and_the_observer_continues() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut ingest = MockIngestMediaUseCase::new();
        ingest.expect_execute().times(0);
        let changes = overflow_then_stop_source();
        let runtime = runtime(ingest, changes, MockMediaSource::new());
        let (_sender, receiver) = watch::channel(false);

        // Act
        runtime.observe(receiver).await;

        // Assert: exactly one overflow warning.
        assert!(logs_contain("WARN"), "expected a warning");
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("kind=queue_overflow"))
                .count();
            (count == 1)
                .then_some(())
                .ok_or_else(|| format!("expected one overflow warning (found {count})"))
        });
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn a_watcher_error_is_reported_once_and_the_observer_stops() {
        // Arrange: the watcher reports a fault rather than a lost event.
        let mut ingest = MockIngestMediaUseCase::new();
        ingest.expect_execute().times(0);
        let mut changes = MockMediaChangeSource::new();
        changes
            .expect_changes()
            .times(1)
            .returning(|| Box::pin(async { Err(MediaChangeSourceError::WatchError) }));
        let runtime = runtime(ingest, changes, MockMediaSource::new());
        let (_sender, receiver) = watch::channel(false);

        // Act
        runtime.observe(receiver).await;

        // Assert: one watcher warning, and no overflow misclassification.
        logs_assert(assert_one_watcher_error);
        assert!(
            !logs_contain("kind=queue_overflow"),
            "a watcher error is not a queue overflow"
        );
    }

    #[tokio::test]
    async fn the_scan_stops_when_shutdown_is_signalled() -> Result<(), Box<dyn Error>> {
        // Arrange: a scan that never finishes.
        let mut ingest = MockIngestMediaUseCase::new();
        ingest
            .expect_execute()
            .times(1)
            .returning(|_| Box::pin(pending()));
        let runtime = Arc::new(runtime(
            ingest,
            MockMediaChangeSource::new(),
            MockMediaSource::new(),
        ));
        let (sender, receiver) = watch::channel(false);

        // Act
        let worker = Arc::clone(&runtime);
        let scan = tokio::spawn(async move { worker.scan(receiver).await });
        let _sent = sender.send(true);

        // Assert
        assert!(
            completes_within(scan, Duration::from_secs(5)).await,
            "the scan worker must stop on shutdown"
        );
        Ok(())
    }

    #[tokio::test]
    async fn the_observer_stops_when_shutdown_is_signalled() -> Result<(), Box<dyn Error>> {
        // Arrange: a source with no further change.
        let mut changes = MockMediaChangeSource::new();
        changes.expect_changes().returning(|| Box::pin(pending()));
        let runtime = Arc::new(runtime(
            MockIngestMediaUseCase::new(),
            changes,
            MockMediaSource::new(),
        ));
        let (sender, receiver) = watch::channel(false);

        // Act
        let observer = Arc::clone(&runtime);
        let handle = tokio::spawn(async move { observer.observe(receiver).await });
        let _sent = sender.send(true);

        // Assert
        assert!(
            completes_within(handle, Duration::from_secs(5)).await,
            "the observer must stop on shutdown"
        );
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn an_absent_root_warns_once_and_startup_completes() {
        // Arrange
        let mut ingest = MockIngestMediaUseCase::new();
        ingest
            .expect_execute()
            .times(1)
            .returning(|_| Box::pin(async { Ok(IngestMediaResponse::default()) }));

        // Act: the runner starts, the scan finishes, and the mount is absent.
        let tasks = super::MediaIngestRunner::new(factory(ingest), false).start();
        tasks.wait().await;

        // Assert: exactly one warning names the absent root. The watcher is
        // not registered when the root is absent, so it cannot add a second
        // registration warning.
        logs_assert(assert_one_absent_root_warning);
    }
}
