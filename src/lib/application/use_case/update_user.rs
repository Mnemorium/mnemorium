use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tracing::error;

use crate::application::port::update_user::UpdateUserCommand;
use crate::application::port::update_user::UpdateUserError;
use crate::application::port::update_user::UpdateUserResponse;
use crate::application::port::update_user::UpdateUserUseCase;
use crate::domain::model::user::Role;
use crate::domain::model::user::UserError;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;

/// Use case implementation for updating the profile of a user.
pub struct UpdateUser<F> {
    /// Factory opening the unit of work wrapping the update.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> UpdateUser<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> UpdateUserUseCase for UpdateUser<F>
where
    F: UnitOfWorkFactory,
    F::Uow: UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: UpdateUserCommand,
    ) -> Pin<Box<dyn Future<Output = Result<UpdateUserResponse, UpdateUserError>> + Send + 'future>>
    {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| UpdateUserError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| UpdateUserError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(UpdateUserError::NoSuchCaller)?;
                if caller.role() != Role::Admin {
                    return Err(UpdateUserError::NotAdmin);
                }

                let mut user = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.user_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| UpdateUserError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(UpdateUserError::NoSuchUser)?;

                if user.id() == 0 {
                    return Err(UpdateUserError::TargetNotModifiable);
                }
                if user.role() == Role::Admin && caller.id() != 0 && caller.id() != user.id() {
                    return Err(UpdateUserError::TargetNotModifiable);
                }
                if command.role().is_some() && caller.id() != 0 {
                    return Err(UpdateUserError::RoleChangeForbidden);
                }

                if let Some(username) = command.username().map(str::to_owned) {
                    user.set_username(username).map_err(|error| match error {
                        UserError::UsernameTooShort => UpdateUserError::InvalidUsername,
                        UserError::InvalidEmail(_) | UserError::Unknown(_) => {
                            UpdateUserError::Unknown(anyhow::Error::new(error))
                        }
                    })?;
                }
                if let Some(email) = command.email().map(str::to_owned) {
                    user.set_email(Some(email)).map_err(|error| match error {
                        UserError::InvalidEmail(_) => UpdateUserError::InvalidEmail,
                        UserError::UsernameTooShort | UserError::Unknown(_) => {
                            UpdateUserError::Unknown(anyhow::Error::new(error))
                        }
                    })?;
                }
                if let Some(role) = command.role() {
                    user.set_role(role);
                }

                let updated_user = unit_of_work
                    .users()
                    .save(user)
                    .await
                    .map_err(|error| UpdateUserError::Unknown(error.into()))?;

                Ok(UpdateUserResponse::new(
                    updated_user.id(),
                    updated_user.username().to_owned(),
                    updated_user.email().map(str::to_owned),
                    updated_user.role(),
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| UpdateUserError::Unknown(error.into()))?;
                    Ok(value)
                }
                Err(error) => {
                    if let Err(rollback_error) = unit_of_work.rollback().await {
                        error!(
                            error = ?rollback_error,
                            "failed to roll back the update user unit of work"
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

    use crate::application::port::update_user::UpdateUserCommand;
    use crate::application::port::update_user::UpdateUserError;
    use crate::application::port::update_user::UpdateUserUseCase as _;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::model::user::UserError;
    use crate::domain::port::configuration_repository::MockConfigurationRepository;
    use crate::domain::port::credential_repository::MockCredentialRepository;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::unit_of_work_factory;

    use super::UpdateUser;

    type UseCase = UpdateUser<TestFactory>;

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
            use_case: UpdateUser::new(Arc::new(factory)),
            committed,
            rolled_back,
        })
    }

    fn user(id: i64, username: &str, email: Option<&str>, role: Role) -> Result<User, UserError> {
        User::try_new(id, username.to_owned(), email.map(str::to_owned), id, role)
    }

    fn command(
        caller_id: i64,
        user_id: i64,
        username: Option<&str>,
        email: Option<&str>,
        role: Option<Role>,
    ) -> UpdateUserCommand {
        UpdateUserCommand::new(
            caller_id,
            user_id,
            username.map(str::to_owned),
            email.map(str::to_owned),
            role,
        )
    }

    /// Expect two searches: caller resolved first, then the requested user.
    fn expect_caller_and_user(
        user_repository: &mut MockUserRepository,
        caller: User,
        target: Result<User, UserError>,
    ) -> Result<(), Box<dyn Error>> {
        let fetched = target?;
        let caller_id = caller.id();
        let fetched_id = fetched.id();
        user_repository
            .expect_search()
            .times(2)
            .withf(move |filter| filter.id == Some(caller_id) || filter.id == Some(fetched_id))
            .returning(move |filter| {
                if filter.id == Some(fetched.id()) {
                    let found = fetched.clone();
                    Box::pin(async move { Ok(vec![found]) })
                } else {
                    let found = caller.clone();
                    Box::pin(async move { Ok(vec![found]) })
                }
            });
        Ok(())
    }

    /// Build a use case whose `save` succeeds and whose searches resolve
    /// `caller` first and `target` second.
    fn use_case_with_save(
        setup: impl FnOnce(&mut MockUserRepository) -> Result<(), Box<dyn Error>>,
    ) -> Result<Harness, Box<dyn Error>> {
        let mut user_repository = MockUserRepository::new();
        setup(&mut user_repository)?;
        user_repository
            .expect_save()
            .times(1)
            .returning(|user| Box::pin(async move { Ok(user) }));

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
            use_case: UpdateUser::new(Arc::new(factory)),
            committed,
            rolled_back,
        })
    }

    #[tokio::test]
    async fn update_user_admin_updates_own_username_and_email_succeeds()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with_save(|user_repository| {
            expect_caller_and_user(
                user_repository,
                user(7, "admin", None, Role::Admin)?,
                user(7, "admin", Some("old@example.com"), Role::Admin),
            )
        })?;
        let command = command(7, 7, Some("renamed-admin"), Some("new@example.com"), None);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.username(), "renamed-admin");
        assert_eq!(response.email(), Some("new@example.com"));
        assert_eq!(response.role(), Role::Admin);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_user_root_changes_role_other_admin_succeeds() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with_save(|user_repository| {
            expect_caller_and_user(
                user_repository,
                user(0, "root", None, Role::Admin)?,
                user(8, "fellow-admin", None, Role::Admin),
            )
        })?;
        let command = command(0, 8, None, None, Some(Role::Standard));

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.role(), Role::Standard);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_user_standard_caller_returns_not_admin() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository| {
            let caller = user(3, "bobby", None, Role::Standard)?;
            user_repository
                .expect_search()
                .times(1)
                .withf(|filter| filter.id == Some(3))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })?;
        let command = command(3, 2, Some("renamed"), None, None);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(UpdateUserError::NotAdmin)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_user_target_root_admin_returns_target_not_modifiable()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository| {
            expect_caller_and_user(
                user_repository,
                user(0, "root", None, Role::Admin)?,
                user(0, "root", None, Role::Admin),
            )
        })?;
        let command = command(0, 0, Some("renamed-root"), None, None);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(UpdateUserError::TargetNotModifiable)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_user_non_root_admin_updates_other_admin_returns_target_not_modifiable()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository| {
            expect_caller_and_user(
                user_repository,
                user(7, "admin", None, Role::Admin)?,
                user(8, "fellow-admin", None, Role::Admin),
            )
        })?;
        let command = command(7, 8, Some("renamed-fellow"), None, None);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(UpdateUserError::TargetNotModifiable)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_user_non_root_admin_changes_role_returns_role_change_forbidden()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository| {
            expect_caller_and_user(
                user_repository,
                user(7, "admin", None, Role::Admin)?,
                user(2, "brad", None, Role::Standard),
            )
        })?;
        let command = command(7, 2, None, None, Some(Role::Standard));

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(UpdateUserError::RoleChangeForbidden)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_user_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            Ok(())
        })?;
        let command = command(999, 2, None, None, None);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(UpdateUserError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_user_unknown_target_returns_no_such_user() -> Result<(), Box<dyn Error>> {
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
        let command = command(1, 2, Some("renamed"), None, None);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(UpdateUserError::NoSuchUser)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_user_short_username_returns_invalid_username() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository| {
            expect_caller_and_user(
                user_repository,
                user(0, "root", None, Role::Admin)?,
                user(2, "brad", None, Role::Standard),
            )
        })?;
        let command = command(0, 2, Some("ap"), None, None);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(UpdateUserError::InvalidUsername)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_user_invalid_email_returns_invalid_email() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository| {
            expect_caller_and_user(
                user_repository,
                user(0, "root", None, Role::Admin)?,
                user(2, "brad", None, Role::Standard),
            )
        })?;
        let command = command(0, 2, None, Some("not-an-email"), None);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(UpdateUserError::InvalidEmail)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_user_search_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|user_repository| {
            user_repository
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;
        let command = command(1, 2, None, None, None);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(UpdateUserError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_user_save_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut user_repository = MockUserRepository::new();
        expect_caller_and_user(
            &mut user_repository,
            user(0, "root", None, Role::Admin)?,
            user(2, "brad", None, Role::Standard),
        )?;
        user_repository
            .expect_save()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));

        let committed = Arc::new(AtomicBool::new(false));
        let rolled_back = Arc::new(AtomicBool::new(false));
        let factory = unit_of_work_factory(
            user_repository,
            MockCredentialRepository::new(),
            MockConfigurationRepository::new(),
            Arc::clone(&committed),
            Arc::clone(&rolled_back),
        );
        let use_case: UseCase = UpdateUser::new(Arc::new(factory));
        let command = command(0, 2, Some("renamed"), None, None);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(UpdateUserError::Unknown(_))));
        assert!(rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
