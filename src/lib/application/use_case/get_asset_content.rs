use crate::application::authorization::{AuthorizationError, allows, authorize};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::{
    AssetTarget, ContentOutcome, GetAssetContentCommand, GetAssetContentError,
    GetAssetContentResponse, GetAssetContentUseCase,
};
use crate::application::use_case::{media_path, media_root};
use crate::domain::model::{Action, File, ResourceKind, ResourceRef};
use crate::domain::port::{
    AssetUnitOfWork, AudioFilter, AudioRepository as _, FileFilter, FileRepository as _,
    FileStorage, IdentityUnitOfWork, ImageFilter, ImageRepository as _, LibraryUnitOfWork,
    UnitOfWork as _, UnitOfWorkFactory, UserFilter, UserRepository as _, UserUnitOfWork,
    VideoFilter, VideoRepository as _,
};

/// Use case implementation for reading the content of one asset.
///
/// The asset is authorized inside the unit of work; only a readable asset's
/// bytes are read from storage, so a denied caller never reaches the file and
/// cannot observe its existence through timing or size (`API-044`). Every
/// request is decided on the spot from its session caller.
pub(crate) struct GetAssetContent<F, S> {
    /// Storage adapter reading the stored bytes.
    file_storage: Arc<S>,
    /// Factory opening the unit of work wrapping the authorization.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, S: FileStorage> GetAssetContent<F, S> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>, file_storage: Arc<S>) -> Self {
        Self {
            file_storage,
            unit_of_work_factory,
        }
    }
}

impl<F, S> GetAssetContentUseCase for GetAssetContent<F, S>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork + IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
    S: FileStorage,
{
    fn execute<'future>(
        &'future self,
        command: GetAssetContentCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<GetAssetContentResponse, GetAssetContentError>>
                + Send
                + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);
        let file_storage = Arc::clone(&self.file_storage);

        Box::pin(async move {
            // The library root is the operator's mount. An absent root makes
            // every content read unavailable, so no authorization read runs
            // against a library that cannot serve bytes (`503`).
            let root_present = file_storage
                .directory_exist(media_root().as_path())
                .await
                .map_err(|error| GetAssetContentError::Unknown(error.into()))?;
            if !root_present {
                return Err(GetAssetContentError::LibraryUnavailable);
            }

            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| GetAssetContentError::Unknown(error.into()))?;

            let target = command.target();
            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| GetAssetContentError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetAssetContentError::NoSuchCaller)?;
                let file = resolve_file(&mut unit_of_work, target).await?;

                let address = ResourceRef::new(ResourceKind::File, file.id());
                let decision = authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    address,
                    action_for(target),
                    None,
                )
                .await;
                if let Err(error) = decision {
                    return match error {
                        AuthorizationError::Unknown(source) => {
                            Err(GetAssetContentError::Unknown(source))
                        }
                        AuthorizationError::Denied(_) => {
                            let discovers = allows(
                                &mut unit_of_work.resources(),
                                &caller,
                                address,
                                Action::Discover,
                                None,
                            )
                            .await;
                            Err(if discovers {
                                GetAssetContentError::Forbidden
                            } else {
                                GetAssetContentError::NoSuchAsset
                            })
                        }
                    };
                }

                let file_name = if matches!(target, AssetTarget::File(_)) {
                    media_original_name(&mut unit_of_work, &file).await?
                } else {
                    None
                };
                Ok((file, file_name))
            }
            .await;

            let (file, file_name) = match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| GetAssetContentError::Unknown(error.into()))?;
                    value
                }
                Err(error) => {
                    // The unit-of-work adapter owns the rollback-failure log (OBS-002).
                    let _rollback = unit_of_work.rollback().await;
                    return Err(error);
                }
            };

            let path = media_path(file.path());
            let size_bytes = file_storage
                .file_size(&path)
                .await
                .map_err(|error| GetAssetContentError::Unknown(error.into()))?
                .ok_or(GetAssetContentError::BytesMissing)?;

            let etag = file
                .integrity_hash()
                .map(|digest| format!("\"{}\"", digest.as_str()));

            if let (Some(header), Some(current_etag)) = (command.if_none_match(), etag.as_deref())
                && header.split(',').any(|raw| {
                    let candidate = raw.trim();
                    candidate == "*"
                        || candidate == current_etag
                        || candidate.strip_prefix("W/") == Some(current_etag)
                })
            {
                return Ok(GetAssetContentResponse::new(
                    file.mime_type_id().to_owned(),
                    size_bytes,
                    file.modified_at(),
                    etag,
                    file_name,
                    ContentOutcome::NotModified,
                    None,
                    None,
                ));
            }

            // Parse the one `Range: bytes=` value inline. A missing header, a
            // header naming another unit, a multi-range value and a malformed
            // value all mean "serve the whole content" (`RFC 9110`), exactly as
            // an absent range does. A parsed value keeps the two optional
            // bounds the grammar declares: `first-last`, `first-` and `-suffix`.
            let mut requested = command.range().and_then(|value| {
                let spec = value.trim().strip_prefix("bytes=")?.trim();
                if spec.contains(',') {
                    return None;
                }
                let (raw_first, raw_last) = spec.split_once('-')?;
                let first_text = raw_first.trim();
                let last_text = raw_last.trim();
                if first_text.is_empty() && last_text.is_empty() {
                    return None;
                }
                let first = first_text.parse::<u64>().ok();
                let last = last_text.parse::<u64>().ok();
                if (!first_text.is_empty() && first.is_none())
                    || (!last_text.is_empty() && last.is_none())
                {
                    return None;
                }
                if let (Some(start), Some(end)) = (first, last)
                    && end < start
                {
                    return None;
                }
                Some((first, last))
            });
            if let (Some(if_range), Some(_)) = (command.if_range(), requested) {
                let validator = if_range.trim();
                let matches = etag.as_deref() == Some(validator)
                    || file
                        .modified_at()
                        .format("%a, %d %b %Y %H:%M:%S GMT")
                        .to_string()
                        == validator;
                if !matches {
                    requested = None;
                }
            }

            // Resolve the request against the opened handle's size: a suffix
            // longer than the content names the whole content, a start at or
            // past the end is unsatisfiable, and empty content satisfies none.
            let selected = requested.map(|(first, last)| {
                if size_bytes == 0 {
                    return None;
                }
                let final_byte = size_bytes.saturating_sub(1);
                match (first, last) {
                    (Some(start), Some(end)) if start <= final_byte => {
                        Some((start, end.min(final_byte)))
                    }
                    (Some(start), _) if start <= final_byte => Some((start, final_byte)),
                    (None, Some(length)) if length > 0 => {
                        Some((size_bytes.saturating_sub(length), final_byte))
                    }
                    _ => None,
                }
            });
            let (outcome, selected_range, start, length) = match selected {
                None => (ContentOutcome::Full, None, 0, None),
                Some(None) => {
                    return Ok(GetAssetContentResponse::new(
                        file.mime_type_id().to_owned(),
                        size_bytes,
                        file.modified_at(),
                        etag,
                        file_name,
                        ContentOutcome::Unsatisfiable,
                        None,
                        None,
                    ));
                }
                Some(Some((start, end))) => (
                    ContentOutcome::Partial,
                    Some((start, end)),
                    start,
                    Some(end.saturating_sub(start).saturating_add(1)),
                ),
            };

            let (reader, served_size) = if command.head() {
                (None, size_bytes)
            } else {
                let opened = file_storage
                    .open_range(&path, start, length)
                    .await
                    .map_err(|error| GetAssetContentError::Unknown(error.into()))?
                    .ok_or(GetAssetContentError::BytesMissing)?;
                (Some(opened.reader), opened.size_bytes)
            };
            Ok(GetAssetContentResponse::new(
                file.mime_type_id().to_owned(),
                served_size,
                file.modified_at(),
                etag,
                file_name,
                outcome,
                selected_range,
                reader,
            ))
        })
    }
}

/// Return the action the addressed content route decides.
fn action_for(target: AssetTarget) -> Action {
    match target {
        AssetTarget::File(_) => Action::Download,
        AssetTarget::Image(_) => Action::View,
        AssetTarget::Audio(_) | AssetTarget::Video(_) => Action::Stream,
    }
}

/// Resolve the original name of a file saved through the file content route.
///
/// A typed media row stores the name the intake assigned; a bare file, or a
/// video that carries no name, falls back to the final segment of its stored
/// path.
async fn media_original_name<U>(
    unit_of_work: &mut U,
    file: &File,
) -> Result<Option<String>, GetAssetContentError>
where
    U: LibraryUnitOfWork,
{
    if let Some(image) = unit_of_work
        .images()
        .search(&ImageFilter {
            file_id: Some(file.id()),
            ..ImageFilter::default()
        })
        .await
        .map_err(|error| GetAssetContentError::Unknown(error.into()))?
        .into_iter()
        .next()
    {
        return Ok(Some(image.name().to_owned()));
    }
    if let Some(audio) = unit_of_work
        .audios()
        .search(&AudioFilter {
            file_id: Some(file.id()),
            ..AudioFilter::default()
        })
        .await
        .map_err(|error| GetAssetContentError::Unknown(error.into()))?
        .into_iter()
        .next()
    {
        return Ok(Some(audio.name().to_owned()));
    }
    Ok(Some(
        file.path()
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(file.path())
            .to_owned(),
    ))
}

/// Resolve the file behind an addressed asset, if the asset exists.
async fn resolve_file<U>(
    unit_of_work: &mut U,
    target: AssetTarget,
) -> Result<File, GetAssetContentError>
where
    U: AssetUnitOfWork + LibraryUnitOfWork,
{
    let file_id = match target {
        AssetTarget::File(file_id) => file_id,
        AssetTarget::Image(image_id) => unit_of_work
            .images()
            .search(&ImageFilter {
                image_id: Some(image_id),
                ..ImageFilter::default()
            })
            .await
            .map_err(|error| GetAssetContentError::Unknown(error.into()))?
            .into_iter()
            .next()
            .ok_or(GetAssetContentError::NoSuchAsset)?
            .file_id(),
        AssetTarget::Audio(audio_id) => unit_of_work
            .audios()
            .search(&AudioFilter {
                id: Some(audio_id),
                ..AudioFilter::default()
            })
            .await
            .map_err(|error| GetAssetContentError::Unknown(error.into()))?
            .into_iter()
            .next()
            .ok_or(GetAssetContentError::NoSuchAsset)?
            .file_id(),
        AssetTarget::Video(video_id) => unit_of_work
            .videos()
            .search(&VideoFilter {
                id: Some(video_id),
                ..VideoFilter::default()
            })
            .await
            .map_err(|error| GetAssetContentError::Unknown(error.into()))?
            .into_iter()
            .next()
            .ok_or(GetAssetContentError::NoSuchAsset)?
            .file_id(),
    };
    unit_of_work
        .files()
        .search(&FileFilter {
            id: Some(file_id),
            ..FileFilter::default()
        })
        .await
        .map_err(|error| GetAssetContentError::Unknown(error.into()))?
        .into_iter()
        .next()
        .ok_or(GetAssetContentError::NoSuchAsset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::NumericID;

    use crate::domain::model::{
        AssetOwner, Audio, File, Image, IntegrityHash, Orientation, Resource, ResourceAccess,
        Visibility,
    };

    use crate::domain::port::{
        MockAudioRepository, MockFileRepository, MockFileStorage, MockImageRepository,
        MockResourceRepository, MockUserRepository, OpenedFile, RepositoryError, StorageError,
    };

    use crate::test_helpers::*;
    use chrono::{NaiveDate, NaiveDateTime};
    use rstest::rstest;
    use std::error::Error;
    use std::io;
    use std::path::Path;
    use std::sync::atomic::Ordering;

    type UseCase = GetAssetContent<TestFactory, MockFileStorage>;

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

    /// Expect the library root to exist.
    fn root_present(storage: &mut MockFileStorage) {
        storage
            .expect_directory_exist()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
    }

    /// Expect the opened file's size, path and reader.
    fn expect_open(
        storage: &mut MockFileStorage,
        path: &Path,
        size_bytes: u64,
        start: u64,
        length: Option<u64>,
        bytes: &'static [u8],
    ) {
        let size_path = path.to_path_buf();
        let open_path = path.to_path_buf();
        storage
            .expect_file_size()
            .withf(move |requested| requested == size_path.as_path())
            .times(1)
            .returning(move |_| Box::pin(async move { Ok(Some(size_bytes)) }));
        storage
            .expect_open_range()
            .times(1)
            .withf(move |requested, requested_start, requested_length| {
                requested == open_path.as_path()
                    && *requested_start == start
                    && *requested_length == length
            })
            .returning(move |_, _, _| {
                let content = bytes.to_vec();
                Box::pin(async move {
                    Ok(Some(OpenedFile {
                        reader: Box::pin(io::Cursor::new(content)),
                        size_bytes,
                    }))
                })
            });
    }

    /// A resource repository whose every addressed resource is public.
    ///
    /// A public resource grants the viewer bundle: `Discover`, `ViewMetadata`,
    /// `View` and `Stream`. It grants no `Download`.
    fn public_resources() -> MockResourceRepository {
        let mut resources = MockResourceRepository::new();
        resources.expect_search().returning(|filter, _caller| {
            let resource = Resource::from_stored(
                1,
                filter.kind.unwrap_or(ResourceKind::File),
                filter.entity_id.unwrap_or_default(),
                Some(99),
                None,
                Visibility::Public,
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
        resources
    }

    /// A resource repository whose every addressed resource is private to another user.
    fn concealed_resources() -> MockResourceRepository {
        let mut resources = MockResourceRepository::new();
        resources.expect_search().returning(|filter, _caller| {
            let resource = Resource::from_stored(
                1,
                filter.kind.unwrap_or(ResourceKind::File),
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
        resources
    }

    fn image(id: NumericID, file_id: NumericID, name: &str) -> Result<Image, Box<dyn Error>> {
        Ok(Image::try_new(
            id,
            name.to_owned(),
            10,
            20,
            Orientation::Portrait,
            NaiveDate::default(),
            file_id,
            None,
        )?)
    }

    fn fixture_with_resources(
        users: MockUserRepository,
        files: MockFileRepository,
        resources: MockResourceRepository,
    ) -> TestUnitOfWork {
        let mut images = MockImageRepository::new();
        images
            .expect_search()
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut audios = MockAudioRepository::new();
        audios
            .expect_search()
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        TestUnitOfWork {
            audios,
            files,
            images,
            resources,
            users,
            ..TestUnitOfWork::default()
        }
    }

    /// A fixture holding `users` and `files` over the default resource stub.
    ///
    /// The image and audio doubles answer with no row, so a file content test
    /// falls back to the stored path's final segment unless it stages one.
    fn fixture(users: MockUserRepository, files: MockFileRepository) -> TestUnitOfWork {
        let mut images = MockImageRepository::new();
        images
            .expect_search()
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut audios = MockAudioRepository::new();
        audios
            .expect_search()
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        TestUnitOfWork {
            audios,
            files,
            images,
            resources: no_resources(),
            users,
            ..TestUnitOfWork::default()
        }
    }

    #[tokio::test]
    async fn a_content_command_carries_every_request_value() {
        // Act
        let command = GetAssetContentCommand::new(9, AssetTarget::File(100))
            .with_head(true)
            .with_range(Some("bytes=0-9".to_owned()))
            .with_if_none_match(Some("etag".to_owned()))
            .with_if_range(Some("if-range".to_owned()));

        // Assert
        assert_eq!(command.caller_id(), 9);
        assert!(command.head());
        assert_eq!(command.range(), Some("bytes=0-9"));
        assert_eq!(command.if_none_match(), Some("etag"));
        assert_eq!(command.if_range(), Some("if-range"));
    }

    #[tokio::test]
    async fn content_of_a_readable_file_carries_its_media_type() -> Result<(), Box<dyn Error>> {
        // Arrange: a library image readable through `ViewMetadata` on the catalogue.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9)?;
        let mut files = MockFileRepository::new();
        let stored_file = file(100, 3, Some(AssetOwner::Library))?;
        files
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_file]) }));
        let harness = AssetFactoryHarness::new(fixture(users, files));
        let mut storage = MockFileStorage::new();
        root_present(&mut storage);
        expect_open(
            &mut storage,
            &media_path("100.png"),
            4,
            0,
            None,
            &[0x89, 0x50, 0x4e, 0x47],
        );
        let use_case: UseCase =
            GetAssetContent::new(Arc::clone(&harness.factory), Arc::new(storage));

        // Act
        let response = use_case
            .execute(GetAssetContentCommand::new(9, AssetTarget::File(100)))
            .await?;

        // Assert
        assert_eq!(response.mime_type_id(), "image/png");
        assert_eq!(response.outcome(), ContentOutcome::Full);
        assert_eq!(response.size_bytes(), 4);
        assert!(response.into_reader().is_some());
        Ok(())
    }
    #[tokio::test]
    async fn a_closed_range_serves_only_the_requested_span() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 3)?;
        let mut files = MockFileRepository::new();
        let stored_file = file(100, 3, None)?;
        files
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_file]) }));
        let harness = AssetFactoryHarness::new(fixture(users, files));
        let mut storage = MockFileStorage::new();
        root_present(&mut storage);
        expect_open(
            &mut storage,
            &media_path("100.png"),
            10,
            2,
            Some(3),
            &[1, 2, 3],
        );
        let use_case: UseCase =
            GetAssetContent::new(Arc::clone(&harness.factory), Arc::new(storage));
        let command = GetAssetContentCommand::new(3, AssetTarget::File(100))
            .with_range(Some("bytes=2-4".to_owned()));

        // Act
        let response = use_case.execute(command).await?;

        // Assert
        assert_eq!(response.outcome(), ContentOutcome::Partial);
        assert_eq!(response.selected_range(), Some((2, 4)));
        assert_eq!(response.size_bytes(), 10);
        Ok(())
    }

    #[rstest]
    #[case::open_ended("bytes=5-", ContentOutcome::Partial, Some((5, 9)), (5, Some(5)))]
    #[case::suffix("bytes=-4", ContentOutcome::Partial, Some((6, 9)), (6, Some(4)))]
    #[case::suffix_longer_than_the_content(
        "bytes=-400",
        ContentOutcome::Partial,
        Some((0, 9)),
        (0, Some(10))
    )]
    #[case::closed_clamped("bytes=8-999", ContentOutcome::Partial, Some((8, 9)), (8, Some(2)))]
    #[case::multi_range_serves_everything(
        "bytes=0-1,4-5",
        ContentOutcome::Full,
        None,
        (0, None)
    )]
    #[case::unknown_unit_serves_everything(
        "items=0-1",
        ContentOutcome::Full,
        None,
        (0, None)
    )]
    #[case::malformed_serves_everything(
        "bytes=abc-def",
        ContentOutcome::Full,
        None,
        (0, None)
    )]
    #[case::reversed_serves_everything("bytes=9-0", ContentOutcome::Full, None, (0, None))]
    #[case::empty_spec_serves_everything("bytes=-", ContentOutcome::Full, None, (0, None))]
    #[tokio::test]
    async fn a_range_value_resolves_against_the_opened_size(
        #[case] header: &str,
        #[case] expected_outcome: ContentOutcome,
        #[case] expected_span: Option<(u64, u64)>,
        #[case] opened: (u64, Option<u64>),
    ) -> Result<(), Box<dyn Error>> {
        // Arrange: file 100 is readable as its owner; the content is ten bytes.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 3)?;
        let mut files = MockFileRepository::new();
        let stored_file = file(100, 3, None)?;
        files
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_file]) }));
        let harness = AssetFactoryHarness::new(fixture(users, files));
        let (open_start, open_length) = opened;
        let mut storage = MockFileStorage::new();
        root_present(&mut storage);
        expect_open(
            &mut storage,
            &media_path("100.png"),
            10,
            open_start,
            open_length,
            &[],
        );
        let use_case: UseCase =
            GetAssetContent::new(Arc::clone(&harness.factory), Arc::new(storage));

        // Act
        let response = use_case
            .execute(
                GetAssetContentCommand::new(3, AssetTarget::File(100))
                    .with_range(Some(header.to_owned())),
            )
            .await?;

        // Assert
        assert_eq!(response.outcome(), expected_outcome);
        assert_eq!(response.selected_range(), expected_span);
        Ok(())
    }

    #[tokio::test]
    async fn a_zero_sized_content_has_no_satisfiable_range() -> Result<(), Box<dyn Error>> {
        // Arrange: the registered file is empty.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 3)?;
        let mut files = MockFileRepository::new();
        let stored_file = file(100, 3, None)?;
        files
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_file]) }));
        let harness = AssetFactoryHarness::new(fixture(users, files));
        let mut storage = MockFileStorage::new();
        root_present(&mut storage);
        storage
            .expect_file_size()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(0)) }));
        storage.expect_open_range().times(0);
        let use_case: UseCase =
            GetAssetContent::new(Arc::clone(&harness.factory), Arc::new(storage));

        // Act
        let response = use_case
            .execute(
                GetAssetContentCommand::new(3, AssetTarget::File(100))
                    .with_range(Some("bytes=0-".to_owned())),
            )
            .await?;

        // Assert
        assert_eq!(response.outcome(), ContentOutcome::Unsatisfiable);
        assert!(response.into_reader().is_none());
        Ok(())
    }

    #[rstest]
    #[case::start_past_the_end("bytes=10-")]
    #[case::zero_length_suffix("bytes=-0")]
    #[tokio::test]
    async fn an_unsatisfiable_range_has_no_reader(
        #[case] header: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 3)?;
        let mut files = MockFileRepository::new();
        let stored_file = file(100, 3, None)?;
        files
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_file]) }));
        let harness = AssetFactoryHarness::new(fixture(users, files));
        let mut storage = MockFileStorage::new();
        root_present(&mut storage);
        storage
            .expect_file_size()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(10)) }));
        storage.expect_open_range().times(0);
        let use_case: UseCase =
            GetAssetContent::new(Arc::clone(&harness.factory), Arc::new(storage));
        let command = GetAssetContentCommand::new(3, AssetTarget::File(100))
            .with_range(Some(header.to_owned()));

        // Act
        let response = use_case.execute(command).await?;

        // Assert
        assert_eq!(response.outcome(), ContentOutcome::Unsatisfiable);
        assert!(response.into_reader().is_none());
        Ok(())
    }

    #[tokio::test]
    async fn a_head_request_answers_without_a_reader() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 3)?;
        let mut files = MockFileRepository::new();
        let stored_file = file(100, 3, None)?;
        files
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_file]) }));
        let harness = AssetFactoryHarness::new(fixture(users, files));
        let mut storage = MockFileStorage::new();
        root_present(&mut storage);
        storage
            .expect_file_size()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(10)) }));
        storage.expect_open_range().times(0);
        let use_case: UseCase =
            GetAssetContent::new(Arc::clone(&harness.factory), Arc::new(storage));

        // Act
        let response = use_case
            .execute(GetAssetContentCommand::new(3, AssetTarget::File(100)).with_head(true))
            .await?;

        // Assert
        assert_eq!(response.outcome(), ContentOutcome::Full);
        assert!(response.into_reader().is_none());
        Ok(())
    }

    #[tokio::test]
    async fn a_matching_if_none_match_answers_not_modified() -> Result<(), Box<dyn Error>> {
        // Arrange: the file carries an integrity digest.
        let digest = "03ac674216f3e15c761ee1a5e255f067953623c8b388b4459e13f978d7c846f4";
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 3)?;
        let mut files = MockFileRepository::new();
        let stored_file = File::try_new(
            100,
            "100.png".to_owned(),
            3,
            false,
            "image/png".to_owned(),
            NaiveDate::default(),
            10,
            NaiveDateTime::default(),
            Some(IntegrityHash::try_new(digest.to_owned()).map_err(Box::<dyn Error>::from)?),
            None,
        )?;
        files
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_file]) }));
        let harness = AssetFactoryHarness::new(fixture(users, files));
        let mut storage = MockFileStorage::new();
        root_present(&mut storage);
        storage
            .expect_file_size()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(10)) }));
        storage.expect_open_range().times(0);
        let use_case: UseCase =
            GetAssetContent::new(Arc::clone(&harness.factory), Arc::new(storage));
        let etag = format!("\"{digest}\"");

        // Act
        let response = use_case
            .execute(
                GetAssetContentCommand::new(3, AssetTarget::File(100))
                    .with_if_none_match(Some(etag.clone())),
            )
            .await?;

        // Assert
        assert_eq!(response.outcome(), ContentOutcome::NotModified);
        assert_eq!(response.etag(), Some(etag.as_str()));
        assert!(response.into_reader().is_none());
        Ok(())
    }

    #[tokio::test]
    async fn a_stale_if_range_serves_the_whole_content() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 3)?;
        let mut files = MockFileRepository::new();
        let stored_file = file(100, 3, None)?;
        files
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_file]) }));
        let harness = AssetFactoryHarness::new(fixture(users, files));
        let mut storage = MockFileStorage::new();
        root_present(&mut storage);
        expect_open(
            &mut storage,
            &media_path("100.png"),
            10,
            0,
            None,
            &[1, 2, 3],
        );
        let use_case: UseCase =
            GetAssetContent::new(Arc::clone(&harness.factory), Arc::new(storage));
        let command = GetAssetContentCommand::new(3, AssetTarget::File(100))
            .with_range(Some("bytes=0-1".to_owned()))
            .with_if_range(Some("\"different\"".to_owned()));

        // Act
        let response = use_case.execute(command).await?;

        // Assert
        assert_eq!(response.outcome(), ContentOutcome::Full);
        Ok(())
    }

    #[tokio::test]
    async fn an_absent_root_is_unavailable_without_authorization() -> Result<(), Box<dyn Error>> {
        // Arrange: the root is absent, so no repository is consulted.
        let harness = AssetFactoryHarness::new(TestUnitOfWork::default());
        let mut storage = MockFileStorage::new();
        storage
            .expect_directory_exist()
            .times(1)
            .returning(|_| Box::pin(async { Ok(false) }));
        storage.expect_file_size().times(0);
        let use_case: UseCase =
            GetAssetContent::new(Arc::clone(&harness.factory), Arc::new(storage));

        // Act
        let result = use_case
            .execute(GetAssetContentCommand::new(9, AssetTarget::File(100)))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(GetAssetContentError::LibraryUnavailable)
        ));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn a_missing_file_under_a_present_root_is_a_server_error() -> Result<(), Box<dyn Error>> {
        // Arrange: the row exists, the bytes do not, and the root does.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 3)?;
        let mut files = MockFileRepository::new();
        let stored_file = file(100, 3, None)?;
        files
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_file]) }));
        let harness = AssetFactoryHarness::new(fixture(users, files));
        let mut storage = MockFileStorage::new();
        root_present(&mut storage);
        storage
            .expect_file_size()
            .times(1)
            .returning(|_| Box::pin(async { Ok(None) }));
        let use_case: UseCase =
            GetAssetContent::new(Arc::clone(&harness.factory), Arc::new(storage));

        // Act
        let result = use_case
            .execute(GetAssetContentCommand::new(3, AssetTarget::File(100)))
            .await;

        // Assert
        assert!(matches!(result, Err(GetAssetContentError::BytesMissing)));
        Ok(())
    }

    #[tokio::test]
    async fn a_storage_failure_is_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = AssetFactoryHarness::new(TestUnitOfWork::default());
        let mut storage = MockFileStorage::new();
        storage
            .expect_directory_exist()
            .times(1)
            .returning(|_| Box::pin(async { Err(StorageError::Unavailable) }));
        let use_case: UseCase =
            GetAssetContent::new(Arc::clone(&harness.factory), Arc::new(storage));

        // Act
        let result = use_case
            .execute(GetAssetContentCommand::new(9, AssetTarget::File(100)))
            .await;

        // Assert
        assert!(matches!(result, Err(GetAssetContentError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    async fn a_repository_failure_is_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 3)?;
        let mut files = MockFileRepository::new();
        files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = AssetFactoryHarness::new(fixture(users, files));
        let mut storage = MockFileStorage::new();
        root_present(&mut storage);
        let use_case: UseCase =
            GetAssetContent::new(Arc::clone(&harness.factory), Arc::new(storage));

        // Act
        let result = use_case
            .execute(GetAssetContentCommand::new(3, AssetTarget::File(100)))
            .await;

        // Assert
        assert!(matches!(result, Err(GetAssetContentError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn file_content_without_download_is_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange: a public file grants the viewer bundle, which includes
        // `View` and excludes `Download`, so the file route refuses.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 3)?;
        let mut files = MockFileRepository::new();
        let stored_file = file(100, 3, None)?;
        files
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_file]) }));
        let harness =
            AssetFactoryHarness::new(fixture_with_resources(users, files, public_resources()));
        let mut storage = MockFileStorage::new();
        root_present(&mut storage);
        storage.expect_file_size().times(0);
        let use_case: UseCase =
            GetAssetContent::new(Arc::clone(&harness.factory), Arc::new(storage));

        // Act
        let result = use_case
            .execute(GetAssetContentCommand::new(3, AssetTarget::File(100)))
            .await;

        // Assert
        assert!(
            matches!(result, Err(GetAssetContentError::Forbidden)),
            "{:?}",
            result.as_ref().err().map(ToString::to_string)
        );
        Ok(())
    }

    #[tokio::test]
    async fn a_public_image_reads_its_content_through_view() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 3)?;
        let mut images = MockImageRepository::new();
        let stored_image = image(19, 100, "cover.png")?;
        images
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_image]) }));
        let mut files = MockFileRepository::new();
        let stored_file = file(100, 3, None)?;
        files
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_file]) }));
        let harness = AssetFactoryHarness::new(TestUnitOfWork {
            files,
            images,
            resources: public_resources(),
            users,
            ..TestUnitOfWork::default()
        });
        let mut storage = MockFileStorage::new();
        root_present(&mut storage);
        expect_open(&mut storage, &media_path("100.png"), 3, 0, None, &[1, 2, 3]);
        let use_case: UseCase =
            GetAssetContent::new(Arc::clone(&harness.factory), Arc::new(storage));

        // Act
        let response = use_case
            .execute(GetAssetContentCommand::new(3, AssetTarget::Image(19)))
            .await?;

        // Assert
        assert_eq!(response.outcome(), ContentOutcome::Full);
        Ok(())
    }

    #[tokio::test]
    async fn a_public_audio_reads_its_content_through_stream() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 3)?;
        let mut audios = MockAudioRepository::new();
        let stored_audio = Audio::try_new(
            7,
            "Track".to_owned(),
            None,
            "mp3".to_owned(),
            None,
            None,
            None,
            100,
        )?;
        audios
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_audio]) }));
        let mut files = MockFileRepository::new();
        let stored_file = file(100, 3, None)?;
        files
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_file]) }));
        let harness = AssetFactoryHarness::new(TestUnitOfWork {
            audios,
            files,
            resources: public_resources(),
            users,
            ..TestUnitOfWork::default()
        });
        let mut storage = MockFileStorage::new();
        root_present(&mut storage);
        expect_open(&mut storage, &media_path("100.png"), 3, 0, None, &[1, 2, 3]);
        let use_case: UseCase =
            GetAssetContent::new(Arc::clone(&harness.factory), Arc::new(storage));

        // Act
        let response = use_case
            .execute(GetAssetContentCommand::new(3, AssetTarget::Audio(7)))
            .await?;

        // Assert
        assert_eq!(response.outcome(), ContentOutcome::Full);
        Ok(())
    }

    #[tokio::test]
    async fn a_refusal_opens_no_bytes() -> Result<(), Box<dyn Error>> {
        // Arrange: the file is public, so the decision refuses `Download`, and
        // the storage doubles assert that neither size nor range is read.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 3)?;
        let mut files = MockFileRepository::new();
        let stored_file = file(100, 3, None)?;
        files
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_file]) }));
        let harness =
            AssetFactoryHarness::new(fixture_with_resources(users, files, public_resources()));
        let mut storage = MockFileStorage::new();
        root_present(&mut storage);
        storage.expect_file_size().times(0);
        storage.expect_open_range().times(0);
        let use_case: UseCase =
            GetAssetContent::new(Arc::clone(&harness.factory), Arc::new(storage));

        // Act
        let result = use_case
            .execute(GetAssetContentCommand::new(3, AssetTarget::File(100)))
            .await;

        // Assert
        assert!(
            matches!(result, Err(GetAssetContentError::Forbidden)),
            "{:?}",
            result.as_ref().err().map(ToString::to_string)
        );
        Ok(())
    }

    #[tokio::test]
    async fn an_undiscoverable_file_is_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange: a private resource owned by another user conceals the file.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 3)?;
        let mut files = MockFileRepository::new();
        let stored_file = file(100, 3, None)?;
        files
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_file]) }));
        let harness =
            AssetFactoryHarness::new(fixture_with_resources(users, files, concealed_resources()));
        let mut storage = MockFileStorage::new();
        root_present(&mut storage);
        storage.expect_file_size().times(0);
        let use_case: UseCase =
            GetAssetContent::new(Arc::clone(&harness.factory), Arc::new(storage));

        // Act
        let result = use_case
            .execute(GetAssetContentCommand::new(3, AssetTarget::File(100)))
            .await;

        // Assert
        assert!(matches!(result, Err(GetAssetContentError::NoSuchAsset)));
        Ok(())
    }

    #[tokio::test]
    async fn file_content_names_the_typed_media_row() -> Result<(), Box<dyn Error>> {
        // Arrange: the file backs an image whose name differs from the path.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 3)?;
        let mut files = MockFileRepository::new();
        let stored_file = file(100, 3, None)?;
        files
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_file]) }));
        let mut images = MockImageRepository::new();
        let stored_image = image(19, 100, "holiday.png")?;
        images
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_image]) }));
        let harness = AssetFactoryHarness::new(TestUnitOfWork {
            files,
            images,
            resources: no_resources(),
            users,
            ..TestUnitOfWork::default()
        });
        let mut storage = MockFileStorage::new();
        root_present(&mut storage);
        expect_open(&mut storage, &media_path("100.png"), 3, 0, None, &[1, 2, 3]);
        let use_case: UseCase =
            GetAssetContent::new(Arc::clone(&harness.factory), Arc::new(storage));

        // Act
        let response = use_case
            .execute(GetAssetContentCommand::new(3, AssetTarget::File(100)))
            .await?;

        // Assert
        assert_eq!(response.file_name(), Some("holiday.png"));
        Ok(())
    }
}
