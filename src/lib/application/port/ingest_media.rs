use std::future::Future;
use std::pin::Pin;

/// Command for one ingest operation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct IngestMediaCommand {
    /// The operation to perform.
    request: IngestRequest,
}

impl IngestMediaCommand {
    /// Create a new command.
    #[must_use]
    pub fn new(request: IngestRequest) -> Self {
        Self { request }
    }

    /// Return the requested operation.
    #[must_use]
    pub fn request(&self) -> &IngestRequest {
        &self.request
    }
}

/// Aggregate outcome of one ingest operation.
///
/// It carries counts only: an individual file's name or path never reaches the
/// response, so the record that logs it cannot leak one (`OBS-003`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct IngestMediaResponse {
    /// Registered files whose size or modification instant differ.
    changed: u64,
    /// Registered files whose missing media row was written.
    completed: u64,
    /// Registered files no longer found under the root.
    missing: u64,
    /// Files whose record and media row were written.
    registered: u64,
    /// Files not registered, for any reason the ingest does not treat as a
    /// failure.
    skipped: u64,
    /// Registered files whose size and modification instant matched.
    untouched: u64,
}

impl IngestMediaResponse {
    /// Return the number of changed files.
    #[must_use]
    pub fn changed(&self) -> u64 {
        self.changed
    }

    /// Return the number of media rows completed.
    #[must_use]
    pub fn completed(&self) -> u64 {
        self.completed
    }

    /// Return the number of missing files.
    #[must_use]
    pub fn missing(&self) -> u64 {
        self.missing
    }

    /// Create a response from its counters.
    #[must_use]
    pub fn new(
        registered: u64,
        completed: u64,
        untouched: u64,
        changed: u64,
        missing: u64,
        skipped: u64,
    ) -> Self {
        Self {
            changed,
            completed,
            missing,
            registered,
            skipped,
            untouched,
        }
    }

    /// Return the number of files registered.
    #[must_use]
    pub fn registered(&self) -> u64 {
        self.registered
    }

    /// Return the number of skipped files.
    #[must_use]
    pub fn skipped(&self) -> u64 {
        self.skipped
    }

    /// Return the number of untouched files.
    #[must_use]
    pub fn untouched(&self) -> u64 {
        self.untouched
    }
}

/// What the ingest should reconcile.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum IngestRequest {
    /// Examine one path relative to the library root.
    Path(String),
    /// Report the file at the relative path as missing.
    Remove(String),
    /// Move a recorded location from `from` to `to`.
    Rename {
        /// The recorded relative path.
        from: String,
        /// The new relative path.
        to: String,
    },
    /// Walk the whole library root.
    Scan,
}

/// Error returned when the ingest fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum IngestMediaError {
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case reconciling the library root with the registered files.
///
/// A scan and an observed change drive the same reconciliation, so the startup
/// walk and the watcher are two callers of one operation rather than two
/// pipelines.
#[cfg_attr(test, mockall::automock)]
pub trait IngestMediaUseCase: Send + Sync {
    /// Reconcile the requested paths and report aggregate counts.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned, so the method is object-safe and the use case can be stored as
    /// `Arc<dyn IngestMediaUseCase>`.
    fn execute<'future>(
        &'future self,
        command: IngestMediaCommand,
    ) -> Pin<Box<dyn Future<Output = Result<IngestMediaResponse, IngestMediaError>> + Send + 'future>>;
}
