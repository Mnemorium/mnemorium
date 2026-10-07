use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use crate::application::port::delete_gallery_item::DeleteGalleryItemCommand;
use crate::application::port::delete_gallery_item::DeleteGalleryItemError;
use crate::application::port::delete_gallery_item::DeleteGalleryItemUseCase;
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

/// Use case implementation for deleting one item of a gallery.
pub struct DeleteGalleryItem<F> {
    /// Factory opening the unit of work wrapping the deletion.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> DeleteGalleryItem<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> DeleteGalleryItemUseCase for DeleteGalleryItem<F>
where
    F: UnitOfWorkFactory,
    F::Uow: LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: DeleteGalleryItemCommand,
    ) -> Pin<Box<dyn Future<Output = Result<(), DeleteGalleryItemError>> + Send + 'future>> {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| DeleteGalleryItemError::Unknown(error.into()))?;

            let result = async {
                let caller = library_access::load_caller(&mut unit_of_work, command.caller_id())
                    .await
                    .map_err(|error| DeleteGalleryItemError::Unknown(error.into()))?
                    .ok_or(DeleteGalleryItemError::NoSuchCaller)?;

                let mut gallery = unit_of_work
                    .galleries()
                    .search(&GalleryFilter {
                        id: Some(command.gallery_id()),
                        ..GalleryFilter::default()
                    })
                    .await
                    .map_err(|error| DeleteGalleryItemError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(DeleteGalleryItemError::NoSuchGallery)?;

                if !library_access::can_delete_item(&gallery, &caller) {
                    security_event::authorization_failed(caller.id(), "delete", "gallery_item");
                    return Err(DeleteGalleryItemError::Forbidden);
                }

                let detail = unit_of_work
                    .galleries()
                    .search_items(&GalleryItemFilter {
                        gallery_id: Some(command.gallery_id()),
                        id: Some(command.gallery_item_id()),
                        ..GalleryItemFilter::default()
                    })
                    .await
                    .map_err(|error| DeleteGalleryItemError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(DeleteGalleryItemError::NoSuchItem)?;
                let media = detail.item().media();

                let deleted = unit_of_work
                    .galleries()
                    .delete_item(detail.item().gallery_item_id())
                    .await
                    .map_err(|error| DeleteGalleryItemError::Unknown(error.into()))?;
                if !deleted {
                    return Err(DeleteGalleryItemError::NoSuchItem);
                }
                let deleted_media = match media {
                    GalleryItemMedia::Image(image_id) => {
                        unit_of_work.media().delete_image(image_id).await
                    }
                    GalleryItemMedia::Video(video_id) => {
                        unit_of_work.media().delete_video(video_id).await
                    }
                }
                .map_err(|error| DeleteGalleryItemError::Unknown(error.into()))?;
                if !deleted_media {
                    // The medium vanished between the item read and the delete;
                    // the destructive delete has still succeeded.
                }

                gallery.touch(Utc::now().naive_utc());
                unit_of_work
                    .galleries()
                    .save(gallery)
                    .await
                    .map_err(|error| DeleteGalleryItemError::Unknown(error.into()))?;

                Ok(())
            }
            .await;

            match result {
                Ok(()) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| DeleteGalleryItemError::Unknown(error.into()))?;
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

    use crate::application::port::delete_gallery_item::DeleteGalleryItemCommand;
    use crate::application::port::delete_gallery_item::DeleteGalleryItemError;
    use crate::application::port::delete_gallery_item::DeleteGalleryItemUseCase as _;
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

    use super::DeleteGalleryItem;

    type UseCase = DeleteGalleryItem<TestFactory>;

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

    fn detail(
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
    async fn delete_gallery_item_owner_deletes_image_and_item() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        let found = detail(42, 3, GalleryItemMedia::Image(7))?;
        galleries
            .expect_search_items()
            .times(1)
            .withf(|filter| filter.gallery_id == Some(3) && filter.id == Some(42))
            .return_once(move |_| {
                let value = found.clone();
                Box::pin(async move { Ok(vec![value]) })
            });
        galleries
            .expect_delete_item()
            .times(1)
            .withf(|id| *id == 42)
            .returning(|_| Box::pin(async { Ok(true) }));
        galleries
            .expect_save()
            .times(1)
            .withf(|saved| saved.last_modified_at() > NaiveDateTime::default())
            .returning(|saved| Box::pin(async move { Ok(saved) }));
        let mut media = MockMediaRepository::new();
        media
            .expect_delete_image()
            .times(1)
            .withf(|id| *id == 7)
            .returning(|_| Box::pin(async { Ok(true) }));
        let harness = harness_with(users, galleries, media);
        let use_case: UseCase = DeleteGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(DeleteGalleryItemCommand::new(4, 3, 42))
            .await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_any_caller_deletes_public_item() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(2, Some(3), "Public", true)?);
        let found = detail(42, 2, GalleryItemMedia::Video(8))?;
        galleries
            .expect_search_items()
            .times(1)
            .return_once(move |_| {
                let value = found.clone();
                Box::pin(async move { Ok(vec![value]) })
            });
        galleries
            .expect_delete_item()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        galleries
            .expect_save()
            .times(1)
            .returning(|saved| Box::pin(async move { Ok(saved) }));
        let mut media = MockMediaRepository::new();
        media
            .expect_delete_video()
            .times(1)
            .withf(|id| *id == 8)
            .returning(|_| Box::pin(async { Ok(true) }));
        let harness = harness_with(users, galleries, media);
        let use_case: UseCase = DeleteGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(DeleteGalleryItemCommand::new(9, 2, 42))
            .await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_other_private_caller_returns_forbidden()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        galleries.expect_delete_item().times(0);
        let harness = harness_with(users, galleries, MockMediaRepository::new());
        let use_case: UseCase = DeleteGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(DeleteGalleryItemCommand::new(9, 3, 42))
            .await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryItemError::Forbidden)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_unknown_item_returns_no_such_item() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        galleries
            .expect_search_items()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        galleries.expect_delete_item().times(0);
        let harness = harness_with(users, galleries, MockMediaRepository::new());
        let use_case: UseCase = DeleteGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(DeleteGalleryItemCommand::new(4, 3, 42))
            .await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryItemError::NoSuchItem)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_unknown_gallery_returns_no_such_gallery()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        galleries
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, galleries, MockMediaRepository::new());
        let use_case: UseCase = DeleteGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(DeleteGalleryItemCommand::new(4, 999, 42))
            .await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryItemError::NoSuchGallery)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_unknown_caller_returns_no_such_caller()
    -> Result<(), Box<dyn Error>> {
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
        let use_case: UseCase = DeleteGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(DeleteGalleryItemCommand::new(999, 3, 42))
            .await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryItemError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>>
    {
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
        let use_case: UseCase = DeleteGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(DeleteGalleryItemCommand::new(4, 3, 42))
            .await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryItemError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut galleries = MockGalleryRepository::new();
        expect_gallery(&mut galleries, gallery(3, Some(4), "Mine", false)?);
        let found = detail(42, 3, GalleryItemMedia::Image(7))?;
        galleries
            .expect_search_items()
            .times(1)
            .return_once(move |_| {
                let value = found.clone();
                Box::pin(async move { Ok(vec![value]) })
            });
        galleries
            .expect_delete_item()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        galleries
            .expect_save()
            .times(1)
            .returning(|saved| Box::pin(async move { Ok(saved) }));
        let mut media = MockMediaRepository::new();
        media
            .expect_delete_image()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let harness = harness_with(users, galleries, media);
        harness.commit_fails.store(true, Ordering::SeqCst);
        let use_case: UseCase = DeleteGalleryItem::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(DeleteGalleryItemCommand::new(4, 3, 42))
            .await;

        // Assert
        assert!(matches!(result, Err(DeleteGalleryItemError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }
}
