use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tracing::error;

use crate::application::port::get_user::GetUserCommand;
use crate::application::port::get_user::GetUserError;
use crate::application::port::get_user::GetUserResponse;
use crate::application::port::get_user::GetUserUseCase;
use crate::domain::model::user::Role;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;

/// Use case implementation for fetching a user by identifier.
pub struct GetUser<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> GetUser<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> GetUserUseCase for GetUser<F>
where
    F: UnitOfWorkFactory,
    F::Uow: UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: GetUserCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetUserResponse, GetUserError>> + Send + 'future>> {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| GetUserError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| GetUserError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetUserError::NoSuchCaller)?;
                if caller.role() != Role::Admin {
                    return Err(GetUserError::NotAdmin);
                }

                let user = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.user_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| GetUserError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetUserError::NoSuchUser)?;

                Ok(GetUserResponse::new(
                    user.id(),
                    user.username().to_owned(),
                    user.email().map(str::to_owned),
                    user.role(),
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| GetUserError::Unknown(error.into()))?;
                    Ok(value)
                }
                Err(error) => {
                    if let Err(rollback_error) = unit_of_work.rollback().await {
                        error!(
                            error = ?rollback_error,
                            "failed to roll back the get user unit of work"
                        );
                    }
                    Err(error)
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;

    use crate::application::port::get_user::GetUserCommand;
    use crate::application::port::get_user::GetUserError;
    use crate::application::port::get_user::GetUserUseCase as _;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::model::user::UserError;
    use crate::domain::port::configuration_repository::MockConfigurationRepository;
    use crate::domain::port::credential_repository::MockCredentialRepository;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::unit_of_work_factory;

    use super::GetUser;

    type UseCase = GetUser<TestFactory>;

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
        setup: impl FnOnce(&mut MockUserRepository) -> Result<(), Box<dyn Error>>,
    ) -> Result<Harness, Box<dyn Error>> {
        let mut user_repository = MockUserRepository::new();
        setup(&mut user_repository)?;

        let committed = Arc::new(AtomicBool::new(false));
        let rolled_back = Arc::new(AtomicBool::new(false));
        let factory = unit_of_work_factory(
            user_repository,
            MockCredentialRepository::new(),
            MockConfigurationRepository::new(),
            Arc::clone(&committed),
            Arc::clone(&rolled_back),
        );

        Ok(Harness {
            use_case: GetUser::new(Arc::new(factory)),
            committed,
            rolled_back,
        })
    }

    fn user(id: i64, username: &str, email: Option<&str>, role: Role) -> Result<User, UserError> {
        User::try_new(id, username.to_owned(), email.map(str::to_owned), id, role)
    }

    fn expect_admin_and(
        user_repository: &mut MockUserRepository,
        target: Result<User, UserError>,
    ) -> Result<(), Box<dyn Error>> {
        let admin = user(1, "admin", None, Role::Admin)?;
        let fetched = target?;
        user_repository
            .expect_search()
            .times(2)
            .withf(|filter| filter.id == Some(1) || filter.id == Some(2))
            .returning(move |filter| {
                if filter.id == Some(1) {
                    let found = admin.clone();
                    Box::pin(async move { Ok(vec![found]) })
                } else {
                    let found = fetched.clone();
                    Box::pin(async move { Ok(vec![found]) })
                }
            });
        Ok(())
    }

    #[tokio::test]
    async fn get_user_admin_caller_returns_standard_target_profile() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository| {
            expect_admin_and(
                user_repository,
                user(2, "brad", Some("brad@example.com"), Role::Standard),
            )
        })?;
        let command = GetUserCommand::new(1, 2);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.id(), 2);
        assert_eq!(response.username(), "brad");
        assert_eq!(response.email(), Some("brad@example.com"));
        assert_eq!(response.role(), Role::Standard);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_user_target_without_email_returns_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository| {
            expect_admin_and(user_repository, user(2, "brad", None, Role::Admin))
        })?;
        let command = GetUserCommand::new(1, 2);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.email(), None);
        Ok(())
    }

    #[tokio::test]
    async fn get_user_admin_caller_fetching_self_returns_profile() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository| {
            expect_admin_and(user_repository, user(1, "admin", None, Role::Admin))
        })?;
        let command = GetUserCommand::new(1, 1);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.id(), 1);
        assert_eq!(response.role(), Role::Admin);
        Ok(())
    }

    #[tokio::test]
    async fn get_user_standard_caller_returns_not_admin() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository| {
            let caller = user(1, "alice", None, Role::Standard)?;
            user_repository
                .expect_search()
                .times(1)
                .withf(|filter| filter.id == Some(1))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })?;
        let command = GetUserCommand::new(1, 2);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(GetUserError::NotAdmin)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_user_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            Ok(())
        })?;
        let command = GetUserCommand::new(999, 2);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(GetUserError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_user_unknown_target_returns_no_such_user() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository| {
            let admin = user(1, "admin", None, Role::Admin)?;
            user_repository
                .expect_search()
                .times(2)
                .withf(|filter| filter.id == Some(1) || filter.id == Some(2))
                .returning(move |filter| {
                    if filter.id == Some(1) {
                        let found = admin.clone();
                        Box::pin(async move { Ok(vec![found]) })
                    } else {
                        Box::pin(async { Ok(Vec::new()) })
                    }
                });
            Ok(())
        })?;
        let command = GetUserCommand::new(1, 2);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(GetUserError::NoSuchUser)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_user_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;
        let command = GetUserCommand::new(1, 2);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(GetUserError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
