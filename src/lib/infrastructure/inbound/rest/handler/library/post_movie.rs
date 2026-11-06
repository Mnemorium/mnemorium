use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use chrono::NaiveDate;
use serde::Deserialize;
use tracing::warn;
use utoipa::ToSchema;

use crate::application::port::create_movie::CreateMovieCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::{DATE_FORMAT, MovieResponse};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to create a film from a video.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PostMovieRequest {
    /// Country of origin as an ISO 3166-1 alpha-2 code, when known.
    #[schema(example = "US")]
    pub country_of_origin: Option<String>,
    /// Identifier of the image holding the film's artwork, when set.
    #[schema(example = 1)]
    pub cover_image_id: Option<NumericID>,
    /// Genres of the film.
    pub genres: Option<Vec<String>>,
    /// Name of the film.
    #[schema(min_length = 1, max_length = 100)]
    pub name: String,
    /// Plot of the film, when known.
    pub plot: Option<String>,
    /// Date of the release, in `YYYY-MM-DD` form, when known.
    #[schema(example = "1959-08-17")]
    pub release_date: Option<String>,
    /// Identifier of the video backing the film.
    #[schema(example = 1)]
    pub video_id: NumericID,
}

/// Create a film from a video.
///
/// Only a caller whose `library.manage` scope covers the whole catalogue may
/// create a film. The write is an upsert by `video_id`: a film already backed
/// by the video is returned unchanged with `200 OK`.
#[utoipa::path(
    post,
    operation_id = "post_movie",
    path = "/movie",
    tag = "library",
    request_body = PostMovieRequest,
    responses(
        (
            status = CREATED,
            body = MovieResponse,
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "URI of the newly created film"),
            ),
            description = "Film created",
        ),
        (
            status = OK,
            body = MovieResponse,
            content_type = "application/hal+json",
            description = "Film already existed"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Malformed request body"
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
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The video is already attached, or the cover is not a library image"
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
            description = "The name, country, genre or release date is invalid, or the video does not exist"
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
    summary = "Create a film from a video"
)]
pub(crate) async fn post_movie(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PostMovieRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(request) = payload.map_err(ApiError::from)?;
    let release_date = match request.release_date.as_deref() {
        Some(value) => Some(NaiveDate::parse_from_str(value, DATE_FORMAT).map_err(|_| {
            warn!(
                target: "security",
                event = "input_validation_failed",
                field = "release_date",
                reason = "invalid_date",
                "rejected a malformed release date"
            );
            ApiError::UnprocessableEntity("the release date must use YYYY-MM-DD".to_owned())
        })?),
        None => None,
    };
    let response = state
        .library_use_case_factory()
        .create_movie()
        .execute(CreateMovieCommand::new(
            caller.user_id(),
            request.video_id,
            request.name,
            request.plot,
            request.country_of_origin,
            release_date,
            request.genres.unwrap_or_default(),
            request.cover_image_id,
        ))
        .await?;
    let body = MovieResponse::from(response.movie());
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
    use crate::application::port::create_movie::{
        CreateMovieError, CreateMovieResponse, CreateMovieUseCase, MockCreateMovieUseCase,
    };
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::test_helpers::*;
    use axum::body::Body;
    use axum::extract::Request;
    use axum::routing::post;
    use rstest::rstest;
    use serde_json::json;
    use std::error::Error;
    use std::sync::Arc;
    use tower::ServiceExt as _;

    /// Send a POST to `/api/v1/library/movie` on behalf of `caller_id`.
    async fn send(
        use_case: MockCreateMovieUseCase,
        caller_id: NumericID,
        body: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_create_movie()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn CreateMovieUseCase>);

        let mut request = Request::builder()
            .method("POST")
            .uri("/api/v1/library/movie")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned()))?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/library/movie", post(post_movie))
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn post_movie_creates_the_film() -> Result<(), Box<dyn Error>> {
        // Arrange
        let movie = sample_movie(7)?;
        let mut use_case = MockCreateMovieUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                command.caller_id() == 1
                    && command.video_id() == 5
                    && command.name() == "Some Like It Hot"
                    && command.plot() == Some("A plot")
                    && command.genres() == ["COMEDY".to_owned()]
            })
            .return_once(move |_| {
                let stored = CreateMovieResponse::new(movie.clone(), true);
                Box::pin(async move { Ok(stored) })
            });

        // Act
        let (status, content_type, payload) = into_parts(
            send(
                use_case,
                1,
                r#"{"video_id":5,"name":"Some Like It Hot","plot":"A plot","genres":["COMEDY"]}"#,
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(payload.get("id"), Some(&json!(7i64)));
        assert_eq!(
            payload.pointer("/_links/self/href"),
            Some(&json!("/api/v1/library/movie/7"))
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_movie_returns_the_location_of_the_new_film() -> Result<(), Box<dyn Error>> {
        // Arrange
        let movie = sample_movie(7)?;
        let mut use_case = MockCreateMovieUseCase::new();
        use_case.expect_execute().times(1).return_once(move |_| {
            let stored = CreateMovieResponse::new(movie.clone(), true);
            Box::pin(async move { Ok(stored) })
        });

        // Act
        let response = send(use_case, 1, r#"{"video_id":5,"name":"Some Like It Hot"}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(
            response
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some("/api/v1/library/movie/7")
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_movie_existing_film_returns_ok() -> Result<(), Box<dyn Error>> {
        // Arrange
        let movie = sample_movie(7)?;
        let mut use_case = MockCreateMovieUseCase::new();
        use_case.expect_execute().times(1).return_once(move |_| {
            let stored = CreateMovieResponse::new(movie.clone(), false);
            Box::pin(async move { Ok(stored) })
        });

        // Act
        let response = send(use_case, 1, r#"{"video_id":5,"name":"Some Like It Hot"}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().get(header::LOCATION).is_none());
        Ok(())
    }

    #[rstest]
    #[case::conflict(
        CreateMovieError::Conflict,
        StatusCode::CONFLICT,
        "the film cannot be created"
    )]
    #[case::forbidden(
        CreateMovieError::Forbidden,
        StatusCode::FORBIDDEN,
        "the caller may not manage the library"
    )]
    #[case::unknown_video(
        CreateMovieError::NoSuchVideo,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a video with this identifier does not exist"
    )]
    #[case::invalid_name(
        CreateMovieError::InvalidName,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the name must not be empty"
    )]
    #[case::name_too_long(
        CreateMovieError::NameTooLong,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the name is too long"
    )]
    #[case::invalid_country(
        CreateMovieError::InvalidCountry,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the country of origin must be an ISO 3166-1 alpha-2 code"
    )]
    #[case::invalid_genre(
        CreateMovieError::InvalidGenre,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a genre identifier must not be empty"
    )]
    #[case::unknown_caller(
        CreateMovieError::NoSuchCaller,
        StatusCode::UNAUTHORIZED,
        "the authenticated user does not exist"
    )]
    #[tokio::test]
    async fn post_movie_error_maps_to_status(
        #[case] error: CreateMovieError,
        #[case] status: StatusCode,
        #[case] message: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCreateMovieUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));

        // Act
        let response = send(use_case, 1, r#"{"video_id":5,"name":"Some Like It Hot"}"#).await?;

        // Assert
        assert_eq!(response.status(), status);
        assert_eq!(error_message_of(&response).as_deref(), Some(message));
        Ok(())
    }

    #[tokio::test]
    async fn post_movie_dependency_failure_is_internal_server_error() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut use_case = MockCreateMovieUseCase::new();
        use_case.expect_execute().times(1).return_once(|_| {
            Box::pin(async { Err(CreateMovieError::Unknown(anyhow::anyhow!("boom"))) })
        });

        // Act
        let response = send(use_case, 1, r#"{"video_id":5,"name":"Some Like It Hot"}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("an unexpected error occurred")
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_movie_invalid_release_date_is_unprocessable() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockCreateMovieUseCase::new();

        // Act
        let response = send(
            use_case,
            1,
            r#"{"video_id":5,"name":"Some Like It Hot","release_date":"1959-13-01"}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[tokio::test]
    async fn post_movie_malformed_body_is_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockCreateMovieUseCase::new();

        // Act
        let response = send(use_case, 1, "not json").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }

    #[tokio::test]
    async fn post_movie_missing_credentials_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange: the real router, wrapped in its `authenticate` layer, with no
        // authorization header.
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("POST")
            .uri("/movie")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"video_id":5,"name":"Some Like It Hot"}"#))?;

        // Act
        let response: Response = axum::Router::from(library_routes(&state))
            .oneshot(request)
            .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
