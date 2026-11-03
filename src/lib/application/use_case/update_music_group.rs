use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::update_music_group::UpdateMusicGroupCommand;
use crate::application::port::update_music_group::UpdateMusicGroupError;
use crate::application::port::update_music_group::UpdateMusicGroupResponse;
use crate::application::port::update_music_group::UpdateMusicGroupUseCase;
use crate::application::security_event;
use crate::domain::model::music_group::MusicGroup;
use crate::domain::model::permission::Action;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::music_group_repository::MusicGroupFilter;
use crate::domain::port::music_group_repository::MusicGroupRepository as _;
use crate::domain::port::permission_repository::PermissionFilter;
use crate::domain::port::permission_repository::PermissionRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::service::authorization::Authorization;
use crate::domain::service::authorization::Decision;
use crate::domain::service::name_policy::NamePolicy;
use crate::domain::service::name_policy::NamePolicyError;

/// Use case implementation for updating a music group.
pub struct UpdateMusicGroup<F> {
    /// Factory opening the unit of work wrapping the update.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> UpdateMusicGroup<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> UpdateMusicGroupUseCase for UpdateMusicGroup<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "RepositoryError is non_exhaustive, so a catch-all arm is required"
    )]
    fn execute<'future>(
        &'future self,
        command: UpdateMusicGroupCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<UpdateMusicGroupResponse, UpdateMusicGroupError>>
                + Send
                + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| UpdateMusicGroupError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| UpdateMusicGroupError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(UpdateMusicGroupError::NoSuchCaller)?;

                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| UpdateMusicGroupError::Unknown(error.into()))?;

                let group = unit_of_work
                    .music_groups()
                    .get(command.group_id())
                    .await
                    .map_err(|error| UpdateMusicGroupError::Unknown(error.into()))?
                    .ok_or(UpdateMusicGroupError::NoSuchGroup)?;

                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::LibraryRead, &group)
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(UpdateMusicGroupError::NoSuchGroup);
                }

                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::LibraryManage, &group)
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(if denial.conceals() {
                        UpdateMusicGroupError::NoSuchGroup
                    } else {
                        UpdateMusicGroupError::Forbidden
                    });
                }

                let name = match command.name() {
                    Some(requested) => {
                        let name =
                            NamePolicy::validate(requested.to_owned()).map_err(
                                |error| match error {
                                    NamePolicyError::Empty => {
                                        security_event::input_validation_failed("name", "empty");
                                        UpdateMusicGroupError::InvalidName
                                    }
                                    NamePolicyError::TooLong => {
                                        security_event::input_validation_failed("name", "too_long");
                                        UpdateMusicGroupError::NameTooLong
                                    }
                                },
                            )?;
                        let collisions = unit_of_work
                            .music_groups()
                            .search(&MusicGroupFilter {
                                stage_name: Some(name.clone()),
                                ..MusicGroupFilter::default()
                            })
                            .await
                            .map_err(|error| UpdateMusicGroupError::Unknown(error.into()))?;
                        if collisions
                            .iter()
                            .any(|collision| collision.id() != group.id())
                        {
                            return Err(UpdateMusicGroupError::NameAlreadyExists);
                        }
                        name
                    }
                    None => group.stage_name().to_owned(),
                };
                let cover_image_id = command.cover_image_id().or(group.cover_image_id());
                let updated = MusicGroup::try_new(group.id(), name, cover_image_id)
                    .map_err(|_| UpdateMusicGroupError::InvalidName)?;

                let stored = unit_of_work
                    .music_groups()
                    .update(updated)
                    .await
                    .map_err(|error| match error {
                        RepositoryError::Conflict => UpdateMusicGroupError::Conflict,
                        other => UpdateMusicGroupError::Unknown(other.into()),
                    })?
                    .ok_or(UpdateMusicGroupError::NoSuchGroup)?;

                Ok(UpdateMusicGroupResponse::new(stored))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| UpdateMusicGroupError::Unknown(error.into()))?;
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

    use crate::application::port::update_music_group::UpdateMusicGroupCommand;
    use crate::application::port::update_music_group::UpdateMusicGroupError;
    use crate::application::port::update_music_group::UpdateMusicGroupUseCase as _;
    use crate::domain::alias::NumericID;
    use crate::domain::model::music_group::MusicGroup;
    use crate::domain::model::permission::Action;
    use crate::domain::model::permission::Permission;
    use crate::domain::model::permission::Scope;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::music_group_repository::MockMusicGroupRepository;
    use crate::domain::port::permission_repository::MockPermissionRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::GalleryFactoryHarness;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::TestUnitOfWork;
    use crate::test_helpers::expect_caller;
    use crate::test_helpers::expect_permissions;
    use crate::test_helpers::no_permissions;

    use super::UpdateMusicGroup;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = UpdateMusicGroup<TestFactory>;

    /// Build a group named `name` with an optional cover.
    fn group(
        id: NumericID,
        name: &str,
        cover_image_id: Option<NumericID>,
    ) -> Result<MusicGroup, Box<dyn Error>> {
        Ok(MusicGroup::try_new(id, name.to_owned(), cover_image_id)?)
    }

    /// Expect the caller load to find an administrator.
    fn expect_admin(
        users: &mut MockUserRepository,
        caller_id: NumericID,
    ) -> Result<(), Box<dyn Error>> {
        let caller = User::try_new(caller_id, "admin".to_owned(), None, caller_id, Role::Admin)?;
        users
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(caller_id))
            .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
        Ok(())
    }

    /// Build a Library test harness around the given mocks.
    fn harness_with(
        users: MockUserRepository,
        groups: MockMusicGroupRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            music_groups: groups,
            permissions: no_permissions(),
            users,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
    }

    #[tokio::test]
    async fn update_music_group_renames_the_group() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        let stored = group(7, "The Group", Some(19))?;
        groups
            .expect_get()
            .times(1)
            .withf(|group_id| *group_id == 7)
            .return_once(move |_| {
                let found = stored.clone();
                Box::pin(async move { Ok(Some(found)) })
            });
        groups
            .expect_search()
            .times(1)
            .withf(|filter| filter.stage_name.as_deref() == Some("The Renamed"))
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let renamed = group(7, "The Renamed", Some(19))?;
        groups
            .expect_update()
            .times(1)
            .withf(|group| {
                group.id() == 7
                    && group.stage_name() == "The Renamed"
                    && group.cover_image_id() == Some(19)
            })
            .return_once(move |_| {
                let saved = renamed.clone();
                Box::pin(async move { Ok(Some(saved)) })
            });
        let harness = harness_with(users, groups);
        let use_case: UseCase = UpdateMusicGroup::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(UpdateMusicGroupCommand::new(
                1,
                7,
                Some("  The Renamed  ".to_owned()),
                None,
            ))
            .await?;

        // Assert
        assert_eq!(response.group().stage_name(), "The Renamed");
        assert_eq!(
            response.group().cover_image_id(),
            Some(19),
            "an update that names no cover keeps the stored artwork"
        );
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_music_group_attaches_a_cover() -> Result<(), Box<dyn Error>> {
        // Arrange: a group without artwork.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        let stored = group(7, "The Group", None)?;
        groups.expect_get().times(1).return_once(move |_| {
            let found = stored.clone();
            Box::pin(async move { Ok(Some(found)) })
        });
        let covered = group(7, "The Group", Some(19))?;
        groups
            .expect_update()
            .times(1)
            .withf(|group| group.stage_name() == "The Group" && group.cover_image_id() == Some(19))
            .return_once(move |_| {
                let saved = covered.clone();
                Box::pin(async move { Ok(Some(saved)) })
            });
        let harness = harness_with(users, groups);
        let use_case: UseCase = UpdateMusicGroup::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(UpdateMusicGroupCommand::new(1, 7, None, Some(19)))
            .await?;

        // Assert
        assert_eq!(response.group().cover_image_id(), Some(19));
        Ok(())
    }

    #[tokio::test]
    async fn update_music_group_standard_user_is_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        let stored = group(7, "The Group", None)?;
        groups.expect_get().times(1).return_once(move |_| {
            let found = stored.clone();
            Box::pin(async move { Ok(Some(found)) })
        });
        let harness = harness_with(users, groups);
        let use_case: UseCase = UpdateMusicGroup::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateMusicGroupCommand::new(
                1,
                7,
                Some("The Renamed".to_owned()),
                None,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateMusicGroupError::Forbidden)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_music_group_duplicate_name_returns_name_already_exists()
    -> Result<(), Box<dyn Error>> {
        // Arrange: another group carries the requested name.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        let stored = group(7, "The Group", None)?;
        groups.expect_get().times(1).return_once(move |_| {
            let found = stored.clone();
            Box::pin(async move { Ok(Some(found)) })
        });
        let collision = group(8, "The Renamed", None)?;
        groups.expect_search().times(1).return_once(move |_| {
            let found = vec![collision.clone()];
            Box::pin(async move { Ok(found) })
        });
        let harness = harness_with(users, groups);
        let use_case: UseCase = UpdateMusicGroup::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateMusicGroupCommand::new(
                1,
                7,
                Some("The Renamed".to_owned()),
                None,
            ))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(UpdateMusicGroupError::NameAlreadyExists)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn update_music_group_empty_name_returns_invalid_name() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        let stored = group(7, "The Group", None)?;
        groups.expect_get().times(1).return_once(move |_| {
            let found = stored.clone();
            Box::pin(async move { Ok(Some(found)) })
        });
        let harness = harness_with(users, groups);
        let use_case: UseCase = UpdateMusicGroup::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateMusicGroupCommand::new(
                1,
                7,
                Some("  ".to_owned()),
                None,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateMusicGroupError::InvalidName)));
        Ok(())
    }

    #[tokio::test]
    async fn update_music_group_default_group_returns_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange: group 0 is protected by its update trigger.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        let stored = group(0, "Unknown", None)?;
        groups.expect_get().times(1).return_once(move |_| {
            let found = stored.clone();
            Box::pin(async move { Ok(Some(found)) })
        });
        groups
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        groups
            .expect_update()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::Conflict) }));
        let harness = harness_with(users, groups);
        let use_case: UseCase = UpdateMusicGroup::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateMusicGroupCommand::new(
                1,
                0,
                Some("Renamed".to_owned()),
                None,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateMusicGroupError::Conflict)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_music_group_unknown_group_returns_no_such_group() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        groups
            .expect_get()
            .times(1)
            .returning(|_| Box::pin(async { Ok(None) }));
        let harness = harness_with(users, groups);
        let use_case: UseCase = UpdateMusicGroup::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateMusicGroupCommand::new(1, 404, None, None))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateMusicGroupError::NoSuchGroup)));
        Ok(())
    }

    #[tokio::test]
    async fn update_music_group_none_override_returns_no_such_group() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the caller is shut out of the library entirely.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        let hidden = group(7, "The Group", None)?;
        groups.expect_get().times(1).return_once(move |_| {
            let found = hidden.clone();
            Box::pin(async move { Ok(Some(found)) })
        });
        let mut permissions = MockPermissionRepository::new();
        expect_permissions(
            &mut permissions,
            1,
            vec![Permission::try_new(1, Action::LibraryRead, Scope::None)?],
        );
        let unit_of_work = TestUnitOfWork {
            music_groups: groups,
            permissions,
            users,
            ..TestUnitOfWork::default()
        };
        let harness = GalleryFactoryHarness::new(unit_of_work);
        let use_case: UseCase = UpdateMusicGroup::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateMusicGroupCommand::new(
                1,
                7,
                Some("The Renamed".to_owned()),
                None,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateMusicGroupError::NoSuchGroup)));
        Ok(())
    }

    #[tokio::test]
    async fn update_music_group_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, MockMusicGroupRepository::new());
        let use_case: UseCase = UpdateMusicGroup::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateMusicGroupCommand::new(999, 7, None, None))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateMusicGroupError::NoSuchCaller)));
        Ok(())
    }

    #[tokio::test]
    async fn update_music_group_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        let stored = group(7, "The Group", None)?;
        groups.expect_get().times(1).return_once(move |_| {
            let found = stored.clone();
            Box::pin(async move { Ok(Some(found)) })
        });
        let saved = group(7, "The Group", None)?;
        groups.expect_update().times(1).return_once(move |_| {
            let updated = saved.clone();
            Box::pin(async move { Ok(Some(updated)) })
        });
        let harness = harness_with(users, groups);
        harness.commit_fails.store(true, Ordering::SeqCst);
        let use_case: UseCase = UpdateMusicGroup::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateMusicGroupCommand::new(1, 7, None, None))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateMusicGroupError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }
}
