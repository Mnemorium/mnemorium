use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::get_series::GetSeriesCommand;
use crate::application::port::get_series::GetSeriesError;
use crate::application::port::get_series::GetSeriesResponse;
use crate::application::port::get_series::GetSeriesSeasonItem;
use crate::application::port::get_series::GetSeriesUseCase;
use crate::application::security_event;
use crate::domain::model::permission::Action;
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
use crate::domain::service::authorization::Authorization;
use crate::domain::service::authorization::Decision;

/// Use case implementation for fetching one show.
pub struct GetSeries<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> GetSeries<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> GetSeriesUseCase for GetSeries<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: GetSeriesCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetSeriesResponse, GetSeriesError>> + Send + 'future>>
    {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| GetSeriesError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| GetSeriesError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetSeriesError::NoSuchCaller)?;

                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| GetSeriesError::Unknown(error.into()))?;

                let shown = unit_of_work
                    .series()
                    .search(&SeriesFilter {
                        id: Some(command.series_id()),
                        ..SeriesFilter::default()
                    })
                    .await
                    .map_err(|error| GetSeriesError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetSeriesError::NoSuchSeries)?;
                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::CatalogRead, &shown)
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(GetSeriesError::NoSuchSeries);
                }

                let seasons = shown
                    .seasons()
                    .iter()
                    .map(|season| {
                        GetSeriesSeasonItem::new(
                            season.index().value(),
                            season.name().map(str::to_owned),
                            season.episodes().len(),
                            season.cover_file_id(),
                        )
                    })
                    .collect();

                Ok(GetSeriesResponse::new(
                    shown.id(),
                    shown.name().to_owned(),
                    shown.plot().to_owned(),
                    shown.country_of_origin().to_owned(),
                    shown.first_air_date(),
                    shown.genre_ids().to_vec(),
                    shown.cover_file_id(),
                    seasons,
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| GetSeriesError::Unknown(error.into()))?;
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

    use crate::application::port::get_series::GetSeriesCommand;
    use crate::application::port::get_series::GetSeriesError;
    use crate::application::port::get_series::GetSeriesResponse;
    use crate::application::port::get_series::GetSeriesSeasonItem;
    use crate::application::port::get_series::GetSeriesUseCase as _;
    use crate::domain::model::season::Season;
    use crate::domain::model::season::SeasonIndex;
    use crate::domain::model::series::Series;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::series_repository::MockSeriesRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::GalleryFactoryHarness;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::TestUnitOfWork;
    use crate::test_helpers::expect_caller;
    use crate::test_helpers::no_permissions;
    use crate::test_helpers::sample_episode;
    use crate::test_helpers::sample_season;
    use crate::test_helpers::sample_series;
    use crate::test_helpers::video_date;

    use super::GetSeries;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = GetSeries<TestFactory>;

    /// Build a Library test harness around the given mocks.
    fn harness_with(
        users: MockUserRepository,
        series: MockSeriesRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            permissions: no_permissions(),
            series,
            users,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
    }

    #[tokio::test]
    async fn get_series_returns_its_metadata_and_season_summaries() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let first_season = sample_season(1, vec![sample_episode(1, 11)?, sample_episode(2, 12)?]);
        let second_season = sample_season(2, Vec::new());
        let shown = sample_series(3, vec![first_season, second_season])?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .withf(|filter| filter.id == Some(3))
            .return_once(move |_| Box::pin(async move { Ok(vec![shown]) }));
        let harness = harness_with(users, series);
        let use_case: UseCase = GetSeries::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(GetSeriesCommand::new(1, 3)).await?;

        // Assert
        assert_eq!(
            response,
            GetSeriesResponse::new(
                3,
                "Series 3".to_owned(),
                "A plot".to_owned(),
                "US".to_owned(),
                video_date(),
                vec!["DRAMA".to_owned()],
                None,
                vec![
                    GetSeriesSeasonItem::new(1, None, 2, None),
                    GetSeriesSeasonItem::new(2, None, 0, None),
                ],
            ),
            "the representation must carry the metadata and season summaries"
        );
        assert!(
            harness.committed.load(Ordering::SeqCst),
            "a successful read must commit"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_series_carries_the_cover_of_the_show_and_each_season() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the show carries artwork, its first season its own, and the
        // second season none.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let covered_season = Season::new(SeasonIndex::new(1), None, Some(20), Vec::new());
        let uncovered_season = Season::new(SeasonIndex::new(2), None, None, Vec::new());
        let shown = Series::try_new(
            3,
            "Series 3".to_owned(),
            "A plot".to_owned(),
            "US".to_owned(),
            video_date(),
            Vec::new(),
            Some(19),
            vec![covered_season, uncovered_season],
        )?;
        assert_eq!(
            shown.cover_file_id(),
            Some(19),
            "a show built with artwork must report the identifier"
        );
        assert_eq!(
            shown.seasons().first().and_then(Season::cover_file_id),
            Some(20),
            "a season built with artwork must report the identifier"
        );
        assert_eq!(
            shown.seasons().get(1).and_then(Season::cover_file_id),
            None,
            "a season built without artwork must report none"
        );
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![shown]) }));
        let harness = harness_with(users, series);
        let use_case: UseCase = GetSeries::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(GetSeriesCommand::new(1, 3)).await?;

        // Assert
        assert_eq!(response.cover_file_id(), Some(19));
        assert_eq!(
            response
                .seasons()
                .first()
                .and_then(GetSeriesSeasonItem::cover_file_id),
            Some(20)
        );
        assert_eq!(
            response
                .seasons()
                .get(1)
                .and_then(GetSeriesSeasonItem::cover_file_id),
            None,
            "a season must not inherit its show's cover"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_series_without_a_season_returns_no_season_summaries() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let shown = sample_series(3, Vec::new())?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![shown]) }));
        let harness = harness_with(users, series);
        let use_case: UseCase = GetSeries::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(GetSeriesCommand::new(1, 3)).await?;

        // Assert
        assert!(
            response.seasons().is_empty(),
            "a show without a season must report none"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_series_unknown_show_returns_no_such_series() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, series);
        let use_case: UseCase = GetSeries::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetSeriesCommand::new(1, 404)).await;

        // Assert
        assert!(
            matches!(result, Err(GetSeriesError::NoSuchSeries)),
            "an unknown show must be reported"
        );
        assert!(
            harness.rolled_back.load(Ordering::SeqCst),
            "an unknown show must roll back"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_series_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, MockSeriesRepository::new());
        let use_case: UseCase = GetSeries::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetSeriesCommand::new(999, 3)).await;

        // Assert
        assert!(
            matches!(result, Err(GetSeriesError::NoSuchCaller)),
            "a deleted caller must be rejected"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_series_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, series);
        let use_case: UseCase = GetSeries::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetSeriesCommand::new(1, 3)).await;

        // Assert
        assert!(
            matches!(result, Err(GetSeriesError::Unknown(_))),
            "a repository failure must map to an unknown error"
        );
        assert!(
            harness.rolled_back.load(Ordering::SeqCst),
            "a repository failure must roll back"
        );
        Ok(())
    }
}
