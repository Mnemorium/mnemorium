use crate::application::authorization::allows_many;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::{
    ListGalleriesCommand, ListGalleriesError, ListGalleriesItem, ListGalleriesResponse,
    ListGalleriesUseCase,
};
use crate::domain::NumericID;
use crate::domain::model::{Action, Gallery, ResourceKind, ResourceRef};
use crate::domain::port::{
    GalleryFilter, GalleryRepository as _, IdentityUnitOfWork, LibraryUnitOfWork, UnitOfWork as _,
    UnitOfWorkFactory, UserFilter, UserRepository as _, UserUnitOfWork,
};

/// Use case implementation for listing the galleries the caller may see.
pub(crate) struct ListGalleries<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> ListGalleries<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> ListGalleriesUseCase for ListGalleries<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: ListGalleriesCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ListGalleriesResponse, ListGalleriesError>> + Send + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| ListGalleriesError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| ListGalleriesError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(ListGalleriesError::NoSuchCaller)?;

                let mut galleries = unit_of_work
                    .galleries()
                    .search(&GalleryFilter {
                        is_public: command.is_public(),
                        name: command.name().map(str::to_owned),
                        user_id: command.owner_id(),
                        ..GalleryFilter::default()
                    })
                    .await
                    .map_err(|error| ListGalleriesError::Unknown(error.into()))?;
                // One batched read decides the whole collection: resolving a
                // row at a time made the decision cost scale with the library.
                let addresses: Vec<ResourceRef> = galleries
                    .iter()
                    .map(|gallery| ResourceRef::new(ResourceKind::Gallery, gallery.id()))
                    .collect();
                let discoverable = allows_many(
                    &mut unit_of_work.resources(),
                    &caller,
                    &addresses,
                    Action::Discover,
                    None,
                )
                .await;
                let mut visible = Vec::with_capacity(galleries.len());
                for (gallery, allowed) in galleries.drain(..).zip(discoverable) {
                    if allowed {
                        visible.push(gallery);
                    }
                }
                galleries = visible;

                let total = galleries.len();
                let offset = command.offset().unwrap_or(0);
                let visible_page: Vec<_> = galleries
                    .into_iter()
                    .skip(offset)
                    .take(command.limit().unwrap_or(usize::MAX))
                    .collect();
                // Count items for the page only: loading every gallery's items
                // to count them is what made a 20-row page cost a join over the
                // whole library.
                let page_ids: Vec<NumericID> = visible_page.iter().map(Gallery::id).collect();
                let item_counts: HashMap<NumericID, usize> = unit_of_work
                    .galleries()
                    .count_items_by_gallery(&page_ids)
                    .await
                    .map_err(|error| ListGalleriesError::Unknown(error.into()))?
                    .into_iter()
                    .map(|(gallery_id, count)| {
                        (gallery_id, usize::try_from(count).unwrap_or(usize::MAX))
                    })
                    .collect();
                let items = visible_page
                    .into_iter()
                    .map(|gallery| {
                        ListGalleriesItem::new(
                            gallery.id(),
                            gallery.user_id(),
                            gallery.name().to_owned(),
                            gallery.is_public(),
                            gallery.created_at(),
                            gallery.last_modified_at(),
                            item_counts.get(&gallery.id()).copied().unwrap_or(0),
                        )
                    })
                    .collect();

                Ok(ListGalleriesResponse::new(items, total))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| ListGalleriesError::Unknown(error.into()))?;
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
    use crate::domain::model::{Gallery, Role, User};

    use crate::domain::port::{MockGalleryRepository, MockUserRepository, RepositoryError};

    use crate::test_helpers::*;
    use chrono::NaiveDateTime;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    type UseCase = ListGalleries<TestFactory>;

    fn gallery(gallery_id: i64, user_id: Option<i64>, name: &str, is_public: bool) -> Gallery {
        let now = NaiveDateTime::default();
        Gallery::new(gallery_id, user_id, name.to_owned(), is_public, now, now)
    }

    // proves: SCN-GALLERY-004-A — Business rule
    #[tokio::test]
    async fn list_galleries_root_admin_sees_every_gallery_with_item_counts()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository,
               caller_id: i64,
               role: Role|
              -> Result<(), Box<dyn Error>> {
            let caller = User::try_new(caller_id, "caller".to_owned(), None, caller_id, role)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 0, Role::Admin)?;
        let mut galleries = MockGalleryRepository::new();
        let public = gallery(2, Some(3), "Public", true);
        let private = gallery(3, Some(4), "Private", false);
        galleries
            .expect_search()
            .times(1)
            .withf(|filter| filter.is_public.is_none() && filter.user_id.is_none())
            .return_once(move |_| {
                let found = vec![public.clone(), private.clone()];
                Box::pin(async move { Ok(found) })
            });
        galleries
            .expect_count_items_by_gallery()
            .times(1)
            .withf(|gallery_ids| gallery_ids == [2, 3].as_slice())
            .return_once(|_| Box::pin(async { Ok(vec![(2, 2), (3, 1)]) }));
        let harness = (move |users: MockUserRepository,
                             galleries: MockGalleryRepository|
              -> GalleryFactoryHarness {
            let unit_of_work = TestUnitOfWork {
                galleries,
                users,
                ..TestUnitOfWork::default()
            };
            GalleryFactoryHarness::new(unit_of_work)
        })(users, galleries);
        let use_case: UseCase = ListGalleries::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(ListGalleriesCommand::new(0, None, None, None, None, None))
            .await?;

        // Assert
        assert_eq!(response.total(), 2);
        assert_eq!(
            response.galleries(),
            &[
                ListGalleriesItem::new(
                    2,
                    Some(3),
                    "Public".to_owned(),
                    true,
                    NaiveDateTime::default(),
                    NaiveDateTime::default(),
                    2,
                ),
                ListGalleriesItem::new(
                    3,
                    Some(4),
                    "Private".to_owned(),
                    false,
                    NaiveDateTime::default(),
                    NaiveDateTime::default(),
                    1,
                ),
            ]
        );
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }
    // proves: SCN-GALLERY-004-B — Business rule
    #[tokio::test]
    async fn list_galleries_forwards_filters_and_paginates() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository,
               caller_id: i64,
               role: Role|
              -> Result<(), Box<dyn Error>> {
            let caller = User::try_new(caller_id, "caller".to_owned(), None, caller_id, role)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 4, Role::Standard)?;
        let first = gallery(1, Some(4), "Trip A", true);
        let second = gallery(2, Some(4), "Trip B", true);
        let third = gallery(3, Some(4), "Trip C", true);
        let mut galleries = MockGalleryRepository::new();
        galleries
            .expect_search()
            .times(1)
            .withf(|filter| {
                filter.name.as_deref() == Some("Trip")
                    && filter.is_public == Some(true)
                    && filter.user_id == Some(4)
            })
            .return_once(move |_| {
                let found = vec![first.clone(), second.clone(), third.clone()];
                Box::pin(async move { Ok(found) })
            });
        galleries
            .expect_count_items_by_gallery()
            .times(1)
            .withf(|gallery_ids| gallery_ids == [2].as_slice())
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = (move |users: MockUserRepository,
                             galleries: MockGalleryRepository|
              -> GalleryFactoryHarness {
            let unit_of_work = TestUnitOfWork {
                galleries,
                users,
                ..TestUnitOfWork::default()
            };
            GalleryFactoryHarness::new(unit_of_work)
        })(users, galleries);
        let use_case: UseCase = ListGalleries::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(ListGalleriesCommand::new(
                4,
                Some("Trip".to_owned()),
                Some(true),
                Some(4),
                Some(1),
                Some(1),
            ))
            .await?;

        // Assert
        assert_eq!(response.total(), 3);
        assert_eq!(
            response
                .galleries()
                .first()
                .map(ListGalleriesItem::gallery_id),
            Some(2)
        );
        Ok(())
    }

    // proves: SCN-GALLERY-004-A — Business rule
    #[tokio::test]
    async fn list_galleries_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = (move |users: MockUserRepository,
                             galleries: MockGalleryRepository|
              -> GalleryFactoryHarness {
            let unit_of_work = TestUnitOfWork {
                galleries,
                users,
                ..TestUnitOfWork::default()
            };
            GalleryFactoryHarness::new(unit_of_work)
        })(users, MockGalleryRepository::new());
        let use_case: UseCase = ListGalleries::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ListGalleriesCommand::new(999, None, None, None, None, None))
            .await;

        // Assert
        assert!(matches!(result, Err(ListGalleriesError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    // proves: SCN-GALLERY-004-A — Business rule
    #[tokio::test]
    async fn list_galleries_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository,
               caller_id: i64,
               role: Role|
              -> Result<(), Box<dyn Error>> {
            let caller = User::try_new(caller_id, "caller".to_owned(), None, caller_id, role)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        galleries
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = (move |users: MockUserRepository,
                             galleries: MockGalleryRepository|
              -> GalleryFactoryHarness {
            let unit_of_work = TestUnitOfWork {
                galleries,
                users,
                ..TestUnitOfWork::default()
            };
            GalleryFactoryHarness::new(unit_of_work)
        })(users, galleries);
        let use_case: UseCase = ListGalleries::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ListGalleriesCommand::new(1, None, None, None, None, None))
            .await;

        // Assert
        assert!(matches!(result, Err(ListGalleriesError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    // proves: SCN-GALLERY-004-A — Business rule
    #[tokio::test]
    async fn list_galleries_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository,
               caller_id: i64,
               role: Role|
              -> Result<(), Box<dyn Error>> {
            let caller = User::try_new(caller_id, "caller".to_owned(), None, caller_id, role)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        galleries
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        galleries
            .expect_count_items_by_gallery()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = (move |users: MockUserRepository,
                             galleries: MockGalleryRepository|
              -> GalleryFactoryHarness {
            let unit_of_work = TestUnitOfWork {
                galleries,
                users,
                ..TestUnitOfWork::default()
            };
            GalleryFactoryHarness::new(unit_of_work)
        })(users, galleries);
        harness.commit_fails.store(true, Ordering::SeqCst);
        let use_case: UseCase = ListGalleries::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ListGalleriesCommand::new(1, None, None, None, None, None))
            .await;

        // Assert
        assert!(matches!(result, Err(ListGalleriesError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }
}
