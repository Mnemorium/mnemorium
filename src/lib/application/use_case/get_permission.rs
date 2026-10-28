use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::get_permission::GetPermissionCommand;
use crate::application::port::get_permission::GetPermissionError;
use crate::application::port::get_permission::GetPermissionResponse;
use crate::application::port::get_permission::GetPermissionUseCase;
use crate::application::security_event;
use crate::domain::model::permission::ACTIONS;
use crate::domain::model::permission::Action;
use crate::domain::model::permission::Global;
use crate::domain::model::permission::PermissionPreset;
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

/// Use case implementation for reading a user's effective permissions.
pub struct GetPermission<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> GetPermission<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> GetPermissionUseCase for GetPermission<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: GetPermissionCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<GetPermissionResponse, GetPermissionError>> + Send + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| GetPermissionError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| GetPermissionError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetPermissionError::NoSuchCaller)?;

                let caller_permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| GetPermissionError::Unknown(error.into()))?;

                if let Decision::Deny(denial) = Authorization::decide(
                    &caller,
                    &caller_permissions,
                    Action::PermissionManage,
                    &Global,
                ) {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(GetPermissionError::Forbidden);
                }

                let target = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.user_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| GetPermissionError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetPermissionError::NoSuchUser)?;

                if target.is_root_admin() {
                    security_event::authorization_failed(
                        caller.id(),
                        Action::PermissionManage.verb(),
                        Action::PermissionManage.target(),
                    );
                    return Err(GetPermissionError::Forbidden);
                }

                let overrides = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(target.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| GetPermissionError::Unknown(error.into()))?;

                let defaults = ACTIONS
                    .into_iter()
                    .filter(|action| action.is_configurable())
                    .map(|action| PermissionPreset::new(action, action.role_scope(target.role())))
                    .collect();

                Ok(GetPermissionResponse::new(
                    target.id(),
                    target.role(),
                    defaults,
                    overrides,
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| GetPermissionError::Unknown(error.into()))?;
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
    use std::sync::atomic::Ordering;

    use crate::application::port::get_permission::GetPermissionCommand;
    use crate::application::port::get_permission::GetPermissionError;
    use crate::application::port::get_permission::GetPermissionResponse;
    use crate::application::port::get_permission::GetPermissionUseCase as _;
    use crate::domain::alias::NumericID;
    use crate::domain::model::permission::Action;
    use crate::domain::model::permission::Permission;
    use crate::domain::model::permission::PermissionPreset;
    use crate::domain::model::permission::Scope;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::permission_repository::MockPermissionRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::GalleryFactoryHarness;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::TestUnitOfWork;

    use super::GetPermission;

    type UseCase = GetPermission<TestFactory>;

    fn user(id: NumericID, role: Role) -> Result<User, Box<dyn Error>> {
        Ok(User::try_new(
            id,
            format!("user{id}"),
            None,
            id.max(1),
            role,
        )?)
    }

    fn assert_default(response: &GetPermissionResponse, action: Action, scope: &Scope) {
        let found = response
            .defaults()
            .iter()
            .find(|candidate| candidate.action() == action);
        assert_eq!(found.map(PermissionPreset::scope), Some(scope));
    }

    fn assert_override(response: &GetPermissionResponse, action: Action, scope: &Scope) {
        let found = response
            .overrides()
            .iter()
            .find(|candidate| candidate.action() == action);
        assert_eq!(found.map(Permission::scope), Some(scope));
    }

    #[tokio::test]
    async fn get_permission_root_admin_reads_effective_defaults() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        let root = user(0, Role::Admin)?;
        let target = user(5, Role::Standard)?;
        users
            .expect_search()
            .times(1)
            .withf(|filter| filter.id == Some(0))
            .return_once(move |_| Box::pin(async move { Ok(vec![root]) }));
        users
            .expect_search()
            .times(1)
            .withf(|filter| filter.id == Some(5))
            .return_once(move |_| Box::pin(async move { Ok(vec![target]) }));
        let mut permissions = MockPermissionRepository::new();
        permissions
            .expect_search()
            .times(2)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let unit_of_work = TestUnitOfWork {
            permissions,
            users,
            ..TestUnitOfWork::default()
        };
        let harness = GalleryFactoryHarness::new(unit_of_work);
        let use_case: UseCase = GetPermission::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(GetPermissionCommand::new(0, 5)).await?;

        // Assert
        assert_eq!(response.user_id(), 5);
        assert_eq!(response.role(), Role::Standard);
        assert_eq!(response.defaults().len(), 15);
        assert!(response.overrides().is_empty());
        assert_default(&response, Action::GalleryRead, &Scope::PublicOrOwned);
        assert_default(&response, Action::UserCreate, &Scope::None);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_permission_reports_a_stored_override() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        let root = user(0, Role::Admin)?;
        let target = user(5, Role::Standard)?;
        users
            .expect_search()
            .times(1)
            .withf(|filter| filter.id == Some(0))
            .return_once(move |_| Box::pin(async move { Ok(vec![root]) }));
        users
            .expect_search()
            .times(1)
            .withf(|filter| filter.id == Some(5))
            .return_once(move |_| Box::pin(async move { Ok(vec![target]) }));
        let stored = Permission::try_new(5, Action::GalleryRead, Scope::All)?;
        let mut permissions = MockPermissionRepository::new();
        permissions
            .expect_search()
            .times(1)
            .withf(|filter| filter.user_id == Some(0))
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        permissions
            .expect_search()
            .times(1)
            .withf(|filter| filter.user_id == Some(5))
            .return_once(move |_| Box::pin(async move { Ok(vec![stored]) }));
        let unit_of_work = TestUnitOfWork {
            permissions,
            users,
            ..TestUnitOfWork::default()
        };
        let harness = GalleryFactoryHarness::new(unit_of_work);
        let use_case: UseCase = GetPermission::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(GetPermissionCommand::new(0, 5)).await?;

        // Assert
        assert_default(&response, Action::GalleryRead, &Scope::PublicOrOwned);
        assert_override(&response, Action::GalleryRead, &Scope::All);
        Ok(())
    }

    #[tokio::test]
    async fn get_permission_admin_caller_returns_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        let admin = user(1, Role::Admin)?;
        users
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![admin]) }));
        let mut permissions = MockPermissionRepository::new();
        permissions
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let unit_of_work = TestUnitOfWork {
            permissions,
            users,
            ..TestUnitOfWork::default()
        };
        let harness = GalleryFactoryHarness::new(unit_of_work);
        let use_case: UseCase = GetPermission::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetPermissionCommand::new(1, 5)).await;

        // Assert
        assert!(matches!(result, Err(GetPermissionError::Forbidden)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_permission_root_target_returns_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        let root = user(0, Role::Admin)?;
        users.expect_search().times(2).returning(move |_| {
            let found = root.clone();
            Box::pin(async move { Ok(vec![found]) })
        });
        let mut permissions = MockPermissionRepository::new();
        permissions
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let unit_of_work = TestUnitOfWork {
            permissions,
            users,
            ..TestUnitOfWork::default()
        };
        let harness = GalleryFactoryHarness::new(unit_of_work);
        let use_case: UseCase = GetPermission::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetPermissionCommand::new(0, 0)).await;

        // Assert
        assert!(matches!(result, Err(GetPermissionError::Forbidden)));
        Ok(())
    }

    #[tokio::test]
    async fn get_permission_permission_search_failure_returns_unknown() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        let root = user(0, Role::Admin)?;
        users
            .expect_search()
            .withf(|filter| filter.id == Some(0))
            .return_once(move |_| Box::pin(async move { Ok(vec![root]) }));
        let mut permissions = MockPermissionRepository::new();
        permissions
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let unit_of_work = TestUnitOfWork {
            permissions,
            users,
            ..TestUnitOfWork::default()
        };
        let harness = GalleryFactoryHarness::new(unit_of_work);
        let use_case: UseCase = GetPermission::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetPermissionCommand::new(0, 5)).await;

        // Assert
        assert!(matches!(result, Err(GetPermissionError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_permission_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let unit_of_work = TestUnitOfWork {
            users,
            ..TestUnitOfWork::default()
        };
        let harness = GalleryFactoryHarness::new(unit_of_work);
        let use_case: UseCase = GetPermission::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetPermissionCommand::new(99, 5)).await;

        // Assert
        assert!(matches!(result, Err(GetPermissionError::NoSuchCaller)));
        Ok(())
    }

    #[tokio::test]
    async fn get_permission_unknown_target_returns_no_such_user() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        let root = user(0, Role::Admin)?;
        users
            .expect_search()
            .times(1)
            .withf(|filter| filter.id == Some(0))
            .return_once(move |_| Box::pin(async move { Ok(vec![root]) }));
        users
            .expect_search()
            .times(1)
            .withf(|filter| filter.id == Some(5))
            .return_once(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut permissions = MockPermissionRepository::new();
        permissions
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let unit_of_work = TestUnitOfWork {
            permissions,
            users,
            ..TestUnitOfWork::default()
        };
        let harness = GalleryFactoryHarness::new(unit_of_work);
        let use_case: UseCase = GetPermission::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetPermissionCommand::new(0, 5)).await;

        // Assert
        assert!(matches!(result, Err(GetPermissionError::NoSuchUser)));
        Ok(())
    }
}
