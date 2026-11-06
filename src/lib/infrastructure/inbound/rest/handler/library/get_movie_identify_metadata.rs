use axum::extract::Path;
use axum::extract::State;
use axum::response::Response;
use tracing::warn;

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

/// Resolve one provider reference into a film metadata document.
///
/// The reference names the provider that issued it; a provider the system does
/// not serve answers `404 Not Found`. The route reads the provider rather than
/// the catalogue, and is guarded by `library.read`. With no provider configured
/// it answers an `available: false` document.
#[utoipa::path(
    get,
    operation_id = "identify_video_movie_metadata",
    path = "/movie/identify/{provider}/{reference}",
    tag = "library",
    params(
        ("provider" = String, Path, description = "Provider that issued the reference"),
        ("reference" = String, Path, description = "Identifier the provider issued"),
    ),
    responses(
        (
            status = OK,
            body = VideoMetadataDocumentResponse,
            content_type = "application/hal+json",
            description = "The provider-independent film metadata document"
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
            description = "The reference does not resolve to a film"
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
    summary = "Resolve a video provider reference into a film's metadata"
)]
pub(crate) async fn get_movie_identify_metadata(
    Path((provider_value, reference_value)): Path<(String, String)>,
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
    let href = format!(
        "/api/v1/library/movie/identify/{}/{reference_value}",
        provider.as_str()
    );
    let target = Target::Movie(ExternalRef::new(provider, reference_value));
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

    use super::get_movie_identify_metadata;
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
    const REFERENCE: &str = "27205";

    /// Film metadata for `Inception`.
    fn movie_metadata() -> MediaMetadata {
        MediaMetadata::Movie {
            artwork: None,
            country_of_origin: Some("US".to_owned()),
            genres: Vec::new(),
            plot: "A plot".to_owned(),
            release_date: NaiveDate::from_ymd_opt(2010, 7, 16),
            title: "Inception".to_owned(),
        }
    }

    /// Send a GET to the film resolve route on behalf of `caller_id`.
    async fn send(
        use_case: MockResolveVideoMetadataUseCase,
        caller_id: NumericID,
        provider: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_resolve_video_metadata()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn ResolveVideoMetadataUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!(
                "/api/v1/library/movie/identify/{provider}/{REFERENCE}"
            ))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/movie/identify/{provider}/{reference}",
                get(get_movie_identify_metadata),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn get_movie_identify_metadata_returns_the_document() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockResolveVideoMetadataUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| matches!(command.target(), Target::Movie(_)))
            .return_once(|_| {
                Box::pin(async { Ok(Some(ResolveVideoMetadataResponse::new(movie_metadata()))) })
            });

        // Act
        let (status, content_type, payload) = into_parts(send(use_case, 1, "tmdb").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(
            payload,
            json!({
                "_links": {
                    "self": { "href": "/api/v1/library/movie/identify/tmdb/27205" }
                },
                "available": true,
                "metadata": {
                    "kind": "movie",
                    "country_of_origin": "US",
                    "genres": [],
                    "plot": "A plot",
                    "release_date": "2010-07-16",
                    "title": "Inception"
                }
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_movie_identify_metadata_rejects_an_unknown_provider() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = MockResolveVideoMetadataUseCase::new();

        // Act
        let response = send(use_case, 1, "musicbrainz").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn get_movie_identify_metadata_reports_an_unknown_reference() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut use_case = MockResolveVideoMetadataUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Ok(None) }));

        // Act
        let response = send(use_case, 1, "tmdb").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn get_movie_identify_metadata_reports_an_unconfigured_provider()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockResolveVideoMetadataUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(ResolveVideoMetadataError::Unavailable) }));

        // Act
        let (status, _, payload) = into_parts(send(use_case, 1, "tmdb").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload.get("available"), Some(&json!(false)));
        Ok(())
    }

    #[tokio::test]
    async fn get_movie_identify_metadata_rejects_another_kind() -> Result<(), Box<dyn Error>> {
        // Arrange: the provider resolves a show under the film route.
        let mut use_case = MockResolveVideoMetadataUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(ResolveVideoMetadataError::KindMismatch) }));

        // Act
        let response = send(use_case, 1, "tmdb").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[tokio::test]
    async fn get_movie_identify_metadata_missing_credentials_returns_unauthorized()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the real router, wrapped in its `authenticate` layer.
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("GET")
            .uri("/movie/identify/tmdb/27205")
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
