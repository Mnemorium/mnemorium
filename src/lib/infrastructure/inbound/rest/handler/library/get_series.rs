use axum::extract::Path;
use axum::extract::State;
use axum::response::Response;

use crate::application::port::get_series::GetSeriesCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::SeriesResponse;
use crate::infrastructure::inbound::rest::handler::library::parse_series_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Fetch one show by its identifier.
///
/// Any authenticated caller may read a show; the show is not owned and carries
/// no visibility flag. The representation carries the show's genres and its
/// season summaries, so a client can navigate to each season.
#[utoipa::path(
    get,
    operation_id = "get_series",
    path = "/library/series/{series_id}",
    tag = "library",
    params(
        ("series_id" = NumericID, Path, description = "Identifier of the show to fetch"),
    ),
    responses(
        (
            status = OK,
            body = SeriesResponse,
            content_type = "application/hal+json",
            description = "Show found"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid show identifier"
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
            description = "Unknown show"
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
    summary = "Fetch a show"
)]
pub async fn get_series(
    Path(series_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let series_id = parse_series_id(&series_id_value)?;
    let response = state
        .library_use_case_factory()
        .get_series()
        .execute(GetSeriesCommand::new(caller.user_id(), series_id))
        .await?;
    Ok(hal_json(SeriesResponse::from(&response)))
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
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::get_series;
    use crate::application::port::get_series::GetSeriesError;
    use crate::application::port::get_series::GetSeriesResponse;
    use crate::application::port::get_series::GetSeriesSeasonItem;
    use crate::application::port::get_series::GetSeriesUseCase;
    use crate::application::port::get_series::MockGetSeriesUseCase;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_parts;
    use crate::test_helpers::video_date;

    /// Send a GET to `/api/v1/library/series/{series}` on behalf of `caller_id`,
    /// injecting the caller identifier the way the auth middleware does.
    async fn send(
        use_case: MockGetSeriesUseCase,
        caller_id: NumericID,
        series_id: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_get_series()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn GetSeriesUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/library/series/{series_id}"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/library/series/{series_id}", get(get_series))
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn get_series_returns_metadata_and_season_summaries() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetSeriesUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| command.caller_id() == 1 && command.series_id() == 7)
            .return_once(|_| {
                Box::pin(async {
                    Ok(GetSeriesResponse::new(
                        7,
                        "Twin Peaks".to_owned(),
                        "A plot".to_owned(),
                        "US".to_owned(),
                        video_date(),
                        vec!["DRAMA".to_owned()],
                        vec![GetSeriesSeasonItem::new(
                            1,
                            Some("Season One".to_owned()),
                            2,
                        )],
                    ))
                })
            });

        // Act
        let (status, content_type, payload) = into_parts(send(use_case, 1, "7").await?).await?;

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
                    "self": { "href": "/api/v1/library/series/7" }
                },
                "id": 7i64,
                "name": "Twin Peaks",
                "plot": "A plot",
                "country_of_origin": "US",
                "first_air_date": "1990-04-08",
                "genres": ["DRAMA"],
                "seasons": [{
                    "_links": {
                        "self": { "href": "/api/v1/library/series/7/season/1" },
                        "series": { "href": "/api/v1/library/series/7" },
                        "episodes": {
                            "href": "/api/v1/library/series/7/season/1/episode"
                        }
                    },
                    "index": 1u32,
                    "name": "Season One",
                    "episode_count": 2i64,
                    "series_id": 7i64
                }]
            }),
            "the representation must carry the metadata and season summaries"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_series_malformed_identifier_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockGetSeriesUseCase::new();

        // Act
        let response = send(use_case, 1, "abc").await?;

        // Assert
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "a malformed identifier must be rejected"
        );
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("invalid show identifier"),
            "the rejection message must be stable"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_series_unknown_show_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetSeriesUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(GetSeriesError::NoSuchSeries) }));

        // Act
        let response = send(use_case, 1, "7").await?;

        // Assert
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "an unknown show must be reported"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_series_unknown_caller_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetSeriesUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(GetSeriesError::NoSuchCaller) }));

        // Act
        let response = send(use_case, 999, "7").await?;

        // Assert
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "a deleted caller must be rejected"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_series_missing_credentials_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange: the real router, wrapped in its `authenticate` layer, with no
        // authorization header.
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("GET")
            .uri("/series/7")
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
