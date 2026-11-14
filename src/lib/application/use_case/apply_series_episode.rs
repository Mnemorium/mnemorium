use crate::application::authorization::authorize;
use crate::domain::model::{ResourceKind, ResourceRef};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::{
    ApplySeriesEpisodeCommand, ApplySeriesEpisodeError, ApplySeriesEpisodeResponse,
    ApplySeriesEpisodeUseCase,
};
use crate::domain::model::{Action, Episode, EpisodeIndex, MediaMetadata, Target};
use crate::domain::port::{
    IdentityUnitOfWork, LibraryUnitOfWork, RepositoryError, SeriesFilter, SeriesRepository as _,
    UnitOfWork as _, UnitOfWorkFactory, UserFilter, UserRepository as _, UserUnitOfWork,
    VideoMetadataProvider,
};

/// Use case implementation for applying external metadata to one episode.
///
/// The provider is resolved through the show's reference and the addressed
/// position; a provider that reports another position is rejected rather than
/// writing another episode's metadata. The video behind the episode is never
/// part of the apply.
pub(crate) struct ApplySeriesEpisode<F, P> {
    /// Configured provider chain, absent when video identification is unavailable.
    metadata_provider: Option<Arc<P>>,
    /// Factory opening the unit of work wrapping the write.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, P: VideoMetadataProvider> ApplySeriesEpisode<F, P> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>, metadata_provider: Option<Arc<P>>) -> Self {
        Self {
            metadata_provider,
            unit_of_work_factory,
        }
    }
}

impl<F, P> ApplySeriesEpisodeUseCase for ApplySeriesEpisode<F, P>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
    P: VideoMetadataProvider,
{
    fn execute<'future>(
        &'future self,
        command: ApplySeriesEpisodeCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ApplySeriesEpisodeResponse, ApplySeriesEpisodeError>>
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
                .map_err(|error| ApplySeriesEpisodeError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| ApplySeriesEpisodeError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(ApplySeriesEpisodeError::NoSuchCaller)?;

                let series = unit_of_work
                    .series()
                    .search(&SeriesFilter {
                        id: Some(command.series_id()),
                        ..SeriesFilter::default()
                    })
                    .await
                    .map_err(|error| ApplySeriesEpisodeError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(ApplySeriesEpisodeError::NoSuchSeries)?;

                let season = series
                    .seasons()
                    .iter()
                    .find(|season| season.index().value() == command.season_index())
                    .ok_or(ApplySeriesEpisodeError::NoSuchSeason)?;
                let stored = season
                    .episodes()
                    .iter()
                    .find(|episode| episode.index().value() == command.episode_index())
                    .ok_or(ApplySeriesEpisodeError::NoSuchEpisode)?
                    .clone();

                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::Series, series.id()),
                    Action::ViewMetadata,
                    None,
                )
                .await
                .map_err(|_| ApplySeriesEpisodeError::NoSuchSeries)?;
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
                        ApplySeriesEpisodeError::NoSuchSeries
                    } else {
                        ApplySeriesEpisodeError::Forbidden
                    }
                })?;

                let Some(provider) = metadata_provider else {
                    return Err(ApplySeriesEpisodeError::ProviderUnavailable);
                };

                let target = Target::Episode {
                    episode: command.episode_index(),
                    reference: command.reference().clone(),
                    season: command.season_index(),
                };
                let metadata = provider
                    .lookup(&target)
                    .await
                    .map_err(|_| ApplySeriesEpisodeError::ProviderFailure)?
                    .ok_or(ApplySeriesEpisodeError::UnknownReference)?;

                let MediaMetadata::Episode {
                    air_date,
                    episode: reported_episode,
                    plot,
                    season: reported_season,
                    title,
                    ..
                } = metadata
                else {
                    return Err(ApplySeriesEpisodeError::InvalidReference);
                };
                if reported_episode != command.episode_index()
                    || reported_season != command.season_index()
                {
                    return Err(ApplySeriesEpisodeError::PositionMismatch);
                }

                let updated = Episode::try_new(
                    EpisodeIndex::new(command.episode_index()),
                    title,
                    plot,
                    air_date,
                    stored.video().clone(),
                )
                .map_err(|_| ApplySeriesEpisodeError::InvalidReference)?;

                let written = unit_of_work
                    .series()
                    .save_episode(command.series_id(), command.season_index(), updated)
                    .await
                    .map_err(|error| {
                        if matches!(
                            error,
                            RepositoryError::AlreadyExist | RepositoryError::Conflict
                        ) {
                            ApplySeriesEpisodeError::Conflict
                        } else {
                            ApplySeriesEpisodeError::Unknown(error.into())
                        }
                    })?
                    .ok_or(ApplySeriesEpisodeError::NoSuchEpisode)?;

                Ok(ApplySeriesEpisodeResponse::new(
                    command.series_id(),
                    command.season_index(),
                    command.episode_index(),
                    written,
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| ApplySeriesEpisodeError::Unknown(error.into()))?;
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
    use allure_cargotest::allure_test;

    use crate::domain::model::{ExternalRef, ProviderId, Role, Series, User};

    use crate::domain::port::{
        MockResourceRepository, MockSeriesRepository, MockUserRepository, MockVideoMetadataProvider,
    };

    use crate::test_helpers::*;
    use chrono::NaiveDate;
    use std::error::Error;

    type UseCase = ApplySeriesEpisode<TestFactory, MockVideoMetadataProvider>;

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
        ApplySeriesEpisode::new(Arc::clone(&harness.factory), provider.map(Arc::new))
    }

    fn command() -> ApplySeriesEpisodeCommand {
        ApplySeriesEpisodeCommand::new(
            1,
            7,
            1,
            1,
            ExternalRef::new(ProviderId::Tmdb, "1920".to_owned()),
        )
    }

    fn loaded(series: &mut MockSeriesRepository, stored: Series) {
        series.expect_search().times(1).return_once(move |_| {
            let found = stored.clone();
            Box::pin(async move { Ok(vec![found]) })
        });
    }

    fn episode_metadata(season: u32, episode: u32) -> MediaMetadata {
        MediaMetadata::Episode {
            air_date: NaiveDate::from_ymd_opt(1991, 4, 8),
            artwork: None,
            episode,
            plot: "A new plot".to_owned(),
            season,
            title: "A new name".to_owned(),
        }
    }

    #[allure_test]
    #[tokio::test]
    async fn apply_series_episode_writes_the_reported_metadata() -> Result<(), Box<dyn Error>> {
        allure.epic("Metadata enrichment");
        allure.feature("REQ-METADATA-ENRICHMENT-024");
        allure.feature("REQ-METADATA-ENRICHMENT-027");
        allure.story("SCN-METADATA-ENRICHMENT-024-A");
        allure.story("SCN-METADATA-ENRICHMENT-027-A");
        allure.story("SCN-METADATA-ENRICHMENT-027-D");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        loaded(
            &mut series,
            sample_series(7, vec![sample_season(1, vec![sample_episode(1, 20)?])])?,
        );
        series
            .expect_save_episode()
            .times(1)
            .returning(|_, _, episode| Box::pin(async move { Ok(Some(episode)) }));
        let mut provider = MockVideoMetadataProvider::new();
        provider
            .expect_lookup()
            .times(1)
            .withf(|target| {
                matches!(
                    target,
                    Target::Episode {
                        season: 1,
                        episode: 1,
                        ..
                    }
                )
            })
            .return_once(|_| Box::pin(async { Ok(Some(episode_metadata(1, 1))) }));
        let harness = harness_with(users, no_resources(), series);

        // Act
        let response = use_case(&harness, Some(provider))
            .execute(command())
            .await?;

        // Assert
        assert_eq!(response.episode().name(), "A new name");
        assert_eq!(
            response.episode().air_date(),
            NaiveDate::from_ymd_opt(1991, 4, 8)
        );
        assert_eq!(
            response.episode().video().id(),
            20,
            "the video must be untouched"
        );
        assert_eq!(response.series_id(), 7);
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn apply_series_episode_rejects_another_position() -> Result<(), Box<dyn Error>> {
        allure.epic("Metadata enrichment");
        allure.feature("REQ-METADATA-ENRICHMENT-027");
        allure.story("SCN-METADATA-ENRICHMENT-027-C");
        // Arrange: the provider reports episode 2 while the command addresses 1.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        loaded(
            &mut series,
            sample_series(7, vec![sample_season(1, vec![sample_episode(1, 20)?])])?,
        );
        let mut provider = MockVideoMetadataProvider::new();
        provider
            .expect_lookup()
            .times(1)
            .return_once(|_| Box::pin(async { Ok(Some(episode_metadata(1, 2))) }));
        let harness = harness_with(users, no_resources(), series);

        // Act
        let result = use_case(&harness, Some(provider)).execute(command()).await;

        // Assert
        assert!(matches!(
            result,
            Err(ApplySeriesEpisodeError::PositionMismatch)
        ));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn apply_series_episode_reports_an_unknown_reference() -> Result<(), Box<dyn Error>> {
        allure.epic("Metadata enrichment");
        allure.feature("REQ-METADATA-ENRICHMENT-024");
        allure.story("SCN-METADATA-ENRICHMENT-024-D");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        loaded(
            &mut series,
            sample_series(7, vec![sample_season(1, vec![sample_episode(1, 20)?])])?,
        );
        let mut provider = MockVideoMetadataProvider::new();
        provider
            .expect_lookup()
            .times(1)
            .return_once(|_| Box::pin(async { Ok(None) }));
        let harness = harness_with(users, no_resources(), series);

        // Act
        let result = use_case(&harness, Some(provider)).execute(command()).await;

        // Assert
        assert!(matches!(
            result,
            Err(ApplySeriesEpisodeError::UnknownReference)
        ));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn apply_series_episode_rejects_another_kind() -> Result<(), Box<dyn Error>> {
        allure.epic("Metadata enrichment");
        allure.feature("REQ-METADATA-ENRICHMENT-024");
        allure.story("SCN-METADATA-ENRICHMENT-024-C");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        loaded(
            &mut series,
            sample_series(7, vec![sample_season(1, vec![sample_episode(1, 20)?])])?,
        );
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
        let harness = harness_with(users, no_resources(), series);

        // Act
        let result = use_case(&harness, Some(provider)).execute(command()).await;

        // Assert
        assert!(matches!(
            result,
            Err(ApplySeriesEpisodeError::InvalidReference)
        ));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn apply_series_episode_reports_an_unconfigured_provider() -> Result<(), Box<dyn Error>> {
        allure.epic("Metadata enrichment");
        allure.feature("REQ-METADATA-ENRICHMENT-024");
        allure.story("SCN-METADATA-ENRICHMENT-024-E");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        loaded(
            &mut series,
            sample_series(7, vec![sample_season(1, vec![sample_episode(1, 20)?])])?,
        );
        let harness = harness_with(users, no_resources(), series);

        // Act
        let result = use_case(&harness, None).execute(command()).await;

        // Assert
        assert!(matches!(
            result,
            Err(ApplySeriesEpisodeError::ProviderUnavailable)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn apply_series_episode_reports_a_missing_episode() -> Result<(), Box<dyn Error>> {
        // Arrange: the show holds the season but no episode.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        loaded(
            &mut series,
            sample_series(7, vec![sample_season(1, Vec::new())])?,
        );
        let harness = harness_with(users, no_resources(), series);

        // Act
        let result = use_case(&harness, None).execute(command()).await;

        // Assert
        assert!(matches!(
            result,
            Err(ApplySeriesEpisodeError::NoSuchEpisode)
        ));
        Ok(())
    }
    #[tokio::test]
    async fn apply_series_episode_dependency_failure_returns_unknown() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the show load fails at the port.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::Unavailable) }));
        let harness = harness_with(users, no_resources(), series);

        // Act
        let result = use_case(&harness, None).execute(command()).await;

        // Assert
        assert!(matches!(result, Err(ApplySeriesEpisodeError::Unknown(_))));
        Ok(())
    }
}
