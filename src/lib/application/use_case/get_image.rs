use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::{GetImageCommand, GetImageError, GetImageResponse, GetImageUseCase};
use crate::application::security_event;
use crate::domain::port::{
    AssetUnitOfWork, ContentLinkProvider, FileFilter, FileRepository as _, IdentityUnitOfWork,
    ImageFilter, ImageRepository as _, LibraryUnitOfWork, PermissionFilter,
    PermissionRepository as _, UnitOfWork as _, UnitOfWorkFactory, UserFilter, UserRepository as _,
    UserUnitOfWork,
};
use crate::domain::service::{Authorization, Decision};

/// Use case implementation for fetching one typed image.
pub(crate) struct GetImage<F, L> {
    /// Adapter minting the playback credential.
    content_link: Arc<L>,
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, L: ContentLinkProvider> GetImage<F, L> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>, content_link: Arc<L>) -> Self {
        Self {
            content_link,
            unit_of_work_factory,
        }
    }
}

impl<F, L> GetImageUseCase for GetImage<F, L>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork + IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
    L: ContentLinkProvider,
{
    fn execute<'future>(
        &'future self,
        command: GetImageCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetImageResponse, GetImageError>> + Send + 'future>>
    {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);
        let content_link = Arc::clone(&self.content_link);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| GetImageError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| GetImageError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetImageError::NoSuchCaller)?;
                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| GetImageError::Unknown(error.into()))?;
                let image = unit_of_work
                    .images()
                    .search(&ImageFilter {
                        image_id: Some(command.image_id()),
                        ..ImageFilter::default()
                    })
                    .await
                    .map_err(|error| GetImageError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetImageError::NoSuchImage)?;
                let file = unit_of_work
                    .files()
                    .search(&FileFilter {
                        id: Some(image.file_id()),
                        ..FileFilter::default()
                    })
                    .await
                    .map_err(|error| GetImageError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetImageError::NoSuchImage)?;

                if let Decision::Deny(denial) =
                    Authorization::decide_asset(&caller, &permissions, &file)
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(GetImageError::NoSuchImage);
                }

                let link = content_link
                    .mint(image.file_id())
                    .await
                    .map_err(|error| GetImageError::Unknown(error.into()))?;
                Ok(GetImageResponse::new(
                    image.image_id(),
                    image.name().to_owned(),
                    image.width_px(),
                    image.height_px(),
                    image.orientation(),
                    image.created_at(),
                    image.file_id(),
                    link,
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| GetImageError::Unknown(error.into()))?;
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
    use crate::domain::NumericID;

    use crate::domain::model::{
        AssetOwner, AssetScope, File, Image, Orientation, ProtectedResource, ResourceKind,
        ResourceRef,
    };

    use crate::domain::port::{
        ContentLink, MockContentLinkProvider, MockFileRepository, MockImageRepository,
        MockUserRepository, RepositoryError,
    };

    use crate::test_helpers::*;
    use chrono::{NaiveDate, NaiveDateTime};
    use std::error::Error;
    use std::sync::atomic::Ordering;

    type UseCase = GetImage<TestFactory, MockContentLinkProvider>;

    /// A signer accepting any number of mints.
    fn signer() -> MockContentLinkProvider {
        let mut signer = MockContentLinkProvider::new();
        signer.expect_mint().times(0..).returning(|_| {
            Box::pin(async { Ok(ContentLink::new(4_102_444_800, "signature".to_owned())) })
        });
        signer
    }

    fn file(
        id: NumericID,
        owner_id: NumericID,
        owner: Option<AssetOwner>,
    ) -> Result<File, Box<dyn Error>> {
        Ok(File::try_new(
            id,
            format!("{id}.png"),
            owner_id,
            false,
            "image/png".to_owned(),
            NaiveDate::default(),
            10,
            NaiveDateTime::default(),
            None,
            None,
        )?
        .with_owner(owner))
    }

    fn resource(
        kind: ResourceKind,
        id: NumericID,
        owner_id: Option<NumericID>,
        is_public: bool,
    ) -> AssetOwner {
        AssetOwner::Resource(ProtectedResource::new(
            ResourceRef::new(kind, id),
            owner_id,
            is_public,
        ))
    }

    fn image(
        id: NumericID,
        file_id: NumericID,
        scope: Option<AssetScope>,
    ) -> Result<Image, Box<dyn Error>> {
        Ok(Image::try_new(
            id,
            format!("{id}.png"),
            10,
            20,
            Orientation::Portrait,
            NaiveDate::default(),
            file_id,
            scope,
        )?)
    }

    fn harness(
        users: MockUserRepository,
        files: MockFileRepository,
        images: MockImageRepository,
    ) -> AssetFactoryHarness {
        AssetFactoryHarness::new(TestUnitOfWork {
            files,
            images,
            permissions: no_permissions(),
            users,
            ..TestUnitOfWork::default()
        })
    }

    fn expect_image(images: &mut MockImageRepository, stored: Image) {
        images
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored]) }));
    }

    fn expect_file(files: &mut MockFileRepository, stored: File) {
        files
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored]) }));
    }

    #[tokio::test]
    async fn get_image_projects_the_typed_representation() -> Result<(), Box<dyn Error>> {
        // Arrange: a library image readable through `library.read`.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9)?;
        let mut images = MockImageRepository::new();
        expect_image(&mut images, image(19, 100, Some(AssetScope::Library))?);
        let mut files = MockFileRepository::new();
        expect_file(&mut files, file(100, 3, Some(AssetOwner::Library))?);
        let harness = harness(users, files, images);
        let use_case: UseCase = GetImage::new(Arc::clone(&harness.factory), Arc::new(signer()));

        // Act
        let response = use_case.execute(GetImageCommand::new(9, 19)).await?;

        // Assert
        assert_eq!(response.id(), 19);
        assert_eq!(response.file_id(), 100);
        assert_eq!(response.width_px(), 10);
        assert_eq!(response.height_px(), 20);
        assert_eq!(response.orientation(), Orientation::Portrait);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_image_unknown_image_returns_no_such_image() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9)?;
        let mut images = MockImageRepository::new();
        images
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness(users, MockFileRepository::new(), images);
        let use_case: UseCase = GetImage::new(Arc::clone(&harness.factory), Arc::new(signer()));

        // Act
        let result = use_case.execute(GetImageCommand::new(9, 999)).await;

        // Assert
        assert!(matches!(result, Err(GetImageError::NoSuchImage)));
        Ok(())
    }

    #[tokio::test]
    async fn get_image_of_a_private_gallery_is_concealed_from_a_non_owner()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9)?;
        let mut images = MockImageRepository::new();
        expect_image(&mut images, image(19, 100, Some(AssetScope::User))?);
        let mut files = MockFileRepository::new();
        expect_file(
            &mut files,
            file(
                100,
                3,
                Some(resource(ResourceKind::Gallery, 2, Some(3), false)),
            )?,
        );
        let harness = harness(users, files, images);
        let use_case: UseCase = GetImage::new(Arc::clone(&harness.factory), Arc::new(signer()));

        // Act
        let result = use_case.execute(GetImageCommand::new(9, 19)).await;

        // Assert
        assert!(matches!(result, Err(GetImageError::NoSuchImage)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_image_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange: the image exists, but its caller does not.
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness(users, MockFileRepository::new(), MockImageRepository::new());
        let use_case: UseCase = GetImage::new(Arc::clone(&harness.factory), Arc::new(signer()));

        // Act
        let result = use_case.execute(GetImageCommand::new(9, 19)).await;

        // Assert
        assert!(matches!(result, Err(GetImageError::NoSuchCaller)));
        Ok(())
    }

    #[tokio::test]
    async fn get_image_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange: the caller load fails before the image resolves.
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness(users, MockFileRepository::new(), MockImageRepository::new());
        let use_case: UseCase = GetImage::new(Arc::clone(&harness.factory), Arc::new(signer()));

        // Act
        let result = use_case.execute(GetImageCommand::new(9, 19)).await;

        // Assert
        assert!(matches!(result, Err(GetImageError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
