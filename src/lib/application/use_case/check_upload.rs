use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tracing::error;

use crate::application::port::check_upload::CheckUploadCommand;
use crate::application::port::check_upload::CheckUploadError;
use crate::application::port::check_upload::CheckUploadResponse;
use crate::application::port::check_upload::CheckUploadUseCase;
use crate::domain::model::upload::MD5_INTEGRITY_LENGTH;
use crate::domain::port::asset_unit_of_work::AssetUnitOfWork;
use crate::domain::port::file_repository::FileFilter;
use crate::domain::port::file_repository::FileRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;

/// Use case implementation for checking whether a file already exists.
pub struct CheckUpload<F> {
    /// Factory opening the unit of work wrapping the lookup.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> CheckUpload<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> CheckUploadUseCase for CheckUpload<F>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: CheckUploadCommand,
    ) -> Pin<Box<dyn Future<Output = Result<CheckUploadResponse, CheckUploadError>> + Send + 'future>>
    {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let md5 = validate_md5(command.md5())?;

            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| CheckUploadError::Unknown(error.into()))?;

            let result = async {
                let file_id = unit_of_work
                    .files()
                    .search(&FileFilter {
                        md5_integrity: Some(md5),
                        user_id: Some(command.user_id()),
                        ..FileFilter::default()
                    })
                    .await
                    .map_err(|error| CheckUploadError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .map(|file| file.id());

                Ok(CheckUploadResponse::new(file_id))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| CheckUploadError::Unknown(error.into()))?;
                    Ok(value)
                }
                Err(error) => {
                    if let Err(rollback_error) = unit_of_work.rollback().await {
                        error!(
                            error = ?rollback_error,
                            "failed to roll back the check upload unit of work"
                        );
                    }
                    Err(error)
                }
            }
        })
    }
}

/// Validate that `md5` is a 32-character hexadecimal string, returning its
/// lowercase form.
///
/// # Errors
///
/// Returns [`CheckUploadError::InvalidMd5`] when the digest does not match.
#[expect(
    clippy::single_call_fn,
    reason = "the digest validation is named after the rule it enforces"
)]
fn validate_md5(md5: &str) -> Result<String, CheckUploadError> {
    let is_hex = md5.len() == MD5_INTEGRITY_LENGTH
        && md5.chars().all(|character| character.is_ascii_hexdigit());
    if is_hex {
        Ok(md5.to_ascii_lowercase())
    } else {
        Err(CheckUploadError::InvalidMd5)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;

    use chrono::NaiveDate;

    use crate::application::port::check_upload::CheckUploadCommand;
    use crate::application::port::check_upload::CheckUploadError;
    use crate::application::port::check_upload::CheckUploadUseCase as _;
    use crate::application::use_case::test_support::asset_factory;
    use crate::domain::model::file::File;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::file_repository::MockFileRepository;
    use crate::domain::port::mime_type_repository::MockMimeTypeRepository;
    use crate::domain::port::upload_repository::MockUploadRepository;

    use super::CheckUpload;

    const DIGEST: &str = "0123456789abcdef0123456789abcdef";
    const OTHER_DIGEST: &str = "fedcba9876543210fedcba9876543210";

    type UseCase = CheckUpload<super::super::test_support::AssetTestUnitOfWorkFactory>;

    /// A use case under test together with its transaction-lifecycle flags.
    struct Harness {
        /// Set when the unit of work is committed.
        committed: Arc<AtomicBool>,
        /// Set when the unit of work is rolled back.
        rolled_back: Arc<AtomicBool>,
        /// The use case under test.
        use_case: UseCase,
    }

    fn use_case_with(
        setup: impl FnOnce(&mut MockFileRepository) -> Result<(), Box<dyn Error>>,
    ) -> Result<Harness, Box<dyn Error>> {
        let mut files = MockFileRepository::new();
        setup(&mut files)?;
        let harness = asset_factory(
            MockUploadRepository::new(),
            files,
            MockMimeTypeRepository::new(),
        );
        Ok(Harness {
            use_case: CheckUpload::new(Arc::clone(&harness.factory)),
            committed: harness.committed,
            rolled_back: harness.rolled_back,
        })
    }

    fn stored_file(id: i64, md5_integrity: &str, user_id: i64) -> Result<File, RepositoryError> {
        File::try_new(
            id,
            format!("files/{id}_clip.mp4"),
            user_id,
            false,
            "video/mp4".to_owned(),
            NaiveDate::from_ymd_opt(2026, 1, 1).unwrap_or_default(),
            md5_integrity.to_owned(),
        )
        .map_err(|_| RepositoryError::OperationFailed)
    }

    #[tokio::test]
    async fn check_upload_owned_file_returns_file_id() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|files| {
            files.expect_search().times(1).returning(|_| {
                let stored = stored_file(7, DIGEST, 3);
                Box::pin(async move { stored.map(|file| vec![file]) })
            });
            Ok(())
        })?;
        let command = CheckUploadCommand::new(DIGEST.to_owned(), 3);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.file_id(), Some(7));
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn check_upload_unknown_digest_returns_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|files| {
            files
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            Ok(())
        })?;
        let command = CheckUploadCommand::new(OTHER_DIGEST.to_owned(), 3);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.file_id(), None);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn check_upload_invalid_md5_returns_invalid_md5() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|_| Ok(()))?;
        let command = CheckUploadCommand::new("not-a-digest".to_owned(), 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CheckUploadError::InvalidMd5)));
        assert!(!harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn check_upload_uppercase_md5_is_normalised() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|files| {
            files.expect_search().times(1).returning(|filter| {
                assert_eq!(filter.md5_integrity.as_deref(), Some(DIGEST));
                let stored = stored_file(9, DIGEST, 3);
                Box::pin(async move { stored.map(|file| vec![file]) })
            });
            Ok(())
        })?;
        let command = CheckUploadCommand::new(DIGEST.to_uppercase(), 3);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.file_id(), Some(9));
        Ok(())
    }

    #[tokio::test]
    async fn check_upload_search_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|files| {
            files
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;
        let command = CheckUploadCommand::new(DIGEST.to_owned(), 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CheckUploadError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
