use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::apply_music_album::ApplyMusicAlbumCommand;
use crate::application::port::apply_music_album::ApplyMusicAlbumError;
use crate::application::port::apply_music_album::ApplyMusicAlbumResponse;
use crate::application::port::apply_music_album::ApplyMusicAlbumUseCase;
use crate::application::security_event;
use crate::domain::model::music_album::MusicAlbum;
use crate::domain::model::music_candidate::MusicTarget;
use crate::domain::model::music_medium::MediumIndex;
use crate::domain::model::music_metadata::Edition;
use crate::domain::model::music_metadata::MusicMetadata;
use crate::domain::model::permission::Action;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::music_album_repository::MusicAlbumFilter;
use crate::domain::port::music_album_repository::MusicAlbumRepository as _;
use crate::domain::port::music_metadata_provider::MusicMetadataProvider;
use crate::domain::port::permission_repository::PermissionFilter;
use crate::domain::port::permission_repository::PermissionRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::service::authorization::Authorization;
use crate::domain::service::authorization::Decision;

/// The genre an applied album carries when the provider reports none.
const DEFAULT_GENRE: &str = "UNKNOWN";

/// The index of the single digital medium an apply materialises.
const DIGITAL_MEDIUM_INDEX: u8 = 1;

/// Use case implementation for applying external metadata to a music album.
pub struct ApplyMusicAlbum<F, P> {
    /// Configured provider, absent when music identification is unavailable.
    metadata_provider: Option<Arc<P>>,
    /// Factory opening the unit of work wrapping the write.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, P: MusicMetadataProvider> ApplyMusicAlbum<F, P> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>, metadata_provider: Option<Arc<P>>) -> Self {
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

                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| ApplyMusicAlbumError::Unknown(error.into()))?;

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

                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::LibraryRead, &album)
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(ApplyMusicAlbumError::NoSuchAlbum);
                }
                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::LibraryManage, &album)
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(if denial.conceals() {
                        ApplyMusicAlbumError::NoSuchAlbum
                    } else {
                        ApplyMusicAlbumError::Forbidden
                    });
                }

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

                let updated = MusicAlbum::try_new(
                    album.id(),
                    title,
                    release_date,
                    production_type,
                    applied_genres,
                    album.music_group_id(),
                    album.music_group_ids().map(<[_]>::to_vec),
                    album.cover_image_id(),
                    album.mediums().to_vec(),
                )
                .map_err(|_| ApplyMusicAlbumError::InvalidReference)?;

                unit_of_work
                    .music_albums()
                    .update(updated)
                    .await
                    .map_err(|error| {
                        if matches!(
                            error,
                            RepositoryError::AlreadyExist | RepositoryError::Conflict
                        ) {
                            ApplyMusicAlbumError::Conflict
                        } else {
                            ApplyMusicAlbumError::Unknown(error.into())
                        }
                    })?
                    .ok_or(ApplyMusicAlbumError::NoSuchAlbum)?;

                unit_of_work
                    .music_albums()
                    .replace_mediums(album.id(), &mediums)
                    .await
                    .map_err(|error| {
                        if matches!(error, RepositoryError::Conflict) {
                            ApplyMusicAlbumError::Conflict
                        } else {
                            ApplyMusicAlbumError::Unknown(error.into())
                        }
                    })?;

                if !command.placements().is_empty() {
                    let placements: Vec<(u8, u8, i64)> = command
                        .placements()
                        .iter()
                        .map(|placement| {
                            (
                                placement.medium_index(),
                                placement.track_index(),
                                placement.recording_id(),
                            )
                        })
                        .collect();
                    unit_of_work
                        .music_albums()
                        .place_tracks(album.id(), &placements)
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
                }

                let stored = unit_of_work
                    .music_albums()
                    .search(&MusicAlbumFilter {
                        id: Some(album.id()),
                        ..MusicAlbumFilter::default()
                    })
                    .await
                    .map_err(|error| ApplyMusicAlbumError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(ApplyMusicAlbumError::NoSuchAlbum)?;

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
    use std::error::Error;
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use chrono::NaiveDate;

    use crate::application::port::apply_music_album::AlbumPlacement;
    use crate::application::port::apply_music_album::ApplyMusicAlbumCommand;
    use crate::application::port::apply_music_album::ApplyMusicAlbumError;
    use crate::application::port::apply_music_album::ApplyMusicAlbumUseCase as _;
    use crate::domain::alias::NumericID;
    use crate::domain::model::media_candidate::ExternalRef;
    use crate::domain::model::media_candidate::ProviderId;
    use crate::domain::model::music_album::MusicAlbum;
    use crate::domain::model::music_album::ProductionType;
    use crate::domain::model::music_medium::MediumIndex;
    use crate::domain::model::music_medium::MusicMedium;
    use crate::domain::model::music_metadata::Edition;
    use crate::domain::model::music_metadata::MediumFormat;
    use crate::domain::model::music_metadata::MediumSummary;
    use crate::domain::model::music_metadata::MusicMetadata;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::port::error::ExternalServiceError;
    use crate::domain::port::music_album_repository::MockMusicAlbumRepository;
    use crate::domain::port::music_metadata_provider::MockMusicMetadataProvider;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::GalleryFactoryHarness;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::TestUnitOfWork;
    use crate::test_helpers::expect_caller;
    use crate::test_helpers::no_permissions;
    use crate::test_helpers::sample_album;

    use super::ApplyMusicAlbum;

    /// The use case under test, bound to the mocked unit-of-work factory and
    /// provider.
    type UseCase = ApplyMusicAlbum<TestFactory, MockMusicMetadataProvider>;

    /// The reference every test resolves.
    fn reference() -> ExternalRef {
        ExternalRef::new(ProviderId::MusicBrainz, "release-group".to_owned())
    }

    /// Expect the caller load to find an administrator.
    fn expect_admin(
        users: &mut MockUserRepository,
        caller_id: NumericID,
    ) -> Result<(), Box<dyn Error>> {
        let caller = User::try_new(caller_id, "admin".to_owned(), None, caller_id, Role::Admin)?;
        users
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(caller_id))
            .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
        Ok(())
    }

    /// Build a Library test harness around the given repository.
    fn harness_with(
        users: MockUserRepository,
        albums: MockMusicAlbumRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            music_albums: albums,
            permissions: no_permissions(),
            users,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
    }

    /// Expect the album load to find `album`.
    fn expect_album(albums: &mut MockMusicAlbumRepository, album: MusicAlbum) {
        let album_id = album.id();
        albums
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(album_id))
            .return_once(move |_| {
                let found = album.clone();
                Box::pin(async move { Ok(vec![found]) })
            });
    }

    /// Expect the post-write album reload.
    fn expect_reload(albums: &mut MockMusicAlbumRepository, album: MusicAlbum) {
        albums.expect_search().times(1).return_once(move |_| {
            let found = album.clone();
            Box::pin(async move { Ok(vec![found]) })
        });
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

    /// Expect the provider lookup to return `metadata`.
    fn expect_lookup(provider: &mut MockMusicMetadataProvider, metadata: MusicMetadata) {
        provider
            .expect_provider()
            .returning(|| ProviderId::MusicBrainz);
        provider.expect_lookup().times(1).return_once(move |_| {
            let resolved = metadata.clone();
            Box::pin(async move { Ok(Some(resolved)) })
        });
    }

    /// The album the write stores, with the applied `DIGITAL` medium.
    fn applied_album(id: NumericID) -> Result<MusicAlbum, Box<dyn Error>> {
        sample_album(
            id,
            vec![MusicMedium::new(MediumIndex::Digital(1), Vec::new())],
        )
    }

    #[tokio::test]
    async fn apply_music_album_without_placements_writes_the_structure()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, sample_album(7, Vec::new())?);
        albums
            .expect_update()
            .times(1)
            .withf(|album| {
                album.id() == 7
                    && album.name() == "Kind of Blue"
                    && album.genre_ids() == ["JAZZ".to_owned()]
                    && album.production_type() == Some(ProductionType::Studio)
            })
            .returning(|album| Box::pin(async move { Ok(Some(album)) }));
        albums
            .expect_replace_mediums()
            .times(1)
            .withf(|album_id, mediums| *album_id == 7 && mediums == [MediumIndex::Digital(1)])
            .returning(|_, _| Box::pin(async { Ok(()) }));
        albums.expect_place_tracks().times(0);
        expect_reload(&mut albums, applied_album(7)?);
        let mut provider = MockMusicMetadataProvider::new();
        expect_lookup(
            &mut provider,
            album_metadata(vec![MediumSummary::new(MediumFormat::Cd, 1, 5)]),
        );
        let harness = harness_with(users, albums);
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

    #[tokio::test]
    async fn apply_music_album_with_placements_moves_the_recordings() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, sample_album(7, Vec::new())?);
        albums
            .expect_update()
            .times(1)
            .returning(|album| Box::pin(async move { Ok(Some(album)) }));
        albums
            .expect_replace_mediums()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        albums
            .expect_place_tracks()
            .times(1)
            .withf(|album_id, placements| *album_id == 7 && placements == [(1, 1, 5)])
            .returning(|_, _| Box::pin(async { Ok(()) }));
        expect_reload(&mut albums, applied_album(7)?);
        let mut provider = MockMusicMetadataProvider::new();
        expect_lookup(
            &mut provider,
            album_metadata(vec![MediumSummary::new(MediumFormat::Cd, 1, 5)]),
        );
        let harness = harness_with(users, albums);
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

    #[tokio::test]
    async fn apply_music_album_multi_cd_release_becomes_cd_mediums() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, sample_album(7, Vec::new())?);
        albums
            .expect_update()
            .times(1)
            .returning(|album| Box::pin(async move { Ok(Some(album)) }));
        albums
            .expect_replace_mediums()
            .times(1)
            .withf(|album_id, mediums| {
                *album_id == 7 && mediums == [MediumIndex::Cd(1), MediumIndex::Cd(2)]
            })
            .returning(|_, _| Box::pin(async { Ok(()) }));
        expect_reload(&mut albums, sample_album(7, Vec::new())?);
        let mut provider = MockMusicMetadataProvider::new();
        expect_lookup(
            &mut provider,
            album_metadata(vec![
                MediumSummary::new(MediumFormat::Cd, 1, 5),
                MediumSummary::new(MediumFormat::Cd, 2, 4),
            ]),
        );
        let harness = harness_with(users, albums);
        let use_case: UseCase =
            ApplyMusicAlbum::new(Arc::clone(&harness.factory), Some(Arc::new(provider)));

        // Act
        use_case
            .execute(ApplyMusicAlbumCommand::new(1, 7, reference(), Vec::new()))
            .await?;

        // Assert: the assertion lives in the `replace_mediums` expectation.
        Ok(())
    }

    #[tokio::test]
    async fn apply_music_album_provider_failure_writes_nothing() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, sample_album(7, Vec::new())?);
        albums.expect_update().times(0);
        let mut provider = MockMusicMetadataProvider::new();
        provider
            .expect_provider()
            .returning(|| ProviderId::MusicBrainz);
        provider
            .expect_lookup()
            .times(1)
            .returning(|_| Box::pin(async { Err(ExternalServiceError::Unavailable) }));
        let harness = harness_with(users, albums);
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

    #[tokio::test]
    async fn apply_music_album_rejects_the_default_album() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, sample_album(0, Vec::new())?);
        let harness = harness_with(users, albums);
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

    #[tokio::test]
    async fn apply_music_album_without_a_provider_is_unavailable() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, sample_album(7, Vec::new())?);
        let harness = harness_with(users, albums);
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

    #[tokio::test]
    async fn apply_music_album_unknown_reference_is_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, sample_album(7, Vec::new())?);
        let mut provider = MockMusicMetadataProvider::new();
        provider
            .expect_provider()
            .returning(|| ProviderId::MusicBrainz);
        provider
            .expect_lookup()
            .times(1)
            .returning(|_| Box::pin(async { Ok(None) }));
        let harness = harness_with(users, albums);
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

    #[tokio::test]
    async fn apply_music_album_standard_user_is_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, sample_album(7, Vec::new())?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = ApplyMusicAlbum::new(
            Arc::clone(&harness.factory),
            Some(Arc::new(MockMusicMetadataProvider::new())),
        );

        // Act
        let result = use_case
            .execute(ApplyMusicAlbumCommand::new(1, 7, reference(), Vec::new()))
            .await;

        // Assert
        assert!(matches!(result, Err(ApplyMusicAlbumError::Forbidden)));
        Ok(())
    }
}
