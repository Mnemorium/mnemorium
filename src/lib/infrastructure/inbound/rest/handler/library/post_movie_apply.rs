use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::response::Response;
use tracing::warn;

use crate::application::port::apply_movie::ApplyMovieCommand;
use crate::domain::alias::NumericID;
use crate::domain::model::configuration::ProviderId;
use crate::domain::model::external_ref::ExternalRef;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::MovieResponse;
use crate::infrastructure::inbound::rest::handler::library::VideoApplyRequest;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Apply external metadata to a film.
///
/// Only a caller whose `library.manage` scope covers the film may write it. The
/// provider reference is resolved once and its name, plot, release date,
/// country of origin and genres replace the stored values; a value the provider
/// does not report is cleared. The cover and the backing video are untouched.
#[utoipa::path(
    post,
    operation_id = "apply_movie_metadata",
    path = "/movie/{movie_id}/apply",
    tag = "library",
    params(
        ("movie_id" = NumericID, Path, description = "Identifier of the film"),
    ),
    request_body = VideoApplyRequest,
    responses(
        (
            status = OK,
            body = MovieResponse,
            content_type = "application/hal+json",
            description = "Metadata applied"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid film identifier or malformed body"
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
            description = "Unknown or invisible film, or the provider does not know the reference"
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
            description = "The provider is not a video provider, or the reference resolves to another kind"
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
    summary = "Apply external metadata to a film"
)]
pub(crate) async fn post_movie_apply(
    Path(movie_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<VideoApplyRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let movie_id = movie_id_value.parse::<NumericID>().map_err(|_| {
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "movie_id",
            reason = "invalid_identifier",
            "rejected a malformed film identifier"
        );
        ApiError::BadRequest("invalid film identifier".to_owned())
    })?;
    let Json(request) = payload.map_err(ApiError::from)?;
    let provider = ProviderId::parse(&request.reference.provider)
        .filter(|provider| provider.serves_video())
        .ok_or_else(|| ApiError::reject_enum("provider"))?;
    let reference = ExternalRef::new(provider, request.reference.id);
    let response = state
        .library_use_case_factory()
        .apply_movie()
        .execute(ApplyMovieCommand::new(
            caller.user_id(),
            movie_id,
            reference,
        ))
        .await?;
    Ok(hal_json(MovieResponse::from(response.movie())))
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

    use super::post_movie_apply;
    use crate::application::port::apply_movie::ApplyMovieError;
    use crate::application::port::apply_movie::ApplyMovieResponse;
    use crate::application::port::apply_movie::ApplyMovieUseCase;
    use crate::application::port::apply_movie::MockApplyMovieUseCase;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::into_parts;
    use crate::test_helpers::sample_movie;

    /// Send a POST to the film apply route on behalf of `caller_id`.
    async fn send(
        use_case: MockApplyMovieUseCase,
        caller_id: NumericID,
        movie_id: &str,
        body: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_apply_movie()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn ApplyMovieUseCase>);

        let mut request = Request::builder()
            .method("POST")
            .uri(format!("/api/v1/library/movie/{movie_id}/apply"))
            .header("Content-Type", "application/json")
            .body(Body::from(body.to_owned()))?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/movie/{movie_id}/apply",
                post(post_movie_apply),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn post_movie_apply_returns_the_stored_film() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockApplyMovieUseCase::new();
        let movie = sample_movie(7)?;
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| command.caller_id() == 1 && command.movie_id() == 7)
            .return_once(move |_| Box::pin(async move { Ok(ApplyMovieResponse::new(movie)) }));

        // Act
        let (status, content_type, payload) = into_parts(
            send(
                use_case,
                1,
                "7",
                r#"{"reference":{"provider":"tmdb","id":"27205"}}"#,
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(payload.get("id"), Some(&json!(7i64)));
        Ok(())
    }

    #[tokio::test]
    async fn post_movie_apply_unknown_film_is_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockApplyMovieUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(ApplyMovieError::NoSuchMovie) }));

        // Act
        let response = send(
            use_case,
            1,
            "7",
            r#"{"reference":{"provider":"tmdb","id":"27205"}}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn post_movie_apply_forbidden_is_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockApplyMovieUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(ApplyMovieError::Forbidden) }));

        // Act
        let response = send(
            use_case,
            1,
            "7",
            r#"{"reference":{"provider":"tmdb","id":"27205"}}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        Ok(())
    }

    #[tokio::test]
    async fn post_movie_apply_conflict_is_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockApplyMovieUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(ApplyMovieError::Conflict) }));

        // Act
        let response = send(
            use_case,
            1,
            "7",
            r#"{"reference":{"provider":"tmdb","id":"27205"}}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::CONFLICT);
        Ok(())
    }

    #[tokio::test]
    async fn post_movie_apply_invalid_reference_is_unprocessable() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockApplyMovieUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(ApplyMovieError::InvalidReference) }));

        // Act
        let response = send(
            use_case,
            1,
            "7",
            r#"{"reference":{"provider":"tmdb","id":"27205"}}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[tokio::test]
    async fn post_movie_apply_rejects_a_music_provider() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockApplyMovieUseCase::new();

        // Act
        let response = send(
            use_case,
            1,
            "7",
            r#"{"reference":{"provider":"musicbrainz","id":"x"}}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[tokio::test]
    async fn post_movie_apply_malformed_identifier_is_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockApplyMovieUseCase::new();

        // Act
        let response = send(
            use_case,
            1,
            "not-a-number",
            r#"{"reference":{"provider":"tmdb","id":"27205"}}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }

    #[tokio::test]
    async fn post_movie_apply_missing_credentials_returns_unauthorized()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the real router, wrapped in its `authenticate` layer.
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("POST")
            .uri("/movie/7/apply")
            .header("Content-Type", "application/json")
            .body(Body::from(
                r#"{"reference":{"provider":"tmdb","id":"27205"}}"#,
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
    async fn post_movie_apply_dependency_failure_is_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockApplyMovieUseCase::new();
        use_case.expect_execute().times(1).return_once(|_| {
            Box::pin(async { Err(ApplyMovieError::Unknown(anyhow::anyhow!("boom"))) })
        });

        // Act
        let response = send(
            use_case,
            1,
            "7",
            r#"{"reference":{"provider":"tmdb","id":"27205"}}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        Ok(())
    }
}
