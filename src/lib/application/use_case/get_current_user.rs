use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::get_current_user::GetCurrentUserCommand;
use crate::application::port::get_current_user::GetCurrentUserError;
use crate::application::port::get_current_user::GetCurrentUserResponse;
use crate::application::port::get_current_user::GetCurrentUserUseCase;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository;

/// Use case implementation for fetching the current user.
pub struct GetCurrentUser<U> {
    /// Repository persisting users.
    user_repository: Arc<U>,
}

impl<U: UserRepository> GetCurrentUser<U> {
    /// Create a new use case.
    #[must_use]
    pub fn new(user_repository: Arc<U>) -> Self {
        Self { user_repository }
    }
}

impl<U: UserRepository> GetCurrentUserUseCase for GetCurrentUser<U> {
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
        let user_repository = Arc::clone(&self.user_repository);

        Box::pin(async move {
            let user = user_repository
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
        })
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use crate::application::port::get_current_user::GetCurrentUserCommand;
    use crate::application::port::get_current_user::GetCurrentUserError;
    use crate::application::port::get_current_user::GetCurrentUserUseCase as _;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::model::user::UserError;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::user_repository::MockUserRepository;

    use super::GetCurrentUser;

    type UseCase = GetCurrentUser<MockUserRepository>;

    fn use_case_with(
        setup: impl FnOnce(&mut MockUserRepository) -> Result<(), Box<dyn Error>>,
    ) -> Result<UseCase, Box<dyn Error>> {
        let mut user_repository = MockUserRepository::new();
        setup(&mut user_repository)?;
        Ok(GetCurrentUser::new(Arc::new(user_repository)))
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
        let use_case = use_case_with(|user_repository| {
            expect_user(
                user_repository,
                user(1, "alice", Some("alice@example.com"))?,
            );
            Ok(())
        })?;
        let command = GetCurrentUserCommand::new(1);

        // Act
        let response = use_case.execute(command).await?;

        // Assert
        assert_eq!(response.id(), 1);
        assert_eq!(response.username(), "alice");
        assert_eq!(response.email(), Some("alice@example.com"));
        assert_eq!(response.role(), Role::Standard);
        Ok(())
    }

    #[tokio::test]
    async fn get_current_user_missing_email_returns_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository| {
            expect_user(user_repository, user(2, "brad", None)?);
            Ok(())
        })?;
        let command = GetCurrentUserCommand::new(2);

        // Act
        let response = use_case.execute(command).await?;

        // Assert
        assert_eq!(response.email(), None);
        Ok(())
    }

    #[tokio::test]
    async fn get_current_user_unknown_caller_returns_no_such_user() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            Ok(())
        })?;
        let command = GetCurrentUserCommand::new(999);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(GetCurrentUserError::NoSuchUser)));
        Ok(())
    }

    #[tokio::test]
    async fn get_current_user_search_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = use_case_with(|user_repository| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;
        let command = GetCurrentUserCommand::new(1);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(GetCurrentUserError::Unknown(_))));
        Ok(())
    }
}
