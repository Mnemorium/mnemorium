pub mod delete_gallery;
pub mod delete_gallery_item;
pub mod get_album_list;
pub mod get_album_track;
pub mod get_album_track_list;
pub mod get_gallery;
pub mod get_gallery_item;
pub mod get_gallery_item_list;
pub mod get_gallery_list;
pub mod post_gallery;
pub mod post_gallery_item;

use axum::Router;
use axum::middleware;
use axum::routing::get;
use chrono::NaiveDate;
use chrono::NaiveDateTime;
use serde::Deserialize;
use serde::Serialize;
use tracing::warn;
use utoipa::ToSchema;

use crate::application::port::add_gallery_item::AddGalleryItemResponse;
use crate::application::port::create_gallery::CreateGalleryResponse;
use crate::application::port::get_album_track::GetAlbumTrackError;
use crate::application::port::get_album_track::GetAlbumTrackResponse;
use crate::application::port::get_gallery::GalleryItemSummary;
use crate::application::port::get_gallery::GetGalleryResponse as GetGalleryResponseData;
use crate::application::port::get_gallery_item::GetGalleryItemResponse;
use crate::application::port::list_album_tracks::ListAlbumTracksError;
use crate::application::port::list_album_tracks::ListAlbumTracksItem;
use crate::application::port::list_albums::ListAlbumsError;
use crate::application::port::list_albums::ListAlbumsItem;
use crate::application::port::list_galleries::ListGalleriesItem;
use crate::domain::alias::NumericID;
use crate::domain::model::gallery_item::GalleryItemMedia;
use crate::domain::model::music_album::ProductionType;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::Link;
use crate::infrastructure::inbound::rest::hal::SelfLinks;
use crate::infrastructure::inbound::rest::handler::library::delete_gallery::delete_gallery;
use crate::infrastructure::inbound::rest::handler::library::delete_gallery_item::delete_gallery_item;
use crate::infrastructure::inbound::rest::handler::library::get_album_list::ListAlbumsQuery;
use crate::infrastructure::inbound::rest::handler::library::get_album_list::get_album_list;
use crate::infrastructure::inbound::rest::handler::library::get_album_track::get_album_track;
use crate::infrastructure::inbound::rest::handler::library::get_album_track_list::get_album_track_list;
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

/// Date format of the release dates the album and track representations carry.
const DATE_FORMAT: &str = "%Y-%m-%d";

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
            items: Link::new(&format!("/api/v1/library/gallery/{gallery_id}/item")),
            self_link: Link::new(&format!("/api/v1/library/gallery/{gallery_id}")),
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
    /// Parse a medium kind from its wire representation (`image`, `video`).
    ///
    /// Returns `None` when `value` is not a member of the discrete list, so the
    /// inbound handler can classify the rejection as `enum_validation_failed`
    /// (`OBS-006`). The names match the `serde(rename_all = "lowercase")`
    /// representation of [`GalleryItemType`].
    #[must_use]
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "image" => Some(Self::Image),
            "video" => Some(Self::Video),
            _ => None,
        }
    }

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
#[derive(Debug, Serialize, ToSchema)]
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
#[derive(Debug, Serialize, ToSchema)]
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
#[derive(Debug, Serialize, ToSchema)]
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

/// HAL links exposed by a music album representation.
///
/// The `self` link addresses the album; the `tracks` link exposes the track
/// collection of the album (`API-037`).
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct AlbumLinks {
    /// Link to the album resource itself.
    #[serde(rename = "self")]
    pub self_link: Link,
    /// Link to the track collection of the album.
    pub tracks: Link,
}

impl AlbumLinks {
    /// Build the links of the album identified by `album_id`.
    #[must_use]
    pub fn for_album(album_id: NumericID) -> Self {
        Self {
            self_link: Link::new(&format!("/api/v1/library/music/album/{album_id}")),
            tracks: Link::new(&format!("/api/v1/library/music/album/{album_id}/track")),
        }
    }
}

/// HAL links exposed by a music album collection representation.
///
/// The `self` link addresses the collection view the representation is about,
/// `next` and `prev` navigate that same view when another page exists, and the
/// templated `search` link exposes the collection filters (`API-037`,
/// `API-042`). The active filter and the page size are carried into every link
/// so a filtered or paginated listing stays navigable.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct AlbumListLinks {
    /// Link to the next page, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<Link>,
    /// Link to the previous page, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev: Option<Link>,
    /// Templated link exposing the collection's search filters.
    pub search: Link,
    /// Link to the collection itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

impl AlbumListLinks {
    /// Build the links of a music album collection page.
    ///
    /// `query` carries the active filter, `limit` and `offset` are the
    /// effective page, `returned` is the number of albums in the page and
    /// `total` the number of matching albums before pagination. `next` is
    /// exposed when the page ends before `total`; `prev` is exposed when the
    /// page does not start at the first album.
    #[must_use]
    pub fn for_page(
        query: &ListAlbumsQuery,
        limit: usize,
        offset: usize,
        returned: usize,
        total: usize,
    ) -> Self {
        let next = (offset.saturating_add(returned) < total)
            .then(|| Link::new(&album_page_href(query, limit, offset.saturating_add(limit))));
        let prev = (offset > 0)
            .then(|| Link::new(&album_page_href(query, limit, offset.saturating_sub(limit))));
        Self {
            next,
            prev,
            search: Link::templated("/api/v1/library/music/album{?name,limit,offset}"),
            self_link: Link::new(&album_page_href(query, limit, offset)),
        }
    }
}

/// Music albums matching the requested filters.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct AlbumListResponse {
    /// The albums of the returned page.
    pub items: Vec<AlbumResponse>,
    /// Navigation links of the collection.
    #[serde(rename = "_links")]
    pub links: AlbumListLinks,
}

/// A music album returned by a listing.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct AlbumResponse {
    /// Genres of the album, ordered by genre identifier.
    pub genres: Vec<String>,
    /// Unique identifier of the album.
    pub id: NumericID,
    /// Links to the album and its track collection.
    #[serde(rename = "_links")]
    pub links: AlbumLinks,
    /// Name of the album.
    pub name: String,
    /// Production type of the album, when set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub production_type: Option<String>,
    /// Date of the release of the album, in `YYYY-MM-DD` form.
    pub release_date: String,
    /// Number of tracks the album contains.
    pub track_count: usize,
}

impl From<&ListAlbumsItem> for AlbumResponse {
    fn from(item: &ListAlbumsItem) -> Self {
        Self {
            genres: item.genres().to_vec(),
            id: item.album_id(),
            links: AlbumLinks::for_album(item.album_id()),
            name: item.name().to_owned(),
            production_type: item
                .production_type()
                .map(ProductionType::as_wire)
                .map(str::to_owned),
            release_date: format_date(item.release_date()),
            track_count: item.track_count(),
        }
    }
}

/// HAL links exposed by a music track representation.
///
/// The `self` link addresses the track; the `album` link exposes the album the
/// track belongs to (`API-034`, `API-037`).
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct TrackLinks {
    /// Link to the album the track belongs to.
    pub album: Link,
    /// Link to the track resource itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

impl TrackLinks {
    /// Build the links of a track identified by its album and track identifiers.
    #[must_use]
    pub fn for_track(album_id: NumericID, track_id: NumericID) -> Self {
        Self {
            album: Link::new(&format!("/api/v1/library/music/album/{album_id}")),
            self_link: Link::new(&format!(
                "/api/v1/library/music/album/{album_id}/track/{track_id}"
            )),
        }
    }
}

/// Tracks of one music album.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct TrackListResponse {
    /// The tracks of the album.
    pub items: Vec<TrackResponse>,
    /// Navigation links of the collection.
    #[serde(rename = "_links")]
    pub links: SelfLinks,
}

/// One music track of an album.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct TrackResponse {
    /// Identifier of the album the track belongs to.
    pub album_id: NumericID,
    /// Identifier of the backing file.
    pub file_id: NumericID,
    /// Date of the first release of the recording, in `YYYY-MM-DD` form.
    pub first_release_date: String,
    /// Unique identifier of the track.
    pub id: NumericID,
    /// ISRC code of the recording, when set.
    pub isrc_code: Option<String>,
    /// Links to the track and its album.
    #[serde(rename = "_links")]
    pub links: TrackLinks,
    /// Index of the medium the track is positioned on.
    pub medium_index: u32,
    /// Name of the track.
    pub name: String,
    /// Position of the track within its medium.
    pub track_index: i64,
}

impl From<&ListAlbumTracksItem> for TrackResponse {
    fn from(item: &ListAlbumTracksItem) -> Self {
        Self {
            album_id: item.album_id(),
            file_id: item.file_id(),
            first_release_date: format_date(item.first_release_date()),
            id: item.track_id(),
            isrc_code: item.isrc_code().map(str::to_owned),
            links: TrackLinks::for_track(item.album_id(), item.track_id()),
            medium_index: item.medium_index(),
            name: item.name().to_owned(),
            track_index: item.track_index(),
        }
    }
}

impl From<GetAlbumTrackResponse> for TrackResponse {
    fn from(response: GetAlbumTrackResponse) -> Self {
        Self {
            album_id: response.album_id(),
            file_id: response.file_id(),
            first_release_date: format_date(response.first_release_date()),
            id: response.track_id(),
            isrc_code: response.isrc_code().map(str::to_owned),
            links: TrackLinks::for_track(response.album_id(), response.track_id()),
            medium_index: response.medium_index(),
            name: response.name().to_owned(),
            track_index: response.track_index(),
        }
    }
}

/// Map a list-albums error to its API error.
impl From<ListAlbumsError> for ApiError {
    fn from(err: ListAlbumsError) -> Self {
        match err {
            ListAlbumsError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            ListAlbumsError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a list-album-tracks error to its API error.
impl From<ListAlbumTracksError> for ApiError {
    fn from(err: ListAlbumTracksError) -> Self {
        match err {
            ListAlbumTracksError::NoSuchAlbum => Self::NotFound(err.to_string()),
            ListAlbumTracksError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            ListAlbumTracksError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a fetch-album-track error to its API error.
impl From<GetAlbumTrackError> for ApiError {
    fn from(err: GetAlbumTrackError) -> Self {
        match err {
            GetAlbumTrackError::NoSuchAlbum | GetAlbumTrackError::NoSuchTrack => {
                Self::NotFound(err.to_string())
            }
            GetAlbumTrackError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetAlbumTrackError::Unknown(_) => Self::InternalServerError,
        }
    }
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

/// Build the href of the music album collection view at `limit`/`offset`,
/// preserving the active filter so a filtered page keeps its navigation
/// (`API-037`).
fn album_page_href(query: &ListAlbumsQuery, limit: usize, offset: usize) -> String {
    let mut href = String::from("/api/v1/library/music/album?");
    if let Some(name) = query.name.as_deref() {
        href.push_str("name=");
        // Percent-encode the filter value with the unreserved character set, so
        // it cannot break out of the query string.
        for byte in name.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                    href.push(char::from(byte));
                }
                _ => {
                    let high = char::from_digit(u32::from(byte >> 4u8), 16)
                        .unwrap_or('0')
                        .to_ascii_uppercase();
                    let low = char::from_digit(u32::from(byte & 0x0fu8), 16)
                        .unwrap_or('0')
                        .to_ascii_uppercase();
                    href.push('%');
                    href.push(high);
                    href.push(low);
                }
            }
        }
        href.push('&');
    }
    href.push_str("limit=");
    href.push_str(&limit.to_string());
    href.push_str("&offset=");
    href.push_str(&offset.to_string());
    href
}

/// Parse a music album identifier from a path segment.
pub(crate) fn parse_album_id(value: &str) -> Result<NumericID, ApiError> {
    value.parse::<NumericID>().map_err(|_| {
        // The malformed value and the parser detail never reach the log
        // (`OBS-003`); only the classification and its declared fields do
        // (`OBS-006`).
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "album_id",
            reason = "invalid_identifier",
            "rejected a malformed music album identifier"
        );
        ApiError::BadRequest("invalid music album identifier".to_owned())
    })
}

/// Render `value` with the representation's date format.
fn format_date(value: NaiveDate) -> String {
    value.format(DATE_FORMAT).to_string()
}

/// Render `value` with the representation's timestamp format.
fn format_timestamp(value: NaiveDateTime) -> String {
    value.format(TIMESTAMP_FORMAT).to_string()
}

/// Routes of the Library bounded context.
///
/// Every route requires an authenticated caller.
pub fn library_routes(state: &AppState) -> Router {
    let album: Router<AppState> = Router::new()
        .route("/music/album", get(get_album_list))
        .route("/music/album/{album_id}/track", get(get_album_track_list))
        .route(
            "/music/album/{album_id}/track/{track_id}",
            get(get_album_track),
        );

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
        .merge(album)
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
            .times(0..=1)
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
            .times(0..=1)
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
            .times(0..=1)
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
