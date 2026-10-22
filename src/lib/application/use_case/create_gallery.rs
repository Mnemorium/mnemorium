use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use crate::application::port::create_gallery::CreateGalleryCommand;
use crate::application::port::create_gallery::CreateGalleryError;
use crate::application::port::create_gallery::CreateGalleryResponse;
use crate::application::port::create_gallery::CreateGalleryUseCase;
use crate::application::security_event;
use crate::domain::model::gallery::Gallery;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::gallery_repository::GalleryFilter;
use crate::domain::port::gallery_repository::GalleryRepository as _;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::service::name_policy::NamePolicy;
use crate::domain::service::name_policy::NamePolicyError;

/// Use case implementation for creating a gallery.
pub struct CreateGallery<F> {
    /// Factory opening the unit of work wrapping the creation.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> CreateGallery<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> CreateGalleryUseCase for CreateGallery<F>
where
    F: UnitOfWorkFactory,
    F::Uow: LibraryUnitOfWork + UserUnitOfWork,
{
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "RepositoryError is non_exhaustive, so a catch-all arm is required"
    )]
    fn execute<'future>(
        &'future self,
        command: CreateGalleryCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<CreateGalleryResponse, CreateGalleryError>> + Send + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| CreateGalleryError::Unknown(error.into()))?;

            let result =
                async {
                    let _caller = unit_of_work
                        .users()
                        .search(&UserFilter {
                            id: Some(command.caller_id()),
                            ..UserFilter::default()
                        })
                        .await
                        .map_err(|error| CreateGalleryError::Unknown(error.into()))?
                        .into_iter()
                        .next()
                        .ok_or(CreateGalleryError::NoSuchCaller)?;

                    let name = NamePolicy::validate(command.name().to_owned()).map_err(
                        |error| match error {
                            NamePolicyError::Empty => {
                                security_event::input_validation_failed("name", "empty");
                                CreateGalleryError::InvalidName
                            }
                            NamePolicyError::TooLong => {
                                security_event::input_validation_failed("name", "too_long");
                                CreateGalleryError::NameTooLong
                            }
                        },
                    )?;
                    let now = Utc::now().naive_utc();
                    let gallery = Gallery::new(
                        0,
                        Some(command.caller_id()),
                        name,
                        command.is_public(),
                        now,
                        now,
                    );

                    let existing = unit_of_work
                        .galleries()
                        .search(&GalleryFilter {
                            name: Some(gallery.name().to_owned()),
                            user_id: Some(command.caller_id()),
                            ..GalleryFilter::default()
                        })
                        .await
                        .map_err(|error| CreateGalleryError::Unknown(error.into()))?;
                    if !existing.is_empty() {
                        return Err(CreateGalleryError::NameAlreadyExists);
                    }

                    let created = unit_of_work.galleries().create(gallery).await.map_err(
                        |error| match error {
                            // A concurrent insert won the race for the per-owner
                            // unique name; the use case translates the port's
                            // `AlreadyExist` so the caller cannot own a second
                            // gallery with the same name either way (`STY-RUST-015`).
                            RepositoryError::AlreadyExist => CreateGalleryError::NameAlreadyExists,
                            other => CreateGalleryError::Unknown(other.into()),
                        },
                    )?;

                    Ok(CreateGalleryResponse::new(
                        created.gallery_id(),
                        command.caller_id(),
                        created.name().to_owned(),
                        created.is_public(),
                        created.created_at(),
                        created.last_modified_at(),
                    ))
                }
                .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| CreateGalleryError::Unknown(error.into()))?;
                    Ok(value)
                }
                Err(error) => {
                    // The unit-of-work adapter owns the rollback-failure log (OBS-002).
                    let _rollback = unit_of_work.rollback().await;
                    Err(error)
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io as stdio;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::PoisonError;
    use std::sync::atomic::Ordering;

    use chrono::NaiveDateTime;
    use tracing::subscriber::DefaultGuard;
    use tracing::subscriber::set_default;
    use tracing_subscriber::fmt;
    use tracing_subscriber::layer::SubscriberExt as _;

    use crate::application::port::create_gallery::CreateGalleryCommand;
    use crate::application::port::create_gallery::CreateGalleryError;
    use crate::application::port::create_gallery::CreateGalleryResponse;
    use crate::application::port::create_gallery::CreateGalleryUseCase as _;
    use crate::domain::model::gallery::Gallery;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::gallery_repository::MockGalleryRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::GalleryFactoryHarness;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::TestUnitOfWork;

    use super::CreateGallery;

    type UseCase = CreateGallery<TestFactory>;

    /// Append-only sink that lets a test read back the lines [`fmt`] emits.
    #[derive(Clone)]
    struct CaptureWriter {
        /// Buffer shared with the test that asserts on the captured output.
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "only the raw `write` and `flush` are meaningful for an in-memory capture buffer"
    )]
    impl stdio::Write for CaptureWriter {
        fn flush(&mut self) -> stdio::Result<()> {
            Ok(())
        }

        fn write(&mut self, buf: &[u8]) -> stdio::Result<usize> {
            let mut buffer = self.buffer.lock().unwrap_or_else(PoisonError::into_inner);
            buffer.extend_from_slice(buf);
            Ok(buf.len())
        }
    }

    /// Hand every formatted event to a fresh clone of the shared buffer.
    #[expect(
        clippy::missing_trait_methods,
        reason = "the default `make_writer_for` already routes through `make_writer`"
    )]
    impl<'writer> fmt::MakeWriter<'writer> for CaptureWriter {
        type Writer = Self;

        fn make_writer(&'writer self) -> Self::Writer {
            self.clone()
        }
    }

    fn gallery(gallery_id: i64, user_id: Option<i64>, name: &str, is_public: bool) -> Gallery {
        let now = NaiveDateTime::default();
        Gallery::new(gallery_id, user_id, name.to_owned(), is_public, now, now)
    }

    fn harness_with(
        users: MockUserRepository,
        galleries: MockGalleryRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            galleries,
            users,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
    }

    /// Expect the pre-create name-collision search to find no gallery.
    fn expect_no_collision(galleries: &mut MockGalleryRepository) {
        galleries
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
    }

    fn expect_caller(users: &mut MockUserRepository, caller_id: i64) -> Result<(), Box<dyn Error>> {
        let caller = User::try_new(
            caller_id,
            "caller".to_owned(),
            None,
            caller_id,
            Role::Standard,
        )?;
        users
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(caller_id))
            .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
        Ok(())
    }

    /// Install a capturing subscriber on the current thread and return the
    /// shared buffer plus the guard that keeps it active.
    ///
    /// The guard must stay alive for the whole execute: `#[tokio::test]` runs on
    /// a current-thread runtime, so the awaited call is polled on this thread and
    /// sees the scoped default dispatcher.
    fn capture_logs() -> (Arc<Mutex<Vec<u8>>>, DefaultGuard) {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::registry().with(
            fmt::layer().with_ansi(false).with_writer(CaptureWriter {
                buffer: Arc::clone(&buffer),
            }),
        );
        let guard = set_default(subscriber);
        (buffer, guard)
    }

    /// Read the captured bytes back as a lossy UTF-8 string.
    fn captured_logs(buffer: &Mutex<Vec<u8>>) -> String {
        let bytes = buffer.lock().unwrap_or_else(PoisonError::into_inner);
        String::from_utf8_lossy(bytes.as_slice()).into_owned()
    }

    #[tokio::test]
    async fn create_gallery_creates_owned_private_gallery() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut galleries = MockGalleryRepository::new();
        let stored = gallery(9, Some(1), "Holidays", false);
        expect_no_collision(&mut galleries);
        galleries
            .expect_create()
            .times(1)
            .withf(|pending| pending.user_id() == Some(1) && !pending.is_public())
            .return_once(move |_| {
                let found = stored.clone();
                Box::pin(async move { Ok(found) })
            });
        let harness = harness_with(users, galleries);
        let use_case: UseCase = CreateGallery::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(CreateGalleryCommand::new(1, "Holidays".to_owned(), false))
            .await?;

        // Assert
        assert_eq!(
            response,
            CreateGalleryResponse::new(
                9,
                1,
                "Holidays".to_owned(),
                false,
                NaiveDateTime::default(),
                NaiveDateTime::default(),
            )
        );
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_gallery_public_flag_is_persisted() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut galleries = MockGalleryRepository::new();
        let stored = gallery(4, Some(1), "Shared", true);
        expect_no_collision(&mut galleries);
        galleries
            .expect_create()
            .times(1)
            .withf(Gallery::is_public)
            .return_once(move |_| {
                let found = stored.clone();
                Box::pin(async move { Ok(found) })
            });
        let harness = harness_with(users, galleries);
        let use_case: UseCase = CreateGallery::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(CreateGalleryCommand::new(1, "Shared".to_owned(), true))
            .await?;

        // Assert
        assert!(response.is_public());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_gallery_trims_surrounding_whitespace() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut galleries = MockGalleryRepository::new();
        let stored = gallery(6, Some(1), "Holidays", false);
        galleries
            .expect_search()
            .times(1)
            .withf(|filter| filter.name.as_deref() == Some("Holidays") && filter.user_id == Some(1))
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        galleries
            .expect_create()
            .times(1)
            .withf(|pending| pending.name() == "Holidays")
            .return_once(move |_| {
                let found = stored.clone();
                Box::pin(async move { Ok(found) })
            });
        let harness = harness_with(users, galleries);
        let use_case: UseCase = CreateGallery::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(CreateGalleryCommand::new(
                1,
                "  Holidays  ".to_owned(),
                false,
            ))
            .await?;

        // Assert
        assert_eq!(response.name(), "Holidays");
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_gallery_empty_name_returns_invalid_name() -> Result<(), Box<dyn Error>> {
        // Arrange
        let (buffer, _capture) = capture_logs();
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut galleries = MockGalleryRepository::new();
        galleries.expect_create().times(0);
        let harness = harness_with(users, galleries);
        let use_case: UseCase = CreateGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateGalleryCommand::new(1, "   ".to_owned(), false))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateGalleryError::InvalidName)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        let logs = captured_logs(&buffer);
        assert_eq!(
            logs.matches("WARN").count(),
            1,
            "the empty-name rejection must emit exactly one warn event: {logs}"
        );
        assert_eq!(
            logs.matches("event=\"input_validation_failed\"").count(),
            1,
            "the empty-name rejection must emit exactly one input_validation_failed event: {logs}"
        );
        assert!(
            logs.contains("security"),
            "the event must target the reserved security target: {logs}"
        );
        assert!(
            logs.contains("field=name"),
            "the event must name the rejected field: {logs}"
        );
        assert!(
            logs.contains("reason=empty"),
            "the event must classify the reason as an empty name: {logs}"
        );
        assert!(
            !logs.contains("\"   \""),
            "the event must not echo the rejected name: {logs}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn create_gallery_long_name_returns_name_too_long() -> Result<(), Box<dyn Error>> {
        // Arrange
        let (buffer, _capture) = capture_logs();
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut galleries = MockGalleryRepository::new();
        galleries.expect_create().times(0);
        let harness = harness_with(users, galleries);
        let use_case: UseCase = CreateGallery::new(Arc::clone(&harness.factory));
        let name = "x".repeat(1_000);

        // Act
        let result = use_case
            .execute(CreateGalleryCommand::new(1, name.clone(), false))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateGalleryError::NameTooLong)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        let logs = captured_logs(&buffer);
        assert_eq!(
            logs.matches("WARN").count(),
            1,
            "the over-long-name rejection must emit exactly one warn event: {logs}"
        );
        assert_eq!(
            logs.matches("event=\"input_validation_failed\"").count(),
            1,
            "the over-long-name rejection must emit exactly one input_validation_failed event: {logs}"
        );
        assert!(
            logs.contains("security"),
            "the event must target the reserved security target: {logs}"
        );
        assert!(
            logs.contains("field=name"),
            "the event must name the rejected field: {logs}"
        );
        assert!(
            logs.contains("reason=too_long"),
            "the event must classify the reason as an over-long name: {logs}"
        );
        assert!(
            !logs.contains(&name),
            "the event must not echo the client-supplied name: {logs}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn create_gallery_existing_name_returns_name_already_exists() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut galleries = MockGalleryRepository::new();
        let existing = gallery(3, Some(1), "Holidays", false);
        galleries
            .expect_search()
            .times(1)
            .withf(|filter| filter.name.as_deref() == Some("Holidays") && filter.user_id == Some(1))
            .return_once(move |_| {
                let found = existing.clone();
                Box::pin(async move { Ok(vec![found]) })
            });
        galleries.expect_create().times(0);
        let harness = harness_with(users, galleries);
        let use_case: UseCase = CreateGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateGalleryCommand::new(1, "Holidays".to_owned(), false))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateGalleryError::NameAlreadyExists)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_gallery_raced_duplicate_name_returns_name_already_exists()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut galleries = MockGalleryRepository::new();
        expect_no_collision(&mut galleries);
        galleries
            .expect_create()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::AlreadyExist) }));
        let harness = harness_with(users, galleries);
        let use_case: UseCase = CreateGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateGalleryCommand::new(1, "Holidays".to_owned(), false))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateGalleryError::NameAlreadyExists)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_gallery_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, MockGalleryRepository::new());
        let use_case: UseCase = CreateGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateGalleryCommand::new(999, "Holidays".to_owned(), false))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateGalleryError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_gallery_caller_load_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, MockGalleryRepository::new());
        let use_case: UseCase = CreateGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateGalleryCommand::new(1, "Holidays".to_owned(), false))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateGalleryError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_gallery_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut galleries = MockGalleryRepository::new();
        expect_no_collision(&mut galleries);
        galleries
            .expect_create()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, galleries);
        let use_case: UseCase = CreateGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateGalleryCommand::new(1, "Holidays".to_owned(), false))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateGalleryError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_gallery_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut galleries = MockGalleryRepository::new();
        let stored = gallery(9, Some(1), "Holidays", false);
        expect_no_collision(&mut galleries);
        galleries.expect_create().times(1).return_once(move |_| {
            let found = stored.clone();
            Box::pin(async move { Ok(found) })
        });
        let harness = harness_with(users, galleries);
        harness.commit_fails.store(true, Ordering::SeqCst);
        let use_case: UseCase = CreateGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateGalleryCommand::new(1, "Holidays".to_owned(), false))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateGalleryError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }
}
