use crate::application::authorization::authorize;
use crate::domain::model::{ResourceKind, ResourceRef};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::{
    GalleryItemSummary, GetGalleryCommand, GetGalleryError, GetGalleryResponse, GetGalleryUseCase,
};
use crate::domain::model::Action;
use crate::domain::port::{
    GalleryFilter, GalleryItemFilter, GalleryRepository as _, IdentityUnitOfWork,
    LibraryUnitOfWork, UnitOfWork as _, UnitOfWorkFactory, UserFilter, UserRepository as _,
    UserUnitOfWork,
};

/// Use case implementation for fetching one gallery and its items.
pub(crate) struct GetGallery<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> GetGallery<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> GetGalleryUseCase for GetGallery<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
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
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| GetGalleryError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetGalleryError::NoSuchCaller)?;

                let gallery = unit_of_work
                    .galleries()
                    .search(&GalleryFilter {
                        gallery_id: Some(command.gallery_id()),
                        ..GalleryFilter::default()
                    })
                    .await
                    .map_err(|error| GetGalleryError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetGalleryError::NoSuchGallery)?;

                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::Gallery, gallery.gallery_id()),
                    Action::View,
                    None,
                )
                .await
                .map_err(|_| GetGalleryError::NoSuchGallery)?;

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
    use crate::domain::model::{Gallery, GalleryItem, GalleryItemMedia, Role, User};
    use allure_cargotest::allure_test;

    use crate::domain::port::{
        GalleryItemDetail, MockGalleryRepository, MockUserRepository, RepositoryError,
    };

    use crate::test_helpers::*;
    use chrono::NaiveDateTime;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    type UseCase = GetGallery<TestFactory>;

    fn gallery(gallery_id: i64, user_id: Option<i64>, name: &str, is_public: bool) -> Gallery {
        let now = NaiveDateTime::default();
        Gallery::new(gallery_id, user_id, name.to_owned(), is_public, now, now)
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
        let unit_of_work = TestUnitOfWork {
            resources: no_resources(),
            galleries,
            users,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
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
            .withf(move |filter| filter.gallery_id == Some(gallery_id))
            .return_once(move |_| Box::pin(async move { Ok(vec![gallery]) }));
    }

    #[allure_test]
    #[tokio::test]
    async fn get_gallery_owner_reads_private_gallery_with_items() -> Result<(), Box<dyn Error>> {
        allure.epic("Gallery");
        allure.feature("REQ-GALLERY-005");
        allure.story("SCN-GALLERY-005-A");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
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

    #[allure_test]
    #[tokio::test]
    async fn get_gallery_root_admin_reads_private_gallery() -> Result<(), Box<dyn Error>> {
        allure.epic("Gallery");
        allure.feature("REQ-GALLERY-005");
        allure.story("SCN-GALLERY-005-A");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 0, Role::Admin)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
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
    #[allure_test]
    #[tokio::test]
    async fn get_gallery_unknown_gallery_returns_no_such_gallery() -> Result<(), Box<dyn Error>> {
        allure.epic("Gallery");
        allure.feature("REQ-GALLERY-005");
        allure.story("SCN-GALLERY-005-B");
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

    #[allure_test]
    #[tokio::test]
    async fn get_gallery_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        allure.epic("Gallery");
        allure.feature("REQ-GALLERY-005");
        allure.story("SCN-GALLERY-005-A");
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

    #[allure_test]
    #[tokio::test]
    async fn get_gallery_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        allure.epic("Gallery");
        allure.feature("REQ-GALLERY-005");
        allure.story("SCN-GALLERY-005-A");
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

    #[allure_test]
    #[tokio::test]
    async fn get_gallery_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        allure.epic("Gallery");
        allure.feature("REQ-GALLERY-005");
        allure.story("SCN-GALLERY-005-A");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false));
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
