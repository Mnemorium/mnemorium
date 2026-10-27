use axum::extract::Path;
use axum::extract::State;
use axum::response::Response;
use tracing::warn;

use crate::application::port::get_series_episode::GetSeriesEpisodeCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::EpisodeResponse;
use crate::infrastructure::inbound::rest::handler::library::parse_season_index;
use crate::infrastructure::inbound::rest::handler::library::parse_series_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Fetch one episode of a season of a show.
///
/// Any authenticated caller may read an episode; the show is not owned and
/// carries no visibility flag. The episode is scoped to the show and season
/// named in the path.
#[utoipa::path(
    get,
    operation_id = "get_series_episode",
    path = "/library/series/{series_id}/season/{season_index}/episode/{episode_index}",
    tag = "library",
    params(
        ("series_id" = NumericID, Path, description = "Identifier of the show holding the episode"),
        ("season_index" = u32, Path, description = "Series-scoped index of the season holding the episode"),
        ("episode_index" = u32, Path, description = "Season-scoped index of the episode"),
    ),
    responses(
        (
            status = OK,
            body = EpisodeResponse,
            content_type = "application/hal+json",
            description = "Episode found"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid show identifier or season or episode index"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown show, season, or episode of the season"
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unexpected error"
        ),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "Fetch a show season episode"
)]
pub async fn get_series_episode(
    Path((series_id_value, season_index_value, episode_index_value)): Path<(
        String,
        String,
        String,
    )>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let series_id = parse_series_id(&series_id_value)?;
    let season_index = parse_season_index(&season_index_value)?;
    let episode_index = episode_index_value.parse::<u32>().map_err(|_| {
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "episode_index",
            reason = "invalid_index",
            "rejected a malformed episode index"
        );
        ApiError::BadRequest("invalid episode index".to_owned())
    })?;
    let response = state
        .library_use_case_factory()
        .get_series_episode()
        .execute(GetSeriesEpisodeCommand::new(
            caller.user_id(),
            series_id,
            season_index,
            episode_index,
        ))
        .await?;
    Ok(hal_json(EpisodeResponse::from(&response)))
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::response::Response;
    use axum::routing::get;
    use rstest::rstest;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::get_series_episode;
    use crate::application::port::get_series_episode::GetSeriesEpisodeError;
    use crate::application::port::get_series_episode::GetSeriesEpisodeResponse;
    use crate::application::port::get_series_episode::GetSeriesEpisodeUseCase;
    use crate::application::port::get_series_episode::MockGetSeriesEpisodeUseCase;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::video_summary::VideoSummary;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_parts;
    use crate::test_helpers::sample_episode;

    /// Send a GET to
    /// `/api/v1/library/series/{series}/season/{season}/episode/{episode}` on
    /// behalf of `caller_id`, injecting the caller identifier the way the auth
    /// middleware does.
    async fn send(
        use_case: MockGetSeriesEpisodeUseCase,
        caller_id: NumericID,
        series_id: &str,
        season_index: &str,
        episode_index: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_get_series_episode()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn GetSeriesEpisodeUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!(
                "/api/v1/library/series/{series_id}/season/{season_index}/episode/{episode_index}"
            ))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/series/{series_id}/season/{season_index}/episode/{episode_index}",
                get(get_series_episode),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn get_series_episode_returns_the_episode() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetSeriesEpisodeUseCase::new();
        let episode = sample_episode(2, 12)?;
        let response = GetSeriesEpisodeResponse::new(
            episode.index().value(),
            7,
            1,
            episode.name().to_owned(),
            episode.plot().to_owned(),
            episode.air_date(),
            VideoSummary::from(episode.video()),
        );
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                command.caller_id() == 1
                    && command.series_id() == 7
                    && command.season_index() == 1
                    && command.episode_index() == 2
            })
            .return_once(move |_| {
                let copy = response.clone();
                Box::pin(async move { Ok(copy) })
            });

        // Act
        let (status, content_type, payload) =
            into_parts(send(use_case, 1, "7", "1", "2").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK, "the lookup must succeed");
        assert_eq!(
            content_type.as_deref(),
            Some("application/hal+json"),
            "the representation must be served as HAL"
        );
        assert_eq!(
            payload,
            json!({
                "_links": {
                    "self": {
                        "href": "/api/v1/library/series/7/season/1/episode/2"
                    },
                    "series": { "href": "/api/v1/library/series/7" },
                    "season": { "href": "/api/v1/library/series/7/season/1" }
                },
                "id": 2u32,
                "series_id": 7i64,
                "season_index": 1u32,
                "name": "Episode 2",
                "plot": "A plot",
                "air_date": "1990-04-08",
                "video": {
                    "id": 12i64,
                    "file_id": 12i64,
                    "duration_ms": 1_000.0f64,
                    "codec": "H264",
                    "width": 1920i64,
                    "height": 1080i64,
                    "scan_type": "PROGRESSIVE"
                }
            }),
            "the representation must carry the episode and its video"
        );
        Ok(())
    }

    #[rstest]
    #[case::series("abc", "1", "2", "invalid show identifier")]
    #[case::season("7", "abc", "2", "invalid season index")]
    #[case::episode("7", "1", "abc", "invalid episode index")]
    #[tokio::test]
    async fn get_series_episode_malformed_identifier_returns_bad_request(
        #[case] series_id: &str,
        #[case] season_index: &str,
        #[case] episode_index: &str,
        #[case] message: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockGetSeriesEpisodeUseCase::new();

        // Act
        let response = send(use_case, 1, series_id, season_index, episode_index).await?;

        // Assert
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "a malformed identifier must be rejected"
        );
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some(message),
            "the rejection message must be stable"
        );
        Ok(())
    }

    #[rstest]
    #[case::unknown_show(GetSeriesEpisodeError::NoSuchSeries)]
    #[case::unknown_season(GetSeriesEpisodeError::NoSuchSeason)]
    #[case::unknown_episode(GetSeriesEpisodeError::NoSuchEpisode)]
    #[tokio::test]
    async fn get_series_episode_not_found_returns_not_found(
        #[case] error: GetSeriesEpisodeError,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetSeriesEpisodeUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async move { Err(error) }));

        // Act
        let response = send(use_case, 1, "7", "1", "2").await?;

        // Assert
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "an unknown show, season or episode must be reported"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_series_episode_unknown_caller_returns_unauthorized() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut use_case = MockGetSeriesEpisodeUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(GetSeriesEpisodeError::NoSuchCaller) }));

        // Act
        let response = send(use_case, 999, "7", "1", "2").await?;

        // Assert
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "a deleted caller must be rejected"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_series_episode_missing_credentials_returns_unauthorized()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the real router, wrapped in its `authenticate` layer, with no
        // authorization header.
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("GET")
            .uri("/series/7/season/1/episode/2")
            .body(Body::empty())?;

        // Act
        let response: Response = library_routes(&state).oneshot(request).await?;

        // Assert
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "a request without credentials must be rejected"
        );
        Ok(())
    }
}
