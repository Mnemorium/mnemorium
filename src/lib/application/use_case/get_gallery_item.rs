use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::get_gallery_item::GetGalleryItemCommand;
use crate::application::port::get_gallery_item::GetGalleryItemError;
use crate::application::port::get_gallery_item::GetGalleryItemResponse;
use crate::application::port::get_gallery_item::GetGalleryItemUseCase;
use crate::application::security_event;
use crate::application::use_case::library_access;
use crate::domain::port::gallery_repository::GalleryFilter;
use crate::domain::port::gallery_repository::GalleryItemFilter;
use crate::domain::port::gallery_repository::GalleryRepository as _;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;

/// Use case implementation for fetching one item of a gallery.
pub struct GetGalleryItem<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> GetGalleryItem<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> GetGalleryItemUseCase for GetGalleryItem<F>
where
    F: UnitOfWorkFactory,
    F::Uow: LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: GetGalleryItemCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<GetGalleryItemResponse, GetGalleryItemError>>
                + Send
                + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| GetGalleryItemError::Unknown(error.into()))?;

            let result = async {
                let caller = library_access::load_caller(&mut unit_of_work, command.caller_id())
                    .await
                    .map_err(|error| GetGalleryItemError::Unknown(error.into()))?
                    .ok_or(GetGalleryItemError::NoSuchCaller)?;

                let gallery = unit_of_work
                    .galleries()
                    .search(&GalleryFilter {
                        id: Some(command.gallery_id()),
                        ..GalleryFilter::default()
                    })
                    .await
                    .map_err(|error| GetGalleryItemError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetGalleryItemError::NoSuchGallery)?;

                if !library_access::can_read_gallery(&gallery, &caller) {
                    security_event::authorization_failed(caller.id(), "read", "gallery_item");
                    return Err(GetGalleryItemError::Forbidden);
                }

                let detail = unit_of_work
                    .galleries()
                    .search_items(&GalleryItemFilter {
                        gallery_id: Some(command.gallery_id()),
                        id: Some(command.gallery_item_id()),
                        ..GalleryItemFilter::default()
                    })
                    .await
                    .map_err(|error| GetGalleryItemError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetGalleryItemError::NoSuchItem)?;
                let item = detail.item();

                Ok(GetGalleryItemResponse::new(
                    item.gallery_item_id(),
                    item.gallery_id(),
                    item.media(),
                    detail.file_id(),
                    detail.name().to_owned(),
                    item.item_index(),
                    item.added_at(),
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| GetGalleryItemError::Unknown(error.into()))?;
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

    use crate::application::port::get_gallery_item::GetGalleryItemCommand;
    use crate::application::port::get_gallery_item::GetGalleryItemError;
    use crate::application::port::get_gallery_item::GetGalleryItemResponse;
    use crate::application::port::get_gallery_item::GetGalleryItemUseCase as _;
    use crate::domain::model::gallery::Gallery;
    use crate::domain::model::gallery_item::GalleryItem;
    use crate::domain::model::gallery_item::GalleryItemMedia;
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

    use super::GetGalleryItem;

    type UseCase = GetGalleryItem<TestFactory>;

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

    fn detail(gallery_item_id: i64, gallery_id: i64) -> Result<GalleryItemDetail, Box<dyn Error>> {
        Ok(GalleryItemDetail::new(
            GalleryItem::try_new(
                gallery_item_id,
                gallery_id,
                Some(7),
                None,
                gallery_item_id,
                NaiveDateTime::default(),
            )?,
            100,
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

    fn expect_gallery(galleries: &mut MockGalleryRepository, gallery: Gallery) {
        let gallery_id = gallery.gallery_id();
        galleries
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(gallery_id))
            .return_once(move |_| Box::pin(async move { Ok(vec![gallery]) }));
    }

    #[tokio::test]
    async fn get_gallery_item_owner_reads_item() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        let found = detail(42, 3)?;
        galleries
            .expect_search_items()
            .times(1)
            .withf(|filter| filter.gallery_id == Some(3) && filter.id == Some(42))
            .return_once(move |_| {
                let value = found.clone();
                Box::pin(async move { Ok(vec![value]) })
            });
        let harness = harness_with(users, galleries);
        let use_case: UseCase = GetGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(GetGalleryItemCommand::new(4, 3, 42))
            .await?;

        // Assert
        assert_eq!(
            response,
            GetGalleryItemResponse::new(
                42,
                3,
                GalleryItemMedia::Image(7),
                100,
                "item42".to_owned(),
                42,
                NaiveDateTime::default(),
            )
        );
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_item_any_caller_reads_public_item() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(2, Some(3), "Public", true)?);
        let found = detail(42, 2)?;
        galleries
            .expect_search_items()
            .times(1)
            .return_once(move |_| {
                let value = found.clone();
                Box::pin(async move { Ok(vec![value]) })
            });
        let harness = harness_with(users, galleries);
        let use_case: UseCase = GetGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(GetGalleryItemCommand::new(9, 2, 42))
            .await?;

        // Assert
        assert_eq!(response.gallery_item_id(), 42);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_item_root_admin_reads_private_item() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 0, Role::Admin)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        let found = detail(42, 3)?;
        galleries
            .expect_search_items()
            .times(1)
            .return_once(move |_| {
                let value = found.clone();
                Box::pin(async move { Ok(vec![value]) })
            });
        let harness = harness_with(users, galleries);
        let use_case: UseCase = GetGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(GetGalleryItemCommand::new(0, 3, 42))
            .await?;

        // Assert
        assert_eq!(response.gallery_item_id(), 42);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_item_non_root_admin_private_item_returns_forbidden()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Admin)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        galleries.expect_search_items().times(0);
        let harness = harness_with(users, galleries);
        let use_case: UseCase = GetGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetGalleryItemCommand::new(1, 3, 42)).await;

        // Assert
        assert!(matches!(result, Err(GetGalleryItemError::Forbidden)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_item_other_caller_private_gallery_returns_forbidden()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        galleries.expect_search_items().times(0);
        let harness = harness_with(users, galleries);
        let use_case: UseCase = GetGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetGalleryItemCommand::new(9, 3, 42)).await;

        // Assert
        assert!(matches!(result, Err(GetGalleryItemError::Forbidden)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_item_unknown_item_returns_no_such_item() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        galleries
            .expect_search_items()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, galleries);
        let use_case: UseCase = GetGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetGalleryItemCommand::new(4, 3, 42)).await;

        // Assert
        assert!(matches!(result, Err(GetGalleryItemError::NoSuchItem)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_item_unknown_gallery_returns_no_such_gallery() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        galleries
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, galleries);
        let use_case: UseCase = GetGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(GetGalleryItemCommand::new(4, 999, 42))
            .await;

        // Assert
        assert!(matches!(result, Err(GetGalleryItemError::NoSuchGallery)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_item_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, MockGalleryRepository::new());
        let use_case: UseCase = GetGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(GetGalleryItemCommand::new(999, 3, 42))
            .await;

        // Assert
        assert!(matches!(result, Err(GetGalleryItemError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_item_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        galleries
            .expect_search_items()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, galleries);
        let use_case: UseCase = GetGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetGalleryItemCommand::new(4, 3, 42)).await;

        // Assert
        assert!(matches!(result, Err(GetGalleryItemError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_item_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        let found = detail(42, 3)?;
        galleries
            .expect_search_items()
            .times(1)
            .return_once(move |_| {
                let value = found.clone();
                Box::pin(async move { Ok(vec![value]) })
            });
        let harness = harness_with(users, galleries);
        harness.commit_fails.store(true, Ordering::SeqCst);
        let use_case: UseCase = GetGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetGalleryItemCommand::new(4, 3, 42)).await;

        // Assert
        assert!(matches!(result, Err(GetGalleryItemError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }
}
