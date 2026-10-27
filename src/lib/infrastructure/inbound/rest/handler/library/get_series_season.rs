use axum::extract::Path;
use axum::extract::State;
use axum::response::Response;

use crate::application::port::get_series_season::GetSeriesSeasonCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::SeasonResponse;
use crate::infrastructure::inbound::rest::handler::library::parse_season_index;
use crate::infrastructure::inbound::rest::handler::library::parse_series_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Fetch one season of a show.
///
/// Any authenticated caller may read a season; the show is not owned and
/// carries no visibility flag. The season is scoped to the show named in the
/// path, so two shows may each hold a season 1.
#[utoipa::path(
    get,
    operation_id = "get_series_season",
    path = "/library/series/{series_id}/season/{season_index}",
    tag = "library",
    params(
        ("series_id" = NumericID, Path, description = "Identifier of the show holding the season"),
        ("season_index" = u32, Path, description = "Series-scoped index of the season"),
    ),
    responses(
        (
            status = OK,
            body = SeasonResponse,
            content_type = "application/hal+json",
            description = "Season found"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid show identifier or season index"
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
            description = "Unknown show, or season of the show"
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
    summary = "Fetch a show season"
)]
pub async fn get_series_season(
    Path((series_id_value, season_index_value)): Path<(String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let series_id = parse_series_id(&series_id_value)?;
    let season_index = parse_season_index(&season_index_value)?;
    let response = state
        .library_use_case_factory()
        .get_series_season()
        .execute(GetSeriesSeasonCommand::new(
            caller.user_id(),
            series_id,
            season_index,
        ))
        .await?;
    Ok(hal_json(SeasonResponse::from(&response)))
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

    use super::get_series_season;
    use crate::application::port::get_series_season::GetSeriesSeasonError;
    use crate::application::port::get_series_season::GetSeriesSeasonResponse;
    use crate::application::port::get_series_season::GetSeriesSeasonUseCase;
    use crate::application::port::get_series_season::MockGetSeriesSeasonUseCase;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_parts;

    /// Send a GET to
    /// `/api/v1/library/series/{series}/season/{season}` on behalf of
    /// `caller_id`, injecting the caller identifier the way the auth middleware
    /// does.
    async fn send(
        use_case: MockGetSeriesSeasonUseCase,
        caller_id: NumericID,
        series_id: &str,
        season_index: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_get_series_season()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn GetSeriesSeasonUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!(
                "/api/v1/library/series/{series_id}/season/{season_index}"
            ))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/series/{series_id}/season/{season_index}",
                get(get_series_season),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn get_series_season_returns_the_season() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetSeriesSeasonUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                command.caller_id() == 1 && command.series_id() == 7 && command.season_index() == 1
            })
            .return_once(|_| {
                Box::pin(async {
                    Ok(GetSeriesSeasonResponse::new(
                        7,
                        1,
                        Some("Season One".to_owned()),
                        3,
                        None,
                    ))
                })
            });

        // Act
        let (status, content_type, payload) =
            into_parts(send(use_case, 1, "7", "1").await?).await?;

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
                    "self": { "href": "/api/v1/library/series/7/season/1" },
                    "series": { "href": "/api/v1/library/series/7" },
                    "episodes": {
                        "href": "/api/v1/library/series/7/season/1/episode"
                    }
                },
                "cover_file_id": null,
                "series_id": 7i64,
                "index": 1u32,
                "name": "Season One",
                "episode_count": 3i64
            }),
            "the representation must carry the season summary"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_series_season_with_artwork_exposes_the_cover_link() -> Result<(), Box<dyn Error>> {
        // Arrange: the season's cover is file 20.
        let mut use_case = MockGetSeriesSeasonUseCase::new();
        use_case.expect_execute().times(1).return_once(|_| {
            Box::pin(async {
                Ok(GetSeriesSeasonResponse::new(
                    7,
                    1,
                    Some("Season One".to_owned()),
                    3,
                    Some(20),
                ))
            })
        });

        // Act
        let (status, _, payload) = into_parts(send(use_case, 1, "7", "1").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload.get("cover_file_id"), Some(&json!(20i64)));
        assert_eq!(
            payload.pointer("/_links/cover/href"),
            Some(&json!("/api/v1/asset/image/20"))
        );
        Ok(())
    }

    #[rstest]
    #[case::series("abc", "1", "invalid show identifier")]
    #[case::season("7", "abc", "invalid season index")]
    #[tokio::test]
    async fn get_series_season_malformed_identifier_returns_bad_request(
        #[case] series_id: &str,
        #[case] season_index: &str,
        #[case] message: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockGetSeriesSeasonUseCase::new();

        // Act
        let response = send(use_case, 1, series_id, season_index).await?;

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
    #[case::unknown_show(GetSeriesSeasonError::NoSuchSeries)]
    #[case::unknown_season(GetSeriesSeasonError::NoSuchSeason)]
    #[tokio::test]
    async fn get_series_season_not_found_returns_not_found(
        #[case] error: GetSeriesSeasonError,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetSeriesSeasonUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async move { Err(error) }));

        // Act
        let response = send(use_case, 1, "7", "1").await?;

        // Assert
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "an unknown show or season must be reported"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_series_season_unknown_caller_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetSeriesSeasonUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(GetSeriesSeasonError::NoSuchCaller) }));

        // Act
        let response = send(use_case, 999, "7", "1").await?;

        // Assert
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "a deleted caller must be rejected"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_series_season_missing_credentials_returns_unauthorized()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the real router, wrapped in its `authenticate` layer, with no
        // authorization header.
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("GET")
            .uri("/series/7/season/1")
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
