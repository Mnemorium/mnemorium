use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::get_user::GetUserCommand;
use crate::application::port::get_user::GetUserError;
use crate::application::port::get_user::GetUserResponse;
use crate::application::port::get_user::GetUserUseCase;
use crate::domain::model::user::Role;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository;

/// Use case implementation for fetching a user by identifier.
pub struct GetUser<U> {
    /// Repository persisting users.
    user_repository: Arc<U>,
}

impl<U: UserRepository> GetUser<U> {
    /// Create a new use case.
    #[must_use]
    pub fn new(user_repository: Arc<U>) -> Self {
        Self { user_repository }
    }
}

impl<U: UserRepository> GetUserUseCase for GetUser<U> {
    fn execute<'future>(
        &'future self,
        command: GetUserCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetUserResponse, GetUserError>> + Send + 'future>> {
        let user_repository = Arc::clone(&self.user_repository);

        Box::pin(async move {
            let caller = user_repository
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

            let user = user_repository
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
        })
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use crate::application::port::get_user::GetUserCommand;
    use crate::application::port::get_user::GetUserError;
    use crate::application::port::get_user::GetUserUseCase as _;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::model::user::UserError;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::user_repository::MockUserRepository;

    use super::GetUser;

    type UseCase = GetUser<MockUserRepository>;

    fn use_case_with(
        setup: impl FnOnce(&mut MockUserRepository) -> Result<(), Box<dyn Error>>,
    ) -> Result<UseCase, Box<dyn Error>> {
        let mut user_repository = MockUserRepository::new();
        setup(&mut user_repository)?;
        Ok(GetUser::new(Arc::new(user_repository)))
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
        let use_case = use_case_with(|user_repository| {
            expect_admin_and(
                user_repository,
                user(2, "brad", Some("brad@example.com"), Role::Standard),
            )
        })?;
        let command = GetUserCommand::new(1, 2);

        // Act
        let response = use_case.execute(command).await?;

        // Assert
        assert_eq!(response.id(), 2);
        assert_eq!(response.username(), "brad");
        assert_eq!(response.email(), Some("brad@example.com"));
        assert_eq!(response.role(), Role::Standard);
        Ok(())
    }

    #[tokio::test]
    async fn get_user_target_without_email_returns_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository| {
            expect_admin_and(user_repository, user(2, "brad", None, Role::Admin))
        })?;
        let command = GetUserCommand::new(1, 2);

        // Act
        let response = use_case.execute(command).await?;

        // Assert
        assert_eq!(response.email(), None);
        Ok(())
    }

    #[tokio::test]
    async fn get_user_admin_caller_fetching_self_returns_profile() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository| {
            expect_admin_and(user_repository, user(1, "admin", None, Role::Admin))
        })?;
        let command = GetUserCommand::new(1, 1);

        // Act
        let response = use_case.execute(command).await?;

        // Assert
        assert_eq!(response.id(), 1);
        assert_eq!(response.role(), Role::Admin);
        Ok(())
    }

    #[tokio::test]
    async fn get_user_standard_caller_returns_not_admin() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository| {
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
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(GetUserError::NotAdmin)));
        Ok(())
    }

    #[tokio::test]
    async fn get_user_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            Ok(())
        })?;
        let command = GetUserCommand::new(999, 2);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(GetUserError::NoSuchCaller)));
        Ok(())
    }

    #[tokio::test]
    async fn get_user_unknown_target_returns_no_such_user() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository| {
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
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(GetUserError::NoSuchUser)));
        Ok(())
    }

    #[tokio::test]
    async fn get_user_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;
        let command = GetUserCommand::new(1, 2);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(GetUserError::Unknown(_))));
        Ok(())
    }
}
