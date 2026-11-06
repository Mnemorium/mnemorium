use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::field_update::FieldUpdate;
use crate::application::port::update_series_season::UpdateSeriesSeasonCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::SeasonResponse;
use crate::infrastructure::inbound::rest::handler::library::deserialize_field_update;
use crate::infrastructure::inbound::rest::handler::library::parse_season_index;
use crate::infrastructure::inbound::rest::handler::library::parse_series_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to update a show season partially.
///
/// An absent field keeps its value; a `null` clears the name or cover. The
/// season's episodes are never part of the payload.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PatchSeriesSeasonRequest {
    /// Identifier of the image holding the season's artwork; `null` clears it.
    #[serde(default, deserialize_with = "deserialize_field_update")]
    #[schema(value_type = Option<NumericID>, nullable = true)]
    pub cover_image_id: FieldUpdate<NumericID>,
    /// Name of the season; `null` clears it.
    #[serde(default, deserialize_with = "deserialize_field_update")]
    #[schema(value_type = Option<String>, nullable = true)]
    pub name: FieldUpdate<String>,
}

/// Update a show season partially.
///
/// Only a caller whose `library.manage` scope covers the show may update it.
#[utoipa::path(
    patch,
    operation_id = "patch_series_season",
    path = "/series/{series_id}/season/{season_index}",
    tag = "library",
    params(
        ("series_id" = NumericID, Path, description = "Identifier of the show holding the season"),
        ("season_index" = u32, Path, description = "Series-scoped index of the season"),
    ),
    request_body = PatchSeriesSeasonRequest,
    responses(
        (
            status = OK,
            body = SeasonResponse,
            content_type = "application/hal+json",
            description = "Season updated"
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
            description = "Unknown or invisible show, or a season the show does not hold"
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
    summary = "Update a show season"
)]
pub(crate) async fn patch_series_season(
    Path((series_id_value, season_index_value)): Path<(String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PatchSeriesSeasonRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let series_id = parse_series_id(&series_id_value)?;
    let season_index = parse_season_index(&season_index_value)?;
    let Json(request) = payload.map_err(ApiError::from)?;
    let response = state
        .library_use_case_factory()
        .update_series_season()
        .execute(UpdateSeriesSeasonCommand::new(
            caller.user_id(),
            series_id,
            season_index,
            request.name,
            request.cover_image_id,
        ))
        .await?;
    let season = response.season();
    Ok(hal_json(SeasonResponse::new(
        series_id,
        season.index().value(),
        season.name().map(str::to_owned),
        season.episodes().len(),
        season.cover_image_id(),
    )))
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
    use axum::routing::patch;
    use rstest::rstest;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::patch_series_season;
    use crate::application::field_update::FieldUpdate;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::update_series_season::MockUpdateSeriesSeasonUseCase;
    use crate::application::port::update_series_season::UpdateSeriesSeasonError;
    use crate::application::port::update_series_season::UpdateSeriesSeasonResponse;
    use crate::application::port::update_series_season::UpdateSeriesSeasonUseCase;
    use crate::domain::alias::NumericID;
    use crate::domain::model::season::Season;
    use crate::domain::model::season::SeasonIndex;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_parts;

    /// Send a PATCH to the season route on behalf of `caller_id`.
    async fn send(
        use_case: MockUpdateSeriesSeasonUseCase,
        caller_id: NumericID,
        series_id: &str,
        season_index: &str,
        body: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_update_series_season()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn UpdateSeriesSeasonUseCase>);

        let mut request = Request::builder()
            .method("PATCH")
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
                patch(patch_series_season),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    /// The stored season the mock returns.
    fn written() -> Season {
        Season::new(
            SeasonIndex::new(1),
            Some("New Name".to_owned()),
            None,
            Vec::new(),
        )
    }

    #[tokio::test]
    async fn patch_series_season_updates_the_season() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockUpdateSeriesSeasonUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                command.caller_id() == 1
                    && command.series_id() == 7
                    && command.season_index() == 1
                    && command.name() == FieldUpdate::Set("New Name".to_owned())
            })
            .return_once(|_| {
                let response = UpdateSeriesSeasonResponse::new(written());
                Box::pin(async move { Ok(response) })
            });

        // Act
        let (status, content_type, payload) =
            into_parts(send(use_case, 1, "7", "1", r#"{"name":"New Name"}"#).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(payload.get("series_id"), Some(&json!(7i64)));
        assert_eq!(payload.get("index"), Some(&json!(1u32)));
        assert_eq!(payload.get("name"), Some(&json!("New Name")));
        assert_eq!(
            payload.pointer("/_links/self/href"),
            Some(&json!("/api/v1/library/series/7/season/1"))
        );
        Ok(())
    }

    #[tokio::test]
    async fn patch_series_season_null_clears_the_name_and_cover() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockUpdateSeriesSeasonUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                command.name() == FieldUpdate::Clear
                    && command.cover_image_id() == FieldUpdate::Clear
            })
            .return_once(|_| {
                let response = UpdateSeriesSeasonResponse::new(Season::new(
                    SeasonIndex::new(1),
                    None,
                    None,
                    Vec::new(),
                ));
                Box::pin(async move { Ok(response) })
            });

        // Act
        let response = send(
            use_case,
            1,
            "7",
            "1",
            r#"{"name":null,"cover_image_id":null}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }

    #[rstest]
    #[case::conflict(
        UpdateSeriesSeasonError::Conflict,
        StatusCode::CONFLICT,
        "the season cannot be updated"
    )]
    #[case::forbidden(
        UpdateSeriesSeasonError::Forbidden,
        StatusCode::FORBIDDEN,
        "the caller may not manage the library"
    )]
    #[case::unknown_series(
        UpdateSeriesSeasonError::NoSuchSeries,
        StatusCode::NOT_FOUND,
        "a show with this identifier does not exist"
    )]
    #[case::unknown_season(
        UpdateSeriesSeasonError::NoSuchSeason,
        StatusCode::NOT_FOUND,
        "a season with this index does not exist in the show"
    )]
    #[case::unknown_caller(
        UpdateSeriesSeasonError::NoSuchCaller,
        StatusCode::UNAUTHORIZED,
        "the authenticated user does not exist"
    )]
    #[tokio::test]
    async fn patch_series_season_error_maps_to_status(
        #[case] error: UpdateSeriesSeasonError,
        #[case] status: StatusCode,
        #[case] message: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockUpdateSeriesSeasonUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));

        // Act
        let response = send(use_case, 1, "7", "1", r#"{"name":"New Name"}"#).await?;

        // Assert
        assert_eq!(response.status(), status);
        assert_eq!(error_message_of(&response).as_deref(), Some(message));
        Ok(())
    }

    #[tokio::test]
    async fn patch_series_season_dependency_failure_is_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockUpdateSeriesSeasonUseCase::new();
        use_case.expect_execute().times(1).return_once(|_| {
            Box::pin(async { Err(UpdateSeriesSeasonError::Unknown(anyhow::anyhow!("boom"))) })
        });

        // Act
        let response = send(use_case, 1, "7", "1", r#"{"name":"New Name"}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        Ok(())
    }

    #[tokio::test]
    async fn patch_series_season_malformed_identifier_is_bad_request() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = MockUpdateSeriesSeasonUseCase::new();

        // Act
        let response = send(use_case, 1, "abc", "1", r#"{"name":"New Name"}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }

    #[rstest]
    #[case::non_numeric("abc")]
    #[case::out_of_range("4294967296")]
    #[tokio::test]
    async fn patch_series_season_malformed_season_index_is_bad_request(
        #[case] season_index: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockUpdateSeriesSeasonUseCase::new();

        // Act
        let response = send(use_case, 1, "7", season_index, r#"{"name":"New Name"}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }

    #[tokio::test]
    async fn patch_series_season_malformed_body_is_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockUpdateSeriesSeasonUseCase::new();

        // Act
        let response = send(use_case, 1, "7", "1", "not json").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }

    #[tokio::test]
    async fn patch_series_season_missing_credentials_returns_unauthorized()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("PATCH")
            .uri("/series/7/season/1")
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
