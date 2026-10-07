use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::delete_gallery::DeleteGalleryCommand;
use crate::application::port::delete_gallery::DeleteGalleryError;
use crate::application::port::delete_gallery::DeleteGalleryUseCase;
use crate::application::security_event;
use crate::application::use_case::library_access;
use crate::domain::model::gallery_item::GalleryItemMedia;
use crate::domain::port::gallery_repository::GalleryFilter;
use crate::domain::port::gallery_repository::GalleryItemFilter;
use crate::domain::port::gallery_repository::GalleryRepository as _;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::media_repository::MediaRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;

/// Use case implementation for deleting a gallery and its items.
pub struct DeleteGallery<F> {
    /// Factory opening the unit of work wrapping the deletion.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> DeleteGallery<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> DeleteGalleryUseCase for DeleteGallery<F>
where
    F: UnitOfWorkFactory,
    F::Uow: LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: DeleteGalleryCommand,
    ) -> Pin<Box<dyn Future<Output = Result<(), DeleteGalleryError>> + Send + 'future>> {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| DeleteGalleryError::Unknown(error.into()))?;

            let result = async {
                let caller = library_access::load_caller(&mut unit_of_work, command.caller_id())
                    .await
                    .map_err(|error| DeleteGalleryError::Unknown(error.into()))?
                    .ok_or(DeleteGalleryError::NoSuchCaller)?;

                let gallery = unit_of_work
                    .galleries()
                    .search(&GalleryFilter {
                        id: Some(command.gallery_id()),
                        ..GalleryFilter::default()
                    })
                    .await
                    .map_err(|error| DeleteGalleryError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(DeleteGalleryError::NoSuchGallery)?;

                if library_access::is_default_gallery(&gallery) {
                    return Err(DeleteGalleryError::DefaultGallery);
                }
                if !library_access::can_delete_gallery(&gallery, &caller) {
                    security_event::authorization_failed(caller.id(), "delete", "gallery");
                    return Err(DeleteGalleryError::Forbidden);
                }

                let items = unit_of_work
                    .galleries()
                    .search_items(&GalleryItemFilter {
                        gallery_id: Some(command.gallery_id()),
                        ..GalleryItemFilter::default()
                    })
                    .await
                    .map_err(|error| DeleteGalleryError::Unknown(error.into()))?;
                for detail in items {
                    unit_of_work
                        .galleries()
                        .delete_item(detail.item().gallery_item_id())
                        .await
                        .map_err(|error| DeleteGalleryError::Unknown(error.into()))?;
                    let deleted = match detail.item().media() {
                        GalleryItemMedia::Image(image_id) => {
                            unit_of_work.media().delete_image(image_id).await
                        }
                        GalleryItemMedia::Video(video_id) => {
                            unit_of_work.media().delete_video(video_id).await
                        }
                    }
                    .map_err(|error| DeleteGalleryError::Unknown(error.into()))?;
                    if !deleted {
                        // The medium vanished between the item read and the
                        // delete; the destructive delete has still succeeded.
                    }
                }

                let deleted = unit_of_work
                    .galleries()
                    .delete(command.gallery_id())
                    .await
                    .map_err(|error| DeleteGalleryError::Unknown(error.into()))?;
                if !deleted {
                    return Err(DeleteGalleryError::NoSuchGallery);
                }

                Ok(())
            }
            .await;

            match result {
                Ok(()) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| DeleteGalleryError::Unknown(error.into()))?;
                    Ok(())
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

    use crate::application::port::delete_gallery::DeleteGalleryCommand;
    use crate::application::port::delete_gallery::DeleteGalleryError;
    use crate::application::port::delete_gallery::DeleteGalleryUseCase as _;
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

    use super::DeleteGallery;

    type UseCase = DeleteGallery<TestFactory>;

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
        media: GalleryItemMedia,
    ) -> Result<GalleryItemDetail, Box<dyn Error>> {
        let (image_id, video_id) = match media {
            GalleryItemMedia::Image(id) => (Some(id), None),
            GalleryItemMedia::Video(id) => (None, Some(id)),
        };
        Ok(GalleryItemDetail::new(
            GalleryItem::try_new(
                gallery_item_id,
                gallery_id,
                image_id,
                video_id,
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
        media: MockMediaRepository,
    ) -> GalleryFactoryHarness {
        gallery_factory(galleries, media, users, MockFileRepository::new())
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
    async fn delete_gallery_owner_deletes_items_media_and_gallery() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        let details = vec![
            item_detail(1, 3, GalleryItemMedia::Image(7))?,
            item_detail(2, 3, GalleryItemMedia::Video(8))?,
        ];
        galleries
            .expect_search_items()
            .times(1)
            .return_once(move |_| {
                let found = details.clone();
                Box::pin(async move { Ok(found) })
            });
        galleries
            .expect_delete_item()
            .times(2)
            .returning(|_| Box::pin(async { Ok(true) }));
        galleries
            .expect_delete()
            .times(1)
            .withf(|id| *id == 3)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut media = MockMediaRepository::new();
        media
            .expect_delete_image()
            .times(1)
            .withf(|id| *id == 7)
            .returning(|_| Box::pin(async { Ok(true) }));
        media
            .expect_delete_video()
            .times(1)
            .withf(|id| *id == 8)
            .returning(|_| Box::pin(async { Ok(true) }));
        let harness = harness_with(users, galleries, media);
        let use_case: UseCase = DeleteGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(DeleteGalleryCommand::new(4, 3)).await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_default_gallery_returns_default_gallery() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Admin)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(0, None, "Default", true)?);
        galleries.expect_search_items().times(0);
        galleries.expect_delete().times(0);
        let harness = harness_with(users, galleries, MockMediaRepository::new());
        let use_case: UseCase = DeleteGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(DeleteGalleryCommand::new(1, 0)).await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryError::DefaultGallery)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_admin_not_owner_returns_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Admin)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        galleries.expect_delete().times(0);
        let harness = harness_with(users, galleries, MockMediaRepository::new());
        let use_case: UseCase = DeleteGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(DeleteGalleryCommand::new(1, 3)).await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryError::Forbidden)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_unknown_gallery_returns_no_such_gallery() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        galleries
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, galleries, MockMediaRepository::new());
        let use_case: UseCase = DeleteGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(DeleteGalleryCommand::new(4, 999)).await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryError::NoSuchGallery)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(
            users,
            MockGalleryRepository::new(),
            MockMediaRepository::new(),
        );
        let use_case: UseCase = DeleteGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(DeleteGalleryCommand::new(999, 3)).await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_dependency_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        galleries
            .expect_search_items()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, galleries, MockMediaRepository::new());
        let use_case: UseCase = DeleteGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(DeleteGalleryCommand::new(4, 3)).await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        galleries
            .expect_search_items()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        galleries
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let harness = harness_with(users, galleries, MockMediaRepository::new());
        harness.commit_fails.store(true, Ordering::SeqCst);
        let use_case: UseCase = DeleteGallery::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(DeleteGalleryCommand::new(4, 3)).await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }
}
