//! Fixtures shared by the Asset use-case test modules.

use std::future::Future;
use std::future::ready;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use crate::domain::port::asset_unit_of_work::AssetUnitOfWork;
use crate::domain::port::error::UnitOfWorkError;
use crate::domain::port::file_repository::FileRepository;
use crate::domain::port::file_repository::MockFileRepository;
use crate::domain::port::mime_type_repository::MimeTypeRepository;
use crate::domain::port::mime_type_repository::MockMimeTypeRepository;
use crate::domain::port::unit_of_work::UnitOfWork;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::upload_repository::MockUploadRepository;
use crate::domain::port::upload_repository::UploadRepository;

/// Fake Asset unit of work wiring mocked repositories and recording its
/// outcome.
pub struct AssetTestUnitOfWork {
    /// Set when `commit` is called.
    pub committed: Arc<AtomicBool>,
    /// File repository.
    pub files: MockFileRepository,
    /// Mime type repository.
    pub mime_types: MockMimeTypeRepository,
    /// Set when `rollback` is called.
    pub rolled_back: Arc<AtomicBool>,
    /// Upload repository.
    pub uploads: MockUploadRepository,
}

impl AssetUnitOfWork for AssetTestUnitOfWork {
    fn files(&mut self) -> impl FileRepository + '_ {
        &mut self.files
    }

    fn mime_types(&mut self) -> impl MimeTypeRepository + '_ {
        &mut self.mime_types
    }

    fn uploads(&mut self) -> impl UploadRepository + '_ {
        &mut self.uploads
    }
}

impl UnitOfWork for AssetTestUnitOfWork {
    fn commit(self) -> impl Future<Output = Result<(), UnitOfWorkError>> + Send {
        self.committed.store(true, Ordering::SeqCst);
        ready(Ok(()))
    }

    fn rollback(self) -> impl Future<Output = Result<(), UnitOfWorkError>> + Send {
        self.rolled_back.store(true, Ordering::SeqCst);
        ready(Ok(()))
    }
}

/// Fake factory handing out a single prepared [`AssetTestUnitOfWork`].
pub struct AssetTestUnitOfWorkFactory {
    /// Taken by the first `begin` call.
    pub unit_of_work: Mutex<Option<AssetTestUnitOfWork>>,
}

impl UnitOfWorkFactory for AssetTestUnitOfWorkFactory {
    type Uow = AssetTestUnitOfWork;

    fn begin(&self) -> impl Future<Output = Result<Self::Uow, UnitOfWorkError>> + Send {
        let unit_of_work = match self.unit_of_work.lock() {
            Ok(mut guard) => guard.take(),
            Err(poisoned) => poisoned.into_inner().take(),
        };
        ready(unit_of_work.ok_or(UnitOfWorkError::OperationFailed))
    }
}

/// Fake factory handing out a queue of prepared [`AssetTestUnitOfWork`]s, one
/// per `begin` call, so retry loops can bind a unit of work per attempt.
pub struct QueueAssetTestUnitOfWorkFactory {
    /// Popped from the front by each `begin` call.
    pub unit_of_works: Mutex<Vec<AssetTestUnitOfWork>>,
}

impl UnitOfWorkFactory for QueueAssetTestUnitOfWorkFactory {
    type Uow = AssetTestUnitOfWork;

    fn begin(&self) -> impl Future<Output = Result<Self::Uow, UnitOfWorkError>> + Send {
        let unit_of_work = match self.unit_of_works.lock() {
            Ok(mut guard) if !guard.is_empty() => Some(guard.remove(0)),
            Ok(_) => None,
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                if guard.is_empty() {
                    None
                } else {
                    Some(guard.remove(0))
                }
            }
        };
        ready(unit_of_work.ok_or(UnitOfWorkError::OperationFailed))
    }
}

/// The unit-of-work fixtures produced by [`asset_factory`].
pub struct AssetFactoryHarness {
    /// Set when the unit of work is committed.
    pub committed: Arc<AtomicBool>,
    /// The factory handed to the use case under test.
    pub factory: Arc<AssetTestUnitOfWorkFactory>,
    /// Set when the unit of work is rolled back.
    pub rolled_back: Arc<AtomicBool>,
}

/// Build the Asset unit-of-work fixtures around the given mocked repositories.
#[must_use]
pub fn asset_factory(
    uploads: MockUploadRepository,
    files: MockFileRepository,
    mime_types: MockMimeTypeRepository,
) -> AssetFactoryHarness {
    let committed = Arc::new(AtomicBool::new(false));
    let rolled_back = Arc::new(AtomicBool::new(false));
    let factory = AssetTestUnitOfWorkFactory {
        unit_of_work: Mutex::new(Some(AssetTestUnitOfWork {
            committed: Arc::clone(&committed),
            files,
            mime_types,
            rolled_back: Arc::clone(&rolled_back),
            uploads,
        })),
    };
    AssetFactoryHarness {
        committed,
        factory: Arc::new(factory),
        rolled_back,
    }
}

/// Build one [`AssetTestUnitOfWork`] and its lifecycle flags.
#[must_use]
pub fn asset_unit_of_work(
    uploads: MockUploadRepository,
    files: MockFileRepository,
    mime_types: MockMimeTypeRepository,
) -> (AssetTestUnitOfWork, Arc<AtomicBool>, Arc<AtomicBool>) {
    let committed = Arc::new(AtomicBool::new(false));
    let rolled_back = Arc::new(AtomicBool::new(false));
    let unit_of_work = AssetTestUnitOfWork {
        committed: Arc::clone(&committed),
        files,
        mime_types,
        rolled_back: Arc::clone(&rolled_back),
        uploads,
    };
    (unit_of_work, committed, rolled_back)
}
