use axum::extract::Path;
use axum::extract::State;
use axum::response::Response;
use tracing::warn;

use crate::application::port::get_gallery_item::GetGalleryItemCommand;
use crate::application::port::get_gallery_item::GetGalleryItemError;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::get_gallery::parse_gallery_id;
use crate::infrastructure::inbound::rest::handler::library::representation::GalleryItemDetailResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Map a fetch-gallery-item error to its API error.
impl From<GetGalleryItemError> for ApiError {
    fn from(err: GetGalleryItemError) -> Self {
        match err {
            GetGalleryItemError::Forbidden => Self::Forbidden(err.to_string()),
            GetGalleryItemError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetGalleryItemError::NoSuchGallery | GetGalleryItemError::NoSuchItem => {
                Self::NotFound(err.to_string())
            }
            GetGalleryItemError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Fetch one item of a gallery by its identifiers.
///
/// Any authenticated caller may read an item of a public gallery; an item of a
/// private gallery is readable by the gallery owner and administrators only.
/// The item is scoped to the gallery named in the path.
#[utoipa::path(
    get,
    operation_id = "get_gallery_item",
    path = "/library/gallery/{id}/item/{item_id}",
    tag = "library",
    params(
        ("id" = NumericID, Path, description = "Identifier of the gallery the item belongs to"),
        ("item_id" = NumericID, Path, description = "Identifier of the item to fetch"),
    ),
    responses(
        (
            status = OK,
            body = GalleryItemDetailResponse,
            content_type = "application/hal+json",
            description = "Item found"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid gallery or item identifier"
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
            description = "Unknown gallery or item"
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
    summary = "Fetch a gallery item"
)]
pub async fn get_gallery_item(
    Path((gallery_id_value, item_id_value)): Path<(String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let gallery_id = parse_gallery_id(&gallery_id_value)?;
    let item_id = parse_item_id(&item_id_value)?;
    let response = state
        .library_use_case_factory()
        .get_gallery_item()
        .execute(GetGalleryItemCommand::new(
            caller.user_id(),
            gallery_id,
            item_id,
        ))
        .await?;
    Ok(hal_json(GalleryItemDetailResponse::from(response)))
}

/// Parse an item identifier from a path segment.
pub(crate) fn parse_item_id(value: &str) -> Result<NumericID, ApiError> {
    value.parse::<NumericID>().map_err(|_| {
        // The malformed value and the parser detail never reach the log
        // (`OBS-003`); only the classification and its declared fields do
        // (`OBS-006`).
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "item_id",
            reason = "invalid_identifier",
            "rejected a malformed item identifier"
        );
        ApiError::BadRequest("invalid item identifier".to_owned())
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

    use super::get_gallery_item;
    use crate::application::port::get_gallery_item::GetGalleryItemCommand;
    use crate::application::port::get_gallery_item::GetGalleryItemError;
    use crate::application::port::get_gallery_item::GetGalleryItemResponse;
    use crate::application::port::get_gallery_item::GetGalleryItemUseCase;
    use crate::application::port::get_gallery_item::MockGetGalleryItemUseCase;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::domain::alias::NumericID;
    use crate::domain::model::gallery_item::GalleryItemMedia;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;

    /// A fixed instant the fixtures timestamp with.
    #[expect(
        clippy::single_call_fn,
        reason = "the named instant keeps the test scenario readable instead of a bare date literal"
    )]
    fn ts() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 6)
            .unwrap_or_default()
            .and_hms_opt(12, 0, 0)
            .unwrap_or_default()
    }

    /// Send a GET to `/api/v1/library/gallery/{id}/item/{item_id}` on behalf of
    /// `caller_id`, injecting the caller identifier the way the auth
    /// middleware does.
    async fn send(
        use_case: MockGetGalleryItemUseCase,
        caller_id: NumericID,
        id: &str,
        item_id: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_get_gallery_item()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn GetGalleryItemUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/library/gallery/{id}/item/{item_id}"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/gallery/{id}/item/{item_id}",
                get(get_gallery_item),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    /// Split `response` into its status, `Content-Type` header and decoded
    /// JSON body.
    #[expect(
        clippy::single_call_fn,
        reason = "the response splitter keeps the happy-path test readable"
    )]
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
    fn expect_error(use_case: &mut MockGetGalleryItemUseCase, error: GetGalleryItemError) {
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));
    }

    #[tokio::test]
    async fn get_gallery_item_existing_item_returns_detail() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetGalleryItemUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(GetGalleryItemCommand::new(1, 3, 42)))
            .return_once(|_| {
                Box::pin(async {
                    Ok(GetGalleryItemResponse::new(
                        42,
                        3,
                        GalleryItemMedia::Image(7),
                        12,
                        "sunset.jpg".to_owned(),
                        0,
                        ts(),
                    ))
                })
            });

        // Act
        let (status, content_type, payload) =
            into_parts(send(use_case, 1, "3", "42").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(
            payload,
            json!({
                "_links": { "self": { "href": "/api/v1/library/gallery/3/item/42" } },
                "id": 42i64,
                "type": "image",
                "media_id": 7i64,
                "file_id": 12i64,
                "name": "sunset.jpg",
                "added_at": "2026-10-06T12:00:00"
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_item_forbidden_returns_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetGalleryItemUseCase::new();
        expect_error(&mut use_case, GetGalleryItemError::Forbidden);

        // Act
        let response = send(use_case, 1, "3", "42").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the caller is not allowed to read this gallery")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_item_unknown_item_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetGalleryItemUseCase::new();
        expect_error(&mut use_case, GetGalleryItemError::NoSuchItem);

        // Act
        let response = send(use_case, 1, "3", "42").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("an item with this identifier does not exist in this gallery")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_item_unknown_gallery_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetGalleryItemUseCase::new();
        expect_error(&mut use_case, GetGalleryItemError::NoSuchGallery);

        // Act
        let response = send(use_case, 1, "3", "42").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_item_unknown_caller_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetGalleryItemUseCase::new();
        expect_error(&mut use_case, GetGalleryItemError::NoSuchCaller);

        // Act
        let response = send(use_case, 999, "3", "42").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_item_dependency_failure_returns_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetGalleryItemUseCase::new();
        expect_error(
            &mut use_case,
            GetGalleryItemError::Unknown(anyhow::anyhow!("boom")),
        );

        // Act
        let response = send(use_case, 1, "3", "42").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_item_invalid_gallery_id_returns_bad_request() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = MockGetGalleryItemUseCase::new();

        // Act
        let response = send(use_case, 1, "abc", "42").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("invalid gallery identifier")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_item_invalid_item_id_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockGetGalleryItemUseCase::new();

        // Act
        let response = send(use_case, 1, "3", "abc").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("invalid item identifier")
        );
        Ok(())
    }
}
