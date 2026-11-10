//! The background upload reaper.
//!
//! A [`UploadReaperRunner`] ticks on the configured interval and runs one
//! bounded [`ReapUploads`](crate::application::use_case::ReapUploads)
//! pass per tick. It starts with the server, never delays readiness, and drains
//! on the shutdown signal like the media ingest.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, watch};
use tokio::task::JoinHandle;
use tokio::time::{MissedTickBehavior, interval};

use crate::application::port::{AssetUseCaseFactory, ReapUploadsCommand};
use crate::application::security_event;

/// Long-lived driver of the upload reaper.
pub(crate) struct UploadReaperRunner {
    /// Factory building the use case removing expired and stale state.
    factory: Arc<dyn AssetUseCaseFactory>,
    /// Interval between two reaping runs.
    interval: Duration,
}

impl UploadReaperRunner {
    /// Create a runner over the Asset use-case factory.
    #[must_use]
    pub(crate) fn new(factory: Arc<dyn AssetUseCaseFactory>, interval: Duration) -> Self {
        Self { factory, interval }
    }

    /// Start the reaping loop on a background task.
    #[must_use]
    pub(crate) fn start(self) -> UploadReaperTasks {
        let (shutdown, receiver) = watch::channel(false);
        let factory = self.factory;
        let interval = self.interval;
        let task = tokio::spawn(async move { reap_loop(factory, interval, receiver).await });
        UploadReaperTasks {
            shutdown,
            task: Mutex::new(Some(task)),
        }
    }
}

/// Handles of the spawned reaper task.
pub(crate) struct UploadReaperTasks {
    /// Cancellation signal for the reaping loop.
    shutdown: watch::Sender<bool>,
    /// The reaping task.
    task: Mutex<Option<JoinHandle<()>>>,
}

impl UploadReaperTasks {
    /// Signal the reaper to stop before axum is drained.
    pub(crate) fn shutdown(&self) {
        let _sent = self.shutdown.send(true);
    }

    /// Drain the reaper after the listener stops.
    pub(crate) async fn wait(&self) {
        if let Some(task) = self.task.lock().await.take() {
            let _joined = task.await;
        }
    }
}

/// Run one bounded reaping pass per interval until `shutdown` is signalled.
///
/// The first tick of a Tokio interval fires immediately; it is consumed so the
/// first pass runs after a full interval and never competes with startup. A
/// missed tick is delayed rather than burst, so a paused runtime does not replay
/// every skipped pass at once.
async fn reap_loop(
    factory: Arc<dyn AssetUseCaseFactory>,
    period: Duration,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut ticker = interval(period);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let _startup_tick = ticker.tick().await;
    loop {
        tokio::select! {
            _instant = ticker.tick() => {
                let reaper = factory.reap_uploads();
                match reaper.execute(ReapUploadsCommand::new()).await {
                    Ok(response) => {
                        // Aggregate counts only: the reaper never names a file or
                        // a path (`OBS-003`, `OBS-006`).
                        security_event::upload_reap(
                            response.expired(),
                            response.orphaned(),
                            response.unsealed(),
                        );
                    }
                    Err(_error) => security_event::application_error("upload_reaper"),
                }
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::error::Error;
    use std::sync::atomic::{AtomicBool, Ordering};

    use mockall::predicate::eq;
    use tokio::time::sleep;
    use tracing_test::traced_test;

    use crate::application::port::{
        MockAssetUseCaseFactory, MockReapUploadsUseCase, ReapUploadsResponse, ReapUploadsUseCase,
    };

    /// Interval short enough for a fast reaping test, in milliseconds.
    const SHORT_REAP_INTERVAL_MILLISECONDS: u64 = 10;

    /// Time the test lets several short intervals elapse.
    const AWAIT_MULTIPLE_INTERVALS_MILLISECONDS: u64 = 200;

    /// Time the test lets a long interval pass without reaching it.
    const AWAIT_SHORT_MILLISECONDS: u64 = 20;

    /// The long interval the shutdown test configures, in seconds.
    const LONG_REAP_INTERVAL_SECONDS: u64 = 30;

    /// Build a factory whose reap use case records that it ran in `ran`.
    fn factory(ran: Arc<AtomicBool>) -> MockAssetUseCaseFactory {
        let mut use_case = MockReapUploadsUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(ReapUploadsCommand::new()))
            .returning(move |_| {
                ran.store(true, Ordering::SeqCst);
                Box::pin(async { Ok(ReapUploadsResponse::new(0, 0, 0)) })
            });
        let mut factory = MockAssetUseCaseFactory::new();
        factory
            .expect_reap_uploads()
            .times(1)
            .return_once(move || Arc::new(use_case) as Arc<dyn ReapUploadsUseCase>);
        factory
    }

    #[tokio::test]
    async fn upload_reaper_runs_on_its_interval_and_stops() -> Result<(), Box<dyn Error>> {
        // Arrange
        let ran = Arc::new(AtomicBool::new(false));
        let tasks = UploadReaperRunner::new(
            Arc::new(factory(Arc::clone(&ran))),
            Duration::from_millis(SHORT_REAP_INTERVAL_MILLISECONDS),
        )
        .start();

        // Act
        sleep(Duration::from_millis(AWAIT_MULTIPLE_INTERVALS_MILLISECONDS)).await;
        let after = ran.load(Ordering::SeqCst);
        tasks.shutdown();
        tasks.wait().await;

        // Assert
        assert!(after, "the reaper must run once its interval elapses");
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn upload_reaper_reports_aggregate_counts_only() -> Result<(), Box<dyn Error>> {
        // Arrange: a use case reporting one count per phase.
        let mut use_case = MockReapUploadsUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .returning(|_| Box::pin(async { Ok(ReapUploadsResponse::new(2, 3, 1)) }));
        let mut factory = MockAssetUseCaseFactory::new();
        factory
            .expect_reap_uploads()
            .times(1)
            .return_once(move || Arc::new(use_case) as Arc<dyn ReapUploadsUseCase>);
        let tasks = UploadReaperRunner::new(
            Arc::new(factory),
            Duration::from_millis(SHORT_REAP_INTERVAL_MILLISECONDS),
        )
        .start();

        // Act
        sleep(Duration::from_millis(AWAIT_MULTIPLE_INTERVALS_MILLISECONDS)).await;
        tasks.shutdown();
        tasks.wait().await;

        // Assert: one record carries the counts and nothing else identifies a
        // file or a path.
        assert!(logs_contain("event=\"upload_reap\""));
        assert!(logs_contain("expired=2"));
        assert!(logs_contain("orphaned=3"));
        assert!(logs_contain("unsealed=1"));
        assert!(
            !logs_contain("data/uploads"),
            "a reaping record must never carry a path"
        );
        Ok(())
    }

    #[tokio::test]
    async fn upload_reaper_does_not_run_before_the_first_interval() -> Result<(), Box<dyn Error>> {
        // Arrange: the interval is far longer than the test's wait, so the
        // immediate Tokio interval tick must have been consumed at startup.
        let ran = Arc::new(AtomicBool::new(false));
        let mut factory = MockAssetUseCaseFactory::new();
        factory.expect_reap_uploads().times(0..);
        let tasks = UploadReaperRunner::new(
            Arc::new(factory),
            Duration::from_secs(LONG_REAP_INTERVAL_SECONDS),
        )
        .start();

        // Act
        sleep(Duration::from_millis(AWAIT_SHORT_MILLISECONDS)).await;
        tasks.shutdown();
        tasks.wait().await;

        // Assert
        assert!(
            !ran.load(Ordering::SeqCst),
            "the reaper must not reap before one full interval"
        );
        Ok(())
    }
}
