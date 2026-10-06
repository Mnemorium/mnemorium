use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::list_users::ListUsersCommand;
use crate::application::port::list_users::ListUsersError;
use crate::application::port::list_users::ListUsersResponse;
use crate::application::port::list_users::ListUsersUseCase;
use crate::application::security_event;
use crate::domain::model::user::Role;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;

/// Use case implementation for listing the users of the instance.
pub struct ListUsers<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> ListUsers<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> ListUsersUseCase for ListUsers<F>
where
    F: UnitOfWorkFactory,
    F::Uow: UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: ListUsersCommand,
    ) -> Pin<
        Box<dyn Future<Output = Result<Vec<ListUsersResponse>, ListUsersError>> + Send + 'future>,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| ListUsersError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| ListUsersError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(ListUsersError::NoSuchCaller)?;
                if caller.role() != Role::Admin {
                    security_event::authorization_failed(caller.id(), "read", "user_list");
                    return Err(ListUsersError::NotAdmin);
                }

                let users = unit_of_work
                    .users()
                    .search(&UserFilter {
                        email: command.email().map(str::to_owned),
                        role: command.role(),
                        username: command.username().map(str::to_owned),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| ListUsersError::Unknown(error.into()))?;

                Ok(users
                    .into_iter()
                    .map(|user| {
                        ListUsersResponse::new(
                            user.id(),
                            user.username().to_owned(),
                            user.email().map(str::to_owned),
                            user.role(),
                        )
                    })
                    .collect())
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| ListUsersError::Unknown(error.into()))?;
                    security_event::sensitive_data_access(command.caller_id(), "user_list");
                    Ok(value)
                }
                Err(error) => {
                    if unit_of_work.rollback().await.is_err() {
                        // The unit-of-work adapter owns the rollback-failure log (OBS-002).
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

    use crate::application::port::list_users::ListUsersCommand;
    use crate::application::port::list_users::ListUsersError;
    use crate::application::port::list_users::ListUsersResponse;
    use crate::application::port::list_users::ListUsersUseCase as _;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::model::user::UserError;
    use crate::domain::port::configuration_repository::MockConfigurationRepository;
    use crate::domain::port::credential_repository::MockCredentialRepository;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::unit_of_work_factory;

    use super::ListUsers;

    type UseCase = ListUsers<TestFactory>;

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
            use_case: ListUsers::new(Arc::new(factory)),
            committed,
            rolled_back,
        })
    }

    fn user(id: i64, username: &str, email: Option<&str>, role: Role) -> Result<User, UserError> {
        User::try_new(id, username.to_owned(), email.map(str::to_owned), id, role)
    }

    fn expect_admin(user_repository: &mut MockUserRepository, admin: User) {
        user_repository
            .expect_search()
            .times(1)
            .withf(|filter| filter.id == Some(1))
            .return_once(move |_| {
                let found = admin.clone();
                Box::pin(async move { Ok(vec![found]) })
            });
    }

    #[tokio::test]
    async fn list_users_admin_caller_returns_matching_users() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository| {
            let admin = user(1, "admin", None, Role::Admin)?;
            let alice = user(2, "alice", Some("alice@example.com"), Role::Standard)?;
            let brad = user(3, "brad", None, Role::Admin)?;
            expect_admin(user_repository, admin);
            user_repository
                .expect_search()
                .times(1)
                .withf(|filter| filter.id.is_none())
                .return_once(move |_| {
                    let found = vec![alice.clone(), brad.clone()];
                    Box::pin(async move { Ok(found) })
                });
            Ok(())
        })?;

        // Act
        let response = harness
            .use_case
            .execute(ListUsersCommand::new(1, None, None, None))
            .await?;

        // Assert
        assert_eq!(
            response,
            vec![
                ListUsersResponse::new(
                    2,
                    "alice".to_owned(),
                    Some("alice@example.com".to_owned()),
                    Role::Standard,
                ),
                ListUsersResponse::new(3, "brad".to_owned(), None, Role::Admin),
            ]
        );
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn list_users_forwards_filters_and_leaves_identifier_unset() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let harness = use_case_with(|user_repository| {
            let admin = user(1, "admin", None, Role::Admin)?;
            let alice = user(2, "alice", Some("alice@example.com"), Role::Standard)?;
            expect_admin(user_repository, admin);
            user_repository
                .expect_search()
                .times(1)
                .withf(|filter| {
                    filter.id.is_none()
                        && filter.email.as_deref() == Some("alice@example.com")
                        && filter.role == Some(Role::Standard)
                        && filter.username.as_deref() == Some("alice")
                })
                .return_once(move |_| {
                    let found = vec![alice.clone()];
                    Box::pin(async move { Ok(found) })
                });
            Ok(())
        })?;

        // Act
        let response = harness
            .use_case
            .execute(ListUsersCommand::new(
                1,
                Some("alice@example.com".to_owned()),
                Some(Role::Standard),
                Some("alice".to_owned()),
            ))
            .await?;

        // Assert
        assert_eq!(
            response,
            vec![ListUsersResponse::new(
                2,
                "alice".to_owned(),
                Some("alice@example.com".to_owned()),
                Role::Standard,
            )]
        );
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn list_users_no_match_returns_empty_list() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository| {
            let admin = user(1, "admin", None, Role::Admin)?;
            expect_admin(user_repository, admin);
            user_repository
                .expect_search()
                .times(1)
                .withf(|filter| filter.id.is_none())
                .return_once(|_| Box::pin(async { Ok(Vec::new()) }));
            Ok(())
        })?;

        // Act
        let response = harness
            .use_case
            .execute(ListUsersCommand::new(1, None, None, None))
            .await?;

        // Assert
        assert!(response.is_empty());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn list_users_standard_caller_returns_not_admin() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository| {
            let caller = user(1, "alice", None, Role::Standard)?;
            user_repository
                .expect_search()
                .times(1)
                .withf(|filter| filter.id == Some(1))
                .return_once(move |_| {
                    let found = vec![caller.clone()];
                    Box::pin(async move { Ok(found) })
                });
            Ok(())
        })?;

        // Act
        let result = harness
            .use_case
            .execute(ListUsersCommand::new(1, None, None, None))
            .await;

        // Assert
        assert!(matches!(result, Err(ListUsersError::NotAdmin)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn list_users_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            Ok(())
        })?;

        // Act
        let result = harness
            .use_case
            .execute(ListUsersCommand::new(999, None, None, None))
            .await;

        // Assert
        assert!(matches!(result, Err(ListUsersError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn list_users_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;

        // Act
        let result = harness
            .use_case
            .execute(ListUsersCommand::new(1, None, None, None))
            .await;

        // Assert
        assert!(matches!(result, Err(ListUsersError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
