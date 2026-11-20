use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::authorization::allows_many;
use crate::application::port::{
    GetFileResponse, ListFilesCommand, ListFilesError, ListFilesResponse, ListFilesUseCase,
};
use crate::domain::model::{Action, ResourceKind, ResourceRef};
use crate::domain::port::{
    AssetUnitOfWork, AudioFilter, AudioRepository as _, FileFilter, FileRepository as _,
    IdentityUnitOfWork, ImageFilter, ImageRepository as _, LibraryUnitOfWork, UnitOfWork as _,
    UnitOfWorkFactory, UserFilter, UserRepository as _, UserUnitOfWork, VideoFilter,
    VideoRepository as _,
};

/// Use case implementation for listing the stored files the caller may discover.
pub(crate) struct ListFiles<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> ListFiles<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> ListFilesUseCase for ListFiles<F>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork + IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: ListFilesCommand,
    ) -> Pin<Box<dyn Future<Output = Result<ListFilesResponse, ListFilesError>> + Send + 'future>>
    {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| ListFilesError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| ListFilesError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(ListFilesError::NoSuchCaller)?;

                let mut files = unit_of_work
                    .files()
                    .search(&FileFilter {
                        path: command.path().map(str::to_owned),
                        ..FileFilter::default()
                    })
                    .await
                    .map_err(|error| ListFilesError::Unknown(error.into()))?;
                // One batched read decides the whole collection: the caller may
                // discover a file only when its `FILE` resource grants it, so a
                // file without a resource is filtered out like a denied one.
                let addresses: Vec<ResourceRef> = files
                    .iter()
                    .map(|file| ResourceRef::new(ResourceKind::File, file.id()))
                    .collect();
                let discoverable = allows_many(
                    &mut unit_of_work.resources(),
                    &caller,
                    &addresses,
                    Action::Discover,
                    None,
                )
                .await;
                let mut visible = Vec::with_capacity(files.len());
                for (file, allowed) in files.drain(..).zip(discoverable) {
                    if allowed {
                        visible.push(file);
                    }
                }
                files = visible;

                let total = files.len();
                let offset = command.offset().unwrap_or(0);
                let page: Vec<_> = files
                    .into_iter()
                    .skip(offset)
                    .take(command.limit().unwrap_or(usize::MAX))
                    .collect();

                // The item is the single-file representation, so it carries the
                // typed medium the file backs exactly as `get_file` reports it.
                let mut items = Vec::with_capacity(page.len());
                for file in page {
                    let image = unit_of_work
                        .images()
                        .search(&ImageFilter {
                            file_id: Some(file.id()),
                            ..ImageFilter::default()
                        })
                        .await
                        .map_err(|error| ListFilesError::Unknown(error.into()))?
                        .into_iter()
                        .next();
                    let audio = unit_of_work
                        .audios()
                        .search(&AudioFilter {
                            file_id: Some(file.id()),
                            ..AudioFilter::default()
                        })
                        .await
                        .map_err(|error| ListFilesError::Unknown(error.into()))?
                        .into_iter()
                        .next();
                    let video = unit_of_work
                        .videos()
                        .search(&VideoFilter {
                            file_id: Some(file.id()),
                            ..VideoFilter::default()
                        })
                        .await
                        .map_err(|error| ListFilesError::Unknown(error.into()))?
                        .into_iter()
                        .next();
                    items.push(GetFileResponse::new(
                        file.id(),
                        file.path().to_owned(),
                        file.mime_type_id().to_owned(),
                        file.size_bytes(),
                        file.uploaded_at(),
                        file.modified_at(),
                        file.origin(),
                        file.is_public(),
                        file.user_id(),
                        image.map(|stored| stored.image_id()),
                        audio.map(|stored| stored.id()),
                        video.map(|stored| stored.id()),
                    ));
                }

                Ok(ListFilesResponse::new(items, total))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| ListFilesError::Unknown(error.into()))?;
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
        ChildKind, File, Grant, GrantEffect, Image, Orientation, Origin, Principal, Resource,
        ResourceAccess, ResourceRole, Role, User, Visibility,
    };
    use crate::domain::port::{
        MockAudioRepository, MockFileRepository, MockImageRepository, MockResourceRepository,
        MockUserRepository, MockVideoRepository, RepositoryError,
    };
    use crate::test_helpers::*;
    use chrono::{NaiveDate, NaiveDateTime};
    use std::error::Error;
    use std::sync::atomic::Ordering;

    type UseCase = ListFiles<TestFactory>;

    /// Build a stored file owned by `owner_id`.
    fn file(id: NumericID, owner_id: NumericID) -> Result<File, Box<dyn Error>> {
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
        )?)
    }

    /// Build a typed image backing `file_id`.
    fn image(id: NumericID, file_id: NumericID) -> Result<Image, Box<dyn Error>> {
        Ok(Image::try_new(
            id,
            format!("{id}.png"),
            10,
            10,
            Orientation::Square,
            NaiveDate::default(),
            file_id,
            None,
        )?)
    }

    /// Build the authenticated caller.
    fn caller(id: NumericID, role: Role) -> Result<User, Box<dyn Error>> {
        Ok(User::try_new(id, "caller".to_owned(), None, id, role)?)
    }

    /// Expect the caller lookup to answer `caller_id`.
    fn expect_caller(
        users: &mut MockUserRepository,
        caller_id: NumericID,
    ) -> Result<(), Box<dyn Error>> {
        let found = caller(caller_id, Role::Standard)?;
        users
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(caller_id))
            .return_once(move |_| Box::pin(async move { Ok(vec![found]) }));
        Ok(())
    }

    /// Build the fixture around the mocked repositories.
    fn harness(
        users: MockUserRepository,
        files: MockFileRepository,
        images: MockImageRepository,
        audios: MockAudioRepository,
        videos: MockVideoRepository,
        resources: MockResourceRepository,
    ) -> AssetFactoryHarness {
        AssetFactoryHarness::new(TestUnitOfWork {
            audios,
            files,
            images,
            resources,
            users,
            videos,
            ..TestUnitOfWork::default()
        })
    }

    /// Expect one typed-media lookup per family for `file_id`, empty by default.
    fn expect_media_lookups(
        images: &mut MockImageRepository,
        audios: &mut MockAudioRepository,
        videos: &mut MockVideoRepository,
        file_id: NumericID,
    ) -> Result<(), Box<dyn Error>> {
        let stored = image(19, file_id)?;
        images
            .expect_search()
            .times(1)
            .withf(move |filter| filter.file_id == Some(file_id))
            .return_once(move |_| Box::pin(async move { Ok(vec![stored]) }));
        audios
            .expect_search()
            .times(1)
            .withf(move |filter| filter.file_id == Some(file_id))
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        videos
            .expect_search()
            .times(1)
            .withf(move |filter| filter.file_id == Some(file_id))
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        Ok(())
    }

    /// Build a `FILE` resource access the caller may discover.
    fn discoverable_file(caller_id: NumericID, file_id: NumericID) -> ResourceAccess {
        let resource = Resource::from_stored(
            1,
            ResourceKind::File,
            file_id,
            Some(caller_id),
            None,
            Visibility::Public,
        );
        let grant = Grant::from_stored(
            1,
            Principal::User(caller_id),
            1,
            ResourceRole::Downloader,
            ChildKind::All,
            GrantEffect::Allow,
        );
        ResourceAccess::new(resource, Vec::new(), vec![grant], Vec::new())
    }

    #[tokio::test]
    async fn list_files_forwards_the_path_filter_and_paginates() -> Result<(), Box<dyn Error>> {
        // Arrange: three files match the collection; the page selects the second.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4)?;
        let mut files = MockFileRepository::new();
        let first = file(1, 4)?;
        let second = file(2, 4)?;
        let third = file(3, 4)?;
        files
            .expect_search()
            .times(1)
            .withf(|filter| filter.path.as_deref() == Some("2.png"))
            .return_once(move |_| {
                let found = vec![first.clone(), second.clone(), third.clone()];
                Box::pin(async move { Ok(found) })
            });
        let mut images = MockImageRepository::new();
        let mut audios = MockAudioRepository::new();
        let mut videos = MockVideoRepository::new();
        expect_media_lookups(&mut images, &mut audios, &mut videos, 2)?;
        let harness = harness(
            users,
            files,
            images,
            audios,
            videos,
            caller_owned_resources(),
        );
        let use_case: UseCase = ListFiles::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(ListFilesCommand::new(
                4,
                Some("2.png".to_owned()),
                Some(1),
                Some(1),
            ))
            .await?;

        // Assert
        assert_eq!(response.total(), 3);
        assert_eq!(
            response.files(),
            &[GetFileResponse::new(
                2,
                "2.png".to_owned(),
                "image/png".to_owned(),
                10,
                NaiveDate::default(),
                NaiveDateTime::default(),
                Origin::Scanned,
                false,
                4,
                Some(19),
                None,
                None,
            )]
        );
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn list_files_drops_a_file_the_caller_may_not_discover() -> Result<(), Box<dyn Error>> {
        // Arrange: only the second file carries a resource the caller may
        // discover, so the first is filtered out of both the page and the total.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4)?;
        let mut files = MockFileRepository::new();
        let hidden = file(1, 3)?;
        let visible = file(2, 4)?;
        files.expect_search().times(1).return_once(move |_| {
            let found = vec![hidden.clone(), visible.clone()];
            Box::pin(async move { Ok(found) })
        });
        let mut resources = MockResourceRepository::new();
        resources
            .expect_search_many()
            .times(1)
            .withf(|addresses, _caller| {
                addresses
                    == [
                        ResourceRef::new(ResourceKind::File, 1),
                        ResourceRef::new(ResourceKind::File, 2),
                    ]
                    .as_slice()
            })
            .return_once(|_addresses, caller| {
                let found = vec![discoverable_file(caller.id(), 2)];
                Box::pin(async move { Ok(found) })
            });
        let mut images = MockImageRepository::new();
        let mut audios = MockAudioRepository::new();
        let mut videos = MockVideoRepository::new();
        expect_media_lookups(&mut images, &mut audios, &mut videos, 2)?;
        let harness = harness(users, files, images, audios, videos, resources);
        let use_case: UseCase = ListFiles::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(ListFilesCommand::new(4, None, None, None))
            .await?;

        // Assert: the concealed file is absent from the page and the total, and
        // the listing is not a denial.
        assert_eq!(response.total(), 1);
        assert_eq!(response.files().first().map(GetFileResponse::id), Some(2));
        Ok(())
    }

    #[tokio::test]
    async fn list_files_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness(
            users,
            MockFileRepository::new(),
            MockImageRepository::new(),
            MockAudioRepository::new(),
            MockVideoRepository::new(),
            caller_owned_resources(),
        );
        let use_case: UseCase = ListFiles::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ListFilesCommand::new(999, None, None, None))
            .await;

        // Assert
        assert!(matches!(result, Err(ListFilesError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn list_files_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4)?;
        let mut files = MockFileRepository::new();
        files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness(
            users,
            files,
            MockImageRepository::new(),
            MockAudioRepository::new(),
            MockVideoRepository::new(),
            caller_owned_resources(),
        );
        let use_case: UseCase = ListFiles::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ListFilesCommand::new(4, None, None, None))
            .await;

        // Assert
        assert!(matches!(result, Err(ListFilesError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
