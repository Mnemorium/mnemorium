use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::Response;
use chrono::NaiveDate;
use serde::Deserialize;
use tracing::warn;
use utoipa::ToSchema;

use crate::application::port::create_series::CreateSeriesCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::DATE_FORMAT;
use crate::infrastructure::inbound::rest::handler::library::SeriesResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to create a show.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PostSeriesRequest {
    /// Country of origin as an ISO 3166-1 alpha-2 code, when known.
    #[schema(example = "US")]
    pub country_of_origin: Option<String>,
    /// Identifier of the image holding the show's artwork, when set.
    #[schema(example = 1)]
    pub cover_image_id: Option<NumericID>,
    /// Date of the first airing, in `YYYY-MM-DD` form, when known.
    #[schema(example = "1990-04-08")]
    pub first_air_date: Option<String>,
    /// Genres of the show.
    pub genres: Option<Vec<String>>,
    /// Name of the show.
    #[schema(min_length = 1, max_length = 100)]
    pub name: String,
    /// Plot of the show, when known.
    pub plot: Option<String>,
}

/// Create a show.
///
/// Only a caller whose `library.manage` scope covers the whole catalogue may
/// create a show. Creation is insert-only: the schema holds no unique show
/// name, so a repeated name creates another show.
#[utoipa::path(
    post,
    operation_id = "post_series",
    path = "/series",
    tag = "library",
    request_body = PostSeriesRequest,
    responses(
        (
            status = CREATED,
            body = SeriesResponse,
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "URI of the newly created show"),
            ),
            description = "Show created",
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
            description = "The name, country, genre or first-air date is invalid"
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
    summary = "Create a show"
)]
pub(crate) async fn post_series(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PostSeriesRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(request) = payload.map_err(ApiError::from)?;
    let first_air_date = match request.first_air_date.as_deref() {
        Some(value) => Some(NaiveDate::parse_from_str(value, DATE_FORMAT).map_err(|_| {
            warn!(
                target: "security",
                event = "input_validation_failed",
                field = "first_air_date",
                reason = "invalid_date",
                "rejected a malformed first-air date"
            );
            ApiError::UnprocessableEntity("the first air date must use YYYY-MM-DD".to_owned())
        })?),
        None => None,
    };
    let response = state
        .library_use_case_factory()
        .create_series()
        .execute(CreateSeriesCommand::new(
            caller.user_id(),
            request.name,
            request.plot,
            request.country_of_origin,
            first_air_date,
            request.genres.unwrap_or_default(),
            request.cover_image_id,
        ))
        .await?;
    let body = SeriesResponse::from(response.series());
    let location = body.links.self_link.href.clone();
    let mut http_response = hal_json(body);
    *http_response.status_mut() = StatusCode::CREATED;
    http_response.headers_mut().insert(
        header::LOCATION,
        HeaderValue::from_str(&location).map_err(|_| ApiError::InternalServerError)?,
    );
    Ok(http_response)
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
    use axum::routing::post;
    use rstest::rstest;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::post_series;
    use crate::application::port::create_series::CreateSeriesError;
    use crate::application::port::create_series::CreateSeriesResponse;
    use crate::application::port::create_series::CreateSeriesUseCase;
    use crate::application::port::create_series::MockCreateSeriesUseCase;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_parts;
    use crate::test_helpers::sample_series;

    /// Send a POST to `/api/v1/library/series` on behalf of `caller_id`.
    async fn send(
        use_case: MockCreateSeriesUseCase,
        caller_id: NumericID,
        body: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_create_series()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn CreateSeriesUseCase>);

        let mut request = Request::builder()
            .method("POST")
            .uri("/api/v1/library/series")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned()))?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/library/series", post(post_series))
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn post_series_creates_the_show() -> Result<(), Box<dyn Error>> {
        // Arrange
        let series = sample_series(7, Vec::new())?;
        let mut use_case = MockCreateSeriesUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                command.caller_id() == 1
                    && command.name() == "Twin Peaks"
                    && command.genres() == ["MYSTERY".to_owned()]
            })
            .return_once(move |_| {
                let stored = CreateSeriesResponse::new(series.clone());
                Box::pin(async move { Ok(stored) })
            });

        // Act
        let (status, content_type, payload) =
            into_parts(send(use_case, 1, r#"{"name":"Twin Peaks","genres":["MYSTERY"]}"#).await?)
                .await?;

        // Assert
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(payload.get("id"), Some(&json!(7i64)));
        assert_eq!(
            payload.pointer("/_links/self/href"),
            Some(&json!("/api/v1/library/series/7"))
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_series_returns_the_location_of_the_new_show() -> Result<(), Box<dyn Error>> {
        // Arrange
        let series = sample_series(7, Vec::new())?;
        let mut use_case = MockCreateSeriesUseCase::new();
        use_case.expect_execute().times(1).return_once(move |_| {
            let stored = CreateSeriesResponse::new(series.clone());
            Box::pin(async move { Ok(stored) })
        });

        // Act
        let response = send(use_case, 1, r#"{"name":"Twin Peaks"}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(
            response
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some("/api/v1/library/series/7")
        );
        Ok(())
    }

    #[rstest]
    #[case::conflict(
        CreateSeriesError::Conflict,
        StatusCode::CONFLICT,
        "the show cannot be created"
    )]
    #[case::forbidden(
        CreateSeriesError::Forbidden,
        StatusCode::FORBIDDEN,
        "the caller may not manage the library"
    )]
    #[case::invalid_name(
        CreateSeriesError::InvalidName,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the name must not be empty"
    )]
    #[case::name_too_long(
        CreateSeriesError::NameTooLong,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the name is too long"
    )]
    #[case::invalid_country(
        CreateSeriesError::InvalidCountry,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the country of origin must be an ISO 3166-1 alpha-2 code"
    )]
    #[case::invalid_genre(
        CreateSeriesError::InvalidGenre,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a genre identifier must not be empty"
    )]
    #[case::unknown_caller(
        CreateSeriesError::NoSuchCaller,
        StatusCode::UNAUTHORIZED,
        "the authenticated user does not exist"
    )]
    #[tokio::test]
    async fn post_series_error_maps_to_status(
        #[case] error: CreateSeriesError,
        #[case] status: StatusCode,
        #[case] message: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCreateSeriesUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));

        // Act
        let response = send(use_case, 1, r#"{"name":"Twin Peaks"}"#).await?;

        // Assert
        assert_eq!(response.status(), status);
        assert_eq!(error_message_of(&response).as_deref(), Some(message));
        Ok(())
    }

    #[tokio::test]
    async fn post_series_dependency_failure_is_internal_server_error() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut use_case = MockCreateSeriesUseCase::new();
        use_case.expect_execute().times(1).return_once(|_| {
            Box::pin(async { Err(CreateSeriesError::Unknown(anyhow::anyhow!("boom"))) })
        });

        // Act
        let response = send(use_case, 1, r#"{"name":"Twin Peaks"}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        Ok(())
    }

    #[tokio::test]
    async fn post_series_invalid_first_air_date_is_unprocessable() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockCreateSeriesUseCase::new();

        // Act
        let response = send(
            use_case,
            1,
            r#"{"name":"Twin Peaks","first_air_date":"1990-13-01"}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[tokio::test]
    async fn post_series_malformed_body_is_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockCreateSeriesUseCase::new();

        // Act
        let response = send(use_case, 1, "not json").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }

    #[tokio::test]
    async fn post_series_missing_credentials_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("POST")
            .uri("/series")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"name":"Twin Peaks"}"#))?;

        // Act
        let response: Response = axum::Router::from(library_routes(&state))
            .oneshot(request)
            .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
