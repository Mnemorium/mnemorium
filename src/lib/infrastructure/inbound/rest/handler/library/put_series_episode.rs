use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use chrono::NaiveDate;
use serde::Deserialize;
use tracing::warn;
use utoipa::ToSchema;

use crate::application::port::put_series_episode::PutSeriesEpisodeCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::{
    DATE_FORMAT, EpisodeResponse, parse_episode_index, parse_season_index, parse_series_id,
};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to create or replace a season episode.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PutSeriesEpisodeRequest {
    /// Date of the first airing, in `YYYY-MM-DD` form, when known.
    #[schema(example = "1991-04-08")]
    pub air_date: Option<String>,
    /// Name of the episode.
    #[schema(min_length = 1, max_length = 100)]
    pub name: String,
    /// Plot of the episode, when known.
    pub plot: Option<String>,
    /// Identifier of the video backing the episode.
    #[schema(example = 1)]
    pub video_id: NumericID,
}

/// Create or replace a season episode.
///
/// Only a caller whose `library.manage` scope covers the whole catalogue may
/// write it. The write is idempotent by index: a free index creates the episode
/// and an existing index replaces it, replacing its video when a different one
/// is named.
#[utoipa::path(
    put,
    operation_id = "put_series_episode",
    path = "/series/{series_id}/season/{season_index}/episode/{episode_index}",
    tag = "library",
    params(
        ("series_id" = NumericID, Path, description = "Identifier of the show holding the episode"),
        ("season_index" = u32, Path, description = "Series-scoped index of the season"),
        ("episode_index" = u32, Path, description = "Season-scoped index of the episode"),
    ),
    request_body = PutSeriesEpisodeRequest,
    responses(
        (
            status = CREATED,
            body = EpisodeResponse,
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "URI of the newly created episode"),
            ),
            description = "Episode created",
        ),
        (
            status = OK,
            body = EpisodeResponse,
            content_type = "application/hal+json",
            description = "Episode replaced"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid show, season or episode identifier"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
        ),
        (
            status = FORBIDDEN,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The caller may not manage the show"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown or invisible show, or a season the show does not hold"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The video is already attached"
        ),
        (
            status = PAYLOAD_TOO_LARGE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body exceeds the maximum allowed size"
        ),
        (
            status = UNSUPPORTED_MEDIA_TYPE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body is not application/json"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The name is invalid, the air date is malformed, or the video does not exist"
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
    summary = "Create or replace a season episode"
)]
pub(crate) async fn put_series_episode(
    Path((series_id_value, season_index_value, episode_index_value)): Path<(
        String,
        String,
        String,
    )>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PutSeriesEpisodeRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let series_id = parse_series_id(&series_id_value)?;
    let season_index = parse_season_index(&season_index_value)?;
    let episode_index = parse_episode_index(&episode_index_value)?;
    let Json(request) = payload.map_err(ApiError::from)?;
    let air_date = match request.air_date.as_deref() {
        Some(value) => Some(NaiveDate::parse_from_str(value, DATE_FORMAT).map_err(|_| {
            warn!(
                target: "security",
                event = "input_validation_failed",
                field = "air_date",
                reason = "invalid_date",
                "rejected a malformed air date"
            );
            ApiError::UnprocessableEntity("the air date must use YYYY-MM-DD".to_owned())
        })?),
        None => None,
    };
    let response = state
        .library_use_case_factory()
        .put_series_episode()
        .execute(PutSeriesEpisodeCommand::new(
            caller.user_id(),
            series_id,
            season_index,
            episode_index,
            request.video_id,
            request.name,
            request.plot,
            air_date,
        ))
        .await?;
    let body = EpisodeResponse::from_episode(series_id, season_index, response.episode());
    let location = body.links.self_link.href.clone();
    let mut http_response = hal_json(body);
    if response.created() {
        *http_response.status_mut() = StatusCode::CREATED;
        http_response.headers_mut().insert(
            header::LOCATION,
            HeaderValue::from_str(&location).map_err(|_| ApiError::InternalServerError)?,
        );
    }
    Ok(http_response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::put_series_episode::{
        MockPutSeriesEpisodeUseCase, PutSeriesEpisodeError, PutSeriesEpisodeResponse,
        PutSeriesEpisodeUseCase,
    };
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::test_helpers::*;
    use axum::body::Body;
    use axum::extract::Request;
    use axum::routing::put;
    use rstest::rstest;
    use serde_json::json;
    use std::error::Error;
    use std::sync::Arc;
    use tower::ServiceExt as _;

    /// Send a PUT to the episode route on behalf of `caller_id`.
    async fn send(
        use_case: MockPutSeriesEpisodeUseCase,
        caller_id: NumericID,
        address: (&str, &str, &str),
        body: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_put_series_episode()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn PutSeriesEpisodeUseCase>);

        let mut request = Request::builder()
            .method("PUT")
            .uri(format!(
                "/api/v1/library/series/{}/season/{}/episode/{}",
                address.0, address.1, address.2
            ))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned()))?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/series/{series_id}/season/{season_index}/episode/{episode_index}",
                put(put_series_episode),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn put_series_episode_creates_the_episode() -> Result<(), Box<dyn Error>> {
        // Arrange
        let episode = sample_episode(1, 5)?;
        let mut use_case = MockPutSeriesEpisodeUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                command.caller_id() == 1
                    && command.series_id() == 7
                    && command.season_index() == 1
                    && command.episode_index() == 1
                    && command.video_id() == 5
                    && command.name() == "Pilot"
            })
            .return_once(move |_| {
                let response = PutSeriesEpisodeResponse::new(episode.clone(), true);
                Box::pin(async move { Ok(response) })
            });

        // Act
        let (status, content_type, payload) = into_parts(
            send(
                use_case,
                1,
                ("7", "1", "1"),
                r#"{"video_id":5,"name":"Pilot","plot":"A plot"}"#,
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(payload.get("id"), Some(&json!(1u32)));
        assert_eq!(payload.get("series_id"), Some(&json!(7i64)));
        assert_eq!(payload.get("season_index"), Some(&json!(1u32)));
        assert_eq!(payload.get("name"), Some(&json!("Episode 1")));
        assert_eq!(
            payload.pointer("/_links/self/href"),
            Some(&json!("/api/v1/library/series/7/season/1/episode/1"))
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_series_episode_returns_the_location_of_the_new_episode()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let episode = sample_episode(1, 5)?;
        let mut use_case = MockPutSeriesEpisodeUseCase::new();
        use_case.expect_execute().times(1).return_once(move |_| {
            let response = PutSeriesEpisodeResponse::new(episode.clone(), true);
            Box::pin(async move { Ok(response) })
        });

        // Act
        let response = send(
            use_case,
            1,
            ("7", "1", "1"),
            r#"{"video_id":5,"name":"Pilot"}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(
            response
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some("/api/v1/library/series/7/season/1/episode/1")
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_series_episode_replaces_the_episode() -> Result<(), Box<dyn Error>> {
        // Arrange
        let episode = sample_episode(1, 5)?;
        let mut use_case = MockPutSeriesEpisodeUseCase::new();
        use_case.expect_execute().times(1).return_once(move |_| {
            let response = PutSeriesEpisodeResponse::new(episode.clone(), false);
            Box::pin(async move { Ok(response) })
        });

        // Act
        let response = send(
            use_case,
            1,
            ("7", "1", "1"),
            r#"{"video_id":5,"name":"Pilot"}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().get(header::LOCATION).is_none());
        Ok(())
    }

    #[rstest]
    #[case::conflict(
        PutSeriesEpisodeError::Conflict,
        StatusCode::CONFLICT,
        "the episode cannot be written"
    )]
    #[case::forbidden(
        PutSeriesEpisodeError::Forbidden,
        StatusCode::FORBIDDEN,
        "the caller may not manage the library"
    )]
    #[case::invalid_name(
        PutSeriesEpisodeError::InvalidName,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the name must not be empty"
    )]
    #[case::name_too_long(
        PutSeriesEpisodeError::NameTooLong,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the name is too long"
    )]
    #[case::unknown_video(
        PutSeriesEpisodeError::NoSuchVideo,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a video with this identifier does not exist"
    )]
    #[case::unknown_season(
        PutSeriesEpisodeError::NoSuchSeason,
        StatusCode::NOT_FOUND,
        "a season with this index does not exist in the show"
    )]
    #[case::unknown_series(
        PutSeriesEpisodeError::NoSuchSeries,
        StatusCode::NOT_FOUND,
        "a show with this identifier does not exist"
    )]
    #[case::unknown_caller(
        PutSeriesEpisodeError::NoSuchCaller,
        StatusCode::UNAUTHORIZED,
        "the authenticated user does not exist"
    )]
    #[tokio::test]
    async fn put_series_episode_error_maps_to_status(
        #[case] error: PutSeriesEpisodeError,
        #[case] status: StatusCode,
        #[case] message: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockPutSeriesEpisodeUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));

        // Act
        let response = send(
            use_case,
            1,
            ("7", "1", "1"),
            r#"{"video_id":5,"name":"Pilot"}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), status);
        assert_eq!(error_message_of(&response).as_deref(), Some(message));
        Ok(())
    }

    #[tokio::test]
    async fn put_series_episode_dependency_failure_is_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockPutSeriesEpisodeUseCase::new();
        use_case.expect_execute().times(1).return_once(|_| {
            Box::pin(async { Err(PutSeriesEpisodeError::Unknown(anyhow::anyhow!("boom"))) })
        });

        // Act
        let response = send(
            use_case,
            1,
            ("7", "1", "1"),
            r#"{"video_id":5,"name":"Pilot"}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        Ok(())
    }

    #[tokio::test]
    async fn put_series_episode_invalid_air_date_is_unprocessable() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockPutSeriesEpisodeUseCase::new();

        // Act
        let response = send(
            use_case,
            1,
            ("7", "1", "1"),
            r#"{"video_id":5,"name":"Pilot","air_date":"1991-13-01"}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[rstest]
    #[case::series("abc", "1", "1")]
    #[case::season("7", "abc", "1")]
    #[case::episode("7", "1", "abc")]
    #[tokio::test]
    async fn put_series_episode_malformed_address_is_bad_request(
        #[case] series_id: &str,
        #[case] season_index: &str,
        #[case] episode_index: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockPutSeriesEpisodeUseCase::new();

        // Act
        let response = send(
            use_case,
            1,
            (series_id, season_index, episode_index),
            r#"{"video_id":5,"name":"Pilot"}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }

    #[tokio::test]
    async fn put_series_episode_malformed_body_is_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockPutSeriesEpisodeUseCase::new();

        // Act
        let response = send(use_case, 1, ("7", "1", "1"), "not json").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }

    #[tokio::test]
    async fn put_series_episode_missing_credentials_returns_unauthorized()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("PUT")
            .uri("/series/7/season/1/episode/1")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"video_id":5,"name":"Pilot"}"#))?;

        // Act
        let response: Response = axum::Router::from(library_routes(&state))
            .oneshot(request)
            .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
