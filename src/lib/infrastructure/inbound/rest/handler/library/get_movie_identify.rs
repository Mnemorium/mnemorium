use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::response::Response;
use serde::Deserialize;
use tracing::warn;
use utoipa::IntoParams;

use crate::application::port::identify_video::{IdentifyVideoCommand, IdentifyVideoError};
use crate::domain::model::media_query::VideoKind;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::VideoCandidateListResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// The name a caller knows about the film work it wants identified.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[non_exhaustive]
pub(crate) struct IdentifyMovieQuery {
    /// Media name to search for; never empty.
    name: String,
}

/// Search the configured video provider for film candidates.
///
/// The route reads a provider rather than the catalogue, so it is guarded by
/// `library.read` and returns only the candidates the film kind names. With no
/// provider configured it answers an `available: false` document.
#[utoipa::path(
    get,
    operation_id = "identify_video_movie",
    path = "/movie/identify",
    tag = "library",
    params(IdentifyMovieQuery),
    responses(
        (
            status = OK,
            body = VideoCandidateListResponse,
            content_type = "application/hal+json",
            description = "Film candidates the configured provider reported, in its order"
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
            description = "The caller may not read the library"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The name is empty"
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
    summary = "Search the configured video metadata provider for films"
)]
pub(crate) async fn get_movie_identify(
    raw_query: Result<Query<IdentifyMovieQuery>, QueryRejection>,
    caller: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Response, ApiError> {
    let Query(query) = raw_query.map_err(ApiError::from)?;
    if query.name.is_empty() {
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "name",
            reason = "empty",
            "rejected an empty video identification name"
        );
        return Err(ApiError::UnprocessableEntity(
            "the name must not be empty".to_owned(),
        ));
    }
    // The representation is the answer to the search view, so its `self` link
    // addresses the surface rather than repeating the caller's filter.
    let href = "/api/v1/library/movie/identify";
    let result = state
        .library_use_case_factory()
        .identify_video()
        .execute(IdentifyVideoCommand::new(
            caller.user_id(),
            VideoKind::Movie,
            query.name,
        ))
        .await;
    match result {
        Ok(response) => Ok(hal_json(VideoCandidateListResponse::new(
            href,
            response.candidates(),
        ))),
        Err(IdentifyVideoError::Unavailable) => {
            Ok(hal_json(VideoCandidateListResponse::unavailable(href)))
        }
        Err(error) => Err(ApiError::from(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::port::identify_video::{
        IdentifyVideoResponse, IdentifyVideoUseCase, MockIdentifyVideoUseCase,
    };
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::domain::alias::NumericID;
    use crate::domain::model::configuration::ProviderId;
    use crate::domain::model::external_ref::ExternalRef;
    use crate::domain::model::video_candidate::Candidate;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::test_helpers::*;
    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::routing::get;
    use serde_json::json;
    use std::error::Error;
    use std::sync::Arc;
    use tower::ServiceExt as _;

    /// A film candidate for `Inception`.
    fn movie_candidate() -> Candidate {
        Candidate::new(
            ExternalRef::new(ProviderId::Tmdb, "27205".to_owned()),
            VideoKind::Movie,
            "Inception".to_owned(),
            Some(2010),
        )
    }

    /// Send a GET to the film identify route on behalf of `caller_id`.
    async fn send(
        use_case: MockIdentifyVideoUseCase,
        caller_id: NumericID,
        query: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_identify_video()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn IdentifyVideoUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/library/movie/identify?{query}"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/library/movie/identify", get(get_movie_identify))
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn get_movie_identify_returns_the_candidates() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockIdentifyVideoUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                command.caller_id() == 1
                    && command.kind() == VideoKind::Movie
                    && command.name() == "Inception (2010).mkv"
            })
            .return_once(|_| {
                Box::pin(async { Ok(IdentifyVideoResponse::new(vec![movie_candidate()])) })
            });

        // Act
        let (status, content_type, payload) =
            into_parts(send(use_case, 1, "name=Inception%20%282010%29.mkv").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            content_type.as_deref(),
            Some("application/hal+json"),
            "the representation must be served as HAL"
        );
        assert_eq!(
            payload,
            json!({
                "_links": { "self": { "href": "/api/v1/library/movie/identify" } },
                "available": true,
                "items": [{
                    "kind": "movie",
                    "reference": { "provider": "tmdb", "id": "27205" },
                    "title": "Inception",
                    "year": 2010u32
                }]
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_movie_identify_reports_an_unconfigured_provider() -> Result<(), Box<dyn Error>> {
        // Arrange: no provider is configured.
        let mut use_case = MockIdentifyVideoUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(IdentifyVideoError::Unavailable) }));

        // Act
        let (status, _, payload) = into_parts(send(use_case, 1, "name=Inception").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload.get("available"), Some(&json!(false)));
        assert_eq!(payload.get("items"), Some(&json!([])));
        Ok(())
    }

    #[tokio::test]
    async fn get_movie_identify_rejects_an_empty_name() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockIdentifyVideoUseCase::new();

        // Act
        let response = send(use_case, 1, "name=").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[tokio::test]
    async fn get_movie_identify_conceals_a_denied_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockIdentifyVideoUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(IdentifyVideoError::NotPermitted) }));

        // Act
        let response = send(use_case, 1, "name=Inception").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn get_movie_identify_missing_credentials_returns_unauthorized()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the real router, wrapped in its `authenticate` layer.
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("GET")
            .uri("/movie/identify?name=Inception")
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
