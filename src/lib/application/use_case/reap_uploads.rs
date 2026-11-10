use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use std::path::Path;

use chrono::{Duration, NaiveDateTime, Utc};

use crate::application::port::{
    ReapUploadsCommand, ReapUploadsError, ReapUploadsResponse, ReapUploadsUseCase,
};
use crate::application::use_case::{staged_path, uploads_path};
use crate::domain::NumericID;
use crate::domain::port::{
    AssetUnitOfWork, FileStorage, UnitOfWork as _, UnitOfWorkFactory, UploadFilter,
    UploadRepository as _,
};

/// Bound after which a verification seal is considered stale, in seconds.
///
/// Twice the 30-second probe bound plus margin: a completion that dies between
/// sealing and unsealing leaves the session writable again after this long.
const SEAL_TIMEOUT_SECONDS: u64 = 300;

/// Use case implementation for reaping upload sessions.
pub(crate) struct ReapUploads<F, S> {
    /// Maximum number of sessions or files one phase handles per run.
    batch_size: usize,
    /// Lifetime of an upload session, in seconds.
    expiry_seconds: u64,
    /// Storage adapter deleting staged content.
    file_storage: Arc<S>,
    /// Factory opening the unit of work wrapping each phase.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, S: FileStorage> ReapUploads<F, S> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(
        unit_of_work_factory: Arc<F>,
        file_storage: Arc<S>,
        expiry_seconds: u64,
        batch_size: usize,
    ) -> Self {
        Self {
            batch_size,
            expiry_seconds,
            file_storage,
            unit_of_work_factory,
        }
    }
}

impl<F, S> ReapUploadsUseCase for ReapUploads<F, S>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork,
    S: FileStorage,
{
    fn execute<'future>(
        &'future self,
        _command: ReapUploadsCommand,
    ) -> Pin<Box<dyn Future<Output = Result<ReapUploadsResponse, ReapUploadsError>> + Send + 'future>>
    {
        let batch_size = self.batch_size;
        let file_storage = Arc::clone(&self.file_storage);
        let expiry_seconds = self.expiry_seconds;
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let now = Utc::now().naive_utc();
            let elapsed = |seconds: u64| -> Result<NaiveDateTime, ReapUploadsError> {
                let bound = i64::try_from(seconds).map_err(|error| {
                    ReapUploadsError::Unknown(
                        anyhow::anyhow!(error).context("the reap bound does not fit in i64"),
                    )
                })?;
                now.checked_sub_signed(Duration::seconds(bound))
                    .ok_or_else(|| {
                        ReapUploadsError::Unknown(anyhow::anyhow!(
                            "the reap bound underflows the current instant"
                        ))
                    })
            };

            // Phase 1: expired pending sessions and their staged content. The
            // file is removed before the row, so a session never points at
            // content that is already gone, and the write lock keeps a
            // concurrent begin from creating a row that reuses the identifier.
            let mut expired = 0u64;
            let expired_before = elapsed(expiry_seconds)?;
            let mut expired_unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| ReapUploadsError::Unknown(error.into()))?;
            let sessions = expired_unit_of_work
                .uploads()
                .search(&UploadFilter {
                    created_at: Some(expired_before),
                    is_finished: Some(false),
                    is_verifying: Some(false),
                    ..UploadFilter::default()
                })
                .await
                .map_err(|error| ReapUploadsError::Unknown(error.into()))?;
            for session in sessions.iter().take(batch_size) {
                discard_staged(&file_storage, session.upload_id()).await;
                let deleted = expired_unit_of_work
                    .uploads()
                    .delete(&UploadFilter {
                        upload_id: Some(session.upload_id()),
                        ..UploadFilter::default()
                    })
                    .await
                    .map_err(|error| ReapUploadsError::Unknown(error.into()))?;
                if deleted {
                    expired = expired.saturating_add(1);
                }
            }
            expired_unit_of_work
                .commit()
                .await
                .map_err(|error| ReapUploadsError::Unknown(error.into()))?;

            // Phase 2: a verification seal that outlived its bound. Unsealing
            // makes the session completable again after a crashed completion.
            let mut unsealed = 0u64;
            let stale_before = elapsed(SEAL_TIMEOUT_SECONDS)?;
            let mut seal_unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| ReapUploadsError::Unknown(error.into()))?;
            let stale = seal_unit_of_work
                .uploads()
                .search(&UploadFilter {
                    is_verifying: Some(true),
                    sealed_at: Some(stale_before),
                    ..UploadFilter::default()
                })
                .await
                .map_err(|error| ReapUploadsError::Unknown(error.into()))?;
            for session in stale.iter().take(batch_size) {
                let mut released = session.clone();
                released.unseal();
                seal_unit_of_work
                    .uploads()
                    .save(released)
                    .await
                    .map_err(|error| ReapUploadsError::Unknown(error.into()))?;
                unsealed = unsealed.saturating_add(1);
            }
            seal_unit_of_work
                .commit()
                .await
                .map_err(|error| ReapUploadsError::Unknown(error.into()))?;

            // Phase 3: staging content no session owns. The listing is taken
            // first, then every entry is checked against the session rows under
            // the write lock: a concurrent begin either committed its row (so
            // its file is kept) or rolled back (and removed its own file).
            let staged = file_storage
                .list_staged(&uploads_path())
                .await
                .map_err(|error| ReapUploadsError::Unknown(error.into()))?;
            let mut orphaned = 0u64;
            let mut staging_unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| ReapUploadsError::Unknown(error.into()))?;
            for path in staged.iter().take(batch_size) {
                let known = match staged_identifier(path) {
                    Some(upload_id) => !staging_unit_of_work
                        .uploads()
                        .search(&UploadFilter {
                            upload_id: Some(upload_id),
                            ..UploadFilter::default()
                        })
                        .await
                        .map_err(|error| ReapUploadsError::Unknown(error.into()))?
                        .is_empty(),
                    None => false,
                };
                if known {
                    continue;
                }
                if file_storage.delete_upload_file(path).await.is_ok() {
                    orphaned = orphaned.saturating_add(1);
                }
            }
            // The phase only reads; the rollback releases the write lock.
            let _rollback = staging_unit_of_work.rollback().await;

            Ok(ReapUploadsResponse::new(expired, orphaned, unsealed))
        })
    }
}

/// Return the upload identifier a staging entry's file name encodes, if any.
///
/// A name the layout does not produce is not a session's content, so the caller
/// treats it as an orphan.
fn staged_identifier(path: &Path) -> Option<NumericID> {
    let name = path.file_name()?.to_str()?;
    name.parse::<NumericID>().ok()
}

/// Best-effort delete the staged file of `upload_id`.
///
/// A missing file is success, and the file-storage adapter owns the failure log
/// (`OBS-002`), so a failed deletion is swallowed here.
async fn discard_staged<S>(file_storage: &Arc<S>, upload_id: NumericID)
where
    S: FileStorage,
{
    if file_storage
        .delete_upload_file(&staged_path(upload_id))
        .await
        .is_err()
    {
        // The file-storage adapter owns the failure log (`OBS-002`).
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::use_case::test_fixtures::TTL_SECONDS;
    use crate::domain::model::{ChunkBitmap, IntegrityHash, Upload};

    use crate::domain::port::{MockFileStorage, MockUploadRepository, RepositoryError};

    use crate::test_helpers::*;
    use chrono::NaiveDateTime;
    use std::error::Error;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tracing_test::traced_test;

    type UseCase = ReapUploads<TestFactory, MockFileStorage>;

    /// A use case under test together with the transaction-lifecycle flags of
    /// its three phases.
    struct Harness {
        /// Set when phase 1 committed.
        phase_one_committed: Arc<AtomicBool>,
        /// Set when phase 3 rolled back its read-only unit of work.
        phase_three_rolled_back: Arc<AtomicBool>,
        /// Set when phase 2 committed.
        phase_two_committed: Arc<AtomicBool>,
        /// The use case under test.
        use_case: UseCase,
    }

    /// Build a reaper whose three phases each open their own unit of work.
    fn use_case_with(
        phase_one: TestUow,
        phase_two: TestUow,
        phase_three: TestUow,
        file_storage: MockFileStorage,
    ) -> Harness {
        let phase_one_committed = Arc::clone(&phase_one.committed);
        let phase_two_committed = Arc::clone(&phase_two.committed);
        let phase_three_rolled_back = Arc::clone(&phase_three.rolled_back);
        let factory = TestFactory {
            unit_of_works: Mutex::new(vec![phase_one, phase_two, phase_three]),
        };
        Harness {
            phase_one_committed,
            phase_three_rolled_back,
            phase_two_committed,
            use_case: ReapUploads::new(Arc::new(factory), Arc::new(file_storage), TTL_SECONDS, 100),
        }
    }

    fn timestamp() -> NaiveDateTime {
        chrono::Utc::now().naive_utc()
    }

    fn upload(upload_id: i64, is_verifying: bool) -> Result<Upload, RepositoryError> {
        let mut bitmap = ChunkBitmap::try_new(1).map_err(|_| RepositoryError::OperationFailed)?;
        bitmap
            .mark_received(0)
            .map_err(|_| RepositoryError::OperationFailed)?;
        Upload::try_new(
            upload_id,
            3,
            "clip.mp4".to_owned(),
            4,
            "video/mp4".to_owned(),
            CHUNK_SIZE,
            IntegrityHash::try_new(DIGEST.to_owned())
                .map_err(|_| RepositoryError::OperationFailed)?,
            bitmap,
            false,
            is_verifying,
            Some(timestamp()),
            timestamp(),
        )
        .map_err(|_| RepositoryError::OperationFailed)
    }

    /// Expect a run with no expired session and no stale seal.
    fn expect_nothing_to_reap(
        phase_one: &mut MockUploadRepository,
        phase_two: &mut MockUploadRepository,
    ) {
        phase_one
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        phase_two
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
    }

    /// Expect the staging listing to be empty.
    fn expect_no_staged_content(file_storage: &mut MockFileStorage) {
        file_storage
            .expect_list_staged()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
    }

    #[tokio::test]
    async fn reap_uploads_removes_an_expired_session_and_its_content() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        phase_one_uploads
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(vec![upload(7, false)?]) }));
        phase_one_uploads
            .expect_delete()
            .times(1)
            .withf(|filter| filter.upload_id == Some(7))
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut phase_two_uploads = MockUploadRepository::new();
        phase_two_uploads
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_delete_upload_file()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
        expect_no_staged_content(&mut file_storage);
        let harness = use_case_with(
            TestUnitOfWork {
                uploads: phase_one_uploads,
                ..TestUnitOfWork::default()
            },
            TestUnitOfWork {
                uploads: phase_two_uploads,
                ..TestUnitOfWork::default()
            },
            TestUnitOfWork::default(),
            file_storage,
        );

        // Act
        let response = harness.use_case.execute(ReapUploadsCommand::new()).await?;

        // Assert
        assert_eq!(response.expired(), 1);
        assert_eq!(response.orphaned(), 0);
        assert_eq!(response.unsealed(), 0);
        assert!(harness.phase_one_committed.load(Ordering::SeqCst));
        assert!(harness.phase_two_committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn reap_uploads_unseals_a_stale_verification() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        phase_one_uploads
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut phase_two_uploads = MockUploadRepository::new();
        phase_two_uploads
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(vec![upload(7, true)?]) }));
        phase_two_uploads
            .expect_save()
            .times(1)
            .returning(|released| Box::pin(async move { Ok(released) }));
        let mut file_storage = MockFileStorage::new();
        expect_no_staged_content(&mut file_storage);
        let harness = use_case_with(
            TestUnitOfWork {
                uploads: phase_one_uploads,
                ..TestUnitOfWork::default()
            },
            TestUnitOfWork {
                uploads: phase_two_uploads,
                ..TestUnitOfWork::default()
            },
            TestUnitOfWork::default(),
            file_storage,
        );

        // Act
        let response = harness.use_case.execute(ReapUploadsCommand::new()).await?;

        // Assert
        assert_eq!(response.unsealed(), 1);
        assert_eq!(response.expired(), 0);
        Ok(())
    }

    #[tokio::test]
    async fn reap_uploads_removes_a_staging_file_no_session_owns() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        phase_one_uploads
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut phase_two_uploads = MockUploadRepository::new();
        phase_two_uploads
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut phase_three_uploads = MockUploadRepository::new();
        phase_three_uploads
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_list_staged()
            .times(1)
            .returning(|_| Box::pin(async { Ok(vec![PathBuf::from("data/uploads/9")]) }));
        file_storage
            .expect_delete_upload_file()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
        let harness = use_case_with(
            TestUnitOfWork {
                uploads: phase_one_uploads,
                ..TestUnitOfWork::default()
            },
            TestUnitOfWork {
                uploads: phase_two_uploads,
                ..TestUnitOfWork::default()
            },
            TestUnitOfWork {
                uploads: phase_three_uploads,
                ..TestUnitOfWork::default()
            },
            file_storage,
        );

        // Act
        let response = harness.use_case.execute(ReapUploadsCommand::new()).await?;

        // Assert
        assert_eq!(response.orphaned(), 1);
        assert!(harness.phase_three_rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn reap_uploads_keeps_a_staging_file_a_session_owns() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        phase_one_uploads
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut phase_two_uploads = MockUploadRepository::new();
        phase_two_uploads
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut phase_three_uploads = MockUploadRepository::new();
        phase_three_uploads
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(vec![upload(9, false)?]) }));
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_list_staged()
            .times(1)
            .returning(|_| Box::pin(async { Ok(vec![PathBuf::from("data/uploads/9")]) }));
        let harness = use_case_with(
            TestUnitOfWork {
                uploads: phase_one_uploads,
                ..TestUnitOfWork::default()
            },
            TestUnitOfWork {
                uploads: phase_two_uploads,
                ..TestUnitOfWork::default()
            },
            TestUnitOfWork {
                uploads: phase_three_uploads,
                ..TestUnitOfWork::default()
            },
            file_storage,
        );

        // Act
        let response = harness.use_case.execute(ReapUploadsCommand::new()).await?;

        // Assert: a live session's staged content is never removed.
        assert_eq!(response.orphaned(), 0);
        Ok(())
    }

    #[tokio::test]
    async fn reap_uploads_removes_a_staging_entry_it_cannot_attribute() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        phase_one_uploads
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut phase_two_uploads = MockUploadRepository::new();
        phase_two_uploads
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_list_staged()
            .times(1)
            .returning(|_| Box::pin(async { Ok(vec![PathBuf::from("data/uploads/junk")]) }));
        file_storage
            .expect_delete_upload_file()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
        let harness = use_case_with(
            TestUnitOfWork {
                uploads: phase_one_uploads,
                ..TestUnitOfWork::default()
            },
            TestUnitOfWork {
                uploads: phase_two_uploads,
                ..TestUnitOfWork::default()
            },
            TestUnitOfWork::default(),
            file_storage,
        );

        // Act
        let response = harness.use_case.execute(ReapUploadsCommand::new()).await?;

        // Assert
        assert_eq!(response.orphaned(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn reap_uploads_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        phase_one_uploads
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = use_case_with(
            TestUnitOfWork {
                uploads: phase_one_uploads,
                ..TestUnitOfWork::default()
            },
            TestUnitOfWork::default(),
            TestUnitOfWork::default(),
            MockFileStorage::new(),
        );

        // Act
        let result = harness.use_case.execute(ReapUploadsCommand::new()).await;

        // Assert
        assert!(matches!(result, Err(ReapUploadsError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn reap_uploads_never_logs_a_path_or_a_name() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_nothing_to_reap(&mut phase_one_uploads, &mut phase_two_uploads);
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_list_staged()
            .times(1)
            .returning(|_| Box::pin(async { Ok(vec![PathBuf::from("data/uploads/9")]) }));
        file_storage
            .expect_delete_upload_file()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
        let mut phase_three_uploads = MockUploadRepository::new();
        phase_three_uploads
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = use_case_with(
            TestUnitOfWork {
                uploads: phase_one_uploads,
                ..TestUnitOfWork::default()
            },
            TestUnitOfWork {
                uploads: phase_two_uploads,
                ..TestUnitOfWork::default()
            },
            TestUnitOfWork {
                uploads: phase_three_uploads,
                ..TestUnitOfWork::default()
            },
            file_storage,
        );

        // Act
        let response = harness.use_case.execute(ReapUploadsCommand::new()).await?;

        // Assert: the run never logs a path or a file name; the driver reports
        // the aggregate counts (`OBS-003`).
        assert_eq!(response.orphaned(), 1);
        assert!(
            !logs_contain("data/uploads"),
            "a reaping run must never log a path"
        );
        assert!(
            !logs_contain("clip.mp4"),
            "a reaping run must never log a file name"
        );
        Ok(())
    }
}
