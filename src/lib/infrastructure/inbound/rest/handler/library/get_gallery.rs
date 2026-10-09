use axum::extract::Path;
use axum::extract::State;
use axum::response::Response;
use tracing::warn;

use crate::application::port::get_gallery::GetGalleryCommand;
use crate::application::port::get_gallery::GetGalleryError;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::representation::GalleryResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Map a fetch-gallery error to its API error.
impl From<GetGalleryError> for ApiError {
    fn from(err: GetGalleryError) -> Self {
        match err {
            GetGalleryError::Forbidden => Self::Forbidden(err.to_string()),
            GetGalleryError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetGalleryError::NoSuchGallery => Self::NotFound(err.to_string()),
            GetGalleryError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Fetch a gallery and its items by its identifier.
///
/// Any authenticated caller may read a public gallery; a private gallery is
/// readable by its owner and the Root Admin only. The representation carries
/// the gallery and its items, ordered by position.
#[utoipa::path(
    get,
    operation_id = "get_gallery",
    path = "/library/gallery/{id}",
    tag = "library",
    params(
        ("id" = NumericID, Path, description = "Identifier of the gallery to fetch"),
    ),
    responses(
        (
            status = OK,
            body = GalleryResponse,
            content_type = "application/hal+json",
            description = "Gallery found"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid gallery identifier"
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
            description = "Caller is not allowed to read the gallery"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown gallery"
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
    summary = "Fetch a gallery by identifier"
)]
pub async fn get_gallery(
    Path(id): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let gallery_id = parse_gallery_id(&id)?;
    let response = state
        .library_use_case_factory()
        .get_gallery()
        .execute(GetGalleryCommand::new(caller.user_id(), gallery_id))
        .await?;
    Ok(hal_json(GalleryResponse::from(response)))
}

/// Parse a gallery identifier from a path segment.
pub(crate) fn parse_gallery_id(value: &str) -> Result<NumericID, ApiError> {
    value.parse::<NumericID>().map_err(|_| {
        // The malformed value and the parser detail never reach the log
        // (`OBS-003`); only the classification and its declared fields do
        // (`OBS-006`).
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "gallery_id",
            reason = "invalid_identifier",
            "rejected a malformed gallery identifier"
        );
        ApiError::BadRequest("invalid gallery identifier".to_owned())
    })
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::body::to_bytes;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::response::Response;
    use axum::routing::get;
    use chrono::NaiveDate;
    use chrono::NaiveDateTime;
    use mockall::predicate::eq;
    use serde_json::Value;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::get_gallery;
    use crate::application::port::get_gallery::GalleryItemSummary;
    use crate::application::port::get_gallery::GetGalleryCommand;
    use crate::application::port::get_gallery::GetGalleryError;
    use crate::application::port::get_gallery::GetGalleryResponse;
    use crate::application::port::get_gallery::GetGalleryUseCase;
    use crate::application::port::get_gallery::MockGetGalleryUseCase;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::domain::alias::NumericID;
    use crate::domain::model::gallery_item::GalleryItemMedia;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;

    /// A fixed instant the fixtures timestamp with.
    fn ts() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 6)
            .unwrap_or_default()
            .and_hms_opt(12, 0, 0)
            .unwrap_or_default()
    }

    /// Send a GET to `/api/v1/library/gallery/{id}` on behalf of `caller_id`,
    /// injecting the caller identifier the way the auth middleware does.
    async fn send(
        use_case: MockGetGalleryUseCase,
        caller_id: NumericID,
        id: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_get_gallery()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn GetGalleryUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/library/gallery/{id}"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/library/gallery/{id}", get(get_gallery))
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    /// Split `response` into its status, `Content-Type` header and decoded
    /// JSON body.
    async fn into_parts(
        response: Response,
    ) -> Result<(StatusCode, Option<String>, Value), Box<dyn Error>> {
        let status = response.status();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let body = serde_json::from_slice(&bytes)?;
        Ok((status, content_type, body))
    }

    /// Make the mocked use case fail with `error`.
    fn expect_error(use_case: &mut MockGetGalleryUseCase, error: GetGalleryError) {
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));
    }

    #[tokio::test]
    async fn get_gallery_existing_gallery_returns_items() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetGalleryUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(GetGalleryCommand::new(1, 3)))
            .return_once(|_| {
                Box::pin(async {
                    Ok(GetGalleryResponse::new(
                        3,
                        Some(2),
                        "Holidays".to_owned(),
                        true,
                        ts(),
                        ts(),
                        vec![GalleryItemSummary::new(
                            42,
                            3,
                            GalleryItemMedia::Image(7),
                            12,
                            "sunset.jpg".to_owned(),
                            0,
                            ts(),
                        )],
                    ))
                })
            });

        // Act
        let (status, content_type, payload) = into_parts(send(use_case, 1, "3").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(
            payload,
            json!({
                "_links": {
                    "self": { "href": "/api/v1/library/gallery/3" },
                    "items": { "href": "/api/v1/library/gallery/3/item" }
                },
                "id": 3i64,
                "name": "Holidays",
                "owner_id": 2i64,
                "is_public": true,
                "created_at": "2026-10-06T12:00:00",
                "last_modified_at": "2026-10-06T12:00:00",
                "item_count": 1i64,
                "items": [{
                    "_links": { "self": { "href": "/api/v1/library/gallery/3/item/42" } },
                    "id": 42i64,
                    "type": "image",
                    "media_id": 7i64,
                    "file_id": 12i64,
                    "name": "sunset.jpg",
                    "added_at": "2026-10-06T12:00:00"
                }]
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_system_gallery_has_null_owner() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetGalleryUseCase::new();
        use_case.expect_execute().times(1).return_once(|_| {
            Box::pin(async {
                Ok(GetGalleryResponse::new(
                    0,
                    None,
                    "Default".to_owned(),
                    true,
                    ts(),
                    ts(),
                    Vec::new(),
                ))
            })
        });

        // Act
        let (status, _, payload) = into_parts(send(use_case, 1, "0").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload.get("owner_id"), Some(&Value::Null));
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_forbidden_returns_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetGalleryUseCase::new();
        expect_error(&mut use_case, GetGalleryError::Forbidden);

        // Act
        let response = send(use_case, 1, "3").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the caller is not allowed to read this gallery")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_unknown_gallery_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetGalleryUseCase::new();
        expect_error(&mut use_case, GetGalleryError::NoSuchGallery);

        // Act
        let response = send(use_case, 1, "3").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("a gallery with this identifier does not exist")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_unknown_caller_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetGalleryUseCase::new();
        expect_error(&mut use_case, GetGalleryError::NoSuchCaller);

        // Act
        let response = send(use_case, 999, "3").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the authenticated user does not exist")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_dependency_failure_returns_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetGalleryUseCase::new();
        expect_error(
            &mut use_case,
            GetGalleryError::Unknown(anyhow::anyhow!("boom")),
        );

        // Act
        let response = send(use_case, 1, "3").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("an unexpected error occurred")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_invalid_id_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockGetGalleryUseCase::new();

        // Act
        let response = send(use_case, 1, "abc").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("invalid gallery identifier")
        );
        Ok(())
    }
}
