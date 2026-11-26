use crate::application::authorization::authorize;
use crate::domain::model::{ResourceKind, ResourceRef};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::{
    ApplyMusicAlbumCommand, ApplyMusicAlbumError, ApplyMusicAlbumResponse, ApplyMusicAlbumUseCase,
};
use crate::domain::model::{
    Action, Edition, MediumIndex, MusicAlbum, MusicMedium, MusicMetadata, MusicRecording,
    MusicTarget, TrackIndex,
};
use crate::domain::port::{
    IdentityUnitOfWork, LibraryUnitOfWork, MusicAlbumFilter, MusicAlbumRepository as _,
    MusicMetadataProvider, MusicRecordingFilter, RepositoryError, UnitOfWork as _,
    UnitOfWorkFactory, UserFilter, UserRepository as _, UserUnitOfWork,
};

/// The genre an applied album carries when the provider reports none.
const DEFAULT_GENRE: &str = "UNKNOWN";

/// The index of the single digital medium an apply materialises.
const DIGITAL_MEDIUM_INDEX: u8 = 1;

/// Use case implementation for applying external metadata to a music album.
pub(crate) struct ApplyMusicAlbum<F, P> {
    /// Configured provider, absent when music identification is unavailable.
    metadata_provider: Option<Arc<P>>,
    /// Factory opening the unit of work wrapping the write.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, P: MusicMetadataProvider> ApplyMusicAlbum<F, P> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>, metadata_provider: Option<Arc<P>>) -> Self {
        Self {
            metadata_provider,
            unit_of_work_factory,
        }
    }
}

impl<F, P> ApplyMusicAlbumUseCase for ApplyMusicAlbum<F, P>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
    P: MusicMetadataProvider,
{
    fn execute<'future>(
        &'future self,
        command: ApplyMusicAlbumCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ApplyMusicAlbumResponse, ApplyMusicAlbumError>>
                + Send
                + 'future,
        >,
    > {
        let metadata_provider = self.metadata_provider.as_ref().map(Arc::clone);
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| ApplyMusicAlbumError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| ApplyMusicAlbumError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(ApplyMusicAlbumError::NoSuchCaller)?;

                let album = unit_of_work
                    .music_albums()
                    .search(&MusicAlbumFilter {
                        id: Some(command.album_id()),
                        ..MusicAlbumFilter::default()
                    })
                    .await
                    .map_err(|error| ApplyMusicAlbumError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(ApplyMusicAlbumError::NoSuchAlbum)?;

                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::MusicAlbum, album.id()),
                    Action::ViewMetadata,
                    None,
                )
                .await
                .map_err(|_| ApplyMusicAlbumError::NoSuchAlbum)?;
                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::MusicAlbum, album.id()),
                    Action::EditMetadata,
                    None,
                )
                .await
                .map_err(|error| {
                    if error.conceals() {
                        ApplyMusicAlbumError::NoSuchAlbum
                    } else {
                        ApplyMusicAlbumError::Forbidden
                    }
                })?;

                if album.id() == 0 {
                    return Err(ApplyMusicAlbumError::DefaultAlbum);
                }

                let Some(provider) = metadata_provider else {
                    return Err(ApplyMusicAlbumError::ProviderUnavailable);
                };
                if command.reference().provider() != provider.provider() {
                    return Err(ApplyMusicAlbumError::UnknownProvider);
                }

                let target = MusicTarget::Album(command.reference().clone());
                let metadata = provider
                    .lookup(&target)
                    .await
                    .map_err(|_| ApplyMusicAlbumError::ProviderFailure)?
                    .ok_or(ApplyMusicAlbumError::UnknownReference)?;

                let MusicMetadata::Album {
                    editions,
                    first_release_date,
                    genres,
                    production_type,
                    title,
                    ..
                } = metadata
                else {
                    return Err(ApplyMusicAlbumError::InvalidReference);
                };

                let edition = editions.first();
                let compact_discs: Vec<_> = edition
                    .map(|released| {
                        released
                            .mediums()
                            .iter()
                            .filter(|medium| medium.format().is_compact_disc())
                            .collect()
                    })
                    .unwrap_or_default();
                let mediums = if compact_discs.len() > 1 {
                    compact_discs
                        .into_iter()
                        .map(|medium| {
                            u8::try_from(medium.index())
                                .map(MediumIndex::Cd)
                                .map_err(|_| ApplyMusicAlbumError::InvalidReference)
                        })
                        .collect::<Result<Vec<_>, _>>()?
                } else if edition.is_some() {
                    vec![MediumIndex::Digital(DIGITAL_MEDIUM_INDEX)]
                } else {
                    Vec::new()
                };
                if !command.placements().iter().all(|placement| {
                    mediums
                        .iter()
                        .any(|medium| medium.index() == placement.medium_index())
                }) {
                    return Err(ApplyMusicAlbumError::InvalidPlacement);
                }

                let applied_genres = if genres.is_empty() {
                    vec![DEFAULT_GENRE.to_owned()]
                } else {
                    genres
                };
                let release_date =
                    first_release_date.or_else(|| edition.and_then(Edition::release_date));

                let mut album_mediums = Vec::with_capacity(mediums.len());
                for medium_index in &mediums {
                    let mut tracks = Vec::new();
                    for placement in command
                        .placements()
                        .iter()
                        .filter(|placement| placement.medium_index() == medium_index.index())
                    {
                        let recording = unit_of_work
                            .music_albums()
                            .search_recordings(&MusicRecordingFilter {
                                id: Some(placement.recording_id()),
                                ..MusicRecordingFilter::default()
                            })
                            .await
                            .map_err(|error| ApplyMusicAlbumError::Unknown(error.into()))?
                            .into_iter()
                            .next()
                            .ok_or(ApplyMusicAlbumError::InvalidPlacement)?;
                        tracks.push(
                            MusicRecording::try_new(
                                recording.id(),
                                recording.name().map(str::to_owned),
                                recording.isrc_code().map(str::to_owned),
                                recording.audio_id(),
                                recording.file_id(),
                                recording.music_group_ids().map(<[_]>::to_vec),
                                TrackIndex::MediumIndex(placement.track_index()),
                            )
                            .map_err(|_| ApplyMusicAlbumError::InvalidPlacement)?,
                        );
                    }
                    album_mediums.push(MusicMedium::new(*medium_index, tracks));
                }

                let updated = MusicAlbum::try_new(
                    album.id(),
                    title,
                    release_date,
                    production_type,
                    applied_genres,
                    album.music_group_id(),
                    album.music_group_ids().map(<[_]>::to_vec),
                    album.cover_image_id(),
                    album_mediums,
                )
                .map_err(|_| ApplyMusicAlbumError::InvalidReference)?;

                let stored = unit_of_work
                    .music_albums()
                    .save(updated)
                    .await
                    .map_err(|error| {
                        if matches!(
                            error,
                            RepositoryError::AlreadyExist | RepositoryError::Conflict
                        ) {
                            ApplyMusicAlbumError::Conflict
                        } else if matches!(error, RepositoryError::DataIntegrityViolation) {
                            ApplyMusicAlbumError::InvalidPlacement
                        } else {
                            ApplyMusicAlbumError::Unknown(error.into())
                        }
                    })?;

                Ok(ApplyMusicAlbumResponse::new(stored))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| ApplyMusicAlbumError::Unknown(error.into()))?;
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
    use crate::application::port::AlbumPlacement;
    use rstest::fixture;

    use crate::domain::NumericID;

    use crate::domain::model::{
        ExternalRef, MediumFormat, MediumSummary, MusicMedium, MusicRecording, ProductionType,
        ProviderId, Resource, ResourceAccess, ResourceKind, Role, TrackIndex, User, Visibility,
    };

    use crate::domain::port::{
        ExternalServiceError, MockMusicAlbumRepository, MockMusicMetadataProvider,
        MockResourceRepository, MockUserRepository,
    };

    use crate::test_helpers::*;
    use chrono::NaiveDate;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    /// The use case under test, bound to the mocked unit-of-work factory and
    /// provider.
    type UseCase = ApplyMusicAlbum<TestFactory, MockMusicMetadataProvider>;

    /// The reference every test resolves.
    #[fixture]
    fn reference() -> ExternalRef {
        ExternalRef::new(ProviderId::MusicBrainz, "release-group".to_owned())
    }

    /// Build a Library test harness whose resource load is the given double.
    fn harness_with_resources(
        users: MockUserRepository,
        albums: MockMusicAlbumRepository,
        resources: MockResourceRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            music_albums: albums,
            resources,
            users,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
    }

    /// The album metadata a provider reports, with one compact disc.
    fn album_metadata(mediums: Vec<MediumSummary>) -> MusicMetadata {
        MusicMetadata::Album {
            artwork: None,
            artist_credits: Vec::new(),
            editions: vec![Edition::new(
                ExternalRef::new(ProviderId::MusicBrainz, "release".to_owned()),
                "Kind of Blue".to_owned(),
                NaiveDate::from_ymd_opt(1959, 8, 17),
                Some("US".to_owned()),
                None,
                mediums,
            )],
            first_release_date: NaiveDate::from_ymd_opt(1959, 8, 17),
            genres: vec!["JAZZ".to_owned()],
            production_type: Some(ProductionType::Studio),
            title: "Kind of Blue".to_owned(),
        }
    }

    // proves: SCN-METADATA-ENRICHMENT-028-E — Business rule
    // proves: SCN-METADATA-ENRICHMENT-028-H — Business rule
    #[tokio::test]
    async fn apply_music_album_without_genres_keeps_the_cover_and_group()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the provider reports neither a genre nor a group, and the album stores both a cover and a group.
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository,
               caller_id: NumericID|
              -> Result<(), Box<dyn Error>> {
            let caller =
                User::try_new(caller_id, "admin".to_owned(), None, caller_id, Role::Admin)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let stored = MusicAlbum::try_new(
            7,
            "Album 7".to_owned(),
            Some(album_release_date()),
            None,
            Vec::new(),
            3,
            None,
            Some(42),
            Vec::new(),
        )?;
        let mut albums = MockMusicAlbumRepository::new();
        (move |albums: &mut MockMusicAlbumRepository, album: MusicAlbum| {
            let album_id = album.id();
            albums
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(album_id))
                .return_once(move |_| {
                    let found = album.clone();
                    Box::pin(async move { Ok(vec![found]) })
                });
        })(&mut albums, stored);
        albums
            .expect_save()
            .times(1)
            .withf(|album| {
                album.id() == 7
                    && album.genre_ids() == ["UNKNOWN".to_owned()]
                    && album.cover_image_id() == Some(42)
                    && album.music_group_id() == 3
            })
            .returning(|album| Box::pin(async move { Ok(album) }));
        albums.expect_save_medium().times(0);
        let mut provider = MockMusicMetadataProvider::new();
        (move |provider: &mut MockMusicMetadataProvider| {
            provider
                .expect_provider()
                .returning(|| ProviderId::MusicBrainz);
            provider.expect_lookup().times(1).return_once(|_| {
                Box::pin(async {
                    Ok(Some(MusicMetadata::Album {
                        artwork: Some(crate::domain::model::Artwork::new(
                            vec![1, 2, 3],
                            "image/png".to_owned(),
                        )),
                        artist_credits: Vec::new(),
                        editions: vec![Edition::new(
                            ExternalRef::new(ProviderId::MusicBrainz, "release".to_owned()),
                            "Kind of Blue".to_owned(),
                            NaiveDate::from_ymd_opt(1959, 8, 17),
                            Some("US".to_owned()),
                            None,
                            vec![MediumSummary::new(MediumFormat::Cd, 1, 5)],
                        )],
                        first_release_date: NaiveDate::from_ymd_opt(1959, 8, 17),
                        genres: Vec::new(),
                        production_type: Some(ProductionType::Studio),
                        title: "Kind of Blue".to_owned(),
                    }))
                })
            });
        })(&mut provider);
        let harness = harness_with_resources(
            users,
            albums,
            (move || -> crate::domain::port::MockResourceRepository {
                let mut resources = (move || -> crate::domain::port::MockResourceRepository {
                    let mut resources = crate::domain::port::MockResourceRepository::new();
                    resources.expect_search().returning(|filter, caller| {
                        let user_id = caller.id();
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            filter
                                .kind
                                .unwrap_or(crate::domain::model::ResourceKind::Library),
                            filter.entity_id.unwrap_or_default(),
                            Some(user_id),
                            None,
                            crate::domain::model::Visibility::Private,
                        );
                        let grant = crate::domain::model::Grant::from_stored(
                            1,
                            crate::domain::model::Principal::User(user_id),
                            1,
                            crate::domain::model::ResourceRole::Downloader,
                            crate::domain::model::ChildKind::All,
                            crate::domain::model::GrantEffect::Allow,
                        );
                        Box::pin(async move {
                            Ok(vec![crate::domain::model::ResourceAccess::new(
                                resource,
                                Vec::new(),
                                vec![grant],
                                Vec::new(),
                            )])
                        })
                    });
                    resources
                        .expect_search_many()
                        .returning(|addresses, caller| {
                            let user_id = caller.id();
                            let found = addresses
                                .iter()
                                .map(|address| {
                                    let resource = crate::domain::model::Resource::from_stored(
                                        1,
                                        address.kind(),
                                        address.id(),
                                        Some(user_id),
                                        None,
                                        crate::domain::model::Visibility::Private,
                                    );
                                    let grant = crate::domain::model::Grant::from_stored(
                                        1,
                                        crate::domain::model::Principal::User(user_id),
                                        1,
                                        crate::domain::model::ResourceRole::Downloader,
                                        crate::domain::model::ChildKind::All,
                                        crate::domain::model::GrantEffect::Allow,
                                    );
                                    crate::domain::model::ResourceAccess::new(
                                        resource,
                                        Vec::new(),
                                        vec![grant],
                                        Vec::new(),
                                    )
                                })
                                .collect::<Vec<_>>();
                            Box::pin(async move { Ok(found) })
                        });
                    resources.expect_create().returning(|new_resource| {
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            new_resource.kind,
                            new_resource.entity_id,
                            new_resource.owner_id,
                            None,
                            new_resource.visibility,
                        );
                        Box::pin(async move { Ok(resource) })
                    });
                    resources
                })();
                resources
                    .expect_delete()
                    .returning(|_| Box::pin(async { Ok(true) }));
                resources
            })(),
        );

        // Act
        let use_case: UseCase =
            ApplyMusicAlbum::new(Arc::clone(&harness.factory), Some(Arc::new(provider)));
        let response = use_case
            .execute(ApplyMusicAlbumCommand::new(1, 7, reference(), Vec::new()))
            .await?;

        // Assert
        assert_eq!(
            response.album().genre_ids(),
            ["UNKNOWN".to_owned()],
            "an album the provider gives no genre must store UNKNOWN"
        );
        assert_eq!(
            response.album().cover_image_id(),
            Some(42),
            "an apply must keep the stored cover and never store provider artwork"
        );
        assert_eq!(
            response.album().music_group_id(),
            3,
            "an apply must keep the stored group"
        );
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    // proves: SCN-METADATA-ENRICHMENT-024-A, SCN-METADATA-ENRICHMENT-028-D — Business rule
    // proves: SCN-METADATA-ENRICHMENT-028-A — Positive
    #[tokio::test]
    async fn apply_music_album_without_placements_writes_the_structure()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository,
               caller_id: NumericID|
              -> Result<(), Box<dyn Error>> {
            let caller =
                User::try_new(caller_id, "admin".to_owned(), None, caller_id, Role::Admin)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        (move |albums: &mut MockMusicAlbumRepository, album: MusicAlbum| {
            let album_id = album.id();
            albums
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(album_id))
                .return_once(move |_| {
                    let found = album.clone();
                    Box::pin(async move { Ok(vec![found]) })
                });
        })(&mut albums, sample_album(7, Vec::new())?);
        albums
            .expect_save()
            .times(1)
            .withf(|album| {
                album.id() == 7
                    && album.name() == "Kind of Blue"
                    && album.genre_ids() == ["JAZZ".to_owned()]
                    && album.production_type() == Some(ProductionType::Studio)
                    && album.mediums().to_vec()
                        == vec![MusicMedium::new(MediumIndex::Digital(1), Vec::new())]
            })
            .returning(|album| Box::pin(async move { Ok(album) }));
        albums.expect_save_medium().times(0);
        let mut provider = MockMusicMetadataProvider::new();
        (move |provider: &mut MockMusicMetadataProvider, metadata: MusicMetadata| {
            provider
                .expect_provider()
                .returning(|| ProviderId::MusicBrainz);
            provider.expect_lookup().times(1).return_once(move |_| {
                let resolved = metadata.clone();
                Box::pin(async move { Ok(Some(resolved)) })
            });
        })(
            &mut provider,
            album_metadata(vec![MediumSummary::new(MediumFormat::Cd, 1, 5)]),
        );
        let harness = (move |users: MockUserRepository,
                             albums: MockMusicAlbumRepository|
              -> GalleryFactoryHarness {
            harness_with_resources(
                users,
                albums,
                (move || -> crate::domain::port::MockResourceRepository {
                    let mut resources = (move || -> crate::domain::port::MockResourceRepository {
                        let mut resources = crate::domain::port::MockResourceRepository::new();
                        resources.expect_search().returning(|filter, caller| {
                            let user_id = caller.id();
                            let resource = crate::domain::model::Resource::from_stored(
                                1,
                                filter
                                    .kind
                                    .unwrap_or(crate::domain::model::ResourceKind::Library),
                                filter.entity_id.unwrap_or_default(),
                                Some(user_id),
                                None,
                                crate::domain::model::Visibility::Private,
                            );
                            let grant = crate::domain::model::Grant::from_stored(
                                1,
                                crate::domain::model::Principal::User(user_id),
                                1,
                                crate::domain::model::ResourceRole::Downloader,
                                crate::domain::model::ChildKind::All,
                                crate::domain::model::GrantEffect::Allow,
                            );
                            Box::pin(async move {
                                Ok(vec![crate::domain::model::ResourceAccess::new(
                                    resource,
                                    Vec::new(),
                                    vec![grant],
                                    Vec::new(),
                                )])
                            })
                        });
                        resources
                            .expect_search_many()
                            .returning(|addresses, caller| {
                                let user_id = caller.id();
                                let found = addresses
                                    .iter()
                                    .map(|address| {
                                        let resource = crate::domain::model::Resource::from_stored(
                                            1,
                                            address.kind(),
                                            address.id(),
                                            Some(user_id),
                                            None,
                                            crate::domain::model::Visibility::Private,
                                        );
                                        let grant = crate::domain::model::Grant::from_stored(
                                            1,
                                            crate::domain::model::Principal::User(user_id),
                                            1,
                                            crate::domain::model::ResourceRole::Downloader,
                                            crate::domain::model::ChildKind::All,
                                            crate::domain::model::GrantEffect::Allow,
                                        );
                                        crate::domain::model::ResourceAccess::new(
                                            resource,
                                            Vec::new(),
                                            vec![grant],
                                            Vec::new(),
                                        )
                                    })
                                    .collect::<Vec<_>>();
                                Box::pin(async move { Ok(found) })
                            });
                        resources.expect_create().returning(|new_resource| {
                            let resource = crate::domain::model::Resource::from_stored(
                                1,
                                new_resource.kind,
                                new_resource.entity_id,
                                new_resource.owner_id,
                                None,
                                new_resource.visibility,
                            );
                            Box::pin(async move { Ok(resource) })
                        });
                        resources
                    })();
                    resources
                        .expect_delete()
                        .returning(|_| Box::pin(async { Ok(true) }));
                    resources
                })(),
            )
        })(users, albums);
        let use_case: UseCase =
            ApplyMusicAlbum::new(Arc::clone(&harness.factory), Some(Arc::new(provider)));

        // Act
        let response = use_case
            .execute(ApplyMusicAlbumCommand::new(1, 7, reference(), Vec::new()))
            .await?;

        // Assert
        assert_eq!(response.album().id(), 7);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    // proves: SCN-METADATA-ENRICHMENT-024-A — Business rule
    // proves: SCN-METADATA-ENRICHMENT-028-B — Alternate flow
    #[tokio::test]
    async fn apply_music_album_with_placements_moves_the_recordings() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository,
               caller_id: NumericID|
              -> Result<(), Box<dyn Error>> {
            let caller =
                User::try_new(caller_id, "admin".to_owned(), None, caller_id, Role::Admin)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        (move |albums: &mut MockMusicAlbumRepository, album: MusicAlbum| {
            let album_id = album.id();
            albums
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(album_id))
                .return_once(move |_| {
                    let found = album.clone();
                    Box::pin(async move { Ok(vec![found]) })
                });
        })(&mut albums, sample_album(7, Vec::new())?);
        let recorded = MusicRecording::try_new(
            5,
            Some("So What".to_owned()),
            None,
            1,
            2,
            None,
            TrackIndex::MediumIndex(1),
        )?;
        albums
            .expect_search_recordings()
            .times(1)
            .withf(|filter| filter.id == Some(5))
            .return_once(move |_| {
                let found = recorded.clone();
                Box::pin(async move { Ok(vec![found]) })
            });
        albums
            .expect_save()
            .times(1)
            .withf(|album| album.id() == 7 && album.mediums().len() == 1)
            .returning(|album| Box::pin(async move { Ok(album) }));
        albums.expect_save_medium().times(0);
        let mut provider = MockMusicMetadataProvider::new();
        (move |provider: &mut MockMusicMetadataProvider, metadata: MusicMetadata| {
            provider
                .expect_provider()
                .returning(|| ProviderId::MusicBrainz);
            provider.expect_lookup().times(1).return_once(move |_| {
                let resolved = metadata.clone();
                Box::pin(async move { Ok(Some(resolved)) })
            });
        })(
            &mut provider,
            album_metadata(vec![MediumSummary::new(MediumFormat::Cd, 1, 5)]),
        );
        let harness = (move |users: MockUserRepository,
                             albums: MockMusicAlbumRepository|
              -> GalleryFactoryHarness {
            harness_with_resources(
                users,
                albums,
                (move || -> crate::domain::port::MockResourceRepository {
                    let mut resources = (move || -> crate::domain::port::MockResourceRepository {
                        let mut resources = crate::domain::port::MockResourceRepository::new();
                        resources.expect_search().returning(|filter, caller| {
                            let user_id = caller.id();
                            let resource = crate::domain::model::Resource::from_stored(
                                1,
                                filter
                                    .kind
                                    .unwrap_or(crate::domain::model::ResourceKind::Library),
                                filter.entity_id.unwrap_or_default(),
                                Some(user_id),
                                None,
                                crate::domain::model::Visibility::Private,
                            );
                            let grant = crate::domain::model::Grant::from_stored(
                                1,
                                crate::domain::model::Principal::User(user_id),
                                1,
                                crate::domain::model::ResourceRole::Downloader,
                                crate::domain::model::ChildKind::All,
                                crate::domain::model::GrantEffect::Allow,
                            );
                            Box::pin(async move {
                                Ok(vec![crate::domain::model::ResourceAccess::new(
                                    resource,
                                    Vec::new(),
                                    vec![grant],
                                    Vec::new(),
                                )])
                            })
                        });
                        resources
                            .expect_search_many()
                            .returning(|addresses, caller| {
                                let user_id = caller.id();
                                let found = addresses
                                    .iter()
                                    .map(|address| {
                                        let resource = crate::domain::model::Resource::from_stored(
                                            1,
                                            address.kind(),
                                            address.id(),
                                            Some(user_id),
                                            None,
                                            crate::domain::model::Visibility::Private,
                                        );
                                        let grant = crate::domain::model::Grant::from_stored(
                                            1,
                                            crate::domain::model::Principal::User(user_id),
                                            1,
                                            crate::domain::model::ResourceRole::Downloader,
                                            crate::domain::model::ChildKind::All,
                                            crate::domain::model::GrantEffect::Allow,
                                        );
                                        crate::domain::model::ResourceAccess::new(
                                            resource,
                                            Vec::new(),
                                            vec![grant],
                                            Vec::new(),
                                        )
                                    })
                                    .collect::<Vec<_>>();
                                Box::pin(async move { Ok(found) })
                            });
                        resources.expect_create().returning(|new_resource| {
                            let resource = crate::domain::model::Resource::from_stored(
                                1,
                                new_resource.kind,
                                new_resource.entity_id,
                                new_resource.owner_id,
                                None,
                                new_resource.visibility,
                            );
                            Box::pin(async move { Ok(resource) })
                        });
                        resources
                    })();
                    resources
                        .expect_delete()
                        .returning(|_| Box::pin(async { Ok(true) }));
                    resources
                })(),
            )
        })(users, albums);
        let use_case: UseCase =
            ApplyMusicAlbum::new(Arc::clone(&harness.factory), Some(Arc::new(provider)));

        // Act
        let response = use_case
            .execute(ApplyMusicAlbumCommand::new(
                1,
                7,
                reference(),
                vec![AlbumPlacement::new(1, 1, 5)],
            ))
            .await?;

        // Assert
        assert_eq!(response.album().id(), 7);
        Ok(())
    }

    // proves: SCN-METADATA-ENRICHMENT-028-C — Business rule
    // proves: SCN-MUSIC-003-A — Business rule
    #[tokio::test]
    async fn apply_music_album_multi_cd_release_becomes_cd_mediums() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository,
               caller_id: NumericID|
              -> Result<(), Box<dyn Error>> {
            let caller =
                User::try_new(caller_id, "admin".to_owned(), None, caller_id, Role::Admin)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        (move |albums: &mut MockMusicAlbumRepository, album: MusicAlbum| {
            let album_id = album.id();
            albums
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(album_id))
                .return_once(move |_| {
                    let found = album.clone();
                    Box::pin(async move { Ok(vec![found]) })
                });
        })(&mut albums, sample_album(7, Vec::new())?);
        albums
            .expect_save()
            .times(1)
            .withf(|album| {
                album.id() == 7
                    && album.mediums().len() == 2
                    && album
                        .mediums()
                        .first()
                        .is_some_and(|medium| medium.index() == MediumIndex::Cd(1))
                    && album
                        .mediums()
                        .get(1)
                        .is_some_and(|medium| medium.index() == MediumIndex::Cd(2))
            })
            .returning(|album| Box::pin(async move { Ok(album) }));
        albums.expect_save_medium().times(0);
        let mut provider = MockMusicMetadataProvider::new();
        (move |provider: &mut MockMusicMetadataProvider, metadata: MusicMetadata| {
            provider
                .expect_provider()
                .returning(|| ProviderId::MusicBrainz);
            provider.expect_lookup().times(1).return_once(move |_| {
                let resolved = metadata.clone();
                Box::pin(async move { Ok(Some(resolved)) })
            });
        })(
            &mut provider,
            album_metadata(vec![
                MediumSummary::new(MediumFormat::Cd, 1, 5),
                MediumSummary::new(MediumFormat::Cd, 2, 4),
            ]),
        );
        let harness = (move |users: MockUserRepository,
                             albums: MockMusicAlbumRepository|
              -> GalleryFactoryHarness {
            harness_with_resources(
                users,
                albums,
                (move || -> crate::domain::port::MockResourceRepository {
                    let mut resources = (move || -> crate::domain::port::MockResourceRepository {
                        let mut resources = crate::domain::port::MockResourceRepository::new();
                        resources.expect_search().returning(|filter, caller| {
                            let user_id = caller.id();
                            let resource = crate::domain::model::Resource::from_stored(
                                1,
                                filter
                                    .kind
                                    .unwrap_or(crate::domain::model::ResourceKind::Library),
                                filter.entity_id.unwrap_or_default(),
                                Some(user_id),
                                None,
                                crate::domain::model::Visibility::Private,
                            );
                            let grant = crate::domain::model::Grant::from_stored(
                                1,
                                crate::domain::model::Principal::User(user_id),
                                1,
                                crate::domain::model::ResourceRole::Downloader,
                                crate::domain::model::ChildKind::All,
                                crate::domain::model::GrantEffect::Allow,
                            );
                            Box::pin(async move {
                                Ok(vec![crate::domain::model::ResourceAccess::new(
                                    resource,
                                    Vec::new(),
                                    vec![grant],
                                    Vec::new(),
                                )])
                            })
                        });
                        resources
                            .expect_search_many()
                            .returning(|addresses, caller| {
                                let user_id = caller.id();
                                let found = addresses
                                    .iter()
                                    .map(|address| {
                                        let resource = crate::domain::model::Resource::from_stored(
                                            1,
                                            address.kind(),
                                            address.id(),
                                            Some(user_id),
                                            None,
                                            crate::domain::model::Visibility::Private,
                                        );
                                        let grant = crate::domain::model::Grant::from_stored(
                                            1,
                                            crate::domain::model::Principal::User(user_id),
                                            1,
                                            crate::domain::model::ResourceRole::Downloader,
                                            crate::domain::model::ChildKind::All,
                                            crate::domain::model::GrantEffect::Allow,
                                        );
                                        crate::domain::model::ResourceAccess::new(
                                            resource,
                                            Vec::new(),
                                            vec![grant],
                                            Vec::new(),
                                        )
                                    })
                                    .collect::<Vec<_>>();
                                Box::pin(async move { Ok(found) })
                            });
                        resources.expect_create().returning(|new_resource| {
                            let resource = crate::domain::model::Resource::from_stored(
                                1,
                                new_resource.kind,
                                new_resource.entity_id,
                                new_resource.owner_id,
                                None,
                                new_resource.visibility,
                            );
                            Box::pin(async move { Ok(resource) })
                        });
                        resources
                    })();
                    resources
                        .expect_delete()
                        .returning(|_| Box::pin(async { Ok(true) }));
                    resources
                })(),
            )
        })(users, albums);
        let use_case: UseCase =
            ApplyMusicAlbum::new(Arc::clone(&harness.factory), Some(Arc::new(provider)));

        // Act
        use_case
            .execute(ApplyMusicAlbumCommand::new(1, 7, reference(), Vec::new()))
            .await?;

        // Assert: the assertion lives in the `save` expectation.
        Ok(())
    }

    // proves: SCN-METADATA-ENRICHMENT-024-E — Error handling
    #[tokio::test]
    async fn apply_music_album_provider_failure_writes_nothing() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository,
               caller_id: NumericID|
              -> Result<(), Box<dyn Error>> {
            let caller =
                User::try_new(caller_id, "admin".to_owned(), None, caller_id, Role::Admin)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        (move |albums: &mut MockMusicAlbumRepository, album: MusicAlbum| {
            let album_id = album.id();
            albums
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(album_id))
                .return_once(move |_| {
                    let found = album.clone();
                    Box::pin(async move { Ok(vec![found]) })
                });
        })(&mut albums, sample_album(7, Vec::new())?);
        albums.expect_save().times(0);
        let mut provider = MockMusicMetadataProvider::new();
        provider
            .expect_provider()
            .returning(|| ProviderId::MusicBrainz);
        provider
            .expect_lookup()
            .times(1)
            .returning(|_| Box::pin(async { Err(ExternalServiceError::Unavailable) }));
        let harness = (move |users: MockUserRepository,
                             albums: MockMusicAlbumRepository|
              -> GalleryFactoryHarness {
            harness_with_resources(
                users,
                albums,
                (move || -> crate::domain::port::MockResourceRepository {
                    let mut resources = (move || -> crate::domain::port::MockResourceRepository {
                        let mut resources = crate::domain::port::MockResourceRepository::new();
                        resources.expect_search().returning(|filter, caller| {
                            let user_id = caller.id();
                            let resource = crate::domain::model::Resource::from_stored(
                                1,
                                filter
                                    .kind
                                    .unwrap_or(crate::domain::model::ResourceKind::Library),
                                filter.entity_id.unwrap_or_default(),
                                Some(user_id),
                                None,
                                crate::domain::model::Visibility::Private,
                            );
                            let grant = crate::domain::model::Grant::from_stored(
                                1,
                                crate::domain::model::Principal::User(user_id),
                                1,
                                crate::domain::model::ResourceRole::Downloader,
                                crate::domain::model::ChildKind::All,
                                crate::domain::model::GrantEffect::Allow,
                            );
                            Box::pin(async move {
                                Ok(vec![crate::domain::model::ResourceAccess::new(
                                    resource,
                                    Vec::new(),
                                    vec![grant],
                                    Vec::new(),
                                )])
                            })
                        });
                        resources
                            .expect_search_many()
                            .returning(|addresses, caller| {
                                let user_id = caller.id();
                                let found = addresses
                                    .iter()
                                    .map(|address| {
                                        let resource = crate::domain::model::Resource::from_stored(
                                            1,
                                            address.kind(),
                                            address.id(),
                                            Some(user_id),
                                            None,
                                            crate::domain::model::Visibility::Private,
                                        );
                                        let grant = crate::domain::model::Grant::from_stored(
                                            1,
                                            crate::domain::model::Principal::User(user_id),
                                            1,
                                            crate::domain::model::ResourceRole::Downloader,
                                            crate::domain::model::ChildKind::All,
                                            crate::domain::model::GrantEffect::Allow,
                                        );
                                        crate::domain::model::ResourceAccess::new(
                                            resource,
                                            Vec::new(),
                                            vec![grant],
                                            Vec::new(),
                                        )
                                    })
                                    .collect::<Vec<_>>();
                                Box::pin(async move { Ok(found) })
                            });
                        resources.expect_create().returning(|new_resource| {
                            let resource = crate::domain::model::Resource::from_stored(
                                1,
                                new_resource.kind,
                                new_resource.entity_id,
                                new_resource.owner_id,
                                None,
                                new_resource.visibility,
                            );
                            Box::pin(async move { Ok(resource) })
                        });
                        resources
                    })();
                    resources
                        .expect_delete()
                        .returning(|_| Box::pin(async { Ok(true) }));
                    resources
                })(),
            )
        })(users, albums);
        let use_case: UseCase =
            ApplyMusicAlbum::new(Arc::clone(&harness.factory), Some(Arc::new(provider)));

        // Act
        let result = use_case
            .execute(ApplyMusicAlbumCommand::new(1, 7, reference(), Vec::new()))
            .await;

        // Assert
        assert!(matches!(result, Err(ApplyMusicAlbumError::ProviderFailure)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    // proves: SCN-METADATA-ENRICHMENT-028-F — Negative
    #[tokio::test]
    async fn apply_music_album_rejects_the_default_album() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository,
               caller_id: NumericID|
              -> Result<(), Box<dyn Error>> {
            let caller =
                User::try_new(caller_id, "admin".to_owned(), None, caller_id, Role::Admin)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        (move |albums: &mut MockMusicAlbumRepository, album: MusicAlbum| {
            let album_id = album.id();
            albums
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(album_id))
                .return_once(move |_| {
                    let found = album.clone();
                    Box::pin(async move { Ok(vec![found]) })
                });
        })(&mut albums, sample_album(0, Vec::new())?);
        let harness = (move |users: MockUserRepository,
                             albums: MockMusicAlbumRepository|
              -> GalleryFactoryHarness {
            harness_with_resources(
                users,
                albums,
                (move || -> crate::domain::port::MockResourceRepository {
                    let mut resources = (move || -> crate::domain::port::MockResourceRepository {
                        let mut resources = crate::domain::port::MockResourceRepository::new();
                        resources.expect_search().returning(|filter, caller| {
                            let user_id = caller.id();
                            let resource = crate::domain::model::Resource::from_stored(
                                1,
                                filter
                                    .kind
                                    .unwrap_or(crate::domain::model::ResourceKind::Library),
                                filter.entity_id.unwrap_or_default(),
                                Some(user_id),
                                None,
                                crate::domain::model::Visibility::Private,
                            );
                            let grant = crate::domain::model::Grant::from_stored(
                                1,
                                crate::domain::model::Principal::User(user_id),
                                1,
                                crate::domain::model::ResourceRole::Downloader,
                                crate::domain::model::ChildKind::All,
                                crate::domain::model::GrantEffect::Allow,
                            );
                            Box::pin(async move {
                                Ok(vec![crate::domain::model::ResourceAccess::new(
                                    resource,
                                    Vec::new(),
                                    vec![grant],
                                    Vec::new(),
                                )])
                            })
                        });
                        resources
                            .expect_search_many()
                            .returning(|addresses, caller| {
                                let user_id = caller.id();
                                let found = addresses
                                    .iter()
                                    .map(|address| {
                                        let resource = crate::domain::model::Resource::from_stored(
                                            1,
                                            address.kind(),
                                            address.id(),
                                            Some(user_id),
                                            None,
                                            crate::domain::model::Visibility::Private,
                                        );
                                        let grant = crate::domain::model::Grant::from_stored(
                                            1,
                                            crate::domain::model::Principal::User(user_id),
                                            1,
                                            crate::domain::model::ResourceRole::Downloader,
                                            crate::domain::model::ChildKind::All,
                                            crate::domain::model::GrantEffect::Allow,
                                        );
                                        crate::domain::model::ResourceAccess::new(
                                            resource,
                                            Vec::new(),
                                            vec![grant],
                                            Vec::new(),
                                        )
                                    })
                                    .collect::<Vec<_>>();
                                Box::pin(async move { Ok(found) })
                            });
                        resources.expect_create().returning(|new_resource| {
                            let resource = crate::domain::model::Resource::from_stored(
                                1,
                                new_resource.kind,
                                new_resource.entity_id,
                                new_resource.owner_id,
                                None,
                                new_resource.visibility,
                            );
                            Box::pin(async move { Ok(resource) })
                        });
                        resources
                    })();
                    resources
                        .expect_delete()
                        .returning(|_| Box::pin(async { Ok(true) }));
                    resources
                })(),
            )
        })(users, albums);
        let use_case: UseCase = ApplyMusicAlbum::new(
            Arc::clone(&harness.factory),
            Some(Arc::new(MockMusicMetadataProvider::new())),
        );

        // Act
        let result = use_case
            .execute(ApplyMusicAlbumCommand::new(1, 0, reference(), Vec::new()))
            .await;

        // Assert
        assert!(matches!(result, Err(ApplyMusicAlbumError::DefaultAlbum)));
        Ok(())
    }

    // proves: SCN-METADATA-ENRICHMENT-024-E — Error handling
    #[tokio::test]
    async fn apply_music_album_without_a_provider_is_unavailable() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository,
               caller_id: NumericID|
              -> Result<(), Box<dyn Error>> {
            let caller =
                User::try_new(caller_id, "admin".to_owned(), None, caller_id, Role::Admin)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        (move |albums: &mut MockMusicAlbumRepository, album: MusicAlbum| {
            let album_id = album.id();
            albums
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(album_id))
                .return_once(move |_| {
                    let found = album.clone();
                    Box::pin(async move { Ok(vec![found]) })
                });
        })(&mut albums, sample_album(7, Vec::new())?);
        let harness = (move |users: MockUserRepository,
                             albums: MockMusicAlbumRepository|
              -> GalleryFactoryHarness {
            harness_with_resources(
                users,
                albums,
                (move || -> crate::domain::port::MockResourceRepository {
                    let mut resources = (move || -> crate::domain::port::MockResourceRepository {
                        let mut resources = crate::domain::port::MockResourceRepository::new();
                        resources.expect_search().returning(|filter, caller| {
                            let user_id = caller.id();
                            let resource = crate::domain::model::Resource::from_stored(
                                1,
                                filter
                                    .kind
                                    .unwrap_or(crate::domain::model::ResourceKind::Library),
                                filter.entity_id.unwrap_or_default(),
                                Some(user_id),
                                None,
                                crate::domain::model::Visibility::Private,
                            );
                            let grant = crate::domain::model::Grant::from_stored(
                                1,
                                crate::domain::model::Principal::User(user_id),
                                1,
                                crate::domain::model::ResourceRole::Downloader,
                                crate::domain::model::ChildKind::All,
                                crate::domain::model::GrantEffect::Allow,
                            );
                            Box::pin(async move {
                                Ok(vec![crate::domain::model::ResourceAccess::new(
                                    resource,
                                    Vec::new(),
                                    vec![grant],
                                    Vec::new(),
                                )])
                            })
                        });
                        resources
                            .expect_search_many()
                            .returning(|addresses, caller| {
                                let user_id = caller.id();
                                let found = addresses
                                    .iter()
                                    .map(|address| {
                                        let resource = crate::domain::model::Resource::from_stored(
                                            1,
                                            address.kind(),
                                            address.id(),
                                            Some(user_id),
                                            None,
                                            crate::domain::model::Visibility::Private,
                                        );
                                        let grant = crate::domain::model::Grant::from_stored(
                                            1,
                                            crate::domain::model::Principal::User(user_id),
                                            1,
                                            crate::domain::model::ResourceRole::Downloader,
                                            crate::domain::model::ChildKind::All,
                                            crate::domain::model::GrantEffect::Allow,
                                        );
                                        crate::domain::model::ResourceAccess::new(
                                            resource,
                                            Vec::new(),
                                            vec![grant],
                                            Vec::new(),
                                        )
                                    })
                                    .collect::<Vec<_>>();
                                Box::pin(async move { Ok(found) })
                            });
                        resources.expect_create().returning(|new_resource| {
                            let resource = crate::domain::model::Resource::from_stored(
                                1,
                                new_resource.kind,
                                new_resource.entity_id,
                                new_resource.owner_id,
                                None,
                                new_resource.visibility,
                            );
                            Box::pin(async move { Ok(resource) })
                        });
                        resources
                    })();
                    resources
                        .expect_delete()
                        .returning(|_| Box::pin(async { Ok(true) }));
                    resources
                })(),
            )
        })(users, albums);
        let use_case: UseCase = ApplyMusicAlbum::new(Arc::clone(&harness.factory), None);

        // Act
        let result = use_case
            .execute(ApplyMusicAlbumCommand::new(1, 7, reference(), Vec::new()))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(ApplyMusicAlbumError::ProviderUnavailable)
        ));
        Ok(())
    }

    // proves: SCN-METADATA-ENRICHMENT-028-G — Negative
    #[tokio::test]
    async fn apply_music_album_rejects_a_placement_naming_a_medium_it_does_not_create()
    -> Result<(), Box<dyn Error>> {
        // Arrange: a single-CD release materialises one digital medium, so a
        // placement at medium 2 names a medium the apply never creates.
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository,
               caller_id: NumericID|
              -> Result<(), Box<dyn Error>> {
            let caller =
                User::try_new(caller_id, "admin".to_owned(), None, caller_id, Role::Admin)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        (move |albums: &mut MockMusicAlbumRepository, album: MusicAlbum| {
            let album_id = album.id();
            albums
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(album_id))
                .return_once(move |_| {
                    let found = album.clone();
                    Box::pin(async move { Ok(vec![found]) })
                });
        })(&mut albums, sample_album(7, Vec::new())?);
        albums.expect_save().times(0);
        albums.expect_save_medium().times(0);
        let mut provider = MockMusicMetadataProvider::new();
        (move |provider: &mut MockMusicMetadataProvider, metadata: MusicMetadata| {
            provider
                .expect_provider()
                .returning(|| ProviderId::MusicBrainz);
            provider.expect_lookup().times(1).return_once(move |_| {
                let resolved = metadata.clone();
                Box::pin(async move { Ok(Some(resolved)) })
            });
        })(
            &mut provider,
            album_metadata(vec![MediumSummary::new(MediumFormat::Cd, 1, 5)]),
        );
        let harness = (move |users: MockUserRepository,
                             albums: MockMusicAlbumRepository|
              -> GalleryFactoryHarness {
            harness_with_resources(
                users,
                albums,
                (move || -> crate::domain::port::MockResourceRepository {
                    let mut resources = (move || -> crate::domain::port::MockResourceRepository {
                        let mut resources = crate::domain::port::MockResourceRepository::new();
                        resources.expect_search().returning(|filter, caller| {
                            let user_id = caller.id();
                            let resource = crate::domain::model::Resource::from_stored(
                                1,
                                filter
                                    .kind
                                    .unwrap_or(crate::domain::model::ResourceKind::Library),
                                filter.entity_id.unwrap_or_default(),
                                Some(user_id),
                                None,
                                crate::domain::model::Visibility::Private,
                            );
                            let grant = crate::domain::model::Grant::from_stored(
                                1,
                                crate::domain::model::Principal::User(user_id),
                                1,
                                crate::domain::model::ResourceRole::Downloader,
                                crate::domain::model::ChildKind::All,
                                crate::domain::model::GrantEffect::Allow,
                            );
                            Box::pin(async move {
                                Ok(vec![crate::domain::model::ResourceAccess::new(
                                    resource,
                                    Vec::new(),
                                    vec![grant],
                                    Vec::new(),
                                )])
                            })
                        });
                        resources
                            .expect_search_many()
                            .returning(|addresses, caller| {
                                let user_id = caller.id();
                                let found = addresses
                                    .iter()
                                    .map(|address| {
                                        let resource = crate::domain::model::Resource::from_stored(
                                            1,
                                            address.kind(),
                                            address.id(),
                                            Some(user_id),
                                            None,
                                            crate::domain::model::Visibility::Private,
                                        );
                                        let grant = crate::domain::model::Grant::from_stored(
                                            1,
                                            crate::domain::model::Principal::User(user_id),
                                            1,
                                            crate::domain::model::ResourceRole::Downloader,
                                            crate::domain::model::ChildKind::All,
                                            crate::domain::model::GrantEffect::Allow,
                                        );
                                        crate::domain::model::ResourceAccess::new(
                                            resource,
                                            Vec::new(),
                                            vec![grant],
                                            Vec::new(),
                                        )
                                    })
                                    .collect::<Vec<_>>();
                                Box::pin(async move { Ok(found) })
                            });
                        resources.expect_create().returning(|new_resource| {
                            let resource = crate::domain::model::Resource::from_stored(
                                1,
                                new_resource.kind,
                                new_resource.entity_id,
                                new_resource.owner_id,
                                None,
                                new_resource.visibility,
                            );
                            Box::pin(async move { Ok(resource) })
                        });
                        resources
                    })();
                    resources
                        .expect_delete()
                        .returning(|_| Box::pin(async { Ok(true) }));
                    resources
                })(),
            )
        })(users, albums);
        let use_case: UseCase =
            ApplyMusicAlbum::new(Arc::clone(&harness.factory), Some(Arc::new(provider)));

        // Act
        let result = use_case
            .execute(ApplyMusicAlbumCommand::new(
                1,
                7,
                reference(),
                vec![AlbumPlacement::new(2, 1, 5)],
            ))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(ApplyMusicAlbumError::InvalidPlacement)
        ));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    // proves: SCN-AUTHORIZATION-038-B — Security
    #[tokio::test]
    async fn apply_music_album_refuses_a_caller_without_edit_metadata() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the caller may view the public album but may not edit it.
        let mut users = MockUserRepository::new();
        (move |users: &mut crate::domain::port::MockUserRepository,
               caller_id: crate::domain::NumericID|
              -> Result<(), Box<dyn std::error::Error>> {
            let caller = crate::domain::model::User::try_new(
                caller_id,
                "caller".to_owned(),
                None,
                caller_id,
                crate::domain::model::Role::Standard,
            )?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        (move |albums: &mut MockMusicAlbumRepository, album: MusicAlbum| {
            let album_id = album.id();
            albums
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(album_id))
                .return_once(move |_| {
                    let found = album.clone();
                    Box::pin(async move { Ok(vec![found]) })
                });
        })(&mut albums, sample_album(7, Vec::new())?);
        albums.expect_save().times(0);
        let harness = harness_with_resources(
            users,
            albums,
            (move |visibility: Visibility| -> MockResourceRepository {
                let mut resources = MockResourceRepository::new();
                resources.expect_search().returning(move |_, _| {
                    let resource = Resource::from_stored(
                        1,
                        ResourceKind::MusicAlbum,
                        7,
                        Some(2),
                        None,
                        visibility,
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
            })(Visibility::Public),
        );
        let use_case: UseCase = ApplyMusicAlbum::new(Arc::clone(&harness.factory), None);

        // Act
        let result = use_case
            .execute(ApplyMusicAlbumCommand::new(1, 7, reference(), Vec::new()))
            .await;

        // Assert
        assert!(matches!(result, Err(ApplyMusicAlbumError::Forbidden)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    // proves: SCN-AUTHORIZATION-038-A — Security
    #[tokio::test]
    async fn apply_music_album_conceals_an_album_the_caller_cannot_view()
    -> Result<(), Box<dyn Error>> {
        // Arrange: a private album owned by another user is not the caller's to see.
        let mut users = MockUserRepository::new();
        (move |users: &mut crate::domain::port::MockUserRepository,
               caller_id: crate::domain::NumericID|
              -> Result<(), Box<dyn std::error::Error>> {
            let caller = crate::domain::model::User::try_new(
                caller_id,
                "caller".to_owned(),
                None,
                caller_id,
                crate::domain::model::Role::Standard,
            )?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        (move |albums: &mut MockMusicAlbumRepository, album: MusicAlbum| {
            let album_id = album.id();
            albums
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(album_id))
                .return_once(move |_| {
                    let found = album.clone();
                    Box::pin(async move { Ok(vec![found]) })
                });
        })(&mut albums, sample_album(7, Vec::new())?);
        albums.expect_save().times(0);
        let harness = harness_with_resources(
            users,
            albums,
            (move |visibility: Visibility| -> MockResourceRepository {
                let mut resources = MockResourceRepository::new();
                resources.expect_search().returning(move |_, _| {
                    let resource = Resource::from_stored(
                        1,
                        ResourceKind::MusicAlbum,
                        7,
                        Some(2),
                        None,
                        visibility,
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
            })(Visibility::Private),
        );
        let use_case: UseCase = ApplyMusicAlbum::new(Arc::clone(&harness.factory), None);

        // Act
        let result = use_case
            .execute(ApplyMusicAlbumCommand::new(1, 7, reference(), Vec::new()))
            .await;

        // Assert
        assert!(matches!(result, Err(ApplyMusicAlbumError::NoSuchAlbum)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    // proves: SCN-METADATA-ENRICHMENT-024-D — Negative
    #[tokio::test]
    async fn apply_music_album_unknown_reference_is_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository,
               caller_id: NumericID|
              -> Result<(), Box<dyn Error>> {
            let caller =
                User::try_new(caller_id, "admin".to_owned(), None, caller_id, Role::Admin)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        (move |albums: &mut MockMusicAlbumRepository, album: MusicAlbum| {
            let album_id = album.id();
            albums
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(album_id))
                .return_once(move |_| {
                    let found = album.clone();
                    Box::pin(async move { Ok(vec![found]) })
                });
        })(&mut albums, sample_album(7, Vec::new())?);
        let mut provider = MockMusicMetadataProvider::new();
        provider
            .expect_provider()
            .returning(|| ProviderId::MusicBrainz);
        provider
            .expect_lookup()
            .times(1)
            .returning(|_| Box::pin(async { Ok(None) }));
        let harness = (move |users: MockUserRepository,
                             albums: MockMusicAlbumRepository|
              -> GalleryFactoryHarness {
            harness_with_resources(
                users,
                albums,
                (move || -> crate::domain::port::MockResourceRepository {
                    let mut resources = (move || -> crate::domain::port::MockResourceRepository {
                        let mut resources = crate::domain::port::MockResourceRepository::new();
                        resources.expect_search().returning(|filter, caller| {
                            let user_id = caller.id();
                            let resource = crate::domain::model::Resource::from_stored(
                                1,
                                filter
                                    .kind
                                    .unwrap_or(crate::domain::model::ResourceKind::Library),
                                filter.entity_id.unwrap_or_default(),
                                Some(user_id),
                                None,
                                crate::domain::model::Visibility::Private,
                            );
                            let grant = crate::domain::model::Grant::from_stored(
                                1,
                                crate::domain::model::Principal::User(user_id),
                                1,
                                crate::domain::model::ResourceRole::Downloader,
                                crate::domain::model::ChildKind::All,
                                crate::domain::model::GrantEffect::Allow,
                            );
                            Box::pin(async move {
                                Ok(vec![crate::domain::model::ResourceAccess::new(
                                    resource,
                                    Vec::new(),
                                    vec![grant],
                                    Vec::new(),
                                )])
                            })
                        });
                        resources
                            .expect_search_many()
                            .returning(|addresses, caller| {
                                let user_id = caller.id();
                                let found = addresses
                                    .iter()
                                    .map(|address| {
                                        let resource = crate::domain::model::Resource::from_stored(
                                            1,
                                            address.kind(),
                                            address.id(),
                                            Some(user_id),
                                            None,
                                            crate::domain::model::Visibility::Private,
                                        );
                                        let grant = crate::domain::model::Grant::from_stored(
                                            1,
                                            crate::domain::model::Principal::User(user_id),
                                            1,
                                            crate::domain::model::ResourceRole::Downloader,
                                            crate::domain::model::ChildKind::All,
                                            crate::domain::model::GrantEffect::Allow,
                                        );
                                        crate::domain::model::ResourceAccess::new(
                                            resource,
                                            Vec::new(),
                                            vec![grant],
                                            Vec::new(),
                                        )
                                    })
                                    .collect::<Vec<_>>();
                                Box::pin(async move { Ok(found) })
                            });
                        resources.expect_create().returning(|new_resource| {
                            let resource = crate::domain::model::Resource::from_stored(
                                1,
                                new_resource.kind,
                                new_resource.entity_id,
                                new_resource.owner_id,
                                None,
                                new_resource.visibility,
                            );
                            Box::pin(async move { Ok(resource) })
                        });
                        resources
                    })();
                    resources
                        .expect_delete()
                        .returning(|_| Box::pin(async { Ok(true) }));
                    resources
                })(),
            )
        })(users, albums);
        let use_case: UseCase =
            ApplyMusicAlbum::new(Arc::clone(&harness.factory), Some(Arc::new(provider)));

        // Act
        let result = use_case
            .execute(ApplyMusicAlbumCommand::new(1, 7, reference(), Vec::new()))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(ApplyMusicAlbumError::UnknownReference)
        ));
        Ok(())
    }
}
