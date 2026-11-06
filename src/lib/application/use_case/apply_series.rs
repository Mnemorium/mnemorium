use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::apply_series::ApplySeriesCommand;
use crate::application::port::apply_series::ApplySeriesError;
use crate::application::port::apply_series::ApplySeriesResponse;
use crate::application::port::apply_series::ApplySeriesUseCase;
use crate::application::security_event;
use crate::domain::model::media_metadata::MediaMetadata;
use crate::domain::model::permission::Action;
use crate::domain::model::series::Series;
use crate::domain::model::video_candidate::Target;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::permission_repository::PermissionFilter;
use crate::domain::port::permission_repository::PermissionRepository as _;
use crate::domain::port::series_repository::SeriesFilter;
use crate::domain::port::series_repository::SeriesRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::port::video_metadata_provider::VideoMetadataProvider;
use crate::domain::service::authorization::Authorization;
use crate::domain::service::authorization::Decision;

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

                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| ApplySeriesError::Unknown(error.into()))?;

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

                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::LibraryRead, &series)
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(ApplySeriesError::NoSuchSeries);
                }
                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::LibraryManage, &series)
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(if denial.conceals() {
                        ApplySeriesError::NoSuchSeries
                    } else {
                        ApplySeriesError::Forbidden
                    });
                }

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

                let stored = unit_of_work
                    .series()
                    .update(updated)
                    .await
                    .map_err(|error| {
                        if matches!(
                            error,
                            RepositoryError::AlreadyExist | RepositoryError::Conflict
                        ) {
                            ApplySeriesError::Conflict
                        } else {
                            ApplySeriesError::Unknown(error.into())
                        }
                    })?
                    .ok_or(ApplySeriesError::NoSuchSeries)?;

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
    use std::error::Error;
    use std::sync::Arc;

    use chrono::NaiveDate;

    use crate::application::port::apply_series::ApplySeriesCommand;
    use crate::application::port::apply_series::ApplySeriesError;
    use crate::application::port::apply_series::ApplySeriesUseCase as _;
    use crate::domain::alias::NumericID;
    use crate::domain::model::configuration::ProviderId;
    use crate::domain::model::external_ref::ExternalRef;
    use crate::domain::model::genre::Genre;
    use crate::domain::model::media_metadata::MediaMetadata;
    use crate::domain::model::series::Series;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::model::video_candidate::Target;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::permission_repository::MockPermissionRepository;
    use crate::domain::port::series_repository::MockSeriesRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::domain::port::video_metadata_provider::MockVideoMetadataProvider;
    use crate::test_helpers::GalleryFactoryHarness;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::TestUnitOfWork;
    use crate::test_helpers::expect_caller;
    use crate::test_helpers::no_permissions;
    use crate::test_helpers::sample_episode;
    use crate::test_helpers::sample_season;
    use crate::test_helpers::sample_series;

    use super::ApplySeries;

    type UseCase = ApplySeries<TestFactory, MockVideoMetadataProvider>;

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
        permissions: MockPermissionRepository,
        series: MockSeriesRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            permissions,
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

    fn command() -> ApplySeriesCommand {
        ApplySeriesCommand::new(1, 7, ExternalRef::new(ProviderId::Tmdb, "1920".to_owned()))
    }

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

    fn loaded(series: &mut MockSeriesRepository, stored: Series) {
        series.expect_search().times(1).return_once(move |_| {
            let found = stored.clone();
            Box::pin(async move { Ok(vec![found]) })
        });
    }

    #[tokio::test]
    async fn apply_series_writes_the_reported_metadata_and_keeps_the_tree()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the show already holds an episode.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_series(7, vec![sample_season(1, vec![sample_episode(1, 20)?])])?;
        let mut series = MockSeriesRepository::new();
        loaded(&mut series, stored);
        series
            .expect_update()
            .times(1)
            .returning(|show| Box::pin(async move { Ok(Some(show)) }));
        let mut provider = MockVideoMetadataProvider::new();
        provider
            .expect_lookup()
            .times(1)
            .withf(|target| matches!(target, Target::Series(_)))
            .return_once(|_| Box::pin(async { Ok(Some(series_metadata())) }));
        let harness = harness_with(users, no_permissions(), series);

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

    #[tokio::test]
    async fn apply_series_clears_what_the_provider_does_not_report() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        loaded(&mut series, sample_series(7, Vec::new())?);
        series
            .expect_update()
            .times(1)
            .returning(|show| Box::pin(async move { Ok(Some(show)) }));
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
        let harness = harness_with(users, no_permissions(), series);

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
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, no_permissions(), series);

        // Act
        let result = use_case(&harness, None).execute(command()).await;

        // Assert
        assert!(matches!(result, Err(ApplySeriesError::NoSuchSeries)));
        Ok(())
    }

    #[tokio::test]
    async fn apply_series_forbids_a_caller_without_manage() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        loaded(&mut series, sample_series(7, Vec::new())?);
        let harness = harness_with(users, no_permissions(), series);

        // Act
        let result = use_case(&harness, None).execute(command()).await;

        // Assert
        assert!(matches!(result, Err(ApplySeriesError::Forbidden)));
        Ok(())
    }

    #[tokio::test]
    async fn apply_series_reports_an_unconfigured_provider() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        loaded(&mut series, sample_series(7, Vec::new())?);
        let harness = harness_with(users, no_permissions(), series);

        // Act
        let result = use_case(&harness, None).execute(command()).await;

        // Assert
        assert!(matches!(result, Err(ApplySeriesError::ProviderUnavailable)));
        Ok(())
    }

    #[tokio::test]
    async fn apply_series_reports_an_unknown_reference() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        loaded(&mut series, sample_series(7, Vec::new())?);
        let mut provider = MockVideoMetadataProvider::new();
        provider
            .expect_lookup()
            .times(1)
            .return_once(|_| Box::pin(async { Ok(None) }));
        let harness = harness_with(users, no_permissions(), series);

        // Act
        let result = use_case(&harness, Some(provider)).execute(command()).await;

        // Assert
        assert!(matches!(result, Err(ApplySeriesError::UnknownReference)));
        Ok(())
    }

    #[tokio::test]
    async fn apply_series_rejects_another_kind() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        loaded(&mut series, sample_series(7, Vec::new())?);
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
        let harness = harness_with(users, no_permissions(), series);

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
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::Unavailable) }));
        let harness = harness_with(users, no_permissions(), series);

        // Act
        let result = use_case(&harness, None).execute(command()).await;

        // Assert
        assert!(matches!(result, Err(ApplySeriesError::Unknown(_))));
        Ok(())
    }
}
