use crate::application::authorization::authorize;
use crate::domain::model::{ResourceKind, ResourceRef};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::{
    ApplySeriesCommand, ApplySeriesError, ApplySeriesResponse, ApplySeriesUseCase,
};
use crate::domain::model::{Action, MediaMetadata, Series, Target};
use crate::domain::port::{
    IdentityUnitOfWork, LibraryUnitOfWork, RepositoryError, SeriesFilter, SeriesRepository as _,
    UnitOfWork as _, UnitOfWorkFactory, UserFilter, UserRepository as _, UserUnitOfWork,
    VideoMetadataProvider,
};

/// Use case implementation for applying external metadata to a show.
///
/// The write is authoritative for the show's metadata only: the provider
/// reports no season tree, so the apply creates no season and no episode, and
/// the cover, seasons and episodes keep their stored values.
pub(crate) struct ApplySeries<F, P> {
    /// Configured provider chain, absent when video identification is unavailable.
    metadata_provider: Option<Arc<P>>,
    /// Factory opening the unit of work wrapping the write.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, P: VideoMetadataProvider> ApplySeries<F, P> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>, metadata_provider: Option<Arc<P>>) -> Self {
        Self {
            metadata_provider,
            unit_of_work_factory,
        }
    }
}

impl<F, P> ApplySeriesUseCase for ApplySeries<F, P>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
    P: VideoMetadataProvider,
{
    fn execute<'future>(
        &'future self,
        command: ApplySeriesCommand,
    ) -> Pin<Box<dyn Future<Output = Result<ApplySeriesResponse, ApplySeriesError>> + Send + 'future>>
    {
        let metadata_provider = self.metadata_provider.as_ref().map(Arc::clone);
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| ApplySeriesError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| ApplySeriesError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(ApplySeriesError::NoSuchCaller)?;

                let series = unit_of_work
                    .series()
                    .search(&SeriesFilter {
                        id: Some(command.series_id()),
                        ..SeriesFilter::default()
                    })
                    .await
                    .map_err(|error| ApplySeriesError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(ApplySeriesError::NoSuchSeries)?;

                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::Series, series.id()),
                    Action::ViewMetadata,
                    None,
                )
                .await
                .map_err(|_| ApplySeriesError::NoSuchSeries)?;
                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::Series, series.id()),
                    Action::EditMetadata,
                    None,
                )
                .await
                .map_err(|error| {
                    if error.conceals() {
                        ApplySeriesError::NoSuchSeries
                    } else {
                        ApplySeriesError::Forbidden
                    }
                })?;

                let Some(provider) = metadata_provider else {
                    return Err(ApplySeriesError::ProviderUnavailable);
                };

                let target = Target::Series(command.reference().clone());
                let metadata = provider
                    .lookup(&target)
                    .await
                    .map_err(|_| ApplySeriesError::ProviderFailure)?
                    .ok_or(ApplySeriesError::UnknownReference)?;

                let MediaMetadata::Series {
                    country_of_origin,
                    first_air_date,
                    genres,
                    plot,
                    title,
                    ..
                } = metadata
                else {
                    return Err(ApplySeriesError::InvalidReference);
                };

                let updated = Series::try_new(
                    series.id(),
                    title,
                    plot,
                    country_of_origin,
                    first_air_date,
                    genres
                        .iter()
                        .map(|genre| genre.as_str().to_owned())
                        .collect(),
                    series.cover_image_id(),
                    series.seasons().to_vec(),
                )
                .map_err(|_| ApplySeriesError::InvalidReference)?;

                let stored = unit_of_work.series().save(updated).await.map_err(|error| {
                    if matches!(
                        error,
                        RepositoryError::AlreadyExist | RepositoryError::Conflict
                    ) {
                        ApplySeriesError::Conflict
                    } else {
                        ApplySeriesError::Unknown(error.into())
                    }
                })?;

                Ok(ApplySeriesResponse::new(stored))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| ApplySeriesError::Unknown(error.into()))?;
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
    use rstest::fixture;

    use crate::domain::model::{ExternalRef, Genre, ProviderId, Role, User};

    use crate::domain::port::{
        MockResourceRepository, MockSeriesRepository, MockUserRepository, MockVideoMetadataProvider,
    };

    use crate::test_helpers::*;
    use chrono::NaiveDate;
    use std::error::Error;

    type UseCase = ApplySeries<TestFactory, MockVideoMetadataProvider>;

    fn harness_with(
        users: MockUserRepository,
        resources: MockResourceRepository,
        series: MockSeriesRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            resources,
            series,
            users,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
    }

    fn use_case(
        harness: &GalleryFactoryHarness,
        provider: Option<MockVideoMetadataProvider>,
    ) -> UseCase {
        ApplySeries::new(Arc::clone(&harness.factory), provider.map(Arc::new))
    }

    #[fixture]
    fn command() -> ApplySeriesCommand {
        ApplySeriesCommand::new(1, 7, ExternalRef::new(ProviderId::Tmdb, "1920".to_owned()))
    }

    #[fixture]
    fn series_metadata() -> MediaMetadata {
        MediaMetadata::Series {
            artwork: None,
            country_of_origin: Some("GB".to_owned()),
            first_air_date: NaiveDate::from_ymd_opt(1991, 1, 2),
            genres: vec![Genre::Drama],
            plot: "A new plot".to_owned(),
            title: "A new name".to_owned(),
        }
    }

    // proves: SCN-METADATA-ENRICHMENT-024-A, SCN-METADATA-ENRICHMENT-026-C — Business rule
    // proves: SCN-METADATA-ENRICHMENT-026-A — Positive
    #[tokio::test]
    async fn apply_series_writes_the_reported_metadata_and_keeps_the_tree()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the show already holds an episode.
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
        let stored = sample_series(7, vec![sample_season(1, vec![sample_episode(1, 20)?])])?;
        let mut series = MockSeriesRepository::new();
        (move |series: &mut MockSeriesRepository, stored: Series| {
            series.expect_search().times(1).return_once(move |_| {
                let found = stored.clone();
                Box::pin(async move { Ok(vec![found]) })
            });
        })(&mut series, stored);
        series
            .expect_save()
            .times(1)
            .returning(|show| Box::pin(async move { Ok(show) }));
        let mut provider = MockVideoMetadataProvider::new();
        provider
            .expect_lookup()
            .times(1)
            .withf(|target| matches!(target, Target::Series(_)))
            .return_once(|_| Box::pin(async { Ok(Some(series_metadata())) }));
        let harness = harness_with(
            users,
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
            series,
        );

        // Act
        let response = use_case(&harness, Some(provider))
            .execute(command())
            .await?;

        // Assert
        let show = response.series();
        assert_eq!(show.name(), "A new name");
        assert_eq!(show.country_of_origin(), Some("GB"));
        assert_eq!(show.first_air_date(), NaiveDate::from_ymd_opt(1991, 1, 2));
        assert_eq!(show.genre_ids(), ["DRAMA"]);
        assert_eq!(show.seasons().len(), 1, "the apply must invent no season");
        assert_eq!(
            show.seasons().first().map(|season| season.episodes().len()),
            Some(1)
        );
        Ok(())
    }

    // proves: SCN-METADATA-ENRICHMENT-024-B, SCN-METADATA-ENRICHMENT-026-B — Business rule
    #[tokio::test]
    async fn apply_series_clears_what_the_provider_does_not_report() -> Result<(), Box<dyn Error>> {
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
        let mut series = MockSeriesRepository::new();
        (move |series: &mut MockSeriesRepository, stored: Series| {
            series.expect_search().times(1).return_once(move |_| {
                let found = stored.clone();
                Box::pin(async move { Ok(vec![found]) })
            });
        })(&mut series, sample_series(7, Vec::new())?);
        series
            .expect_save()
            .times(1)
            .returning(|show| Box::pin(async move { Ok(show) }));
        let mut provider = MockVideoMetadataProvider::new();
        provider.expect_lookup().times(1).return_once(|_| {
            Box::pin(async {
                Ok(Some(MediaMetadata::Series {
                    artwork: None,
                    country_of_origin: None,
                    first_air_date: None,
                    genres: Vec::new(),
                    plot: String::new(),
                    title: "A new name".to_owned(),
                }))
            })
        });
        let harness = harness_with(
            users,
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
            series,
        );

        // Act
        let response = use_case(&harness, Some(provider))
            .execute(command())
            .await?;

        // Assert
        assert_eq!(response.series().country_of_origin(), None);
        assert_eq!(response.series().first_air_date(), None);
        assert!(response.series().genre_ids().is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn apply_series_reports_a_missing_show() -> Result<(), Box<dyn Error>> {
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
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(
            users,
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
            series,
        );

        // Act
        let result = use_case(&harness, None).execute(command()).await;

        // Assert
        assert!(matches!(result, Err(ApplySeriesError::NoSuchSeries)));
        Ok(())
    }
    // proves: SCN-METADATA-ENRICHMENT-024-E — Error handling
    #[tokio::test]
    async fn apply_series_reports_an_unconfigured_provider() -> Result<(), Box<dyn Error>> {
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
        let mut series = MockSeriesRepository::new();
        (move |series: &mut MockSeriesRepository, stored: Series| {
            series.expect_search().times(1).return_once(move |_| {
                let found = stored.clone();
                Box::pin(async move { Ok(vec![found]) })
            });
        })(&mut series, sample_series(7, Vec::new())?);
        let harness = harness_with(
            users,
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
            series,
        );

        // Act
        let result = use_case(&harness, None).execute(command()).await;

        // Assert
        assert!(matches!(result, Err(ApplySeriesError::ProviderUnavailable)));
        Ok(())
    }

    // proves: SCN-METADATA-ENRICHMENT-024-D — Negative
    #[tokio::test]
    async fn apply_series_reports_an_unknown_reference() -> Result<(), Box<dyn Error>> {
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
        let mut series = MockSeriesRepository::new();
        (move |series: &mut MockSeriesRepository, stored: Series| {
            series.expect_search().times(1).return_once(move |_| {
                let found = stored.clone();
                Box::pin(async move { Ok(vec![found]) })
            });
        })(&mut series, sample_series(7, Vec::new())?);
        let mut provider = MockVideoMetadataProvider::new();
        provider
            .expect_lookup()
            .times(1)
            .return_once(|_| Box::pin(async { Ok(None) }));
        let harness = harness_with(
            users,
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
            series,
        );

        // Act
        let result = use_case(&harness, Some(provider)).execute(command()).await;

        // Assert
        assert!(matches!(result, Err(ApplySeriesError::UnknownReference)));
        Ok(())
    }

    // proves: SCN-METADATA-ENRICHMENT-024-C — Error handling
    #[tokio::test]
    async fn apply_series_rejects_another_kind() -> Result<(), Box<dyn Error>> {
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
        let mut series = MockSeriesRepository::new();
        (move |series: &mut MockSeriesRepository, stored: Series| {
            series.expect_search().times(1).return_once(move |_| {
                let found = stored.clone();
                Box::pin(async move { Ok(vec![found]) })
            });
        })(&mut series, sample_series(7, Vec::new())?);
        let mut provider = MockVideoMetadataProvider::new();
        provider.expect_lookup().times(1).return_once(|_| {
            Box::pin(async {
                Ok(Some(MediaMetadata::Movie {
                    artwork: None,
                    country_of_origin: None,
                    genres: Vec::new(),
                    plot: String::new(),
                    release_date: None,
                    title: "Inception".to_owned(),
                }))
            })
        });
        let harness = harness_with(
            users,
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
            series,
        );

        // Act
        let result = use_case(&harness, Some(provider)).execute(command()).await;

        // Assert
        assert!(matches!(result, Err(ApplySeriesError::InvalidReference)));
        Ok(())
    }

    #[tokio::test]
    async fn apply_series_dependency_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange: the show load fails at the port.
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
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::Unavailable) }));
        let harness = harness_with(
            users,
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
            series,
        );

        // Act
        let result = use_case(&harness, None).execute(command()).await;

        // Assert
        assert!(matches!(result, Err(ApplySeriesError::Unknown(_))));
        Ok(())
    }
}
