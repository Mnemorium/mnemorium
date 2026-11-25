use crate::application::authorization::authorize;
use crate::application::{FieldUpdate, cover_is_viewable};
use crate::domain::model::{ResourceKind, ResourceRef};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::{
    UpdateSeriesSeasonCommand, UpdateSeriesSeasonError, UpdateSeriesSeasonResponse,
    UpdateSeriesSeasonUseCase,
};
use crate::domain::model::{Action, Season};
use crate::domain::port::{
    IdentityUnitOfWork, LibraryUnitOfWork, RepositoryError, SeriesFilter, SeriesRepository as _,
    UnitOfWork as _, UnitOfWorkFactory, UserFilter, UserRepository as _, UserUnitOfWork,
};

/// Use case implementation for updating a show season partially.
pub(crate) struct UpdateSeriesSeason<F> {
    /// Factory opening the unit of work wrapping the update.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> UpdateSeriesSeason<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> UpdateSeriesSeasonUseCase for UpdateSeriesSeason<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: UpdateSeriesSeasonCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<UpdateSeriesSeasonResponse, UpdateSeriesSeasonError>>
                + Send
                + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| UpdateSeriesSeasonError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| UpdateSeriesSeasonError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(UpdateSeriesSeasonError::NoSuchCaller)?;

                let series = unit_of_work
                    .series()
                    .search(&SeriesFilter {
                        id: Some(command.series_id()),
                        ..SeriesFilter::default()
                    })
                    .await
                    .map_err(|error| UpdateSeriesSeasonError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(UpdateSeriesSeasonError::NoSuchSeries)?;

                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::Series, series.id()),
                    Action::View,
                    None,
                )
                .await
                .map_err(|_| UpdateSeriesSeasonError::NoSuchSeries)?;
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
                        UpdateSeriesSeasonError::NoSuchSeries
                    } else {
                        UpdateSeriesSeasonError::Forbidden
                    }
                })?;

                let season = series
                    .seasons()
                    .iter()
                    .find(|season| season.index().value() == command.season_index())
                    .ok_or(UpdateSeriesSeasonError::NoSuchSeason)?;

                let name = command.name().resolve(season.name().map(str::to_owned));
                if let FieldUpdate::Set(cover_image_id) = command.cover_image_id()
                    && !cover_is_viewable(&mut unit_of_work, &caller, Some(cover_image_id)).await
                {
                    return Err(UpdateSeriesSeasonError::Conflict);
                }
                let cover_image_id = command.cover_image_id().resolve(season.cover_image_id());

                let updated = Season::new(
                    season.index(),
                    name,
                    cover_image_id,
                    season.episodes().to_vec(),
                );

                let stored = unit_of_work
                    .series()
                    .save_season(series.id(), updated)
                    .await
                    .map_err(|error| {
                        if matches!(
                            error,
                            RepositoryError::Conflict | RepositoryError::DataIntegrityViolation
                        ) {
                            UpdateSeriesSeasonError::Conflict
                        } else {
                            UpdateSeriesSeasonError::Unknown(error.into())
                        }
                    })?
                    .ok_or(UpdateSeriesSeasonError::NoSuchSeason)?;

                Ok(UpdateSeriesSeasonResponse::new(stored))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| UpdateSeriesSeasonError::Unknown(error.into()))?;
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
    use crate::application::FieldUpdate;
    use rstest::fixture;

    use crate::domain::NumericID;

    use crate::domain::model::{Role, SeasonIndex, Series, User};

    use crate::domain::port::{MockResourceRepository, MockSeriesRepository, MockUserRepository};

    use crate::test_helpers::*;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = UpdateSeriesSeason<TestFactory>;

    /// Build a Library test harness around the given mocks.
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

    /// A show holding season 1 with one episode and a stored name and cover.
    #[fixture]
    fn series_with_season() -> Result<Series, Box<dyn Error>> {
        let season = Season::new(
            SeasonIndex::new(1),
            Some("Old Name".to_owned()),
            Some(9),
            vec![sample_episode(1, 20)?],
        );
        sample_series(7, vec![season])
    }

    // proves: SCN-SERIES-011-A, SCN-SERIES-011-C
    #[tokio::test]
    async fn update_series_season_replaces_name_and_cover_keeping_episodes()
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
        let mut series = MockSeriesRepository::new();
        (move |series: &mut MockSeriesRepository, stored: Series| {
            let series_id = stored.id();
            series
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(series_id))
                .return_once(move |_| {
                    let found = stored.clone();
                    Box::pin(async move { Ok(vec![found]) })
                });
        })(&mut series, series_with_season()?);
        let written = Season::new(
            SeasonIndex::new(1),
            Some("New Name".to_owned()),
            Some(9),
            vec![sample_episode(1, 20)?],
        );
        series
            .expect_save_season()
            .times(1)
            .withf(|series_id, season| {
                *series_id == 7
                    && season.index().value() == 1
                    && season.name() == Some("New Name")
                    && season.cover_image_id() == Some(9)
                    && season.episodes().len() == 1
            })
            .return_once(move |_, _| {
                let copy = written.clone();
                Box::pin(async move { Ok(Some(copy)) })
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
        let use_case: UseCase = UpdateSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(UpdateSeriesSeasonCommand::new(
                1,
                7,
                1,
                FieldUpdate::Set("New Name".to_owned()),
                FieldUpdate::Set(9),
            ))
            .await?;

        // Assert
        assert_eq!(response.season().name(), Some("New Name"));
        assert_eq!(response.season().episodes().len(), 1);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    // proves: SCN-SERIES-011-B
    #[tokio::test]
    async fn update_series_season_null_name_and_cover_clear() -> Result<(), Box<dyn Error>> {
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
            let series_id = stored.id();
            series
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(series_id))
                .return_once(move |_| {
                    let found = stored.clone();
                    Box::pin(async move { Ok(vec![found]) })
                });
        })(&mut series, series_with_season()?);
        let written = Season::new(SeasonIndex::new(1), None, None, Vec::new());
        series
            .expect_save_season()
            .times(1)
            .withf(|_, season| season.name().is_none() && season.cover_image_id().is_none())
            .return_once(move |_, _| {
                let copy = written.clone();
                Box::pin(async move { Ok(Some(copy)) })
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
        let use_case: UseCase = UpdateSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(UpdateSeriesSeasonCommand::new(
                1,
                7,
                1,
                FieldUpdate::Clear,
                FieldUpdate::Clear,
            ))
            .await?;

        // Assert
        assert!(response.season().name().is_none());
        assert!(response.season().cover_image_id().is_none());
        Ok(())
    }

    // proves: SCN-SERIES-011-D
    #[tokio::test]
    async fn update_series_season_absent_season_returns_no_such_season()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the show holds no season at index 1.
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
            let series_id = stored.id();
            series
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(series_id))
                .return_once(move |_| {
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
        let use_case: UseCase = UpdateSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateSeriesSeasonCommand::new(
                1,
                7,
                1,
                FieldUpdate::Absent,
                FieldUpdate::Absent,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateSeriesSeasonError::NoSuchSeason)));
        Ok(())
    }

    // proves: SCN-SERIES-011-D
    #[tokio::test]
    async fn update_series_season_unknown_show_returns_no_such_series() -> Result<(), Box<dyn Error>>
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
        let use_case: UseCase = UpdateSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateSeriesSeasonCommand::new(
                1,
                7,
                1,
                FieldUpdate::Absent,
                FieldUpdate::Absent,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateSeriesSeasonError::NoSuchSeries)));
        Ok(())
    }
    // proves: SCN-SERIES-011-A
    #[tokio::test]
    async fn update_series_season_invalid_cover_is_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange: the repository rejects the cover through the scope trigger.
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
            let series_id = stored.id();
            series
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(series_id))
                .return_once(move |_| {
                    let found = stored.clone();
                    Box::pin(async move { Ok(vec![found]) })
                });
        })(&mut series, series_with_season()?);
        series
            .expect_save_season()
            .times(1)
            .returning(|_, _| Box::pin(async { Err(RepositoryError::Conflict) }));
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
        let use_case: UseCase = UpdateSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateSeriesSeasonCommand::new(
                1,
                7,
                1,
                FieldUpdate::Absent,
                FieldUpdate::Set(11),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateSeriesSeasonError::Conflict)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    // proves: SCN-SERIES-011-D
    #[tokio::test]
    async fn update_series_season_absent_at_write_returns_no_such_season()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the season disappears between the read and the write.
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
            let series_id = stored.id();
            series
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(series_id))
                .return_once(move |_| {
                    let found = stored.clone();
                    Box::pin(async move { Ok(vec![found]) })
                });
        })(&mut series, series_with_season()?);
        series
            .expect_save_season()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(None) }));
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
        let use_case: UseCase = UpdateSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateSeriesSeasonCommand::new(
                1,
                7,
                1,
                FieldUpdate::Absent,
                FieldUpdate::Absent,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateSeriesSeasonError::NoSuchSeason)));
        Ok(())
    }
}
