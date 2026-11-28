use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use tracing::warn;

use crate::application::authorization::authorize;
use crate::application::port::{
    CreateGalleryCommand, CreateGalleryError, CreateGalleryResponse, CreateGalleryUseCase,
};
use crate::domain::model::{Action, ChildKind, Gallery, ResourceKind, ResourceRef, Visibility};
use crate::domain::port::{
    GalleryFilter, GalleryRepository as _, IdentityUnitOfWork, LibraryUnitOfWork, NewResource,
    RepositoryError, ResourceRepository as _, UnitOfWork as _, UnitOfWorkFactory, UserFilter,
    UserRepository as _, UserUnitOfWork,
};
use crate::domain::service::{NamePolicy, NamePolicyError};

/// Use case implementation for creating a gallery.
pub(crate) struct CreateGallery<F> {
    /// Factory opening the unit of work wrapping the creation.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> CreateGallery<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> CreateGalleryUseCase for CreateGallery<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: CreateGalleryCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<CreateGalleryResponse, CreateGalleryError>> + Send + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| CreateGalleryError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| CreateGalleryError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(CreateGalleryError::NoSuchCaller)?;

                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::GalleryCollection, 0),
                    Action::CreateChild,
                    Some(ChildKind::Gallery),
                )
                .await
                .map_err(|_| CreateGalleryError::Forbidden)?;

                let name =
                    NamePolicy::validate(command.name().to_owned()).map_err(
                        |error| match error {
                            NamePolicyError::Empty => {
                                warn!(
                                    target: "security",
                                    event = "input_validation_failed",
                                    field = %"name",
                                    reason = %"empty",
                                    "rejected a client-supplied value"
                                );
                                CreateGalleryError::InvalidName
                            }
                            NamePolicyError::SurroundingWhitespace => {
                                warn!(
                                    target: "security",
                                    event = "input_validation_failed",
                                    field = %"name",
                                    reason = %"surrounding_whitespace",
                                    "rejected a client-supplied value"
                                );
                                CreateGalleryError::InvalidName
                            }
                            NamePolicyError::TooLong => {
                                warn!(
                                    target: "security",
                                    event = "input_validation_failed",
                                    field = %"name",
                                    reason = %"too_long",
                                    "rejected a client-supplied value"
                                );
                                CreateGalleryError::NameTooLong
                            }
                        },
                    )?;
                let now = Utc::now().naive_utc();
                let gallery = Gallery::new(
                    0,
                    Some(command.caller_id()),
                    name,
                    command.is_public(),
                    now,
                    now,
                );

                let existing = unit_of_work
                    .galleries()
                    .search(&GalleryFilter {
                        name: Some(gallery.name().to_owned()),
                        user_id: Some(command.caller_id()),
                        ..GalleryFilter::default()
                    })
                    .await
                    .map_err(|error| CreateGalleryError::Unknown(error.into()))?;
                if !existing.is_empty() {
                    return Err(CreateGalleryError::NameAlreadyExists);
                }

                let created = unit_of_work
                    .galleries()
                    .create(gallery)
                    .await
                    .map_err(|error| match error {
                        // A concurrent insert won the race for the per-owner
                        // unique name; the use case translates the port's
                        // `AlreadyExist` so the caller cannot own a second
                        // gallery with the same name either way (`STY-RUST-014`).
                        RepositoryError::AlreadyExist => CreateGalleryError::NameAlreadyExists,
                        // Every other variant maps to `Unknown`; naming them
                        // keeps a new port variant a compile error here.
                        RepositoryError::Conflict
                        | RepositoryError::DataIntegrityViolation
                        | RepositoryError::ConcurrencyConflict
                        | RepositoryError::OperationFailed
                        | RepositoryError::Timeout
                        | RepositoryError::Unavailable
                        | RepositoryError::Unknown(_) => CreateGalleryError::Unknown(error.into()),
                    })?;

                unit_of_work
                    .resources()
                    .create(NewResource {
                        kind: ResourceKind::Gallery,
                        entity_id: created.id(),
                        owner_id: Some(command.caller_id()),
                        auth_parent: Some(ResourceRef::new(ResourceKind::Library, 0)),
                        visibility: if created.is_public() {
                            Visibility::Public
                        } else {
                            Visibility::Private
                        },
                    })
                    .await
                    .map_err(|error| CreateGalleryError::Unknown(error.into()))?;

                Ok(CreateGalleryResponse::new(
                    created.id(),
                    command.caller_id(),
                    created.name().to_owned(),
                    created.is_public(),
                    created.created_at(),
                    created.last_modified_at(),
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| CreateGalleryError::Unknown(error.into()))?;
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
    use super::*;
    use crate::domain::model::{Resource, ResourceAccess, Role, User, Visibility};

    use crate::domain::port::{MockGalleryRepository, MockResourceRepository, MockUserRepository};

    use crate::test_helpers::*;
    use chrono::NaiveDateTime;
    use std::error::Error;
    use std::sync::atomic::Ordering;
    use tracing_test::traced_test;

    type UseCase = CreateGallery<TestFactory>;

    fn gallery(gallery_id: i64, user_id: Option<i64>, name: &str, is_public: bool) -> Gallery {
        let now = NaiveDateTime::default();
        Gallery::new(gallery_id, user_id, name.to_owned(), is_public, now, now)
    }

    fn harness_with_resources(
        users: MockUserRepository,
        resources: MockResourceRepository,
        galleries: MockGalleryRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            galleries,
            resources,
            users,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
    }

    // proves: SCN-GALLERY-003-A — Positive
    #[tokio::test]
    async fn create_gallery_creates_owned_private_gallery() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository, caller_id: i64| -> Result<(), Box<dyn Error>> {
            let caller = User::try_new(
                caller_id,
                "caller".to_owned(),
                None,
                caller_id,
                Role::Standard,
            )?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut galleries = MockGalleryRepository::new();
        let stored = gallery(9, Some(1), "Holidays", false);
        (move |galleries: &mut MockGalleryRepository| {
            galleries
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        })(&mut galleries);
        galleries
            .expect_create()
            .times(1)
            .withf(|pending| pending.user_id() == Some(1) && !pending.is_public())
            .return_once(move |_| {
                let found = stored.clone();
                Box::pin(async move { Ok(found) })
            });
        let harness = (move |users: MockUserRepository,
                             galleries: MockGalleryRepository|
              -> GalleryFactoryHarness {
            harness_with_resources(
                users,
                (move || -> crate::domain::port::MockResourceRepository {
                    let mut resources = crate::domain::port::MockResourceRepository::new();
                    resources.expect_search().returning(|filter, caller| {
                        let user_id = caller.id();
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            filter
                                .kind
                                .unwrap_or(crate::domain::model::ResourceKind::Library),
                            filter.entity_id.unwrap_or_default(),
                            Some(user_id),
                            None,
                            crate::domain::model::Visibility::Private,
                        );
                        let grant = crate::domain::model::Grant::from_stored(
                            1,
                            crate::domain::model::Principal::User(user_id),
                            1,
                            crate::domain::model::ResourceRole::Downloader,
                            crate::domain::model::ChildKind::All,
                            crate::domain::model::GrantEffect::Allow,
                        );
                        Box::pin(async move {
                            Ok(vec![crate::domain::model::ResourceAccess::new(
                                resource,
                                Vec::new(),
                                vec![grant],
                                Vec::new(),
                            )])
                        })
                    });
                    resources
                        .expect_search_many()
                        .returning(|addresses, caller| {
                            let user_id = caller.id();
                            let found = addresses
                                .iter()
                                .map(|address| {
                                    let resource = crate::domain::model::Resource::from_stored(
                                        1,
                                        address.kind(),
                                        address.id(),
                                        Some(user_id),
                                        None,
                                        crate::domain::model::Visibility::Private,
                                    );
                                    let grant = crate::domain::model::Grant::from_stored(
                                        1,
                                        crate::domain::model::Principal::User(user_id),
                                        1,
                                        crate::domain::model::ResourceRole::Downloader,
                                        crate::domain::model::ChildKind::All,
                                        crate::domain::model::GrantEffect::Allow,
                                    );
                                    crate::domain::model::ResourceAccess::new(
                                        resource,
                                        Vec::new(),
                                        vec![grant],
                                        Vec::new(),
                                    )
                                })
                                .collect::<Vec<_>>();
                            Box::pin(async move { Ok(found) })
                        });
                    resources.expect_create().returning(|new_resource| {
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            new_resource.kind,
                            new_resource.entity_id,
                            new_resource.owner_id,
                            None,
                            new_resource.visibility,
                        );
                        Box::pin(async move { Ok(resource) })
                    });
                    resources
                })(),
                galleries,
            )
        })(users, galleries);
        let use_case: UseCase = CreateGallery::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(CreateGalleryCommand::new(1, "Holidays".to_owned(), false))
            .await?;

        // Assert
        assert_eq!(
            response,
            CreateGalleryResponse::new(
                9,
                1,
                "Holidays".to_owned(),
                false,
                NaiveDateTime::default(),
                NaiveDateTime::default(),
            )
        );
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    // proves: SCN-GALLERY-003-B — Positive
    #[tokio::test]
    async fn create_gallery_public_flag_is_persisted() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository, caller_id: i64| -> Result<(), Box<dyn Error>> {
            let caller = User::try_new(
                caller_id,
                "caller".to_owned(),
                None,
                caller_id,
                Role::Standard,
            )?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut galleries = MockGalleryRepository::new();
        let stored = gallery(4, Some(1), "Shared", true);
        (move |galleries: &mut MockGalleryRepository| {
            galleries
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        })(&mut galleries);
        galleries
            .expect_create()
            .times(1)
            .withf(Gallery::is_public)
            .return_once(move |_| {
                let found = stored.clone();
                Box::pin(async move { Ok(found) })
            });
        let harness = (move |users: MockUserRepository,
                             galleries: MockGalleryRepository|
              -> GalleryFactoryHarness {
            harness_with_resources(
                users,
                (move || -> crate::domain::port::MockResourceRepository {
                    let mut resources = crate::domain::port::MockResourceRepository::new();
                    resources.expect_search().returning(|filter, caller| {
                        let user_id = caller.id();
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            filter
                                .kind
                                .unwrap_or(crate::domain::model::ResourceKind::Library),
                            filter.entity_id.unwrap_or_default(),
                            Some(user_id),
                            None,
                            crate::domain::model::Visibility::Private,
                        );
                        let grant = crate::domain::model::Grant::from_stored(
                            1,
                            crate::domain::model::Principal::User(user_id),
                            1,
                            crate::domain::model::ResourceRole::Downloader,
                            crate::domain::model::ChildKind::All,
                            crate::domain::model::GrantEffect::Allow,
                        );
                        Box::pin(async move {
                            Ok(vec![crate::domain::model::ResourceAccess::new(
                                resource,
                                Vec::new(),
                                vec![grant],
                                Vec::new(),
                            )])
                        })
                    });
                    resources
                        .expect_search_many()
                        .returning(|addresses, caller| {
                            let user_id = caller.id();
                            let found = addresses
                                .iter()
                                .map(|address| {
                                    let resource = crate::domain::model::Resource::from_stored(
                                        1,
                                        address.kind(),
                                        address.id(),
                                        Some(user_id),
                                        None,
                                        crate::domain::model::Visibility::Private,
                                    );
                                    let grant = crate::domain::model::Grant::from_stored(
                                        1,
                                        crate::domain::model::Principal::User(user_id),
                                        1,
                                        crate::domain::model::ResourceRole::Downloader,
                                        crate::domain::model::ChildKind::All,
                                        crate::domain::model::GrantEffect::Allow,
                                    );
                                    crate::domain::model::ResourceAccess::new(
                                        resource,
                                        Vec::new(),
                                        vec![grant],
                                        Vec::new(),
                                    )
                                })
                                .collect::<Vec<_>>();
                            Box::pin(async move { Ok(found) })
                        });
                    resources.expect_create().returning(|new_resource| {
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            new_resource.kind,
                            new_resource.entity_id,
                            new_resource.owner_id,
                            None,
                            new_resource.visibility,
                        );
                        Box::pin(async move { Ok(resource) })
                    });
                    resources
                })(),
                galleries,
            )
        })(users, galleries);
        let use_case: UseCase = CreateGallery::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(CreateGalleryCommand::new(1, "Shared".to_owned(), true))
            .await?;

        // Assert
        assert!(response.is_public());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    // proves: SCN-GALLERY-003-D — Negative
    #[tokio::test]
    #[traced_test]
    async fn create_gallery_surrounding_whitespace_returns_invalid_name()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository, caller_id: i64| -> Result<(), Box<dyn Error>> {
            let caller = User::try_new(
                caller_id,
                "caller".to_owned(),
                None,
                caller_id,
                Role::Standard,
            )?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut galleries = MockGalleryRepository::new();
        galleries.expect_search().times(0);
        galleries.expect_create().times(0);
        let harness = (move |users: MockUserRepository,
                             galleries: MockGalleryRepository|
              -> GalleryFactoryHarness {
            harness_with_resources(
                users,
                (move || -> crate::domain::port::MockResourceRepository {
                    let mut resources = crate::domain::port::MockResourceRepository::new();
                    resources.expect_search().returning(|filter, caller| {
                        let user_id = caller.id();
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            filter
                                .kind
                                .unwrap_or(crate::domain::model::ResourceKind::Library),
                            filter.entity_id.unwrap_or_default(),
                            Some(user_id),
                            None,
                            crate::domain::model::Visibility::Private,
                        );
                        let grant = crate::domain::model::Grant::from_stored(
                            1,
                            crate::domain::model::Principal::User(user_id),
                            1,
                            crate::domain::model::ResourceRole::Downloader,
                            crate::domain::model::ChildKind::All,
                            crate::domain::model::GrantEffect::Allow,
                        );
                        Box::pin(async move {
                            Ok(vec![crate::domain::model::ResourceAccess::new(
                                resource,
                                Vec::new(),
                                vec![grant],
                                Vec::new(),
                            )])
                        })
                    });
                    resources
                        .expect_search_many()
                        .returning(|addresses, caller| {
                            let user_id = caller.id();
                            let found = addresses
                                .iter()
                                .map(|address| {
                                    let resource = crate::domain::model::Resource::from_stored(
                                        1,
                                        address.kind(),
                                        address.id(),
                                        Some(user_id),
                                        None,
                                        crate::domain::model::Visibility::Private,
                                    );
                                    let grant = crate::domain::model::Grant::from_stored(
                                        1,
                                        crate::domain::model::Principal::User(user_id),
                                        1,
                                        crate::domain::model::ResourceRole::Downloader,
                                        crate::domain::model::ChildKind::All,
                                        crate::domain::model::GrantEffect::Allow,
                                    );
                                    crate::domain::model::ResourceAccess::new(
                                        resource,
                                        Vec::new(),
                                        vec![grant],
                                        Vec::new(),
                                    )
                                })
                                .collect::<Vec<_>>();
                            Box::pin(async move { Ok(found) })
                        });
                    resources.expect_create().returning(|new_resource| {
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            new_resource.kind,
                            new_resource.entity_id,
                            new_resource.owner_id,
                            None,
                            new_resource.visibility,
                        );
                        Box::pin(async move { Ok(resource) })
                    });
                    resources
                })(),
                galleries,
            )
        })(users, galleries);
        let use_case: UseCase = CreateGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateGalleryCommand::new(1, " Holidays ".to_owned(), false))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateGalleryError::InvalidName)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        assert!(
            logs_contain("reason=surrounding_whitespace"),
            "the event must classify the rejection"
        );
        Ok(())
    }

    // proves: SCN-GALLERY-003-D — Negative
    #[tokio::test]
    #[traced_test]
    async fn create_gallery_empty_name_returns_invalid_name() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository, caller_id: i64| -> Result<(), Box<dyn Error>> {
            let caller = User::try_new(
                caller_id,
                "caller".to_owned(),
                None,
                caller_id,
                Role::Standard,
            )?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut galleries = MockGalleryRepository::new();
        galleries.expect_create().times(0);
        let harness = (move |users: MockUserRepository,
                             galleries: MockGalleryRepository|
              -> GalleryFactoryHarness {
            harness_with_resources(
                users,
                (move || -> crate::domain::port::MockResourceRepository {
                    let mut resources = crate::domain::port::MockResourceRepository::new();
                    resources.expect_search().returning(|filter, caller| {
                        let user_id = caller.id();
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            filter
                                .kind
                                .unwrap_or(crate::domain::model::ResourceKind::Library),
                            filter.entity_id.unwrap_or_default(),
                            Some(user_id),
                            None,
                            crate::domain::model::Visibility::Private,
                        );
                        let grant = crate::domain::model::Grant::from_stored(
                            1,
                            crate::domain::model::Principal::User(user_id),
                            1,
                            crate::domain::model::ResourceRole::Downloader,
                            crate::domain::model::ChildKind::All,
                            crate::domain::model::GrantEffect::Allow,
                        );
                        Box::pin(async move {
                            Ok(vec![crate::domain::model::ResourceAccess::new(
                                resource,
                                Vec::new(),
                                vec![grant],
                                Vec::new(),
                            )])
                        })
                    });
                    resources
                        .expect_search_many()
                        .returning(|addresses, caller| {
                            let user_id = caller.id();
                            let found = addresses
                                .iter()
                                .map(|address| {
                                    let resource = crate::domain::model::Resource::from_stored(
                                        1,
                                        address.kind(),
                                        address.id(),
                                        Some(user_id),
                                        None,
                                        crate::domain::model::Visibility::Private,
                                    );
                                    let grant = crate::domain::model::Grant::from_stored(
                                        1,
                                        crate::domain::model::Principal::User(user_id),
                                        1,
                                        crate::domain::model::ResourceRole::Downloader,
                                        crate::domain::model::ChildKind::All,
                                        crate::domain::model::GrantEffect::Allow,
                                    );
                                    crate::domain::model::ResourceAccess::new(
                                        resource,
                                        Vec::new(),
                                        vec![grant],
                                        Vec::new(),
                                    )
                                })
                                .collect::<Vec<_>>();
                            Box::pin(async move { Ok(found) })
                        });
                    resources.expect_create().returning(|new_resource| {
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            new_resource.kind,
                            new_resource.entity_id,
                            new_resource.owner_id,
                            None,
                            new_resource.visibility,
                        );
                        Box::pin(async move { Ok(resource) })
                    });
                    resources
                })(),
                galleries,
            )
        })(users, galleries);
        let use_case: UseCase = CreateGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateGalleryCommand::new(1, "   ".to_owned(), false))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateGalleryError::InvalidName)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        logs_assert(|lines| {
            let count = lines.iter().filter(|line| line.contains("WARN")).count();
            (count == 1).then_some(()).ok_or_else(|| {
                format!("the empty-name rejection must emit exactly one warn event (found {count})")
            })
        });
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("event=\"input_validation_failed\""))
                .count();
            (count == 1)
                .then_some(())
                .ok_or_else(|| format!(
                    "the empty-name rejection must emit exactly one input_validation_failed event (found {count})"
                ))
        });
        assert!(
            logs_contain("security"),
            "the event must target the reserved security target"
        );
        assert!(
            logs_contain("field=name"),
            "the event must name the rejected field"
        );
        assert!(
            logs_contain("reason=empty"),
            "the event must classify the reason as an empty name"
        );
        assert!(
            !logs_contain("\"   \""),
            "the event must not echo the rejected name"
        );
        Ok(())
    }

    // proves: SCN-GALLERY-003-D — Negative
    #[tokio::test]
    #[traced_test]
    async fn create_gallery_long_name_returns_name_too_long() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository, caller_id: i64| -> Result<(), Box<dyn Error>> {
            let caller = User::try_new(
                caller_id,
                "caller".to_owned(),
                None,
                caller_id,
                Role::Standard,
            )?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut galleries = MockGalleryRepository::new();
        galleries.expect_create().times(0);
        let harness = (move |users: MockUserRepository,
                             galleries: MockGalleryRepository|
              -> GalleryFactoryHarness {
            harness_with_resources(
                users,
                (move || -> crate::domain::port::MockResourceRepository {
                    let mut resources = crate::domain::port::MockResourceRepository::new();
                    resources.expect_search().returning(|filter, caller| {
                        let user_id = caller.id();
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            filter
                                .kind
                                .unwrap_or(crate::domain::model::ResourceKind::Library),
                            filter.entity_id.unwrap_or_default(),
                            Some(user_id),
                            None,
                            crate::domain::model::Visibility::Private,
                        );
                        let grant = crate::domain::model::Grant::from_stored(
                            1,
                            crate::domain::model::Principal::User(user_id),
                            1,
                            crate::domain::model::ResourceRole::Downloader,
                            crate::domain::model::ChildKind::All,
                            crate::domain::model::GrantEffect::Allow,
                        );
                        Box::pin(async move {
                            Ok(vec![crate::domain::model::ResourceAccess::new(
                                resource,
                                Vec::new(),
                                vec![grant],
                                Vec::new(),
                            )])
                        })
                    });
                    resources
                        .expect_search_many()
                        .returning(|addresses, caller| {
                            let user_id = caller.id();
                            let found = addresses
                                .iter()
                                .map(|address| {
                                    let resource = crate::domain::model::Resource::from_stored(
                                        1,
                                        address.kind(),
                                        address.id(),
                                        Some(user_id),
                                        None,
                                        crate::domain::model::Visibility::Private,
                                    );
                                    let grant = crate::domain::model::Grant::from_stored(
                                        1,
                                        crate::domain::model::Principal::User(user_id),
                                        1,
                                        crate::domain::model::ResourceRole::Downloader,
                                        crate::domain::model::ChildKind::All,
                                        crate::domain::model::GrantEffect::Allow,
                                    );
                                    crate::domain::model::ResourceAccess::new(
                                        resource,
                                        Vec::new(),
                                        vec![grant],
                                        Vec::new(),
                                    )
                                })
                                .collect::<Vec<_>>();
                            Box::pin(async move { Ok(found) })
                        });
                    resources.expect_create().returning(|new_resource| {
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            new_resource.kind,
                            new_resource.entity_id,
                            new_resource.owner_id,
                            None,
                            new_resource.visibility,
                        );
                        Box::pin(async move { Ok(resource) })
                    });
                    resources
                })(),
                galleries,
            )
        })(users, galleries);
        let use_case: UseCase = CreateGallery::new(Arc::clone(&harness.factory));
        let name = "x".repeat(1_000);

        // Act
        let result = use_case
            .execute(CreateGalleryCommand::new(1, name.clone(), false))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateGalleryError::NameTooLong)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        logs_assert(|lines| {
            let count = lines.iter().filter(|line| line.contains("WARN")).count();
            (count == 1).then_some(()).ok_or_else(|| {
                format!(
                    "the over-long-name rejection must emit exactly one warn event (found {count})"
                )
            })
        });
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("event=\"input_validation_failed\""))
                .count();
            (count == 1)
                .then_some(())
                .ok_or_else(|| format!(
                    "the over-long-name rejection must emit exactly one input_validation_failed event (found {count})"
                ))
        });
        assert!(
            logs_contain("security"),
            "the event must target the reserved security target"
        );
        assert!(
            logs_contain("field=name"),
            "the event must name the rejected field"
        );
        assert!(
            logs_contain("reason=too_long"),
            "the event must classify the reason as an over-long name"
        );
        assert!(
            !logs_contain(&name),
            "the event must not echo the client-supplied name"
        );
        Ok(())
    }

    // proves: SCN-GALLERY-003-C — Negative
    #[tokio::test]
    async fn create_gallery_existing_name_returns_name_already_exists() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository, caller_id: i64| -> Result<(), Box<dyn Error>> {
            let caller = User::try_new(
                caller_id,
                "caller".to_owned(),
                None,
                caller_id,
                Role::Standard,
            )?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut galleries = MockGalleryRepository::new();
        let existing = gallery(3, Some(1), "Holidays", false);
        galleries
            .expect_search()
            .times(1)
            .withf(|filter| filter.name.as_deref() == Some("Holidays") && filter.user_id == Some(1))
            .return_once(move |_| {
                let found = existing.clone();
                Box::pin(async move { Ok(vec![found]) })
            });
        galleries.expect_create().times(0);
        let harness = (move |users: MockUserRepository,
                             galleries: MockGalleryRepository|
              -> GalleryFactoryHarness {
            harness_with_resources(
                users,
                (move || -> crate::domain::port::MockResourceRepository {
                    let mut resources = crate::domain::port::MockResourceRepository::new();
                    resources.expect_search().returning(|filter, caller| {
                        let user_id = caller.id();
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            filter
                                .kind
                                .unwrap_or(crate::domain::model::ResourceKind::Library),
                            filter.entity_id.unwrap_or_default(),
                            Some(user_id),
                            None,
                            crate::domain::model::Visibility::Private,
                        );
                        let grant = crate::domain::model::Grant::from_stored(
                            1,
                            crate::domain::model::Principal::User(user_id),
                            1,
                            crate::domain::model::ResourceRole::Downloader,
                            crate::domain::model::ChildKind::All,
                            crate::domain::model::GrantEffect::Allow,
                        );
                        Box::pin(async move {
                            Ok(vec![crate::domain::model::ResourceAccess::new(
                                resource,
                                Vec::new(),
                                vec![grant],
                                Vec::new(),
                            )])
                        })
                    });
                    resources
                        .expect_search_many()
                        .returning(|addresses, caller| {
                            let user_id = caller.id();
                            let found = addresses
                                .iter()
                                .map(|address| {
                                    let resource = crate::domain::model::Resource::from_stored(
                                        1,
                                        address.kind(),
                                        address.id(),
                                        Some(user_id),
                                        None,
                                        crate::domain::model::Visibility::Private,
                                    );
                                    let grant = crate::domain::model::Grant::from_stored(
                                        1,
                                        crate::domain::model::Principal::User(user_id),
                                        1,
                                        crate::domain::model::ResourceRole::Downloader,
                                        crate::domain::model::ChildKind::All,
                                        crate::domain::model::GrantEffect::Allow,
                                    );
                                    crate::domain::model::ResourceAccess::new(
                                        resource,
                                        Vec::new(),
                                        vec![grant],
                                        Vec::new(),
                                    )
                                })
                                .collect::<Vec<_>>();
                            Box::pin(async move { Ok(found) })
                        });
                    resources.expect_create().returning(|new_resource| {
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            new_resource.kind,
                            new_resource.entity_id,
                            new_resource.owner_id,
                            None,
                            new_resource.visibility,
                        );
                        Box::pin(async move { Ok(resource) })
                    });
                    resources
                })(),
                galleries,
            )
        })(users, galleries);
        let use_case: UseCase = CreateGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateGalleryCommand::new(1, "Holidays".to_owned(), false))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateGalleryError::NameAlreadyExists)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    // proves: SCN-GALLERY-003-E — Security
    #[tokio::test]
    async fn create_gallery_denied_on_the_collection_returns_forbidden()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the `GALLERY_COLLECTION` surface belongs to another principal
        // and carries no allow, so the `CreateChild` decision refuses.
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository, caller_id: i64| -> Result<(), Box<dyn Error>> {
            let caller = User::try_new(
                caller_id,
                "caller".to_owned(),
                None,
                caller_id,
                Role::Standard,
            )?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut resources = MockResourceRepository::new();
        resources
            .expect_search()
            .times(1)
            .return_once(|filter, _caller| {
                let resource = Resource::from_stored(
                    1,
                    ResourceKind::GalleryCollection,
                    filter.entity_id.unwrap_or_default(),
                    Some(99),
                    None,
                    Visibility::Private,
                );
                Box::pin(async move {
                    Ok(vec![ResourceAccess::new(
                        resource,
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                    )])
                })
            });
        resources.expect_create().times(0);
        let mut galleries = MockGalleryRepository::new();
        galleries.expect_search().times(0);
        galleries.expect_create().times(0);
        let harness = harness_with_resources(users, resources, galleries);
        let use_case: UseCase = CreateGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateGalleryCommand::new(1, "Holidays".to_owned(), false))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateGalleryError::Forbidden)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        assert!(!harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    // proves: SCN-GALLERY-003-C — Negative
    #[tokio::test]
    async fn create_gallery_raced_duplicate_name_returns_name_already_exists()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository, caller_id: i64| -> Result<(), Box<dyn Error>> {
            let caller = User::try_new(
                caller_id,
                "caller".to_owned(),
                None,
                caller_id,
                Role::Standard,
            )?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut galleries = MockGalleryRepository::new();
        (move |galleries: &mut MockGalleryRepository| {
            galleries
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        })(&mut galleries);
        galleries
            .expect_create()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::AlreadyExist) }));
        let harness = (move |users: MockUserRepository,
                             galleries: MockGalleryRepository|
              -> GalleryFactoryHarness {
            harness_with_resources(
                users,
                (move || -> crate::domain::port::MockResourceRepository {
                    let mut resources = crate::domain::port::MockResourceRepository::new();
                    resources.expect_search().returning(|filter, caller| {
                        let user_id = caller.id();
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            filter
                                .kind
                                .unwrap_or(crate::domain::model::ResourceKind::Library),
                            filter.entity_id.unwrap_or_default(),
                            Some(user_id),
                            None,
                            crate::domain::model::Visibility::Private,
                        );
                        let grant = crate::domain::model::Grant::from_stored(
                            1,
                            crate::domain::model::Principal::User(user_id),
                            1,
                            crate::domain::model::ResourceRole::Downloader,
                            crate::domain::model::ChildKind::All,
                            crate::domain::model::GrantEffect::Allow,
                        );
                        Box::pin(async move {
                            Ok(vec![crate::domain::model::ResourceAccess::new(
                                resource,
                                Vec::new(),
                                vec![grant],
                                Vec::new(),
                            )])
                        })
                    });
                    resources
                        .expect_search_many()
                        .returning(|addresses, caller| {
                            let user_id = caller.id();
                            let found = addresses
                                .iter()
                                .map(|address| {
                                    let resource = crate::domain::model::Resource::from_stored(
                                        1,
                                        address.kind(),
                                        address.id(),
                                        Some(user_id),
                                        None,
                                        crate::domain::model::Visibility::Private,
                                    );
                                    let grant = crate::domain::model::Grant::from_stored(
                                        1,
                                        crate::domain::model::Principal::User(user_id),
                                        1,
                                        crate::domain::model::ResourceRole::Downloader,
                                        crate::domain::model::ChildKind::All,
                                        crate::domain::model::GrantEffect::Allow,
                                    );
                                    crate::domain::model::ResourceAccess::new(
                                        resource,
                                        Vec::new(),
                                        vec![grant],
                                        Vec::new(),
                                    )
                                })
                                .collect::<Vec<_>>();
                            Box::pin(async move { Ok(found) })
                        });
                    resources.expect_create().returning(|new_resource| {
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            new_resource.kind,
                            new_resource.entity_id,
                            new_resource.owner_id,
                            None,
                            new_resource.visibility,
                        );
                        Box::pin(async move { Ok(resource) })
                    });
                    resources
                })(),
                galleries,
            )
        })(users, galleries);
        let use_case: UseCase = CreateGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateGalleryCommand::new(1, "Holidays".to_owned(), false))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateGalleryError::NameAlreadyExists)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    // proves: SCN-GALLERY-003-A — Positive
    #[tokio::test]
    async fn create_gallery_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = (move |users: MockUserRepository,
                             galleries: MockGalleryRepository|
              -> GalleryFactoryHarness {
            harness_with_resources(
                users,
                (move || -> crate::domain::port::MockResourceRepository {
                    let mut resources = crate::domain::port::MockResourceRepository::new();
                    resources.expect_search().returning(|filter, caller| {
                        let user_id = caller.id();
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            filter
                                .kind
                                .unwrap_or(crate::domain::model::ResourceKind::Library),
                            filter.entity_id.unwrap_or_default(),
                            Some(user_id),
                            None,
                            crate::domain::model::Visibility::Private,
                        );
                        let grant = crate::domain::model::Grant::from_stored(
                            1,
                            crate::domain::model::Principal::User(user_id),
                            1,
                            crate::domain::model::ResourceRole::Downloader,
                            crate::domain::model::ChildKind::All,
                            crate::domain::model::GrantEffect::Allow,
                        );
                        Box::pin(async move {
                            Ok(vec![crate::domain::model::ResourceAccess::new(
                                resource,
                                Vec::new(),
                                vec![grant],
                                Vec::new(),
                            )])
                        })
                    });
                    resources
                        .expect_search_many()
                        .returning(|addresses, caller| {
                            let user_id = caller.id();
                            let found = addresses
                                .iter()
                                .map(|address| {
                                    let resource = crate::domain::model::Resource::from_stored(
                                        1,
                                        address.kind(),
                                        address.id(),
                                        Some(user_id),
                                        None,
                                        crate::domain::model::Visibility::Private,
                                    );
                                    let grant = crate::domain::model::Grant::from_stored(
                                        1,
                                        crate::domain::model::Principal::User(user_id),
                                        1,
                                        crate::domain::model::ResourceRole::Downloader,
                                        crate::domain::model::ChildKind::All,
                                        crate::domain::model::GrantEffect::Allow,
                                    );
                                    crate::domain::model::ResourceAccess::new(
                                        resource,
                                        Vec::new(),
                                        vec![grant],
                                        Vec::new(),
                                    )
                                })
                                .collect::<Vec<_>>();
                            Box::pin(async move { Ok(found) })
                        });
                    resources.expect_create().returning(|new_resource| {
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            new_resource.kind,
                            new_resource.entity_id,
                            new_resource.owner_id,
                            None,
                            new_resource.visibility,
                        );
                        Box::pin(async move { Ok(resource) })
                    });
                    resources
                })(),
                galleries,
            )
        })(users, MockGalleryRepository::new());
        let use_case: UseCase = CreateGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateGalleryCommand::new(999, "Holidays".to_owned(), false))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateGalleryError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    // proves: SCN-GALLERY-003-A — Positive
    #[tokio::test]
    async fn create_gallery_caller_load_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = (move |users: MockUserRepository,
                             galleries: MockGalleryRepository|
              -> GalleryFactoryHarness {
            harness_with_resources(
                users,
                (move || -> crate::domain::port::MockResourceRepository {
                    let mut resources = crate::domain::port::MockResourceRepository::new();
                    resources.expect_search().returning(|filter, caller| {
                        let user_id = caller.id();
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            filter
                                .kind
                                .unwrap_or(crate::domain::model::ResourceKind::Library),
                            filter.entity_id.unwrap_or_default(),
                            Some(user_id),
                            None,
                            crate::domain::model::Visibility::Private,
                        );
                        let grant = crate::domain::model::Grant::from_stored(
                            1,
                            crate::domain::model::Principal::User(user_id),
                            1,
                            crate::domain::model::ResourceRole::Downloader,
                            crate::domain::model::ChildKind::All,
                            crate::domain::model::GrantEffect::Allow,
                        );
                        Box::pin(async move {
                            Ok(vec![crate::domain::model::ResourceAccess::new(
                                resource,
                                Vec::new(),
                                vec![grant],
                                Vec::new(),
                            )])
                        })
                    });
                    resources
                        .expect_search_many()
                        .returning(|addresses, caller| {
                            let user_id = caller.id();
                            let found = addresses
                                .iter()
                                .map(|address| {
                                    let resource = crate::domain::model::Resource::from_stored(
                                        1,
                                        address.kind(),
                                        address.id(),
                                        Some(user_id),
                                        None,
                                        crate::domain::model::Visibility::Private,
                                    );
                                    let grant = crate::domain::model::Grant::from_stored(
                                        1,
                                        crate::domain::model::Principal::User(user_id),
                                        1,
                                        crate::domain::model::ResourceRole::Downloader,
                                        crate::domain::model::ChildKind::All,
                                        crate::domain::model::GrantEffect::Allow,
                                    );
                                    crate::domain::model::ResourceAccess::new(
                                        resource,
                                        Vec::new(),
                                        vec![grant],
                                        Vec::new(),
                                    )
                                })
                                .collect::<Vec<_>>();
                            Box::pin(async move { Ok(found) })
                        });
                    resources.expect_create().returning(|new_resource| {
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            new_resource.kind,
                            new_resource.entity_id,
                            new_resource.owner_id,
                            None,
                            new_resource.visibility,
                        );
                        Box::pin(async move { Ok(resource) })
                    });
                    resources
                })(),
                galleries,
            )
        })(users, MockGalleryRepository::new());
        let use_case: UseCase = CreateGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateGalleryCommand::new(1, "Holidays".to_owned(), false))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateGalleryError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    // proves: SCN-GALLERY-003-A — Positive
    #[tokio::test]
    async fn create_gallery_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository, caller_id: i64| -> Result<(), Box<dyn Error>> {
            let caller = User::try_new(
                caller_id,
                "caller".to_owned(),
                None,
                caller_id,
                Role::Standard,
            )?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut galleries = MockGalleryRepository::new();
        (move |galleries: &mut MockGalleryRepository| {
            galleries
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        })(&mut galleries);
        galleries
            .expect_create()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = (move |users: MockUserRepository,
                             galleries: MockGalleryRepository|
              -> GalleryFactoryHarness {
            harness_with_resources(
                users,
                (move || -> crate::domain::port::MockResourceRepository {
                    let mut resources = crate::domain::port::MockResourceRepository::new();
                    resources.expect_search().returning(|filter, caller| {
                        let user_id = caller.id();
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            filter
                                .kind
                                .unwrap_or(crate::domain::model::ResourceKind::Library),
                            filter.entity_id.unwrap_or_default(),
                            Some(user_id),
                            None,
                            crate::domain::model::Visibility::Private,
                        );
                        let grant = crate::domain::model::Grant::from_stored(
                            1,
                            crate::domain::model::Principal::User(user_id),
                            1,
                            crate::domain::model::ResourceRole::Downloader,
                            crate::domain::model::ChildKind::All,
                            crate::domain::model::GrantEffect::Allow,
                        );
                        Box::pin(async move {
                            Ok(vec![crate::domain::model::ResourceAccess::new(
                                resource,
                                Vec::new(),
                                vec![grant],
                                Vec::new(),
                            )])
                        })
                    });
                    resources
                        .expect_search_many()
                        .returning(|addresses, caller| {
                            let user_id = caller.id();
                            let found = addresses
                                .iter()
                                .map(|address| {
                                    let resource = crate::domain::model::Resource::from_stored(
                                        1,
                                        address.kind(),
                                        address.id(),
                                        Some(user_id),
                                        None,
                                        crate::domain::model::Visibility::Private,
                                    );
                                    let grant = crate::domain::model::Grant::from_stored(
                                        1,
                                        crate::domain::model::Principal::User(user_id),
                                        1,
                                        crate::domain::model::ResourceRole::Downloader,
                                        crate::domain::model::ChildKind::All,
                                        crate::domain::model::GrantEffect::Allow,
                                    );
                                    crate::domain::model::ResourceAccess::new(
                                        resource,
                                        Vec::new(),
                                        vec![grant],
                                        Vec::new(),
                                    )
                                })
                                .collect::<Vec<_>>();
                            Box::pin(async move { Ok(found) })
                        });
                    resources.expect_create().returning(|new_resource| {
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            new_resource.kind,
                            new_resource.entity_id,
                            new_resource.owner_id,
                            None,
                            new_resource.visibility,
                        );
                        Box::pin(async move { Ok(resource) })
                    });
                    resources
                })(),
                galleries,
            )
        })(users, galleries);
        let use_case: UseCase = CreateGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateGalleryCommand::new(1, "Holidays".to_owned(), false))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateGalleryError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    // proves: SCN-GALLERY-003-A — Positive
    #[tokio::test]
    async fn create_gallery_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository, caller_id: i64| -> Result<(), Box<dyn Error>> {
            let caller = User::try_new(
                caller_id,
                "caller".to_owned(),
                None,
                caller_id,
                Role::Standard,
            )?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut galleries = MockGalleryRepository::new();
        let stored = gallery(9, Some(1), "Holidays", false);
        (move |galleries: &mut MockGalleryRepository| {
            galleries
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        })(&mut galleries);
        galleries.expect_create().times(1).return_once(move |_| {
            let found = stored.clone();
            Box::pin(async move { Ok(found) })
        });
        let harness = (move |users: MockUserRepository,
                             galleries: MockGalleryRepository|
              -> GalleryFactoryHarness {
            harness_with_resources(
                users,
                (move || -> crate::domain::port::MockResourceRepository {
                    let mut resources = crate::domain::port::MockResourceRepository::new();
                    resources.expect_search().returning(|filter, caller| {
                        let user_id = caller.id();
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            filter
                                .kind
                                .unwrap_or(crate::domain::model::ResourceKind::Library),
                            filter.entity_id.unwrap_or_default(),
                            Some(user_id),
                            None,
                            crate::domain::model::Visibility::Private,
                        );
                        let grant = crate::domain::model::Grant::from_stored(
                            1,
                            crate::domain::model::Principal::User(user_id),
                            1,
                            crate::domain::model::ResourceRole::Downloader,
                            crate::domain::model::ChildKind::All,
                            crate::domain::model::GrantEffect::Allow,
                        );
                        Box::pin(async move {
                            Ok(vec![crate::domain::model::ResourceAccess::new(
                                resource,
                                Vec::new(),
                                vec![grant],
                                Vec::new(),
                            )])
                        })
                    });
                    resources
                        .expect_search_many()
                        .returning(|addresses, caller| {
                            let user_id = caller.id();
                            let found = addresses
                                .iter()
                                .map(|address| {
                                    let resource = crate::domain::model::Resource::from_stored(
                                        1,
                                        address.kind(),
                                        address.id(),
                                        Some(user_id),
                                        None,
                                        crate::domain::model::Visibility::Private,
                                    );
                                    let grant = crate::domain::model::Grant::from_stored(
                                        1,
                                        crate::domain::model::Principal::User(user_id),
                                        1,
                                        crate::domain::model::ResourceRole::Downloader,
                                        crate::domain::model::ChildKind::All,
                                        crate::domain::model::GrantEffect::Allow,
                                    );
                                    crate::domain::model::ResourceAccess::new(
                                        resource,
                                        Vec::new(),
                                        vec![grant],
                                        Vec::new(),
                                    )
                                })
                                .collect::<Vec<_>>();
                            Box::pin(async move { Ok(found) })
                        });
                    resources.expect_create().returning(|new_resource| {
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            new_resource.kind,
                            new_resource.entity_id,
                            new_resource.owner_id,
                            None,
                            new_resource.visibility,
                        );
                        Box::pin(async move { Ok(resource) })
                    });
                    resources
                })(),
                galleries,
            )
        })(users, galleries);
        harness.commit_fails.store(true, Ordering::SeqCst);
        let use_case: UseCase = CreateGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateGalleryCommand::new(1, "Holidays".to_owned(), false))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateGalleryError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }
}
