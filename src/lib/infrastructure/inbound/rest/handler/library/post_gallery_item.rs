use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::Response;

use crate::application::port::add_gallery_item::AddGalleryItemCommand;
use crate::application::port::add_gallery_item::AddGalleryItemError;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::get_gallery::parse_gallery_id;
use crate::infrastructure::inbound::rest::handler::library::representation::GalleryItemDetailResponse;
use crate::infrastructure::inbound::rest::handler::library::representation::PostGalleryItemRequest;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Map an add-gallery-item error to its API error.
impl From<AddGalleryItemError> for ApiError {
    fn from(err: AddGalleryItemError) -> Self {
        match err {
            AddGalleryItemError::AlreadyAssigned => Self::Conflict(err.to_string()),
            AddGalleryItemError::Forbidden | AddGalleryItemError::NotOwnedMedia => {
                Self::Forbidden(err.to_string())
            }
            AddGalleryItemError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            AddGalleryItemError::NoSuchGallery | AddGalleryItemError::NoSuchMedia => {
                Self::NotFound(err.to_string())
            }
            AddGalleryItemError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Add one of the caller's media to a gallery.
///
/// Any authenticated caller may add an item to a public gallery; a private
/// gallery accepts items from its owner and administrators only. A caller may
/// only add media it owns, unless it is an administrator. A medium already in a
/// gallery is rejected with `409`.
#[utoipa::path(
    post,
    operation_id = "post_gallery_item",
    path = "/library/gallery/{id}/item",
    tag = "library",
    request_body = PostGalleryItemRequest,
    params(
        ("id" = NumericID, Path, description = "Identifier of the gallery the media is added to"),
    ),
    responses(
        (
            status = CREATED,
            body = GalleryItemDetailResponse,
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "URI of the newly added item"),
            ),
            description = "Item added",
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid gallery identifier or malformed request body"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
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
            description = "The request body does not match the expected schema"
        ),
        (
            status = FORBIDDEN,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Caller may not add items to the gallery, or does not own the medium"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown gallery or medium"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The medium already belongs to a gallery"
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
    summary = "Add an item to a gallery"
)]
pub async fn post_gallery_item(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    Path(id): Path<String>,
    payload: Result<Json<PostGalleryItemRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let gallery_id = parse_gallery_id(&id)?;
    let Json(request) = payload.map_err(ApiError::from)?;
    let media = request.media_type.media(request.media_id);
    let response = state
        .library_use_case_factory()
        .add_gallery_item()
        .execute(AddGalleryItemCommand::new(
            caller.user_id(),
            gallery_id,
            media,
        ))
        .await?;
    let body = GalleryItemDetailResponse::from(response);
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
    use axum::body::to_bytes;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::response::Response;
    use axum::routing::post;
    use chrono::NaiveDate;
    use chrono::NaiveDateTime;
    use mockall::predicate::eq;
    use serde_json::Value;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::post_gallery_item;
    use crate::application::port::add_gallery_item::AddGalleryItemCommand;
    use crate::application::port::add_gallery_item::AddGalleryItemError;
    use crate::application::port::add_gallery_item::AddGalleryItemResponse;
    use crate::application::port::add_gallery_item::AddGalleryItemUseCase;
    use crate::application::port::add_gallery_item::MockAddGalleryItemUseCase;
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

    /// Send `body` to `/api/v1/library/gallery/{id}/item` on behalf of
    /// `caller_id`, injecting the caller identifier the way the auth
    /// middleware does.
    async fn send(
        use_case: MockAddGalleryItemUseCase,
        caller_id: NumericID,
        id: &str,
        body: Body,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_add_gallery_item()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn AddGalleryItemUseCase>);

        let mut request = Request::builder()
            .method("POST")
            .uri(format!("/api/v1/library/gallery/{id}/item"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(body)?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/library/gallery/{id}/item", post(post_gallery_item))
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    /// Split `response` into its status, `Location` header and decoded JSON
    /// body.
    async fn into_parts(
        response: Response,
    ) -> Result<(StatusCode, Option<String>, Value), Box<dyn Error>> {
        let status = response.status();
        let location = response
            .headers()
            .get(header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let body = serde_json::from_slice(&bytes)?;
        Ok((status, location, body))
    }

    /// Make the mocked use case fail with `error`.
    fn expect_error(use_case: &mut MockAddGalleryItemUseCase, error: AddGalleryItemError) {
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));
    }

    #[tokio::test]
    async fn post_gallery_item_image_returns_created_item_with_location()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockAddGalleryItemUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(AddGalleryItemCommand::new(
                1,
                3,
                GalleryItemMedia::Image(7),
            )))
            .return_once(|_| {
                Box::pin(async {
                    Ok(AddGalleryItemResponse::new(
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
        let (status, location, payload) = into_parts(
            send(
                use_case,
                1,
                "3",
                Body::from(json!({ "type": "image", "media_id": 7i64 }).to_string()),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            location.as_deref(),
            Some("/api/v1/library/gallery/3/item/42")
        );
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
    async fn post_gallery_item_video_kind_is_forwarded() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockAddGalleryItemUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(AddGalleryItemCommand::new(
                1,
                3,
                GalleryItemMedia::Video(9),
            )))
            .return_once(|_| {
                Box::pin(async {
                    Ok(AddGalleryItemResponse::new(
                        43,
                        3,
                        GalleryItemMedia::Video(9),
                        13,
                        "clip.mp4".to_owned(),
                        1,
                        ts(),
                    ))
                })
            });

        // Act
        let (status, _, payload) = into_parts(
            send(
                use_case,
                1,
                "3",
                Body::from(json!({ "type": "video", "media_id": 9i64 }).to_string()),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(payload.get("type"), Some(&json!("video")));
        Ok(())
    }

    #[tokio::test]
    async fn post_gallery_item_already_assigned_returns_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockAddGalleryItemUseCase::new();
        expect_error(&mut use_case, AddGalleryItemError::AlreadyAssigned);

        // Act
        let response = send(
            use_case,
            1,
            "3",
            Body::from(json!({ "type": "image", "media_id": 7i64 }).to_string()),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the medium already belongs to a gallery")
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_gallery_item_not_owned_media_returns_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockAddGalleryItemUseCase::new();
        expect_error(&mut use_case, AddGalleryItemError::NotOwnedMedia);

        // Act
        let response = send(
            use_case,
            1,
            "3",
            Body::from(json!({ "type": "image", "media_id": 7i64 }).to_string()),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the caller does not own this medium")
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_gallery_item_forbidden_returns_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockAddGalleryItemUseCase::new();
        expect_error(&mut use_case, AddGalleryItemError::Forbidden);

        // Act
        let response = send(
            use_case,
            1,
            "3",
            Body::from(json!({ "type": "image", "media_id": 7i64 }).to_string()),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        Ok(())
    }

    #[tokio::test]
    async fn post_gallery_item_unknown_media_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockAddGalleryItemUseCase::new();
        expect_error(&mut use_case, AddGalleryItemError::NoSuchMedia);

        // Act
        let response = send(
            use_case,
            1,
            "3",
            Body::from(json!({ "type": "image", "media_id": 7i64 }).to_string()),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn post_gallery_item_unknown_gallery_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockAddGalleryItemUseCase::new();
        expect_error(&mut use_case, AddGalleryItemError::NoSuchGallery);

        // Act
        let response = send(
            use_case,
            1,
            "3",
            Body::from(json!({ "type": "image", "media_id": 7i64 }).to_string()),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn post_gallery_item_unknown_caller_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockAddGalleryItemUseCase::new();
        expect_error(&mut use_case, AddGalleryItemError::NoSuchCaller);

        // Act
        let response = send(
            use_case,
            999,
            "3",
            Body::from(json!({ "type": "image", "media_id": 7i64 }).to_string()),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[tokio::test]
    async fn post_gallery_item_dependency_failure_returns_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockAddGalleryItemUseCase::new();
        expect_error(
            &mut use_case,
            AddGalleryItemError::Unknown(anyhow::anyhow!("boom")),
        );

        // Act
        let response = send(
            use_case,
            1,
            "3",
            Body::from(json!({ "type": "image", "media_id": 7i64 }).to_string()),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        Ok(())
    }

    #[tokio::test]
    async fn post_gallery_item_invalid_type_returns_unprocessable_entity()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockAddGalleryItemUseCase::new();

        // Act
        let response = send(
            use_case,
            1,
            "3",
            Body::from(json!({ "type": "audio", "media_id": 7i64 }).to_string()),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[tokio::test]
    async fn post_gallery_item_invalid_gallery_id_returns_bad_request() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = MockAddGalleryItemUseCase::new();

        // Act
        let response = send(
            use_case,
            1,
            "abc",
            Body::from(json!({ "type": "image", "media_id": 7i64 }).to_string()),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("invalid gallery identifier")
        );
        Ok(())
    }
}
