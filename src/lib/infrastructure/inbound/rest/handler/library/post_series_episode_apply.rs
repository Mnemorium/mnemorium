use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::response::Response;

use crate::application::port::apply_series_episode::ApplySeriesEpisodeCommand;
use crate::domain::alias::NumericID;
use crate::domain::model::configuration::ProviderId;
use crate::domain::model::external_ref::ExternalRef;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::EpisodeResponse;
use crate::infrastructure::inbound::rest::handler::library::VideoApplyRequest;
use crate::infrastructure::inbound::rest::handler::library::parse_episode_index;
use crate::infrastructure::inbound::rest::handler::library::parse_season_index;
use crate::infrastructure::inbound::rest::handler::library::parse_series_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Apply external metadata to one episode.
///
/// Only a caller whose `library.manage` scope covers the show may write it. The
/// provider is asked once for the addressed position; a provider that reports
/// another position is rejected. The name, plot and air date are written, an
/// air date the provider does not report is cleared, and the video behind the
/// episode is untouched.
#[utoipa::path(
    post,
    operation_id = "apply_series_episode_metadata",
    path = "/series/{series_id}/season/{season_index}/episode/{episode_index}/apply",
    tag = "library",
    params(
        ("series_id" = NumericID, Path, description = "Identifier of the show holding the episode"),
        ("season_index" = u32, Path, description = "Series-scoped index of the season"),
        ("episode_index" = u32, Path, description = "Season-scoped index of the episode"),
    ),
    request_body = VideoApplyRequest,
    responses(
        (
            status = OK,
            body = EpisodeResponse,
            content_type = "application/hal+json",
            description = "Metadata applied"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid identifier or index, or malformed body"
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
            description = "The caller may not manage the library"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown or invisible show, season or episode, or the provider does not know the reference"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The applied metadata conflicts with the stored catalogue"
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
            description = "The provider is not a video provider, the reference resolves to another kind, or the provider reports another position"
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unexpected error, the provider failed, or no video metadata provider is configured"
        ),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "Apply external metadata to an episode"
)]
pub(crate) async fn post_series_episode_apply(
    Path((series_id_value, season_index_value, episode_index_value)): Path<(
        String,
        String,
        String,
    )>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<VideoApplyRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let series_id = parse_series_id(&series_id_value)?;
    let season_index = parse_season_index(&season_index_value)?;
    let episode_index = parse_episode_index(&episode_index_value)?;
    let Json(request) = payload.map_err(ApiError::from)?;
    let provider = ProviderId::parse(&request.reference.provider)
        .filter(|provider| provider.serves_video())
        .ok_or_else(|| ApiError::reject_enum("provider"))?;
    let reference = ExternalRef::new(provider, request.reference.id);
    let response = state
        .library_use_case_factory()
        .apply_series_episode()
        .execute(ApplySeriesEpisodeCommand::new(
            caller.user_id(),
            series_id,
            season_index,
            episode_index,
            reference,
        ))
        .await?;
    Ok(hal_json(EpisodeResponse::from_episode(
        response.series_id(),
        response.season_index(),
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
    use axum::response::Response;
    use axum::routing::post;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::post_series_episode_apply;
    use crate::application::port::apply_series_episode::ApplySeriesEpisodeError;
    use crate::application::port::apply_series_episode::ApplySeriesEpisodeResponse;
    use crate::application::port::apply_series_episode::ApplySeriesEpisodeUseCase;
    use crate::application::port::apply_series_episode::MockApplySeriesEpisodeUseCase;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::into_parts;
    use crate::test_helpers::sample_episode;

    /// Send a POST to the episode apply route on behalf of `caller_id`.
    async fn send(
        use_case: MockApplySeriesEpisodeUseCase,
        caller_id: NumericID,
        path: (&str, &str, &str),
        body: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_apply_series_episode()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn ApplySeriesEpisodeUseCase>);

        let (series_id, season_index, episode_index) = path;
        let mut request = Request::builder()
            .method("POST")
            .uri(format!(
                "/api/v1/library/series/{series_id}/season/{season_index}/episode/{episode_index}/apply"
            ))
            .header("Content-Type", "application/json")
            .body(Body::from(body.to_owned()))?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/series/{series_id}/season/{season_index}/episode/{episode_index}/apply",
                post(post_series_episode_apply),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn post_series_episode_apply_returns_the_stored_episode() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockApplySeriesEpisodeUseCase::new();
        let episode = sample_episode(1, 20)?;
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                command.caller_id() == 1
                    && command.series_id() == 7
                    && command.season_index() == 1
                    && command.episode_index() == 1
            })
            .return_once(move |_| {
                Box::pin(async move { Ok(ApplySeriesEpisodeResponse::new(7, 1, 1, episode)) })
            });

        // Act
        let (status, content_type, payload) = into_parts(
            send(
                use_case,
                1,
                ("7", "1", "1"),
                r#"{"reference":{"provider":"tmdb","id":"1920"}}"#,
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(payload.get("id"), Some(&json!(1u32)));
        assert_eq!(payload.get("series_id"), Some(&json!(7i64)));
        Ok(())
    }

    #[tokio::test]
    async fn post_series_episode_apply_unknown_episode_is_not_found() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut use_case = MockApplySeriesEpisodeUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(ApplySeriesEpisodeError::NoSuchEpisode) }));

        // Act
        let response = send(
            use_case,
            1,
            ("7", "1", "1"),
            r#"{"reference":{"provider":"tmdb","id":"1920"}}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn post_series_episode_apply_position_mismatch_is_unprocessable()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockApplySeriesEpisodeUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(ApplySeriesEpisodeError::PositionMismatch) }));

        // Act
        let response = send(
            use_case,
            1,
            ("7", "1", "1"),
            r#"{"reference":{"provider":"tmdb","id":"1920"}}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[tokio::test]
    async fn post_series_episode_apply_forbidden_is_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockApplySeriesEpisodeUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(ApplySeriesEpisodeError::Forbidden) }));

        // Act
        let response = send(
            use_case,
            1,
            ("7", "1", "1"),
            r#"{"reference":{"provider":"tmdb","id":"1920"}}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        Ok(())
    }

    #[tokio::test]
    async fn post_series_episode_apply_malformed_index_is_bad_request() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = MockApplySeriesEpisodeUseCase::new();

        // Act
        let response = send(
            use_case,
            1,
            ("7", "not-a-number", "1"),
            r#"{"reference":{"provider":"tmdb","id":"1920"}}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }

    #[tokio::test]
    async fn post_series_episode_apply_missing_credentials_returns_unauthorized()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the real router, wrapped in its `authenticate` layer.
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("POST")
            .uri("/series/7/season/1/episode/1/apply")
            .header("Content-Type", "application/json")
            .body(Body::from(
                r#"{"reference":{"provider":"tmdb","id":"1920"}}"#,
            ))?;

        // Act
        let response: Response = axum::Router::from(library_routes(&state))
            .oneshot(request)
            .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[tokio::test]
    async fn post_series_episode_apply_conflict_is_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockApplySeriesEpisodeUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(ApplySeriesEpisodeError::Conflict) }));

        // Act
        let response = send(
            use_case,
            1,
            ("7", "1", "1"),
            r#"{"reference":{"provider":"tmdb","id":"1920"}}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::CONFLICT);
        Ok(())
    }

    #[tokio::test]
    async fn post_series_episode_apply_dependency_failure_is_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockApplySeriesEpisodeUseCase::new();
        use_case.expect_execute().times(1).return_once(|_| {
            Box::pin(async { Err(ApplySeriesEpisodeError::Unknown(anyhow::anyhow!("boom"))) })
        });

        // Act
        let response = send(
            use_case,
            1,
            ("7", "1", "1"),
            r#"{"reference":{"provider":"tmdb","id":"1920"}}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        Ok(())
    }
}
