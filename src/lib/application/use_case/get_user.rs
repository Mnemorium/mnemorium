use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::get_user::GetUserCommand;
use crate::application::port::get_user::GetUserError;
use crate::application::port::get_user::GetUserResponse;
use crate::application::port::get_user::GetUserUseCase;
use crate::application::security_event;
use crate::domain::model::permission::Action;
use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
use crate::domain::port::permission_repository::PermissionFilter;
use crate::domain::port::permission_repository::PermissionRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::service::authorization::Authorization;
use crate::domain::service::authorization::Decision;

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
    F::Uow: IdentityUnitOfWork + UserUnitOfWork,
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

                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| GetUserError::Unknown(error.into()))?;
                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::UserRead, &user)
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(GetUserError::NotAdmin);
                }

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
                    security_event::sensitive_data_access(command.caller_id(), "user");
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
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;

    use crate::application::port::get_user::GetUserCommand;
    use crate::application::port::get_user::GetUserError;
    use crate::application::port::get_user::GetUserUseCase as _;
    use crate::domain::model::permission::Action;
    use crate::domain::model::permission::Permission;
    use crate::domain::model::permission::Scope;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::model::user::UserError;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::permission_repository::MockPermissionRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::TestUnitOfWork;
    use crate::test_helpers::expect_permissions;
    use crate::test_helpers::no_permissions;

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

        let unit_of_work = TestUnitOfWork {
            permissions: no_permissions(),
            users: user_repository,
            ..TestUnitOfWork::default()
        };
        let committed = Arc::clone(&unit_of_work.committed);
        let rolled_back = Arc::clone(&unit_of_work.rolled_back);
        let factory = TestFactory::new(unit_of_work);

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
                .withf(|filter| filter.id == Some(1))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            let target = user(2, "brad", None, Role::Standard)?;
            user_repository
                .expect_search()
                .withf(|filter| filter.id == Some(2))
                .return_once(move |_| Box::pin(async move { Ok(vec![target]) }));
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
    async fn get_user_standard_caller_granted_read_returns_profile() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut user_repository = MockUserRepository::new();
        let caller = user(1, "alice", None, Role::Standard)?;
        user_repository
            .expect_search()
            .withf(|filter| filter.id == Some(1))
            .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
        let target = user(2, "brad", None, Role::Standard)?;
        user_repository
            .expect_search()
            .withf(|filter| filter.id == Some(2))
            .return_once(move |_| Box::pin(async move { Ok(vec![target]) }));
        let mut permissions = MockPermissionRepository::new();
        expect_permissions(
            &mut permissions,
            1,
            vec![Permission::try_new(1, Action::UserRead, Scope::All)?],
        );
        let unit_of_work = TestUnitOfWork {
            permissions,
            users: user_repository,
            ..TestUnitOfWork::default()
        };
        let factory = TestFactory::new(unit_of_work);
        let use_case: UseCase = GetUser::new(Arc::new(factory));
        let command = GetUserCommand::new(1, 2);

        // Act
        let response = use_case.execute(command).await?;

        // Assert
        assert_eq!(response.id(), 2);
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
                .times(1)
                .withf(|filter| filter.id == Some(1))
                .return_once(move |_| {
                    let found = admin;
                    Box::pin(async move { Ok(vec![found]) })
                });
            user_repository
                .expect_search()
                .times(1)
                .withf(|filter| filter.id == Some(2))
                .return_once(|_| Box::pin(async { Ok(Vec::new()) }));
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

    #[tokio::test]
    async fn user_debug_redacts_personal_data_but_keeps_id_and_role() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the domain model has no test module of its own (`TEST-019`),
        // so its redaction contract is covered here, as `load_configuration` does
        // for `Jwt`, `Security` and `Configuration`.
        let user = User::try_new(
            7,
            "alice".to_owned(),
            Some("alice@example.com".to_owned()),
            9,
            Role::Standard,
        )?;

        // Act
        let debug = format!("{user:?}");

        // Assert
        assert!(
            !debug.contains("alice@example.com"),
            "Debug output must not contain the email address, got: {debug}"
        );
        assert!(
            !debug.contains("alice"),
            "Debug output must not contain the username, got: {debug}"
        );
        assert!(
            debug.contains("id: 7"),
            "Debug output must still contain the user identifier, got: {debug}"
        );
        assert!(
            debug.contains("role: Standard"),
            "Debug output must still contain the role, got: {debug}"
        );
        Ok(())
    }
}
