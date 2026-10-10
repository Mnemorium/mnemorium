pub mod delete_gallery;
pub mod delete_gallery_item;
pub mod get_gallery;
pub mod get_gallery_item;
pub mod get_gallery_item_list;
pub mod get_gallery_list;
pub mod post_gallery;
pub mod post_gallery_item;

use axum::Router;
use axum::middleware;
use axum::routing::get;
use chrono::NaiveDateTime;
use serde::Deserialize;
use serde::Serialize;
use tracing::warn;
use utoipa::ToSchema;

use crate::application::port::add_gallery_item::AddGalleryItemResponse;
use crate::application::port::create_gallery::CreateGalleryResponse;
use crate::application::port::get_gallery::GalleryItemSummary;
use crate::application::port::get_gallery::GetGalleryResponse as GetGalleryResponseData;
use crate::application::port::get_gallery_item::GetGalleryItemResponse;
use crate::application::port::list_galleries::ListGalleriesItem;
use crate::domain::alias::NumericID;
use crate::domain::model::gallery_item::GalleryItemMedia;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::Link;
use crate::infrastructure::inbound::rest::hal::SelfLinks;
use crate::infrastructure::inbound::rest::handler::library::delete_gallery::delete_gallery;
use crate::infrastructure::inbound::rest::handler::library::delete_gallery_item::delete_gallery_item;
use crate::infrastructure::inbound::rest::handler::library::get_gallery::get_gallery;
use crate::infrastructure::inbound::rest::handler::library::get_gallery_item::get_gallery_item;
use crate::infrastructure::inbound::rest::handler::library::get_gallery_item_list::get_gallery_item_list;
use crate::infrastructure::inbound::rest::handler::library::get_gallery_list::get_gallery_list;
use crate::infrastructure::inbound::rest::handler::library::post_gallery::post_gallery;
use crate::infrastructure::inbound::rest::handler::library::post_gallery_item::post_gallery_item;
use crate::infrastructure::inbound::rest::middleware::auth::authenticate;
use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;

/// Date and time format of the gallery representations, per the asset
/// representations.
const TIMESTAMP_FORMAT: &str = "%Y-%m-%dT%H:%M:%S";

/// HAL links exposed by a gallery representation.
///
/// The `self` link addresses the gallery; the `items` link exposes the alias
/// path where its items are served (`API-037`).
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct GalleryLinks {
    /// Link to the item collection of the gallery.
    pub items: Link,
    /// Link to the gallery resource itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

impl GalleryLinks {
    /// Build the links of the gallery identified by `gallery_id`.
    #[must_use]
    pub fn for_gallery(gallery_id: NumericID) -> Self {
        Self {
            items: Link::new(&gallery_items_href(gallery_id)),
            self_link: Link::new(&gallery_self_href(gallery_id)),
        }
    }
}

/// Kind of the single medium a gallery item references.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum GalleryItemType {
    /// The item references an image.
    Image,
    /// The item references a video.
    Video,
}

impl GalleryItemType {
    /// Build the medium reference of `media_id` for this kind.
    #[must_use]
    pub fn media(self, media_id: NumericID) -> GalleryItemMedia {
        match self {
            Self::Image => GalleryItemMedia::Image(media_id),
            Self::Video => GalleryItemMedia::Video(media_id),
        }
    }
}

impl From<GalleryItemMedia> for GalleryItemType {
    fn from(media: GalleryItemMedia) -> Self {
        match media {
            GalleryItemMedia::Image(_) => Self::Image,
            GalleryItemMedia::Video(_) => Self::Video,
        }
    }
}

/// One item of a gallery, without the metadata of its medium.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct GalleryItemResponse {
    /// Date and time at which the item was added.
    pub added_at: String,
    /// Identifier of the backing file.
    pub file_id: NumericID,
    /// Unique identifier of the item.
    pub id: NumericID,
    /// Link to the item resource itself.
    #[serde(rename = "_links")]
    pub links: SelfLinks,
    /// Identifier of the referenced medium.
    pub media_id: NumericID,
    /// Kind of the referenced medium.
    #[serde(rename = "type")]
    pub media_type: GalleryItemType,
    /// Name of the item: the image name, or the file path for a video.
    pub name: String,
}

impl From<&GalleryItemSummary> for GalleryItemResponse {
    fn from(summary: &GalleryItemSummary) -> Self {
        let gallery_id = summary.gallery_id();
        let item_id = summary.gallery_item_id();
        Self {
            added_at: format_timestamp(summary.added_at()),
            file_id: summary.file_id(),
            id: item_id,
            links: SelfLinks::new(&gallery_item_self_href(gallery_id, item_id)),
            media_id: summary.media().media_id(),
            media_type: GalleryItemType::from(summary.media()),
            name: summary.name().to_owned(),
        }
    }
}

/// Detailed item of a gallery, returned by the item endpoints.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct GalleryItemDetailResponse {
    /// Date and time at which the item was added.
    pub added_at: String,
    /// Identifier of the backing file.
    pub file_id: NumericID,
    /// Unique identifier of the item.
    pub id: NumericID,
    /// Link to the item resource itself.
    #[serde(rename = "_links")]
    pub links: SelfLinks,
    /// Identifier of the referenced medium.
    pub media_id: NumericID,
    /// Kind of the referenced medium.
    #[serde(rename = "type")]
    pub media_type: GalleryItemType,
    /// Name of the item: the image name, or the file path for a video.
    pub name: String,
}

impl GalleryItemDetailResponse {
    /// Assemble the detailed item from its parts.
    fn new(
        gallery_id: NumericID,
        item_id: NumericID,
        media: GalleryItemMedia,
        file_id: NumericID,
        name: String,
        added_at: NaiveDateTime,
    ) -> Self {
        Self {
            added_at: format_timestamp(added_at),
            file_id,
            id: item_id,
            links: SelfLinks::new(&gallery_item_self_href(gallery_id, item_id)),
            media_id: media.media_id(),
            media_type: GalleryItemType::from(media),
            name,
        }
    }
}

impl From<GetGalleryItemResponse> for GalleryItemDetailResponse {
    fn from(response: GetGalleryItemResponse) -> Self {
        Self::new(
            response.gallery_id(),
            response.gallery_item_id(),
            response.media(),
            response.file_id(),
            response.name().to_owned(),
            response.added_at(),
        )
    }
}

impl From<AddGalleryItemResponse> for GalleryItemDetailResponse {
    fn from(response: AddGalleryItemResponse) -> Self {
        Self::new(
            response.gallery_id(),
            response.gallery_item_id(),
            response.media(),
            response.file_id(),
            response.name().to_owned(),
            response.added_at(),
        )
    }
}

/// Gallery returned by a successful lookup or creation.
///
/// On a collection listing, `items` stays empty and `item_count` communicates
/// the size of the gallery.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct GalleryResponse {
    /// Date and time at which the gallery was created.
    pub created_at: String,
    /// Unique identifier of the gallery.
    pub id: NumericID,
    /// Whether any authenticated user may read and moderate the gallery.
    pub is_public: bool,
    /// Number of items the gallery contains.
    pub item_count: usize,
    /// Items of the gallery, ordered by position.
    pub items: Vec<GalleryItemResponse>,
    /// Date and time of the last modification of the gallery.
    pub last_modified_at: String,
    /// Links to the gallery and its item collection.
    #[serde(rename = "_links")]
    pub links: GalleryLinks,
    /// Name of the gallery.
    pub name: String,
    /// Identifier of the owning user, absent for the system gallery.
    pub owner_id: Option<NumericID>,
}

impl From<CreateGalleryResponse> for GalleryResponse {
    fn from(response: CreateGalleryResponse) -> Self {
        Self {
            created_at: format_timestamp(response.created_at()),
            id: response.gallery_id(),
            is_public: response.is_public(),
            item_count: 0,
            items: Vec::new(),
            last_modified_at: format_timestamp(response.last_modified_at()),
            links: GalleryLinks::for_gallery(response.gallery_id()),
            name: response.name().to_owned(),
            owner_id: Some(response.user_id()),
        }
    }
}

impl From<ListGalleriesItem> for GalleryResponse {
    fn from(item: ListGalleriesItem) -> Self {
        Self {
            created_at: format_timestamp(item.created_at()),
            id: item.gallery_id(),
            is_public: item.is_public(),
            item_count: item.item_count(),
            items: Vec::new(),
            last_modified_at: format_timestamp(item.last_modified_at()),
            links: GalleryLinks::for_gallery(item.gallery_id()),
            name: item.name().to_owned(),
            owner_id: item.user_id(),
        }
    }
}

impl From<GetGalleryResponseData> for GalleryResponse {
    fn from(response: GetGalleryResponseData) -> Self {
        Self {
            created_at: format_timestamp(response.created_at()),
            id: response.gallery_id(),
            is_public: response.is_public(),
            item_count: response.item_count(),
            items: response
                .items()
                .iter()
                .map(GalleryItemResponse::from)
                .collect(),
            last_modified_at: format_timestamp(response.last_modified_at()),
            links: GalleryLinks::for_gallery(response.gallery_id()),
            name: response.name().to_owned(),
            owner_id: response.user_id(),
        }
    }
}

/// Canonical URI reference of the gallery resource identified by `gallery_id`.
///
/// Shared by the gallery representations and the creation response so the
/// `_links.self` target and the `Location` header cannot drift (`API-032`).
#[expect(
    clippy::single_call_fn,
    reason = "centralising the canonical URI so `_links.self` and `Location` cannot drift is the point"
)]
#[must_use]
pub(crate) fn gallery_self_href(gallery_id: NumericID) -> String {
    format!("/api/v1/library/gallery/{gallery_id}")
}

/// Canonical URI reference of the item collection of `gallery_id`.
///
/// This is the alias path of `GET /library/gallery/{id}/item`; the `_links.self`
/// of the collection representation keeps pointing at the canonical gallery
/// resource (`API-042`).
#[expect(
    clippy::single_call_fn,
    reason = "centralising the items URI keeps the gallery links discoverable from one place (`API-037`)"
)]
#[must_use]
pub(crate) fn gallery_items_href(gallery_id: NumericID) -> String {
    format!("/api/v1/library/gallery/{gallery_id}/item")
}

/// Canonical URI reference of the gallery item identified by `item_id`.
#[must_use]
pub(crate) fn gallery_item_self_href(gallery_id: NumericID, item_id: NumericID) -> String {
    format!("/api/v1/library/gallery/{gallery_id}/item/{item_id}")
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

/// Render `value` with the representation's timestamp format.
fn format_timestamp(value: NaiveDateTime) -> String {
    value.format(TIMESTAMP_FORMAT).to_string()
}

/// Routes of the Library bounded context.
///
/// Every route requires an authenticated caller.
pub fn library_routes(state: &AppState) -> Router {
    let gallery: Router<AppState> = Router::new()
        .route("/gallery", get(get_gallery_list).post(post_gallery))
        .route("/gallery/{id}", get(get_gallery).delete(delete_gallery))
        .route(
            "/gallery/{id}/item",
            get(get_gallery_item_list).post(post_gallery_item),
        )
        .route(
            "/gallery/{id}/item/{item_id}",
            get(get_gallery_item).delete(delete_gallery_item),
        );

    gallery
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            authenticate::<JwtTokenProvider>,
        ))
        .with_state(state.clone())
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
    use chrono::NaiveDate;
    use chrono::NaiveDateTime;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::library_routes;
    use crate::application::port::create_gallery::CreateGalleryResponse;
    use crate::application::port::create_gallery::CreateGalleryUseCase;
    use crate::application::port::create_gallery::MockCreateGalleryUseCase;
    use crate::application::port::delete_gallery::DeleteGalleryUseCase;
    use crate::application::port::delete_gallery::MockDeleteGalleryUseCase;
    use crate::application::port::get_gallery::GetGalleryResponse;
    use crate::application::port::get_gallery::GetGalleryUseCase;
    use crate::application::port::get_gallery::MockGetGalleryUseCase;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::domain::port::token_provider::TokenProvider as _;
    use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;
    use crate::test_helpers::TEST_JWT_SECRET;
    use crate::test_helpers::app_state_with_library;

    /// A fixed instant the fixtures timestamp with.
    fn ts() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 6)
            .unwrap_or_default()
            .and_hms_opt(12, 0, 0)
            .unwrap_or_default()
    }

    /// Mint a bearer token the real `authenticate` layer accepts.
    async fn bearer() -> Result<String, Box<dyn Error>> {
        let provider = JwtTokenProvider::new(TEST_JWT_SECRET.to_owned(), 3600);
        Ok(provider.issue(1).await?.value().to_owned())
    }

    #[tokio::test]
    async fn library_routes_gallery_route_reaches_handler() -> Result<(), Box<dyn Error>> {
        // Arrange: the real router, wrapped in its `authenticate` layer.
        let mut get_gallery = MockGetGalleryUseCase::new();
        get_gallery.expect_execute().times(1).return_once(|_| {
            Box::pin(async {
                Ok(GetGalleryResponse::new(
                    3,
                    Some(1),
                    "Holidays".to_owned(),
                    false,
                    ts(),
                    ts(),
                    Vec::new(),
                ))
            })
        });
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_get_gallery()
            .times(1)
            .return_once(move || Arc::new(get_gallery) as Arc<dyn GetGalleryUseCase>);
        let state = app_state_with_library(Arc::new(factory))?;
        let request = Request::builder()
            .method("GET")
            .uri("/gallery/3")
            .header(header::AUTHORIZATION, format!("Bearer {}", bearer().await?))
            .body(Body::empty())?;

        // Act
        let response: Response = library_routes(&state).oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn library_routes_create_route_reaches_handler() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut create_gallery = MockCreateGalleryUseCase::new();
        create_gallery.expect_execute().times(1).return_once(|_| {
            Box::pin(async {
                Ok(CreateGalleryResponse::new(
                    5,
                    1,
                    "Holidays".to_owned(),
                    false,
                    ts(),
                    ts(),
                ))
            })
        });
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_create_gallery()
            .times(1)
            .return_once(move || Arc::new(create_gallery) as Arc<dyn CreateGalleryUseCase>);
        let state = app_state_with_library(Arc::new(factory))?;
        let request = Request::builder()
            .method("POST")
            .uri("/gallery")
            .header(header::AUTHORIZATION, format!("Bearer {}", bearer().await?))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({ "name": "Holidays" }).to_string()))?;

        // Act
        let response: Response = library_routes(&state).oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::CREATED);
        Ok(())
    }

    #[tokio::test]
    async fn library_routes_delete_route_reaches_handler() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut delete_gallery = MockDeleteGalleryUseCase::new();
        delete_gallery
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Ok(()) }));
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_delete_gallery()
            .times(1)
            .return_once(move || Arc::new(delete_gallery) as Arc<dyn DeleteGalleryUseCase>);
        let state = app_state_with_library(Arc::new(factory))?;
        let request = Request::builder()
            .method("DELETE")
            .uri("/gallery/3")
            .header(header::AUTHORIZATION, format!("Bearer {}", bearer().await?))
            .body(Body::empty())?;

        // Act
        let response: Response = library_routes(&state).oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        Ok(())
    }
}
