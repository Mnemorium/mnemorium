use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::response::Response;
use chrono::NaiveDate;
use serde::Deserialize;
use tracing::warn;
use utoipa::ToSchema;

use crate::application::field_update::FieldUpdate;
use crate::application::port::update_series_episode::UpdateSeriesEpisodeCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::DATE_FORMAT;
use crate::infrastructure::inbound::rest::handler::library::EpisodeResponse;
use crate::infrastructure::inbound::rest::handler::library::deserialize_field_update;
use crate::infrastructure::inbound::rest::handler::library::parse_episode_index;
use crate::infrastructure::inbound::rest::handler::library::parse_season_index;
use crate::infrastructure::inbound::rest::handler::library::parse_series_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to update a season episode partially.
///
/// An absent field keeps its value; a `null` clears the air date. The backing
/// video is never part of the payload.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PatchSeriesEpisodeRequest {
    /// Date of the first airing in `YYYY-MM-DD` form; `null` clears it.
    #[serde(default, deserialize_with = "deserialize_field_update")]
    #[schema(value_type = Option<String>, nullable = true)]
    pub air_date: FieldUpdate<String>,
    /// Name of the episode.
    #[schema(min_length = 1, max_length = 100)]
    pub name: Option<String>,
    /// Plot of the episode; omit to keep it.
    pub plot: Option<String>,
}

/// Update a season episode partially.
///
/// Only a caller whose `library.manage` scope covers the show may update it.
#[utoipa::path(
    patch,
    operation_id = "patch_series_episode",
    path = "/series/{series_id}/season/{season_index}/episode/{episode_index}",
    tag = "library",
    params(
        ("series_id" = NumericID, Path, description = "Identifier of the show holding the episode"),
        ("season_index" = u32, Path, description = "Series-scoped index of the season"),
        ("episode_index" = u32, Path, description = "Season-scoped index of the episode"),
    ),
    request_body = PatchSeriesEpisodeRequest,
    responses(
        (
            status = OK,
            body = EpisodeResponse,
            content_type = "application/hal+json",
            description = "Episode updated"
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
            description = "Unknown or invisible show, season or episode"
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
            description = "The name is invalid or the air date is malformed"
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
    summary = "Update a season episode"
)]
pub(crate) async fn patch_series_episode(
    Path((series_id_value, season_index_value, episode_index_value)): Path<(
        String,
        String,
        String,
    )>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PatchSeriesEpisodeRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let series_id = parse_series_id(&series_id_value)?;
    let season_index = parse_season_index(&season_index_value)?;
    let episode_index = parse_episode_index(&episode_index_value)?;
    let Json(request) = payload.map_err(ApiError::from)?;
    let air_date = request.air_date.try_map(|value| {
        NaiveDate::parse_from_str(&value, DATE_FORMAT).map_err(|_| {
            warn!(
                target: "security",
                event = "input_validation_failed",
                field = "air_date",
                reason = "invalid_date",
                "rejected a malformed air date"
            );
            ApiError::UnprocessableEntity("the air date must use YYYY-MM-DD".to_owned())
        })
    })?;
    let response = state
        .library_use_case_factory()
        .update_series_episode()
        .execute(UpdateSeriesEpisodeCommand::new(
            caller.user_id(),
            series_id,
            season_index,
            episode_index,
            request.name,
            request.plot,
            air_date,
        ))
        .await?;
    Ok(hal_json(EpisodeResponse::from_episode(
        series_id,
        season_index,
        response.episode(),
    )))
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::response::Response;
    use axum::routing::patch;
    use rstest::rstest;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::patch_series_episode;
    use crate::application::field_update::FieldUpdate;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::update_series_episode::MockUpdateSeriesEpisodeUseCase;
    use crate::application::port::update_series_episode::UpdateSeriesEpisodeError;
    use crate::application::port::update_series_episode::UpdateSeriesEpisodeResponse;
    use crate::application::port::update_series_episode::UpdateSeriesEpisodeUseCase;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_parts;
    use crate::test_helpers::sample_episode;

    /// Send a PATCH to the episode route on behalf of `caller_id`.
    async fn send(
        use_case: MockUpdateSeriesEpisodeUseCase,
        caller_id: NumericID,
        address: (&str, &str, &str),
        body: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_update_series_episode()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn UpdateSeriesEpisodeUseCase>);

        let mut request = Request::builder()
            .method("PATCH")
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
                patch(patch_series_episode),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn patch_series_episode_updates_the_episode() -> Result<(), Box<dyn Error>> {
        // Arrange
        let episode = sample_episode(1, 5)?;
        let mut use_case = MockUpdateSeriesEpisodeUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                command.caller_id() == 1
                    && command.series_id() == 7
                    && command.season_index() == 1
                    && command.episode_index() == 1
                    && command.name() == Some("New Name")
            })
            .return_once(move |_| {
                let response = UpdateSeriesEpisodeResponse::new(episode.clone());
                Box::pin(async move { Ok(response) })
            });

        // Act
        let (status, content_type, payload) =
            into_parts(send(use_case, 1, ("7", "1", "1"), r#"{"name":"New Name"}"#).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(payload.get("id"), Some(&json!(1u32)));
        assert_eq!(payload.get("series_id"), Some(&json!(7i64)));
        assert_eq!(payload.get("season_index"), Some(&json!(1u32)));
        assert_eq!(
            payload.pointer("/_links/self/href"),
            Some(&json!("/api/v1/library/series/7/season/1/episode/1"))
        );
        Ok(())
    }

    #[tokio::test]
    async fn patch_series_episode_null_clears_the_air_date() -> Result<(), Box<dyn Error>> {
        // Arrange
        let episode = sample_episode(1, 5)?;
        let mut use_case = MockUpdateSeriesEpisodeUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| command.air_date() == FieldUpdate::Clear)
            .return_once(move |_| {
                let response = UpdateSeriesEpisodeResponse::new(episode.clone());
                Box::pin(async move { Ok(response) })
            });

        // Act
        let response = send(use_case, 1, ("7", "1", "1"), r#"{"air_date":null}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }

    #[rstest]
    #[case::forbidden(
        UpdateSeriesEpisodeError::Forbidden,
        StatusCode::FORBIDDEN,
        "the caller may not manage the library"
    )]
    #[case::invalid_name(
        UpdateSeriesEpisodeError::InvalidName,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the name must not be empty"
    )]
    #[case::name_too_long(
        UpdateSeriesEpisodeError::NameTooLong,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the name is too long"
    )]
    #[case::unknown_episode(
        UpdateSeriesEpisodeError::NoSuchEpisode,
        StatusCode::NOT_FOUND,
        "an episode with this index does not exist in the season"
    )]
    #[case::unknown_season(
        UpdateSeriesEpisodeError::NoSuchSeason,
        StatusCode::NOT_FOUND,
        "a season with this index does not exist in the show"
    )]
    #[case::unknown_series(
        UpdateSeriesEpisodeError::NoSuchSeries,
        StatusCode::NOT_FOUND,
        "a show with this identifier does not exist"
    )]
    #[case::unknown_caller(
        UpdateSeriesEpisodeError::NoSuchCaller,
        StatusCode::UNAUTHORIZED,
        "the authenticated user does not exist"
    )]
    #[tokio::test]
    async fn patch_series_episode_error_maps_to_status(
        #[case] error: UpdateSeriesEpisodeError,
        #[case] status: StatusCode,
        #[case] message: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockUpdateSeriesEpisodeUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));

        // Act
        let response = send(use_case, 1, ("7", "1", "1"), r#"{"name":"New Name"}"#).await?;

        // Assert
        assert_eq!(response.status(), status);
        assert_eq!(error_message_of(&response).as_deref(), Some(message));
        Ok(())
    }

    #[tokio::test]
    async fn patch_series_episode_dependency_failure_is_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockUpdateSeriesEpisodeUseCase::new();
        use_case.expect_execute().times(1).return_once(|_| {
            Box::pin(async { Err(UpdateSeriesEpisodeError::Unknown(anyhow::anyhow!("boom"))) })
        });

        // Act
        let response = send(use_case, 1, ("7", "1", "1"), r#"{"name":"New Name"}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        Ok(())
    }

    #[tokio::test]
    async fn patch_series_episode_invalid_air_date_is_unprocessable() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = MockUpdateSeriesEpisodeUseCase::new();

        // Act
        let response = send(use_case, 1, ("7", "1", "1"), r#"{"air_date":"1991-13-01"}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[rstest]
    #[case::series("abc", "1", "1")]
    #[case::season("7", "abc", "1")]
    #[case::episode("7", "1", "abc")]
    #[tokio::test]
    async fn patch_series_episode_malformed_address_is_bad_request(
        #[case] series_id: &str,
        #[case] season_index: &str,
        #[case] episode_index: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockUpdateSeriesEpisodeUseCase::new();

        // Act
        let response = send(
            use_case,
            1,
            (series_id, season_index, episode_index),
            r#"{"name":"New Name"}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }

    #[tokio::test]
    async fn patch_series_episode_malformed_body_is_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockUpdateSeriesEpisodeUseCase::new();

        // Act
        let response = send(use_case, 1, ("7", "1", "1"), "not json").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }

    #[tokio::test]
    async fn patch_series_episode_missing_credentials_returns_unauthorized()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("PATCH")
            .uri("/series/7/season/1/episode/1")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"name":"New Name"}"#))?;

        // Act
        let response: Response = axum::Router::from(library_routes(&state))
            .oneshot(request)
            .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
