use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::get_permission::GetPermissionResponse;
use crate::application::port::replace_permission::ReplacePermissionCommand;
use crate::application::port::replace_permission::ReplacePermissionError;
use crate::application::port::replace_permission::ReplacePermissionUseCase;
use crate::application::security_event;
use crate::domain::model::permission::ACTIONS;
use crate::domain::model::permission::Action;
use crate::domain::model::permission::Global;
use crate::domain::model::permission::Permission;
use crate::domain::model::permission::PermissionError;
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

/// Use case implementation for replacing a user's permission overrides.
pub struct ReplacePermission<F> {
    /// Factory opening the unit of work wrapping the replacement.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> ReplacePermission<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> ReplacePermissionUseCase for ReplacePermission<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: ReplacePermissionCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<GetPermissionResponse, ReplacePermissionError>>
                + Send
                + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| ReplacePermissionError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| ReplacePermissionError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(ReplacePermissionError::NoSuchCaller)?;

                let caller_permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| ReplacePermissionError::Unknown(error.into()))?;

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
                    return Err(ReplacePermissionError::Forbidden);
                }

                let target = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.user_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| ReplacePermissionError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(ReplacePermissionError::NoSuchUser)?;

                if target.is_root_admin() {
                    security_event::authorization_failed(
                        caller.id(),
                        Action::PermissionManage.verb(),
                        Action::PermissionManage.target(),
                    );
                    return Err(ReplacePermissionError::Forbidden);
                }

                let mut permissions = Vec::with_capacity(command.permissions().len());
                for request in command.permissions() {
                    permissions.push(
                        Permission::try_new(target.id(), request.action(), request.scope().clone())
                            .map_err(map_permission_error)?,
                    );
                }
                Permission::validate_set(&permissions).map_err(map_permission_error)?;

                unit_of_work
                    .permissions()
                    .delete_all(target.id())
                    .await
                    .map_err(|error| ReplacePermissionError::Unknown(error.into()))?;
                for permission in permissions {
                    unit_of_work
                        .permissions()
                        .save(permission)
                        .await
                        .map_err(|error| ReplacePermissionError::Unknown(error.into()))?;
                }

                let stored = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(target.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| ReplacePermissionError::Unknown(error.into()))?;

                let defaults = ACTIONS
                    .into_iter()
                    .filter(|action| action.is_configurable())
                    .map(|action| PermissionPreset::new(action, action.role_scope(target.role())))
                    .collect();

                Ok(GetPermissionResponse::new(
                    target.id(),
                    target.role(),
                    defaults,
                    stored,
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| ReplacePermissionError::Unknown(error.into()))?;
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

/// Map a permission validation failure onto the replacement error.
#[expect(
    clippy::needless_pass_by_value,
    reason = "the validation error is mapped by value like every other error in the use case"
)]
fn map_permission_error(error: PermissionError) -> ReplacePermissionError {
    match error {
        PermissionError::DuplicateAction => ReplacePermissionError::DuplicateAction,
        PermissionError::DuplicateResource
        | PermissionError::EmptyListed
        | PermissionError::NotConfigurable
        | PermissionError::UnsupportedResourceKind
        | PermissionError::UnsupportedScope
        | PermissionError::Unknown(_) => ReplacePermissionError::InvalidPermission,
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use crate::application::port::replace_permission::PermissionRequest;
    use crate::application::port::replace_permission::ReplacePermissionCommand;
    use crate::application::port::replace_permission::ReplacePermissionError;
    use crate::application::port::replace_permission::ReplacePermissionUseCase as _;
    use crate::domain::alias::NumericID;
    use crate::domain::model::permission::Action;
    use crate::domain::model::permission::Permission;
    use crate::domain::model::permission::ResourceKind;
    use crate::domain::model::permission::ResourceRef;
    use crate::domain::model::permission::Scope;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::permission_repository::MockPermissionRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::GalleryFactoryHarness;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::TestUnitOfWork;

    use super::ReplacePermission;

    type UseCase = ReplacePermission<TestFactory>;

    fn user(id: NumericID, role: Role) -> Result<User, Box<dyn Error>> {
        Ok(User::try_new(
            id,
            format!("user{id}"),
            None,
            id.max(1),
            role,
        )?)
    }

    /// Build a harness whose caller is the Root Admin and whose target is
    /// user `5`; `permissions` is the repository mock the test configures.
    fn harness(
        permissions: MockPermissionRepository,
    ) -> Result<GalleryFactoryHarness, Box<dyn Error>> {
        let mut users = MockUserRepository::new();
        let root = user(0, Role::Admin)?;
        let target = user(5, Role::Standard)?;
        users.expect_search().times(2).returning(move |filter| {
            let found = if filter.id == Some(0) {
                root.clone()
            } else {
                target.clone()
            };
            Box::pin(async move { Ok(vec![found]) })
        });
        let unit_of_work = TestUnitOfWork {
            permissions,
            users,
            ..TestUnitOfWork::default()
        };
        Ok(GalleryFactoryHarness::new(unit_of_work))
    }

    fn caller_permission_load(permissions: &mut MockPermissionRepository) {
        permissions
            .expect_search()
            .withf(|filter| filter.user_id == Some(0))
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
    }

    #[tokio::test]
    async fn replace_permission_root_admin_replaces_the_set() -> Result<(), Box<dyn Error>> {
        // Arrange
        let stored = vec![
            Permission::try_new(5, Action::GalleryRead, Scope::All)?,
            Permission::try_new(
                5,
                Action::PlaylistRead,
                Scope::Listed(vec![ResourceRef::new(ResourceKind::MusicPlaylist, 3)]),
            )?,
        ];
        let mut permissions = MockPermissionRepository::new();
        caller_permission_load(&mut permissions);
        permissions
            .expect_delete_all()
            .times(1)
            .withf(|user_id| *user_id == 5)
            .returning(|_| Box::pin(async { Ok(false) }));
        permissions
            .expect_save()
            .times(2)
            .returning(|permission| Box::pin(async move { Ok(permission) }));
        let returned = stored.clone();
        permissions
            .expect_search()
            .times(1)
            .withf(|filter| filter.user_id == Some(5))
            .return_once(move |_| {
                let found = returned.clone();
                Box::pin(async move { Ok(found) })
            });
        let harness = harness(permissions)?;
        let use_case: UseCase = ReplacePermission::new(Arc::clone(&harness.factory));
        let command = ReplacePermissionCommand::new(
            0,
            5,
            vec![
                PermissionRequest::new(Action::GalleryRead, Scope::All),
                PermissionRequest::new(
                    Action::PlaylistRead,
                    Scope::Listed(vec![ResourceRef::new(ResourceKind::MusicPlaylist, 3)]),
                ),
            ],
        );

        // Act
        let response = use_case.execute(command).await?;

        // Assert
        assert_eq!(response.defaults().len(), 15);
        assert!(response.overrides().iter().any(|permission| {
            permission.action() == Action::GalleryRead && permission.scope() == &Scope::All
        }));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn replace_permission_empty_set_resets_the_overrides() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut permissions = MockPermissionRepository::new();
        caller_permission_load(&mut permissions);
        permissions
            .expect_delete_all()
            .times(1)
            .withf(|user_id| *user_id == 5)
            .returning(|_| Box::pin(async { Ok(true) }));
        permissions
            .expect_search()
            .times(1)
            .withf(|filter| filter.user_id == Some(5))
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness(permissions)?;
        let use_case: UseCase = ReplacePermission::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(ReplacePermissionCommand::new(0, 5, Vec::new()))
            .await?;

        // Assert
        assert!(response.overrides().is_empty());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn replace_permission_unsupported_scope_stores_nothing() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut permissions = MockPermissionRepository::new();
        caller_permission_load(&mut permissions);
        let harness = harness(permissions)?;
        let use_case: UseCase = ReplacePermission::new(Arc::clone(&harness.factory));
        let command = ReplacePermissionCommand::new(
            0,
            5,
            vec![PermissionRequest::new(Action::UserCreate, Scope::Owned)],
        );

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(
            result,
            Err(ReplacePermissionError::InvalidPermission)
        ));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn replace_permission_empty_listed_stores_nothing() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut permissions = MockPermissionRepository::new();
        caller_permission_load(&mut permissions);
        let harness = harness(permissions)?;
        let use_case: UseCase = ReplacePermission::new(Arc::clone(&harness.factory));
        let command = ReplacePermissionCommand::new(
            0,
            5,
            vec![PermissionRequest::new(
                Action::GalleryRead,
                Scope::Listed(Vec::new()),
            )],
        );

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(
            result,
            Err(ReplacePermissionError::InvalidPermission)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn replace_permission_duplicate_listed_resource_stores_nothing()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut permissions = MockPermissionRepository::new();
        caller_permission_load(&mut permissions);
        let harness = harness(permissions)?;
        let use_case: UseCase = ReplacePermission::new(Arc::clone(&harness.factory));
        let reference = ResourceRef::new(ResourceKind::Gallery, 2);
        let command = ReplacePermissionCommand::new(
            0,
            5,
            vec![PermissionRequest::new(
                Action::GalleryRead,
                Scope::Listed(vec![reference, reference]),
            )],
        );

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(
            result,
            Err(ReplacePermissionError::InvalidPermission)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn replace_permission_mismatched_resource_kind_stores_nothing()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut permissions = MockPermissionRepository::new();
        caller_permission_load(&mut permissions);
        let harness = harness(permissions)?;
        let use_case: UseCase = ReplacePermission::new(Arc::clone(&harness.factory));
        let command = ReplacePermissionCommand::new(
            0,
            5,
            vec![PermissionRequest::new(
                Action::GalleryRead,
                Scope::Listed(vec![ResourceRef::new(ResourceKind::User, 2)]),
            )],
        );

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(
            result,
            Err(ReplacePermissionError::InvalidPermission)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn replace_permission_non_configurable_action_stores_nothing()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut permissions = MockPermissionRepository::new();
        caller_permission_load(&mut permissions);
        let harness = harness(permissions)?;
        let use_case: UseCase = ReplacePermission::new(Arc::clone(&harness.factory));
        let command = ReplacePermissionCommand::new(
            0,
            5,
            vec![PermissionRequest::new(Action::UploadRead, Scope::Owned)],
        );

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(
            result,
            Err(ReplacePermissionError::InvalidPermission)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn replace_permission_duplicate_action_stores_nothing() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut permissions = MockPermissionRepository::new();
        caller_permission_load(&mut permissions);
        let harness = harness(permissions)?;
        let use_case: UseCase = ReplacePermission::new(Arc::clone(&harness.factory));
        let command = ReplacePermissionCommand::new(
            0,
            5,
            vec![
                PermissionRequest::new(Action::GalleryRead, Scope::All),
                PermissionRequest::new(Action::GalleryRead, Scope::None),
            ],
        );

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(
            result,
            Err(ReplacePermissionError::DuplicateAction)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn replace_permission_search_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
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
        let use_case: UseCase = ReplacePermission::new(Arc::clone(&harness.factory));
        let command = ReplacePermissionCommand::new(
            0,
            5,
            vec![PermissionRequest::new(Action::GalleryRead, Scope::All)],
        );

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(ReplacePermissionError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn replace_permission_admin_caller_returns_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        let admin = user(1, Role::Admin)?;
        users
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![admin]) }));
        let target = user(5, Role::Standard)?;
        users.expect_search().returning(move |_| {
            let found = target.clone();
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
        let use_case: UseCase = ReplacePermission::new(Arc::clone(&harness.factory));
        let command = ReplacePermissionCommand::new(
            1,
            5,
            vec![PermissionRequest::new(Action::GalleryRead, Scope::All)],
        );

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(ReplacePermissionError::Forbidden)));
        Ok(())
    }

    #[tokio::test]
    async fn replace_permission_root_target_returns_forbidden() -> Result<(), Box<dyn Error>> {
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
        permissions.expect_delete_all().times(0);
        permissions.expect_save().times(0);
        let unit_of_work = TestUnitOfWork {
            permissions,
            users,
            ..TestUnitOfWork::default()
        };
        let harness = GalleryFactoryHarness::new(unit_of_work);
        let use_case: UseCase = ReplacePermission::new(Arc::clone(&harness.factory));
        let command = ReplacePermissionCommand::new(
            0,
            0,
            vec![PermissionRequest::new(Action::GalleryRead, Scope::All)],
        );

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(ReplacePermissionError::Forbidden)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn replace_permission_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>>
    {
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
        let use_case: UseCase = ReplacePermission::new(Arc::clone(&harness.factory));
        let command = ReplacePermissionCommand::new(
            99,
            5,
            vec![PermissionRequest::new(Action::GalleryRead, Scope::All)],
        );

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(ReplacePermissionError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn replace_permission_unknown_target_returns_no_such_user() -> Result<(), Box<dyn Error>>
    {
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
        let use_case: UseCase = ReplacePermission::new(Arc::clone(&harness.factory));
        let command = ReplacePermissionCommand::new(
            0,
            5,
            vec![PermissionRequest::new(Action::GalleryRead, Scope::All)],
        );

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(ReplacePermissionError::NoSuchUser)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
