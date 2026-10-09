use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::get_gallery::GalleryItemSummary;
use crate::application::port::get_gallery::GetGalleryCommand;
use crate::application::port::get_gallery::GetGalleryError;
use crate::application::port::get_gallery::GetGalleryResponse;
use crate::application::port::get_gallery::GetGalleryUseCase;
use crate::application::security_event;
use crate::application::use_case::library_access;
use crate::domain::port::gallery_repository::GalleryFilter;
use crate::domain::port::gallery_repository::GalleryItemFilter;
use crate::domain::port::gallery_repository::GalleryRepository as _;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;

/// Use case implementation for fetching one gallery and its items.
pub struct GetGallery<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> GetGallery<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> GetGalleryUseCase for GetGallery<F>
where
    F: UnitOfWorkFactory,
    F::Uow: LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: GetGalleryCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetGalleryResponse, GetGalleryError>> + Send + 'future>>
    {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| GetGalleryError::Unknown(error.into()))?;

            let result = async {
                let caller = library_access::load_caller(&mut unit_of_work, command.caller_id())
                    .await
                    .map_err(|error| GetGalleryError::Unknown(error.into()))?
                    .ok_or(GetGalleryError::NoSuchCaller)?;

                let gallery = unit_of_work
                    .galleries()
                    .search(&GalleryFilter {
                        id: Some(command.gallery_id()),
                        ..GalleryFilter::default()
                    })
                    .await
                    .map_err(|error| GetGalleryError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetGalleryError::NoSuchGallery)?;

                if !library_access::can_read_gallery(&gallery, &caller) {
                    security_event::authorization_failed(caller.id(), "read", "gallery");
                    return Err(GetGalleryError::Forbidden);
                }

                let items = unit_of_work
                    .galleries()
                    .search_items(&GalleryItemFilter {
                        gallery_id: Some(command.gallery_id()),
                        ..GalleryItemFilter::default()
                    })
                    .await
                    .map_err(|error| GetGalleryError::Unknown(error.into()))?
                    .into_iter()
                    .map(|detail| {
                        let item = detail.item();
                        GalleryItemSummary::new(
                            item.gallery_item_id(),
                            item.gallery_id(),
                            item.media(),
                            detail.file_id(),
                            detail.name().to_owned(),
                            item.item_index(),
                            item.added_at(),
                        )
                    })
                    .collect();

                Ok(GetGalleryResponse::new(
                    gallery.gallery_id(),
                    gallery.user_id(),
                    gallery.name().to_owned(),
                    gallery.is_public(),
                    gallery.created_at(),
                    gallery.last_modified_at(),
                    items,
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| GetGalleryError::Unknown(error.into()))?;
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

    use crate::application::port::get_gallery::GalleryItemSummary;
    use crate::application::port::get_gallery::GetGalleryCommand;
    use crate::application::port::get_gallery::GetGalleryError;
    use crate::application::port::get_gallery::GetGalleryUseCase as _;
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

    use super::GetGallery;

    type UseCase = GetGallery<TestFactory>;

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
    ) -> Result<GalleryItemDetail, Box<dyn Error>> {
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
    async fn get_gallery_owner_reads_private_gallery_with_items() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        let details = vec![item_detail(1, 3)?, item_detail(2, 3)?];
        galleries
            .expect_search_items()
            .times(1)
            .return_once(move |_| {
                let found = details.clone();
                Box::pin(async move { Ok(found) })
            });
        let harness = harness_with(users, galleries);
        let use_case: UseCase = GetGallery::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(GetGalleryCommand::new(4, 3)).await?;

        // Assert
        assert_eq!(response.gallery_id(), 3);
        assert_eq!(response.item_count(), 2);
        assert_eq!(
            response.items().first(),
            Some(&GalleryItemSummary::new(
                1,
                3,
                GalleryItemMedia::Image(7),
                100,
                "item1".to_owned(),
                1,
                NaiveDateTime::default(),
            ))
        );
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_any_caller_reads_public_gallery() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(2, Some(3), "Public", true)?);
        galleries
            .expect_search_items()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, galleries);
        let use_case: UseCase = GetGallery::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(GetGalleryCommand::new(9, 2)).await?;

        // Assert
        assert_eq!(response.gallery_id(), 2);
        assert!(response.items().is_empty());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_root_admin_reads_private_gallery() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 0, Role::Admin)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        galleries
            .expect_search_items()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, galleries);
        let use_case: UseCase = GetGallery::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(GetGalleryCommand::new(0, 3)).await?;

        // Assert
        assert_eq!(response.gallery_id(), 3);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_non_root_admin_private_gallery_returns_forbidden()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Admin)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        galleries.expect_search_items().times(0);
        let harness = harness_with(users, galleries);
        let use_case: UseCase = GetGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetGalleryCommand::new(1, 3)).await;

        // Assert
        assert!(matches!(result, Err(GetGalleryError::Forbidden)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_other_caller_private_gallery_returns_forbidden()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        galleries.expect_search_items().times(0);
        let harness = harness_with(users, galleries);
        let use_case: UseCase = GetGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetGalleryCommand::new(9, 3)).await;

        // Assert
        assert!(matches!(result, Err(GetGalleryError::Forbidden)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_unknown_gallery_returns_no_such_gallery() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        galleries
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, galleries);
        let use_case: UseCase = GetGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetGalleryCommand::new(1, 999)).await;

        // Assert
        assert!(matches!(result, Err(GetGalleryError::NoSuchGallery)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, MockGalleryRepository::new());
        let use_case: UseCase = GetGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetGalleryCommand::new(999, 1)).await;

        // Assert
        assert!(matches!(result, Err(GetGalleryError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        galleries
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, galleries);
        let use_case: UseCase = GetGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetGalleryCommand::new(1, 2)).await;

        // Assert
        assert!(matches!(result, Err(GetGalleryError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
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
        harness.commit_fails.store(true, Ordering::SeqCst);
        let use_case: UseCase = GetGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetGalleryCommand::new(4, 3)).await;

        // Assert
        assert!(matches!(result, Err(GetGalleryError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }
}
