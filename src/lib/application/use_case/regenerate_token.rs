use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tracing::info;

use crate::application::port::{
    RegenerateTokenCommand, RegenerateTokenError, RegenerateTokenUseCase,
};
use crate::domain::port::{
    IssuedToken, TokenProvider, UnitOfWork as _, UnitOfWorkFactory, UserFilter,
    UserRepository as _, UserUnitOfWork,
};

/// Use case implementation for regenerating the caller's access token.
pub(crate) struct RegenerateToken<F, P> {
    /// Provider issuing and validating tokens.
    token_provider: Arc<P>,
    /// Factory opening the unit of work wrapping the caller lookup.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, P: TokenProvider> RegenerateToken<F, P> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>, token_provider: Arc<P>) -> Self {
        Self {
            token_provider,
            unit_of_work_factory,
        }
    }
}

impl<F, P> RegenerateTokenUseCase for RegenerateToken<F, P>
where
    F: UnitOfWorkFactory,
    F::Uow: UserUnitOfWork,
    P: TokenProvider,
{
    fn execute<'future>(
        &'future self,
        command: RegenerateTokenCommand,
    ) -> Pin<Box<dyn Future<Output = Result<IssuedToken, RegenerateTokenError>> + Send + 'future>>
    {
        let token_provider = Arc::clone(&self.token_provider);
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| RegenerateTokenError::Unknown(error.into()))?;

            let result = async {
                // The caller's account may have been deleted after the token
                // was issued, so a valid token alone cannot mint a successor.
                let user = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.user_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| RegenerateTokenError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(RegenerateTokenError::NoSuchUser)?;

                let token = token_provider
                    .issue(user.id())
                    .await
                    .map_err(|error| RegenerateTokenError::Unknown(error.into()))?;

                Ok((user.id(), token))
            }
            .await;

            match result {
                Ok((actor, token)) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| RegenerateTokenError::Unknown(error.into()))?;
                    info!(
                        target: "security",
                        event = "token_regenerated",
                        actor,
                        "regenerated a token"
                    );
                    Ok(token)
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
    use super::*;
    use crate::domain::model::{Role, User, UserError};

    use crate::domain::port::{
        IssuedToken, MockTokenProvider, MockUserRepository, RepositoryError, TokenProviderError,
    };

    use crate::test_helpers::*;
    use std::error::Error;
    use std::sync::atomic::{AtomicBool, Ordering};

    type UseCase = RegenerateToken<TestFactory, MockTokenProvider>;

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
        setup: impl FnOnce(
            &mut MockUserRepository,
            &mut MockTokenProvider,
        ) -> Result<(), Box<dyn Error>>,
    ) -> Result<Harness, Box<dyn Error>> {
        let mut user_repository = MockUserRepository::new();
        let mut token_provider = MockTokenProvider::new();

        setup(&mut user_repository, &mut token_provider)?;

        let unit_of_work = TestUnitOfWork {
            users: user_repository,
            ..TestUnitOfWork::default()
        };
        let committed = Arc::clone(&unit_of_work.committed);
        let rolled_back = Arc::clone(&unit_of_work.rolled_back);
        let factory = TestFactory::new(unit_of_work);

        Ok(Harness {
            use_case: RegenerateToken::new(Arc::new(factory), Arc::new(token_provider)),
            committed,
            rolled_back,
        })
    }

    fn user(id: i64) -> Result<User, UserError> {
        User::try_new(id, format!("user{id}"), None, id, Role::Standard)
    }

    fn command(user_id: i64) -> RegenerateTokenCommand {
        RegenerateTokenCommand::new(user_id)
    }

    #[tokio::test]
    async fn regenerate_token_issues_a_token_for_the_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, token_provider| {
            let own_user = user(7)?;
            user_repository
                .expect_search()
                .times(1)
                .returning(move |_| {
                    let stored = own_user.clone();
                    Box::pin(async move { Ok(vec![stored]) })
                });
            token_provider
                .expect_issue()
                .withf(|user_id| *user_id == 7)
                .times(1)
                .returning(|_| Box::pin(async { Ok(IssuedToken::new("fresh".to_owned(), 3600)) }));
            Ok(())
        })?;

        // Act
        let response = harness.use_case.execute(command(7)).await?;

        // Assert
        assert_eq!(response.value(), "fresh");
        assert_eq!(response.expires_in(), 3600);
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn regenerate_token_for_a_deleted_account_is_rejected() -> Result<(), Box<dyn Error>> {
        // Arrange: the token is valid, but its subject no longer exists.
        let harness = use_case_with(|user_repository, token_provider| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            token_provider.expect_issue().never();
            Ok(())
        })?;

        // Act
        let result = harness.use_case.execute(command(7)).await;

        // Assert
        assert!(matches!(result, Err(RegenerateTokenError::NoSuchUser)));
        assert!(!harness.committed.load(Ordering::SeqCst));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn regenerate_token_issue_failure_is_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, token_provider| {
            let own_user = user(7)?;
            user_repository
                .expect_search()
                .times(1)
                .returning(move |_| {
                    let stored = own_user.clone();
                    Box::pin(async move { Ok(vec![stored]) })
                });
            token_provider
                .expect_issue()
                .times(1)
                .returning(|_| Box::pin(async { Err(TokenProviderError::OperationFailed) }));
            Ok(())
        })?;

        // Act
        let result = harness.use_case.execute(command(7)).await;

        // Assert
        assert!(matches!(result, Err(RegenerateTokenError::Unknown(_))));
        assert!(!harness.committed.load(Ordering::SeqCst));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn regenerate_token_repository_failure_is_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository, token_provider| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            token_provider.expect_issue().never();
            Ok(())
        })?;

        // Act
        let result = harness.use_case.execute(command(7)).await;

        // Assert
        assert!(matches!(result, Err(RegenerateTokenError::Unknown(_))));
        assert!(!harness.committed.load(Ordering::SeqCst));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
