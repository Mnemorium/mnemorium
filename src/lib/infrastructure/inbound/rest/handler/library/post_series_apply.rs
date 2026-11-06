use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::response::Response;

use crate::application::port::apply_series::ApplySeriesCommand;
use crate::domain::alias::NumericID;
use crate::domain::model::configuration::ProviderId;
use crate::domain::model::external_ref::ExternalRef;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::{
    SeriesResponse, VideoApplyRequest, parse_series_id,
};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Apply external metadata to a show.
///
/// Only a caller whose `library.manage` scope covers the show may write it. The
/// provider reference is resolved once and its name, plot, first-air date,
/// country of origin and genres replace the stored values; a value the provider
/// does not report is cleared. The cover, seasons and episodes are untouched,
/// and the apply never creates a season or an episode.
#[utoipa::path(
    post,
    operation_id = "apply_series_metadata",
    path = "/series/{series_id}/apply",
    tag = "library",
    params(
        ("series_id" = NumericID, Path, description = "Identifier of the show"),
    ),
    request_body = VideoApplyRequest,
    responses(
        (
            status = OK,
            body = SeriesResponse,
            content_type = "application/hal+json",
            description = "Metadata applied"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid show identifier or malformed body"
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
            description = "Unknown or invisible show, or the provider does not know the reference"
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
    summary = "Apply external metadata to a show"
)]
pub(crate) async fn post_series_apply(
    Path(series_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<VideoApplyRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let series_id = parse_series_id(&series_id_value)?;
    let Json(request) = payload.map_err(ApiError::from)?;
    let provider = ProviderId::parse(&request.reference.provider)
        .filter(|provider| provider.serves_video())
        .ok_or_else(|| ApiError::reject_enum("provider"))?;
    let reference = ExternalRef::new(provider, request.reference.id);
    let response = state
        .library_use_case_factory()
        .apply_series()
        .execute(ApplySeriesCommand::new(
            caller.user_id(),
            series_id,
            reference,
        ))
        .await?;
    Ok(hal_json(SeriesResponse::from(response.series())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::port::apply_series::{
        ApplySeriesError, ApplySeriesResponse, ApplySeriesUseCase, MockApplySeriesUseCase,
    };
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::test_helpers::*;
    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::routing::post;
    use serde_json::json;
    use std::error::Error;
    use std::sync::Arc;
    use tower::ServiceExt as _;

    /// Send a POST to the show apply route on behalf of `caller_id`.
    async fn send(
        use_case: MockApplySeriesUseCase,
        caller_id: NumericID,
        series_id: &str,
        body: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_apply_series()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn ApplySeriesUseCase>);

        let mut request = Request::builder()
            .method("POST")
            .uri(format!("/api/v1/library/series/{series_id}/apply"))
            .header("Content-Type", "application/json")
            .body(Body::from(body.to_owned()))?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/series/{series_id}/apply",
                post(post_series_apply),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn post_series_apply_returns_the_stored_show() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockApplySeriesUseCase::new();
        let series = sample_series(7, Vec::new())?;
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| command.caller_id() == 1 && command.series_id() == 7)
            .return_once(move |_| Box::pin(async move { Ok(ApplySeriesResponse::new(series)) }));

        // Act
        let (status, content_type, payload) = into_parts(
            send(
                use_case,
                1,
                "7",
                r#"{"reference":{"provider":"tmdb","id":"1920"}}"#,
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
    async fn post_series_apply_unknown_show_is_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockApplySeriesUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(ApplySeriesError::NoSuchSeries) }));

        // Act
        let response = send(
            use_case,
            1,
            "7",
            r#"{"reference":{"provider":"tmdb","id":"1920"}}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn post_series_apply_forbidden_is_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockApplySeriesUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(ApplySeriesError::Forbidden) }));

        // Act
        let response = send(
            use_case,
            1,
            "7",
            r#"{"reference":{"provider":"tmdb","id":"1920"}}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        Ok(())
    }

    #[tokio::test]
    async fn post_series_apply_invalid_reference_is_unprocessable() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockApplySeriesUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(ApplySeriesError::InvalidReference) }));

        // Act
        let response = send(
            use_case,
            1,
            "7",
            r#"{"reference":{"provider":"tmdb","id":"1920"}}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[tokio::test]
    async fn post_series_apply_malformed_identifier_is_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockApplySeriesUseCase::new();

        // Act
        let response = send(
            use_case,
            1,
            "not-a-number",
            r#"{"reference":{"provider":"tmdb","id":"1920"}}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }

    #[tokio::test]
    async fn post_series_apply_missing_credentials_returns_unauthorized()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the real router, wrapped in its `authenticate` layer.
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("POST")
            .uri("/series/7/apply")
            .header("Content-Type", "application/json")
            .body(Body::from(
                r#"{"reference":{"provider":"tmdb","id":"1920"}}"#,
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
    async fn post_series_apply_conflict_is_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockApplySeriesUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(ApplySeriesError::Conflict) }));

        // Act
        let response = send(
            use_case,
            1,
            "7",
            r#"{"reference":{"provider":"tmdb","id":"1920"}}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::CONFLICT);
        Ok(())
    }

    #[tokio::test]
    async fn post_series_apply_dependency_failure_is_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockApplySeriesUseCase::new();
        use_case.expect_execute().times(1).return_once(|_| {
            Box::pin(async { Err(ApplySeriesError::Unknown(anyhow::anyhow!("boom"))) })
        });

        // Act
        let response = send(
            use_case,
            1,
            "7",
            r#"{"reference":{"provider":"tmdb","id":"1920"}}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        Ok(())
    }
}
