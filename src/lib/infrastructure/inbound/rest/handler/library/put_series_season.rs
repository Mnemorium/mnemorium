use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::port::put_series_season::PutSeriesSeasonCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::SeasonResponse;
use crate::infrastructure::inbound::rest::handler::library::parse_season_index;
use crate::infrastructure::inbound::rest::handler::library::parse_series_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to create or replace a show season.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PutSeriesSeasonRequest {
    /// Identifier of the image holding the season's artwork, when set.
    #[schema(example = 1)]
    pub cover_image_id: Option<NumericID>,
    /// Name of the season, when set.
    pub name: Option<String>,
}

/// Create or replace a show season.
///
/// Only a caller whose `library.manage` scope covers the whole catalogue may
/// write it. The write is idempotent by index: a free index creates the season
/// and an existing index replaces its metadata, keeping its episodes.
#[utoipa::path(
    put,
    operation_id = "put_series_season",
    path = "/series/{series_id}/season/{season_index}",
    tag = "library",
    params(
        ("series_id" = NumericID, Path, description = "Identifier of the show holding the season"),
        ("season_index" = u32, Path, description = "Series-scoped index of the season"),
    ),
    request_body = PutSeriesSeasonRequest,
    responses(
        (
            status = CREATED,
            body = SeasonResponse,
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "URI of the newly created season"),
            ),
            description = "Season created",
        ),
        (
            status = OK,
            body = SeasonResponse,
            content_type = "application/hal+json",
            description = "Season replaced"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid show identifier or season index"
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
            description = "Unknown or invisible show"
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
            description = "The cover is not a library image"
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
    summary = "Create or replace a show season"
)]
pub(crate) async fn put_series_season(
    Path((series_id_value, season_index_value)): Path<(String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PutSeriesSeasonRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let series_id = parse_series_id(&series_id_value)?;
    let season_index = parse_season_index(&season_index_value)?;
    let Json(request) = payload.map_err(ApiError::from)?;
    let response = state
        .library_use_case_factory()
        .put_series_season()
        .execute(PutSeriesSeasonCommand::new(
            caller.user_id(),
            series_id,
            season_index,
            request.name,
            request.cover_image_id,
        ))
        .await?;
    let season = response.season();
    let body = SeasonResponse::new(
        series_id,
        season.index().value(),
        season.name().map(str::to_owned),
        season.episodes().len(),
        season.cover_image_id(),
    );
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
    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::response::Response;
    use axum::routing::put;
    use rstest::rstest;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::put_series_season;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::put_series_season::MockPutSeriesSeasonUseCase;
    use crate::application::port::put_series_season::PutSeriesSeasonError;
    use crate::application::port::put_series_season::PutSeriesSeasonResponse;
    use crate::application::port::put_series_season::PutSeriesSeasonUseCase;
    use crate::domain::alias::NumericID;
    use crate::domain::model::season::Season;
    use crate::domain::model::season::SeasonIndex;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_parts;

    /// Send a PUT to the season route on behalf of `caller_id`.
    async fn send(
        use_case: MockPutSeriesSeasonUseCase,
        caller_id: NumericID,
        series_id: &str,
        season_index: &str,
        body: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_put_series_season()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn PutSeriesSeasonUseCase>);

        let mut request = Request::builder()
            .method("PUT")
            .uri(format!(
                "/api/v1/library/series/{series_id}/season/{season_index}"
            ))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned()))?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/series/{series_id}/season/{season_index}",
                put(put_series_season),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    /// The stored season the mock returns.
    fn written() -> Season {
        Season::new(
            SeasonIndex::new(1),
            Some("Season One".to_owned()),
            Some(9),
            Vec::new(),
        )
    }

    #[tokio::test]
    async fn put_series_season_creates_the_season() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockPutSeriesSeasonUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                command.caller_id() == 1
                    && command.series_id() == 7
                    && command.season_index() == 1
                    && command.name() == Some("Season One")
                    && command.cover_image_id() == Some(9)
            })
            .return_once(|_| {
                let response = PutSeriesSeasonResponse::new(written(), true);
                Box::pin(async move { Ok(response) })
            });

        // Act
        let (status, content_type, payload) = into_parts(
            send(
                use_case,
                1,
                "7",
                "1",
                r#"{"name":"Season One","cover_image_id":9}"#,
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(payload.get("series_id"), Some(&json!(7i64)));
        assert_eq!(payload.get("index"), Some(&json!(1u32)));
        assert_eq!(payload.get("name"), Some(&json!("Season One")));
        assert_eq!(payload.get("episode_count"), Some(&json!(0usize)));
        assert_eq!(
            payload.pointer("/_links/self/href"),
            Some(&json!("/api/v1/library/series/7/season/1"))
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_series_season_returns_the_location_of_the_new_season() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut use_case = MockPutSeriesSeasonUseCase::new();
        use_case.expect_execute().times(1).return_once(|_| {
            let response = PutSeriesSeasonResponse::new(written(), true);
            Box::pin(async move { Ok(response) })
        });

        // Act
        let response = send(use_case, 1, "7", "1", r#"{"name":"Season One"}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(
            response
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some("/api/v1/library/series/7/season/1")
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_series_season_replaces_the_season() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockPutSeriesSeasonUseCase::new();
        use_case.expect_execute().times(1).return_once(|_| {
            let response = PutSeriesSeasonResponse::new(written(), false);
            Box::pin(async move { Ok(response) })
        });

        // Act
        let response = send(use_case, 1, "7", "1", r#"{"name":"Season One"}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().get(header::LOCATION).is_none());
        Ok(())
    }

    #[rstest]
    #[case::conflict(
        PutSeriesSeasonError::Conflict,
        StatusCode::CONFLICT,
        "the season cannot be written"
    )]
    #[case::forbidden(
        PutSeriesSeasonError::Forbidden,
        StatusCode::FORBIDDEN,
        "the caller may not manage the library"
    )]
    #[case::unknown_series(
        PutSeriesSeasonError::NoSuchSeries,
        StatusCode::NOT_FOUND,
        "a show with this identifier does not exist"
    )]
    #[case::unknown_caller(
        PutSeriesSeasonError::NoSuchCaller,
        StatusCode::UNAUTHORIZED,
        "the authenticated user does not exist"
    )]
    #[tokio::test]
    async fn put_series_season_error_maps_to_status(
        #[case] error: PutSeriesSeasonError,
        #[case] status: StatusCode,
        #[case] message: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockPutSeriesSeasonUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));

        // Act
        let response = send(use_case, 1, "7", "1", r#"{"name":"Season One"}"#).await?;

        // Assert
        assert_eq!(response.status(), status);
        assert_eq!(error_message_of(&response).as_deref(), Some(message));
        Ok(())
    }

    #[tokio::test]
    async fn put_series_season_dependency_failure_is_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockPutSeriesSeasonUseCase::new();
        use_case.expect_execute().times(1).return_once(|_| {
            Box::pin(async { Err(PutSeriesSeasonError::Unknown(anyhow::anyhow!("boom"))) })
        });

        // Act
        let response = send(use_case, 1, "7", "1", r#"{"name":"Season One"}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        Ok(())
    }

    #[tokio::test]
    async fn put_series_season_malformed_series_identifier_is_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockPutSeriesSeasonUseCase::new();

        // Act
        let response = send(use_case, 1, "abc", "1", r#"{"name":"Season One"}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }

    #[rstest]
    #[case::non_numeric("abc")]
    #[case::out_of_range("4294967296")]
    #[tokio::test]
    async fn put_series_season_malformed_season_index_is_bad_request(
        #[case] season_index: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockPutSeriesSeasonUseCase::new();

        // Act
        let response = send(use_case, 1, "7", season_index, r#"{"name":"Season One"}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }

    #[tokio::test]
    async fn put_series_season_malformed_body_is_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockPutSeriesSeasonUseCase::new();

        // Act
        let response = send(use_case, 1, "7", "1", "not json").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }

    #[tokio::test]
    async fn put_series_season_missing_credentials_returns_unauthorized()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("PUT")
            .uri("/series/7/season/1")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"name":"Season One"}"#))?;

        // Act
        let response: Response = axum::Router::from(library_routes(&state))
            .oneshot(request)
            .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
