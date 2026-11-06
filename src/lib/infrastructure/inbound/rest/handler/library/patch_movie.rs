use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::response::Response;
use chrono::NaiveDate;
use serde::Deserialize;
use tracing::warn;
use utoipa::ToSchema;

use crate::application::field_update::FieldUpdate;
use crate::application::port::update_movie::UpdateMovieCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::{
    DATE_FORMAT, MovieResponse, deserialize_field_update,
};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to update a film partially.
///
/// An absent field keeps its value; a `null` clears an optional value. The
/// backing video is never part of the payload.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PatchMovieRequest {
    /// Country of origin as an ISO 3166-1 alpha-2 code; `null` clears it.
    #[serde(default, deserialize_with = "deserialize_field_update")]
    #[schema(value_type = Option<String>, nullable = true)]
    pub country_of_origin: FieldUpdate<String>,
    /// Identifier of the image holding the film's artwork; `null` clears it.
    #[serde(default, deserialize_with = "deserialize_field_update")]
    #[schema(value_type = Option<NumericID>, nullable = true)]
    pub cover_image_id: FieldUpdate<NumericID>,
    /// Genres of the film; omit to keep them.
    pub genres: Option<Vec<String>>,
    /// Name of the film.
    #[schema(min_length = 1, max_length = 100)]
    pub name: Option<String>,
    /// Plot of the film; omit to keep it.
    pub plot: Option<String>,
    /// Date of the release in `YYYY-MM-DD` form; `null` clears it.
    #[serde(default, deserialize_with = "deserialize_field_update")]
    #[schema(value_type = Option<String>, nullable = true)]
    pub release_date: FieldUpdate<String>,
}

/// Update a film partially.
///
/// Only a caller whose `library.manage` scope covers the film may update it. An
/// absent field keeps its value and a `null` clears an optional value.
#[utoipa::path(
    patch,
    operation_id = "patch_movie",
    path = "/movie/{movie_id}",
    tag = "library",
    params(
        ("movie_id" = NumericID, Path, description = "Identifier of the film to update"),
    ),
    request_body = PatchMovieRequest,
    responses(
        (
            status = OK,
            body = MovieResponse,
            content_type = "application/hal+json",
            description = "Film updated"
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
            description = "The caller may not manage the film"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown or invisible film"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The cover is not a library image"
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
            description = "The name, country, genre or release date is invalid"
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
    summary = "Update a film"
)]
pub(crate) async fn patch_movie(
    Path(movie_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PatchMovieRequest>, JsonRejection>,
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
    let release_date = request.release_date.try_map(|value| {
        NaiveDate::parse_from_str(&value, DATE_FORMAT).map_err(|_| {
            warn!(
                target: "security",
                event = "input_validation_failed",
                field = "release_date",
                reason = "invalid_date",
                "rejected a malformed release date"
            );
            ApiError::UnprocessableEntity("the release date must use YYYY-MM-DD".to_owned())
        })
    })?;
    let response = state
        .library_use_case_factory()
        .update_movie()
        .execute(UpdateMovieCommand::new(
            caller.user_id(),
            movie_id,
            request.name,
            request.plot,
            request.country_of_origin,
            release_date,
            request.genres,
            request.cover_image_id,
        ))
        .await?;
    Ok(hal_json(MovieResponse::from(response.movie())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::update_movie::{
        MockUpdateMovieUseCase, UpdateMovieError, UpdateMovieResponse, UpdateMovieUseCase,
    };
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::test_helpers::*;
    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::{StatusCode, header};
    use axum::routing::patch;
    use rstest::rstest;
    use serde_json::json;
    use std::error::Error;
    use std::sync::Arc;
    use tower::ServiceExt as _;

    /// Send a PATCH to `/api/v1/library/movie/{movie}` on behalf of `caller_id`.
    async fn send(
        use_case: MockUpdateMovieUseCase,
        caller_id: NumericID,
        movie_id: &str,
        body: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_update_movie()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn UpdateMovieUseCase>);

        let mut request = Request::builder()
            .method("PATCH")
            .uri(format!("/api/v1/library/movie/{movie_id}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned()))?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/library/movie/{movie_id}", patch(patch_movie))
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn patch_movie_updates_the_film() -> Result<(), Box<dyn Error>> {
        // Arrange
        let movie = sample_movie(7)?;
        let mut use_case = MockUpdateMovieUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                command.caller_id() == 1
                    && command.movie_id() == 7
                    && command.name() == Some("New Name")
            })
            .return_once(move |_| {
                let stored = UpdateMovieResponse::new(movie.clone());
                Box::pin(async move { Ok(stored) })
            });

        // Act
        let (status, content_type, payload) =
            into_parts(send(use_case, 1, "7", r#"{"name":"New Name"}"#).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(payload.get("id"), Some(&json!(7i64)));
        Ok(())
    }

    #[tokio::test]
    async fn patch_movie_null_clears_the_release_date() -> Result<(), Box<dyn Error>> {
        // Arrange
        let movie = sample_movie(7)?;
        let mut use_case = MockUpdateMovieUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| command.release_date() == FieldUpdate::Clear)
            .return_once(move |_| {
                let stored = UpdateMovieResponse::new(movie.clone());
                Box::pin(async move { Ok(stored) })
            });

        // Act
        let response = send(use_case, 1, "7", r#"{"release_date":null}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }

    #[rstest]
    #[case::conflict(
        UpdateMovieError::Conflict,
        StatusCode::CONFLICT,
        "the film cannot be updated"
    )]
    #[case::forbidden(
        UpdateMovieError::Forbidden,
        StatusCode::FORBIDDEN,
        "the caller may not manage the library"
    )]
    #[case::unknown_movie(
        UpdateMovieError::NoSuchMovie,
        StatusCode::NOT_FOUND,
        "a film with this identifier does not exist"
    )]
    #[case::invalid_name(
        UpdateMovieError::InvalidName,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the name must not be empty"
    )]
    #[case::name_too_long(
        UpdateMovieError::NameTooLong,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the name is too long"
    )]
    #[case::invalid_country(
        UpdateMovieError::InvalidCountry,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the country of origin must be an ISO 3166-1 alpha-2 code"
    )]
    #[case::invalid_genre(
        UpdateMovieError::InvalidGenre,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a genre identifier must not be empty"
    )]
    #[case::unknown_caller(
        UpdateMovieError::NoSuchCaller,
        StatusCode::UNAUTHORIZED,
        "the authenticated user does not exist"
    )]
    #[tokio::test]
    async fn patch_movie_error_maps_to_status(
        #[case] error: UpdateMovieError,
        #[case] status: StatusCode,
        #[case] message: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockUpdateMovieUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));

        // Act
        let response = send(use_case, 1, "7", r#"{"name":"New Name"}"#).await?;

        // Assert
        assert_eq!(response.status(), status);
        assert_eq!(error_message_of(&response).as_deref(), Some(message));
        Ok(())
    }

    #[tokio::test]
    async fn patch_movie_dependency_failure_is_internal_server_error() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut use_case = MockUpdateMovieUseCase::new();
        use_case.expect_execute().times(1).return_once(|_| {
            Box::pin(async { Err(UpdateMovieError::Unknown(anyhow::anyhow!("boom"))) })
        });

        // Act
        let response = send(use_case, 1, "7", r#"{"name":"New Name"}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        Ok(())
    }

    #[tokio::test]
    async fn patch_movie_malformed_identifier_is_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockUpdateMovieUseCase::new();

        // Act
        let response = send(use_case, 1, "abc", r#"{"name":"New Name"}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }

    #[tokio::test]
    async fn patch_movie_invalid_release_date_is_unprocessable() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockUpdateMovieUseCase::new();

        // Act
        let response = send(use_case, 1, "7", r#"{"release_date":"1959-13-01"}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[tokio::test]
    async fn patch_movie_malformed_body_is_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockUpdateMovieUseCase::new();

        // Act
        let response = send(use_case, 1, "7", "not json").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }

    #[tokio::test]
    async fn patch_movie_missing_credentials_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("PATCH")
            .uri("/movie/7")
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
