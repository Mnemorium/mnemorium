use axum::extract::Path;
use axum::extract::State;
use axum::response::Response;
use serde::Deserialize;
use serde::Serialize;
use utoipa::ToSchema;

use crate::application::port::get_image::GetImageCommand;
use crate::application::port::get_image::GetImageError;
use crate::application::port::get_image::GetImageResponse as GetImageResponseData;
use crate::domain::alias::NumericID;
use crate::domain::model::image::Orientation;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::Link;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::asset::asset_file_href;
use crate::infrastructure::inbound::rest::handler::asset::asset_image_href;
use crate::infrastructure::inbound::rest::handler::asset::format_date;
use crate::infrastructure::inbound::rest::handler::asset::parse_asset_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// HAL links exposed by a typed-image representation.
///
/// The `self` link addresses the image and `file` names the stored file it
/// backs (`API-032`, `API-034`, `API-037`).
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct ImageLinks {
    /// Link to the stored file backing the image.
    pub file: Link,
    /// Link to the image resource itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

impl ImageLinks {
    /// Build the links of the image identified by `image_id`.
    #[must_use]
    pub fn for_image(image_id: NumericID, file_id: NumericID) -> Self {
        Self {
            file: Link::new(&asset_file_href(file_id)),
            self_link: Link::new(&asset_image_href(image_id)),
        }
    }
}

/// Representation of a typed image.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct ImageResponse {
    /// Date at which the image was created, as `YYYY-MM-DD`.
    #[schema(example = json!("2026-01-01"))]
    pub created_at: String,
    /// Identifier of the stored file backing the image.
    pub file_id: NumericID,
    /// Height of the image, in pixels.
    pub height_px: i64,
    /// Unique identifier of the image.
    pub id: NumericID,
    /// Links to the image and its backing file.
    #[serde(rename = "_links")]
    pub links: ImageLinks,
    /// Name of the image.
    #[schema(example = json!("cover.png"))]
    pub name: String,
    /// Orientation of the image.
    pub orientation: Orientation,
    /// Width of the image, in pixels.
    pub width_px: i64,
}

impl ImageResponse {
    /// Map a fetched image onto its HTTP representation.
    #[must_use]
    pub fn from_response(response: &GetImageResponseData) -> Self {
        Self {
            created_at: format_date(response.created_at()),
            file_id: response.file_id(),
            height_px: response.height_px(),
            id: response.id(),
            links: ImageLinks::for_image(response.id(), response.file_id()),
            name: response.name().to_owned(),
            orientation: response.orientation(),
            width_px: response.width_px(),
        }
    }
}

/// Map a fetch-image error to its API error.
impl From<GetImageError> for ApiError {
    fn from(err: GetImageError) -> Self {
        match err {
            GetImageError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetImageError::NoSuchImage => Self::NotFound(err.to_string()),
            GetImageError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Fetch one typed image by its identifier.
///
/// The image follows the read rule of the resource that owns its file. An
/// unknown image and an unreadable image are one outcome, `404 Not Found`
/// (`API-044`).
#[utoipa::path(
    get,
    operation_id = "get_image",
    path = "/asset/image/{image_id}",
    tag = "asset",
    params(
        ("image_id" = NumericID, Path, description = "Identifier of the image to fetch"),
    ),
    responses(
        (
            status = OK,
            body = ImageResponse,
            content_type = "application/hal+json",
            description = "Image found"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid image identifier"
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
            description = "Unknown or unreadable image"
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
    summary = "Fetch a typed image"
)]
pub async fn get_image(
    Path(image_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let image_id = parse_asset_id(&image_id_value, "image_id", "invalid image identifier")?;
    let response = state
        .asset_use_case_factory()
        .get_image()
        .execute(GetImageCommand::new(caller.user_id(), image_id))
        .await?;
    Ok(hal_json(ImageResponse::from_response(&response)))
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
    use chrono::NaiveDate;
    use mockall::predicate::eq;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::get_image;
    use crate::application::port::asset_use_case_factory::MockAssetUseCaseFactory;
    use crate::application::port::get_image::GetImageCommand;
    use crate::application::port::get_image::GetImageError;
    use crate::application::port::get_image::GetImageResponse as GetImageResponseData;
    use crate::application::port::get_image::GetImageUseCase;
    use crate::application::port::get_image::MockGetImageUseCase;
    use crate::domain::alias::NumericID;
    use crate::domain::model::image::Orientation;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_asset;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_status_and_body;

    /// Send a GET to `/api/v1/asset/image/{image_id}` on behalf of `caller_id`.
    async fn send(
        use_case: MockGetImageUseCase,
        caller_id: NumericID,
        image_id: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockAssetUseCaseFactory::new();
        factory
            .expect_get_image()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn GetImageUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/asset/image/{image_id}"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/asset/image/{image_id}", get(get_image))
            .with_state(app_state_with_asset(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn get_image_returns_the_representation_and_links() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetImageUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(GetImageCommand::new(3, 19)))
            .return_once(|_| {
                Box::pin(async {
                    Ok(GetImageResponseData::new(
                        19,
                        "cover.png".to_owned(),
                        10,
                        20,
                        Orientation::Portrait,
                        NaiveDate::default(),
                        100,
                    ))
                })
            });

        // Act
        let response = send(use_case, 3, "19").await?;
        let (status, payload) = into_status_and_body(response).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            payload,
            json!({
                "id": 19i64,
                "name": "cover.png",
                "width_px": 10i64,
                "height_px": 20i64,
                "orientation": "PORTRAIT",
                "created_at": "1970-01-01",
                "file_id": 100i64,
                "_links": {
                    "self": { "href": "/api/v1/asset/image/19" },
                    "file": { "href": "/api/v1/asset/file/100" },
                },
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_image_unknown_image_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetImageUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(GetImageError::NoSuchImage) }));

        // Act
        let response = send(use_case, 3, "19").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn get_image_malformed_identifier_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockGetImageUseCase::new();

        // Act
        let response = send(use_case, 3, "abc").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("invalid image identifier")
        );
        Ok(())
    }
}
