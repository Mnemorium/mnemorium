use axum::extract::Path;
use axum::extract::State;
use axum::response::Response;
use tracing::warn;

use crate::application::port::get_movie::GetMovieCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::MovieResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Fetch one film by its identifier.
///
/// Any authenticated caller may read a film; the film is not owned and carries
/// no visibility flag. The representation carries the film's genres and the
/// video that backs it.
#[utoipa::path(
    get,
    operation_id = "get_movie",
    path = "/library/movie/{movie_id}",
    tag = "library",
    params(
        ("movie_id" = NumericID, Path, description = "Identifier of the film to fetch"),
    ),
    responses(
        (
            status = OK,
            body = MovieResponse,
            content_type = "application/hal+json",
            description = "Film found"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid film identifier"
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
            description = "Unknown film"
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
    summary = "Fetch a film"
)]
pub async fn get_movie(
    Path(movie_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let movie_id = movie_id_value.parse::<NumericID>().map_err(|_| {
        // The malformed value and the parser detail never reach the log
        // (`OBS-003`); only the classification and its declared fields do
        // (`OBS-006`).
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "movie_id",
            reason = "invalid_identifier",
            "rejected a malformed film identifier"
        );
        ApiError::BadRequest("invalid film identifier".to_owned())
    })?;
    let response = state
        .library_use_case_factory()
        .get_movie()
        .execute(GetMovieCommand::new(caller.user_id(), movie_id))
        .await?;
    Ok(hal_json(MovieResponse::from(&response)))
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

    use super::get_movie;
    use crate::application::port::get_movie::GetMovieError;
    use crate::application::port::get_movie::GetMovieResponse;
    use crate::application::port::get_movie::GetMovieUseCase;
    use crate::application::port::get_movie::MockGetMovieUseCase;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::video_summary::VideoSummary;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_parts;
    use crate::test_helpers::sample_movie;

    /// Send a GET to `/api/v1/library/movie/{movie}` on behalf of `caller_id`,
    /// injecting the caller identifier the way the auth middleware does.
    async fn send(
        use_case: MockGetMovieUseCase,
        caller_id: NumericID,
        movie_id: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_get_movie()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn GetMovieUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/library/movie/{movie_id}"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/library/movie/{movie_id}", get(get_movie))
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn get_movie_returns_the_film() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetMovieUseCase::new();
        let source = sample_movie(7)?;
        let response = GetMovieResponse::new(
            source.id(),
            source.name().to_owned(),
            source.plot().to_owned(),
            source.country_of_origin().to_owned(),
            source.release_date(),
            source.genre_ids().to_vec(),
            None,
            VideoSummary::from(source.video()),
        );
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| command.caller_id() == 1 && command.movie_id() == 7)
            .return_once(move |_| {
                let copy = response.clone();
                Box::pin(async move { Ok(copy) })
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
                    "self": { "href": "/api/v1/library/movie/7" }
                },
                "cover_file_id": null,
                "id": 7i64,
                "name": "Movie 7",
                "plot": "A plot",
                "country_of_origin": "US",
                "release_date": "1990-04-08",
                "genres": ["DRAMA"],
                "video": {
                    "id": 7i64,
                    "file_id": 7i64,
                    "duration_ms": 1_000.0f64,
                    "codec": "H264",
                    "width": 1920i64,
                    "height": 1080i64,
                    "scan_type": "PROGRESSIVE"
                }
            }),
            "the representation must carry the film and its video"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_movie_with_artwork_exposes_the_cover_link() -> Result<(), Box<dyn Error>> {
        // Arrange: the film's cover is file 19.
        let mut use_case = MockGetMovieUseCase::new();
        let source = sample_movie(7)?;
        let response = GetMovieResponse::new(
            source.id(),
            source.name().to_owned(),
            source.plot().to_owned(),
            source.country_of_origin().to_owned(),
            source.release_date(),
            source.genre_ids().to_vec(),
            Some(19),
            VideoSummary::from(source.video()),
        );
        use_case.expect_execute().times(1).return_once(move |_| {
            let copy = response.clone();
            Box::pin(async move { Ok(copy) })
        });

        // Act
        let (status, _, payload) = into_parts(send(use_case, 1, "7").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload.get("cover_file_id"), Some(&json!(19i64)));
        assert_eq!(
            payload.pointer("/_links/cover/href"),
            Some(&json!("/api/v1/asset/image/19")),
            "the representation must link the film's cover file"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_movie_malformed_identifier_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockGetMovieUseCase::new();

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
            Some("invalid film identifier"),
            "the rejection message must be stable"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_movie_unknown_film_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetMovieUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(GetMovieError::NoSuchMovie) }));

        // Act
        let response = send(use_case, 1, "7").await?;

        // Assert
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "an unknown film must be reported"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_movie_unknown_caller_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetMovieUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(GetMovieError::NoSuchCaller) }));

        // Act
        let response = send(use_case, 999, "7").await?;

        // Assert
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "a deleted caller must be rejected"
        );
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the authenticated user does not exist"),
            "the error message must be stable"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_movie_missing_credentials_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange: the real router, wrapped in its `authenticate` layer, with no
        // authorization header.
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("GET")
            .uri("/movie/7")
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
