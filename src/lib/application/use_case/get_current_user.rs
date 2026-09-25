use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tracing::error;

use crate::application::port::get_current_user::GetCurrentUserCommand;
use crate::application::port::get_current_user::GetCurrentUserError;
use crate::application::port::get_current_user::GetCurrentUserResponse;
use crate::application::port::get_current_user::GetCurrentUserUseCase;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;

/// Use case implementation for fetching the current user.
pub struct GetCurrentUser<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> GetCurrentUser<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> GetCurrentUserUseCase for GetCurrentUser<F>
where
    F: UnitOfWorkFactory,
    F::Uow: UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: GetCurrentUserCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<GetCurrentUserResponse, GetCurrentUserError>>
                + Send
                + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| GetCurrentUserError::Unknown(error.into()))?;

            let result = async {
                let user = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.user_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| GetCurrentUserError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetCurrentUserError::NoSuchUser)?;

                Ok(GetCurrentUserResponse::new(
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
                        .map_err(|error| GetCurrentUserError::Unknown(error.into()))?;
                    Ok(value)
                }
                Err(error) => {
                    if let Err(rollback_error) = unit_of_work.rollback().await {
                        error!(
                            error = ?rollback_error,
                            "failed to roll back the get current user unit of work"
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

    use crate::application::port::get_current_user::GetCurrentUserCommand;
    use crate::application::port::get_current_user::GetCurrentUserError;
    use crate::application::port::get_current_user::GetCurrentUserUseCase as _;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::model::user::UserError;
    use crate::domain::port::configuration_repository::MockConfigurationRepository;
    use crate::domain::port::credential_repository::MockCredentialRepository;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::unit_of_work_factory;

    use super::GetCurrentUser;

    type UseCase = GetCurrentUser<TestFactory>;

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
            use_case: GetCurrentUser::new(Arc::new(factory)),
            committed,
            rolled_back,
        })
    }

    fn user(id: i64, username: &str, email: Option<&str>) -> Result<User, UserError> {
        User::try_new(
            id,
            username.to_owned(),
            email.map(str::to_owned),
            id,
            Role::Standard,
        )
    }

    fn expect_user(user_repository: &mut MockUserRepository, user: User) {
        user_repository
            .expect_search()
            .times(1)
            .returning(move |_| {
                let own_user = user.clone();
                Box::pin(async move { Ok(vec![own_user]) })
            });
    }

    #[tokio::test]
    async fn get_current_user_existing_caller_returns_profile() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository| {
            expect_user(
                user_repository,
                user(1, "alice", Some("alice@example.com"))?,
            );
            Ok(())
        })?;
        let command = GetCurrentUserCommand::new(1);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.id(), 1);
        assert_eq!(response.username(), "alice");
        assert_eq!(response.email(), Some("alice@example.com"));
        assert_eq!(response.role(), Role::Standard);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_current_user_missing_email_returns_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository| {
            expect_user(user_repository, user(2, "brad", None)?);
            Ok(())
        })?;
        let command = GetCurrentUserCommand::new(2);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.email(), None);
        Ok(())
    }

    #[tokio::test]
    async fn get_current_user_unknown_caller_returns_no_such_user() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            Ok(())
        })?;
        let command = GetCurrentUserCommand::new(999);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(GetCurrentUserError::NoSuchUser)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_current_user_search_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;
        let command = GetCurrentUserCommand::new(1);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(GetCurrentUserError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
