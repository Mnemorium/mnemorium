use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::list_galleries::ListGalleriesCommand;
use crate::application::port::list_galleries::ListGalleriesError;
use crate::application::port::list_galleries::ListGalleriesItem;
use crate::application::port::list_galleries::ListGalleriesResponse;
use crate::application::port::list_galleries::ListGalleriesUseCase;
use crate::application::use_case::library_access;
use crate::domain::port::gallery_repository::GalleryFilter;
use crate::domain::port::gallery_repository::GalleryItemFilter;
use crate::domain::port::gallery_repository::GalleryRepository as _;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;

/// Use case implementation for listing the galleries the caller may see.
pub struct ListGalleries<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> ListGalleries<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> ListGalleriesUseCase for ListGalleries<F>
where
    F: UnitOfWorkFactory,
    F::Uow: LibraryUnitOfWork + UserUnitOfWork,
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
                let caller = library_access::load_caller(&mut unit_of_work, command.caller_id())
                    .await
                    .map_err(|error| ListGalleriesError::Unknown(error.into()))?
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
                galleries.retain(|gallery| library_access::can_read_gallery(gallery, &caller));

                let mut item_counts: HashMap<i64, usize> = HashMap::new();
                for detail in unit_of_work
                    .galleries()
                    .search_items(&GalleryItemFilter::default())
                    .await
                    .map_err(|error| ListGalleriesError::Unknown(error.into()))?
                {
                    let count = item_counts.entry(detail.item().gallery_id()).or_default();
                    *count = count.saturating_add(1);
                }

                let total = galleries.len();
                let offset = command.offset().unwrap_or(0);
                let page = galleries
                    .into_iter()
                    .skip(offset)
                    .take(command.limit().unwrap_or(usize::MAX))
                    .map(|gallery| {
                        ListGalleriesItem::new(
                            gallery.gallery_id(),
                            gallery.user_id(),
                            gallery.name().to_owned(),
                            gallery.is_public(),
                            gallery.created_at(),
                            gallery.last_modified_at(),
                            item_counts.get(&gallery.gallery_id()).copied().unwrap_or(0),
                        )
                    })
                    .collect();

                Ok(ListGalleriesResponse::new(page, total))
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
    use std::sync::atomic::Ordering;

    use chrono::NaiveDateTime;

    use crate::application::port::list_galleries::ListGalleriesCommand;
    use crate::application::port::list_galleries::ListGalleriesError;
    use crate::application::port::list_galleries::ListGalleriesItem;
    use crate::application::port::list_galleries::ListGalleriesUseCase as _;
    use crate::domain::model::gallery::Gallery;
    use crate::domain::model::gallery_item::GalleryItem;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::file_repository::MockFileRepository;
    use crate::domain::port::gallery_repository::GalleryItemDetail;
    use crate::domain::port::gallery_repository::MockGalleryRepository;
    use crate::domain::port::media_repository::MockMediaRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::GalleryFactoryHarness;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::gallery_factory;

    use super::ListGalleries;

    type UseCase = ListGalleries<TestFactory>;

    fn gallery(
        gallery_id: i64,
        user_id: Option<i64>,
        name: &str,
        is_public: bool,
    ) -> Result<Gallery, Box<dyn Error>> {
        let now = NaiveDateTime::default();
        Ok(Gallery::try_new(
            gallery_id,
            user_id,
            name.to_owned(),
            is_public,
            now,
            now,
        )?)
    }

    fn item_detail(
        gallery_item_id: i64,
        gallery_id: i64,
        image_id: i64,
    ) -> Result<GalleryItemDetail, Box<dyn Error>> {
        Ok(GalleryItemDetail::new(
            GalleryItem::try_new(
                gallery_item_id,
                gallery_id,
                Some(image_id),
                None,
                gallery_item_id,
                NaiveDateTime::default(),
            )?,
            100i64.saturating_add(image_id),
            format!("item{gallery_item_id}"),
        ))
    }

    fn harness_with(
        users: MockUserRepository,
        galleries: MockGalleryRepository,
    ) -> GalleryFactoryHarness {
        gallery_factory(
            galleries,
            MockMediaRepository::new(),
            users,
            MockFileRepository::new(),
        )
    }

    fn expect_caller(
        users: &mut MockUserRepository,
        caller_id: i64,
        role: Role,
    ) -> Result<(), Box<dyn Error>> {
        let caller = User::try_new(caller_id, "caller".to_owned(), None, caller_id, role)?;
        users
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(caller_id))
            .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
        Ok(())
    }

    #[tokio::test]
    async fn list_galleries_admin_sees_every_gallery_with_item_counts() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Admin)?;
        let mut galleries = MockGalleryRepository::new();
        let public = gallery(2, Some(3), "Public", true)?;
        let private = gallery(3, Some(4), "Private", false)?;
        galleries
            .expect_search()
            .times(1)
            .withf(|filter| filter.is_public.is_none() && filter.user_id.is_none())
            .return_once(move |_| {
                let found = vec![public.clone(), private.clone()];
                Box::pin(async move { Ok(found) })
            });
        let details = vec![
            item_detail(1, 2, 7)?,
            item_detail(2, 2, 8)?,
            item_detail(3, 3, 9)?,
        ];
        galleries
            .expect_search_items()
            .times(1)
            .return_once(move |_| {
                let found = details.clone();
                Box::pin(async move { Ok(found) })
            });
        let harness = harness_with(users, galleries);
        let use_case: UseCase = ListGalleries::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(ListGalleriesCommand::new(1, None, None, None, None, None))
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

    #[tokio::test]
    async fn list_galleries_standard_caller_sees_public_and_owned_only()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        let public = gallery(2, Some(3), "Public", true)?;
        let own = gallery(3, Some(4), "Mine", false)?;
        let other_private = gallery(5, Some(9), "Other", false)?;
        galleries.expect_search().times(1).return_once(move |_| {
            let found = vec![public.clone(), own.clone(), other_private.clone()];
            Box::pin(async move { Ok(found) })
        });
        galleries
            .expect_search_items()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, galleries);
        let use_case: UseCase = ListGalleries::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(ListGalleriesCommand::new(4, None, None, None, None, None))
            .await?;

        // Assert
        assert_eq!(response.total(), 2);
        assert_eq!(
            response
                .galleries()
                .first()
                .map(ListGalleriesItem::gallery_id),
            Some(2)
        );
        assert_eq!(
            response
                .galleries()
                .get(1)
                .map(ListGalleriesItem::gallery_id),
            Some(3)
        );
        Ok(())
    }

    #[tokio::test]
    async fn list_galleries_forwards_filters_and_paginates() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let first = gallery(1, Some(4), "Trip A", true)?;
        let second = gallery(2, Some(4), "Trip B", true)?;
        let third = gallery(3, Some(4), "Trip C", true)?;
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
            .expect_search_items()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, galleries);
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

    #[tokio::test]
    async fn list_galleries_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, MockGalleryRepository::new());
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

    #[tokio::test]
    async fn list_galleries_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        galleries
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, galleries);
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

    #[tokio::test]
    async fn list_galleries_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        galleries
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        galleries
            .expect_search_items()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, galleries);
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
