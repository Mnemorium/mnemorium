use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::extract::rejection::QueryRejection;
use axum::response::Response;
use serde::Deserialize;
use tracing::warn;
use utoipa::IntoParams;

use crate::application::port::resolve_video_metadata::ResolveVideoMetadataCommand;
use crate::application::port::resolve_video_metadata::ResolveVideoMetadataError;
use crate::domain::model::configuration::ProviderId;
use crate::domain::model::external_ref::ExternalRef;
use crate::domain::model::video_candidate::Target;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::VideoMetadataDocumentResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// The optional episode position a show reference may address.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[non_exhaustive]
pub(crate) struct IdentifySeriesMetadataQuery {
    /// Index of the episode within its season; required with `season`.
    episode: Option<String>,
    /// Index of the season within the show; required with `episode`.
    season: Option<String>,
}

/// Resolve one provider reference into a show or episode metadata document.
///
/// Without a position the reference resolves to the show; with both `season` and
/// `episode` it resolves to that episode through the show's reference. A partial
/// or out-of-range position is rejected. A provider the system does not serve
/// answers `404 Not Found`, and no configured provider answers an
/// `available: false` document.
#[utoipa::path(
    get,
    operation_id = "identify_video_series_metadata",
    path = "/series/identify/{provider}/{reference}",
    tag = "library",
    params(
        ("provider" = String, Path, description = "Provider that issued the reference"),
        ("reference" = String, Path, description = "Identifier the provider issued"),
        IdentifySeriesMetadataQuery,
    ),
    responses(
        (
            status = OK,
            body = VideoMetadataDocumentResponse,
            content_type = "application/hal+json",
            description = "The provider-independent show or episode metadata document"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Malformed query string"
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
            description = "The provider is not served, the caller may not read the library, or the provider does not know the reference"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Only one of season and episode was supplied, a position is not a 32-bit unsigned integer, or the provider reported another kind or position"
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
    summary = "Resolve a video provider reference into a show or episode's metadata"
)]
pub(crate) async fn get_series_identify_metadata(
    Path((provider_value, reference_value)): Path<(String, String)>,
    raw_query: Result<Query<IdentifySeriesMetadataQuery>, QueryRejection>,
    caller: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Response, ApiError> {
    let provider = ProviderId::parse(&provider_value)
        .filter(|provider| provider.serves_video())
        .ok_or_else(|| {
            warn!(
                target: "security",
                event = "input_validation_failed",
                field = "provider",
                reason = "unknown_provider",
                "rejected an unknown video metadata provider"
            );
            ApiError::NotFound("the video metadata provider is not configured".to_owned())
        })?;
    let Query(query) = raw_query.map_err(ApiError::from)?;
    let position = match (query.season, query.episode) {
        (None, None) => None,
        (Some(season_value), Some(episode_value)) => {
            let season = season_value.parse::<u32>().map_err(|_| {
                warn!(
                    target: "security",
                    event = "input_validation_failed",
                    field = "episode_position",
                    reason = "invalid_value",
                    "rejected an invalid episode position"
                );
                ApiError::UnprocessableEntity(
                    "the season and episode must be supplied together as 32-bit unsigned integers"
                        .to_owned(),
                )
            })?;
            let episode = episode_value.parse::<u32>().map_err(|_| {
                warn!(
                    target: "security",
                    event = "input_validation_failed",
                    field = "episode_position",
                    reason = "invalid_value",
                    "rejected an invalid episode position"
                );
                ApiError::UnprocessableEntity(
                    "the season and episode must be supplied together as 32-bit unsigned integers"
                        .to_owned(),
                )
            })?;
            Some((season, episode))
        }
        _ => {
            warn!(
                target: "security",
                event = "input_validation_failed",
                field = "episode_position",
                reason = "invalid_value",
                "rejected an invalid episode position"
            );
            return Err(ApiError::UnprocessableEntity(
                "the season and episode must be supplied together as 32-bit unsigned integers"
                    .to_owned(),
            ));
        }
    };
    let href = match position {
        Some((season, episode)) => format!(
            "/api/v1/library/series/identify/{}/{reference_value}?season={season}&episode={episode}",
            provider.as_str()
        ),
        None => format!(
            "/api/v1/library/series/identify/{}/{reference_value}",
            provider.as_str()
        ),
    };
    let reference = ExternalRef::new(provider, reference_value);
    let target = match position {
        Some((season, episode)) => Target::Episode {
            episode,
            reference,
            season,
        },
        None => Target::Series(reference),
    };
    let result = state
        .library_use_case_factory()
        .resolve_video_metadata()
        .execute(ResolveVideoMetadataCommand::new(caller.user_id(), target))
        .await;
    match result {
        Ok(Some(response)) => Ok(hal_json(VideoMetadataDocumentResponse::new(
            &href,
            response.metadata(),
        ))),
        Ok(None) => Err(ApiError::NotFound(
            "the video metadata provider does not know the reference".to_owned(),
        )),
        Err(ResolveVideoMetadataError::Unavailable) => {
            Ok(hal_json(VideoMetadataDocumentResponse::unavailable(&href)))
        }
        Err(error) => Err(ApiError::from(error)),
    }
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
    use chrono::NaiveDate;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::get_series_identify_metadata;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::resolve_video_metadata::MockResolveVideoMetadataUseCase;
    use crate::application::port::resolve_video_metadata::ResolveVideoMetadataError;
    use crate::application::port::resolve_video_metadata::ResolveVideoMetadataResponse;
    use crate::application::port::resolve_video_metadata::ResolveVideoMetadataUseCase;
    use crate::domain::alias::NumericID;
    use crate::domain::model::media_metadata::MediaMetadata;
    use crate::domain::model::video_candidate::Target;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::into_parts;

    /// The reference every test resolves.
    const REFERENCE: &str = "1920";

    /// Show metadata for `Twin Peaks`.
    fn series_metadata() -> MediaMetadata {
        MediaMetadata::Series {
            artwork: None,
            country_of_origin: None,
            first_air_date: None,
            genres: Vec::new(),
            plot: "A plot".to_owned(),
            title: "Twin Peaks".to_owned(),
        }
    }

    /// Episode metadata for the addressed position.
    fn episode_metadata(season: u32, episode: u32) -> MediaMetadata {
        MediaMetadata::Episode {
            air_date: NaiveDate::from_ymd_opt(1990, 4, 8),
            artwork: None,
            episode,
            plot: "A plot".to_owned(),
            season,
            title: "Pilot".to_owned(),
        }
    }

    /// Send a GET to the show resolve route on behalf of `caller_id`.
    async fn send(
        use_case: MockResolveVideoMetadataUseCase,
        caller_id: NumericID,
        query: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_resolve_video_metadata()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn ResolveVideoMetadataUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!(
                "/api/v1/library/series/identify/tmdb/{REFERENCE}{query}"
            ))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/series/identify/{provider}/{reference}",
                get(get_series_identify_metadata),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn get_series_identify_metadata_returns_the_show() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockResolveVideoMetadataUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| matches!(command.target(), Target::Series(_)))
            .return_once(|_| {
                Box::pin(async { Ok(Some(ResolveVideoMetadataResponse::new(series_metadata()))) })
            });

        // Act
        let (status, _, payload) = into_parts(send(use_case, 1, "").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload.pointer("/metadata/kind"), Some(&json!("series")));
        Ok(())
    }

    #[tokio::test]
    async fn get_series_identify_metadata_returns_the_episode() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockResolveVideoMetadataUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                matches!(
                    command.target(),
                    Target::Episode {
                        season: 1,
                        episode: 2,
                        ..
                    }
                )
            })
            .return_once(|_| {
                Box::pin(async {
                    Ok(Some(ResolveVideoMetadataResponse::new(episode_metadata(
                        1, 2,
                    ))))
                })
            });

        // Act
        let (status, _, payload) =
            into_parts(send(use_case, 1, "?season=1&episode=2").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload.pointer("/metadata/kind"), Some(&json!("episode")));
        assert_eq!(
            payload.pointer("/metadata/episode"),
            Some(&json!(2u32)),
            "the document must carry the addressed position"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_series_identify_metadata_rejects_a_partial_position() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = MockResolveVideoMetadataUseCase::new();

        // Act
        let response = send(use_case, 1, "?season=1").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[tokio::test]
    async fn get_series_identify_metadata_rejects_an_out_of_range_position()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockResolveVideoMetadataUseCase::new();

        // Act
        let response = send(use_case, 1, "?season=-1&episode=2").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[tokio::test]
    async fn get_series_identify_metadata_rejects_another_position() -> Result<(), Box<dyn Error>> {
        // Arrange: the provider reports another position.
        let mut use_case = MockResolveVideoMetadataUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(ResolveVideoMetadataError::PositionMismatch) }));

        // Act
        let response = send(use_case, 1, "?season=1&episode=2").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[tokio::test]
    async fn get_series_identify_metadata_rejects_an_unknown_provider() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut request = Request::builder()
            .method("GET")
            .uri("/api/v1/library/series/identify/musicbrainz/1920")
            .body(Body::empty())?;
        request.extensions_mut().insert(AuthenticatedUser::from(1));

        // Act
        let factory = MockLibraryUseCaseFactory::new();
        let state = app_state_with_library(Arc::new(factory))?;
        let response = axum::Router::new()
            .route(
                "/api/v1/library/series/identify/{provider}/{reference}",
                get(get_series_identify_metadata),
            )
            .with_state(state)
            .oneshot(request)
            .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn get_series_identify_metadata_reports_an_unconfigured_provider()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockResolveVideoMetadataUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(ResolveVideoMetadataError::Unavailable) }));

        // Act
        let (status, _, payload) = into_parts(send(use_case, 1, "").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload.get("available"), Some(&json!(false)));
        Ok(())
    }

    #[tokio::test]
    async fn get_series_identify_metadata_missing_credentials_returns_unauthorized()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the real router, wrapped in its `authenticate` layer.
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("GET")
            .uri("/series/identify/tmdb/1920")
            .body(Body::empty())?;

        // Act
        let response: Response = axum::Router::from(library_routes(&state))
            .oneshot(request)
            .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
