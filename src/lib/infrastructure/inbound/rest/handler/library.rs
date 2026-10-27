pub mod delete_gallery;
pub mod delete_gallery_item;
pub mod delete_playlist;
pub mod delete_playlist_track;
pub mod get_album;
pub mod get_album_list;
pub mod get_album_medium;
pub mod get_album_track;
pub mod get_album_track_list;
pub mod get_gallery;
pub mod get_gallery_item;
pub mod get_gallery_item_list;
pub mod get_gallery_list;
pub mod get_movie;
pub mod get_movie_list;
pub mod get_playlist;
pub mod get_playlist_list;
pub mod get_playlist_track;
pub mod get_playlist_track_list;
pub mod get_series;
pub mod get_series_episode;
pub mod get_series_episode_list;
pub mod get_series_list;
pub mod get_series_season;
pub mod patch_playlist;
pub mod post_gallery;
pub mod post_gallery_item;
pub mod post_playlist;
pub mod put_playlist_track;

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
use crate::application::port::create_playlist::CreatePlaylistError;
use crate::application::port::delete_playlist::DeletePlaylistError;
use crate::application::port::get_album::GetAlbumError;
use crate::application::port::get_album::GetAlbumResponse as GetAlbumResponseData;
use crate::application::port::get_album_medium::GetAlbumMediumError;
use crate::application::port::get_album_medium::GetAlbumMediumResponse;
use crate::application::port::get_album_track::GetAlbumTrackError;
use crate::application::port::get_album_track::GetAlbumTrackResponse;
use crate::application::port::get_gallery::GalleryItemSummary;
use crate::application::port::get_gallery::GetGalleryResponse as GetGalleryResponseData;
use crate::application::port::get_gallery_item::GetGalleryItemResponse;
use crate::application::port::get_movie::GetMovieError;
use crate::application::port::get_movie::GetMovieResponse as GetMovieResponseData;
use crate::application::port::get_playlist::GetPlaylistError;
use crate::application::port::get_playlist_track::GetPlaylistTrackError;
use crate::application::port::get_series::GetSeriesError;
use crate::application::port::get_series::GetSeriesResponse as GetSeriesResponseData;
use crate::application::port::get_series_episode::GetSeriesEpisodeError;
use crate::application::port::get_series_episode::GetSeriesEpisodeResponse;
use crate::application::port::get_series_season::GetSeriesSeasonError;
use crate::application::port::get_series_season::GetSeriesSeasonResponse;
use crate::application::port::list_album_tracks::ListAlbumTracksError;
use crate::application::port::list_album_tracks::ListAlbumTracksItem;
use crate::application::port::list_albums::ListAlbumsError;
use crate::application::port::list_albums::ListAlbumsItem;
use crate::application::port::list_galleries::ListGalleriesItem;
use crate::application::port::list_movies::ListMoviesError;
use crate::application::port::list_movies::ListMoviesItem;
use crate::application::port::list_playlists::ListPlaylistsError;
use crate::application::port::list_playlists::ListPlaylistsItem;
use crate::application::port::list_series::ListSeriesError;
use crate::application::port::list_series::ListSeriesItem;
use crate::application::port::list_series_episodes::ListSeriesEpisodesError;
use crate::application::port::list_series_episodes::ListSeriesEpisodesItem;
use crate::application::port::remove_playlist_track::RemovePlaylistTrackError;
use crate::application::port::reorder_playlist_tracks::ReorderPlaylistTracksError;
use crate::application::port::update_playlist::UpdatePlaylistError;
use crate::application::port::video_summary::VideoSummary;
use crate::domain::alias::NumericID;
use crate::domain::model::gallery_item::GalleryItemMedia;
use crate::domain::model::music_album::ProductionType;
use crate::domain::model::music_playlist::MusicPlaylist;
use crate::domain::model::music_recording::MusicRecording;
use crate::domain::model::video::ScanType;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::Link;
use crate::infrastructure::inbound::rest::hal::SelfLinks;
use crate::infrastructure::inbound::rest::handler::library::delete_gallery::delete_gallery;
use crate::infrastructure::inbound::rest::handler::library::delete_gallery_item::delete_gallery_item;
use crate::infrastructure::inbound::rest::handler::library::delete_playlist::delete_playlist;
use crate::infrastructure::inbound::rest::handler::library::delete_playlist_track::delete_playlist_track;
use crate::infrastructure::inbound::rest::handler::library::get_album::get_album;
use crate::infrastructure::inbound::rest::handler::library::get_album_list::ListAlbumsQuery;
use crate::infrastructure::inbound::rest::handler::library::get_album_list::get_album_list;
use crate::infrastructure::inbound::rest::handler::library::get_album_medium::get_album_medium;
use crate::infrastructure::inbound::rest::handler::library::get_album_track::get_album_track;
use crate::infrastructure::inbound::rest::handler::library::get_album_track_list::get_album_track_list;
use crate::infrastructure::inbound::rest::handler::library::get_gallery::get_gallery;
use crate::infrastructure::inbound::rest::handler::library::get_gallery_item::get_gallery_item;
use crate::infrastructure::inbound::rest::handler::library::get_gallery_item_list::get_gallery_item_list;
use crate::infrastructure::inbound::rest::handler::library::get_gallery_list::get_gallery_list;
use crate::infrastructure::inbound::rest::handler::library::get_movie::get_movie;
use crate::infrastructure::inbound::rest::handler::library::get_movie_list::ListMoviesQuery;
use crate::infrastructure::inbound::rest::handler::library::get_movie_list::get_movie_list;
use crate::infrastructure::inbound::rest::handler::library::get_playlist::get_playlist;
use crate::infrastructure::inbound::rest::handler::library::get_playlist_list::ListPlaylistsQuery;
use crate::infrastructure::inbound::rest::handler::library::get_playlist_list::get_playlist_list;
use crate::infrastructure::inbound::rest::handler::library::get_playlist_track::get_playlist_track;
use crate::infrastructure::inbound::rest::handler::library::get_playlist_track_list::get_playlist_track_list;
use crate::infrastructure::inbound::rest::handler::library::get_series::get_series;
use crate::infrastructure::inbound::rest::handler::library::get_series_episode::get_series_episode;
use crate::infrastructure::inbound::rest::handler::library::get_series_episode_list::get_series_episode_list;
use crate::infrastructure::inbound::rest::handler::library::get_series_list::ListSeriesQuery;
use crate::infrastructure::inbound::rest::handler::library::get_series_list::get_series_list;
use crate::infrastructure::inbound::rest::handler::library::get_series_season::get_series_season;
use crate::infrastructure::inbound::rest::handler::library::patch_playlist::patch_playlist;
use crate::infrastructure::inbound::rest::handler::library::post_gallery::post_gallery;
use crate::infrastructure::inbound::rest::handler::library::post_gallery_item::post_gallery_item;
use crate::infrastructure::inbound::rest::handler::library::post_playlist::post_playlist;
use crate::infrastructure::inbound::rest::handler::library::put_playlist_track::put_playlist_track;
use crate::infrastructure::inbound::rest::middleware::auth::authenticate;
use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;

/// Default number of items a listing returns when `limit` is omitted.
pub(crate) const DEFAULT_LIMIT: usize = 20;

/// Largest number of items a listing may return.
pub(crate) const MAX_LIMIT: usize = 100;

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
/// The `self` link addresses the album; the album's mediums are embedded in the
/// representation and each medium carries its own links (`API-037`). A cover is
/// exposed as the `cover` link when the album stores artwork.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct AlbumLinks {
    /// Link to the album's cover file, when the album stores artwork.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover: Option<Link>,
    /// Link to the album resource itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

impl AlbumLinks {
    /// Build the links of the album identified by `album_id`.
    #[must_use]
    pub fn for_album(album_id: NumericID, cover_file_id: Option<NumericID>) -> Self {
        Self {
            cover: cover_file_id.map(|file_id| Link::new(&asset_image_href(file_id))),
            self_link: Link::new(&format!("/api/v1/library/music/album/{album_id}")),
        }
    }
}

/// HAL links exposed by a music medium representation.
///
/// The `self` link addresses the medium, the `album` link the album that holds
/// it, and the `tracks` link the medium's track collection (`API-034`,
/// `API-037`).
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct MediumLinks {
    /// Link to the album that holds the medium.
    pub album: Link,
    /// Link to the medium resource itself.
    #[serde(rename = "self")]
    pub self_link: Link,
    /// Link to the track collection of the medium.
    pub tracks: Link,
}

impl MediumLinks {
    /// Build the links of the medium identified by its album and index.
    #[must_use]
    pub fn for_medium(album_id: NumericID, medium_index: u8) -> Self {
        Self {
            album: Link::new(&format!("/api/v1/library/music/album/{album_id}")),
            self_link: Link::new(&format!(
                "/api/v1/library/music/album/{album_id}/medium/{medium_index}"
            )),
            tracks: Link::new(&format!(
                "/api/v1/library/music/album/{album_id}/medium/{medium_index}/track"
            )),
        }
    }
}

/// One medium of a music album.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct MediumResponse {
    /// Album-scoped index of the medium.
    pub index: u8,
    /// Links to the medium, its album and its track collection.
    #[serde(rename = "_links")]
    pub links: MediumLinks,
    /// Stored type of the medium (`CD`).
    #[serde(rename = "type")]
    pub medium_type: String,
    /// Number of recordings the medium sequences.
    pub track_count: usize,
}

impl MediumResponse {
    /// Assemble a medium representation from its parts.
    #[must_use]
    pub fn new(
        album_id: NumericID,
        medium_index: u8,
        medium_type: String,
        track_count: usize,
    ) -> Self {
        Self {
            index: medium_index,
            links: MediumLinks::for_medium(album_id, medium_index),
            track_count,
            medium_type,
        }
    }
}

impl From<GetAlbumMediumResponse> for MediumResponse {
    fn from(response: GetAlbumMediumResponse) -> Self {
        Self::new(
            response.album_id(),
            response.medium_index(),
            response.medium_type().to_owned(),
            response.track_count(),
        )
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

/// A music album representation.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct AlbumResponse {
    /// Identifier of the file holding the album's artwork, when set.
    pub cover_file_id: Option<NumericID>,
    /// Genres of the album, ordered by genre identifier.
    pub genres: Vec<String>,
    /// Unique identifier of the album.
    pub id: NumericID,
    /// Links to the album.
    #[serde(rename = "_links")]
    pub links: AlbumLinks,
    /// Mediums of the album, ordered by medium index.
    pub mediums: Vec<MediumResponse>,
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
            cover_file_id: item.cover_file_id(),
            genres: item.genres().to_vec(),
            id: item.album_id(),
            links: AlbumLinks::for_album(item.album_id(), item.cover_file_id()),
            mediums: item
                .mediums()
                .iter()
                .map(|medium| {
                    MediumResponse::new(
                        item.album_id(),
                        medium.medium_index(),
                        medium.medium_type().to_owned(),
                        medium.track_count(),
                    )
                })
                .collect(),
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

impl From<&GetAlbumResponseData> for AlbumResponse {
    fn from(response: &GetAlbumResponseData) -> Self {
        Self {
            cover_file_id: response.cover_file_id(),
            genres: response.genres().to_vec(),
            id: response.album_id(),
            links: AlbumLinks::for_album(response.album_id(), response.cover_file_id()),
            mediums: response
                .mediums()
                .iter()
                .map(|medium| {
                    MediumResponse::new(
                        response.album_id(),
                        medium.medium_index(),
                        medium.medium_type().to_owned(),
                        medium.track_count(),
                    )
                })
                .collect(),
            name: response.name().to_owned(),
            production_type: response
                .production_type()
                .map(ProductionType::as_wire)
                .map(str::to_owned),
            release_date: format_date(response.release_date()),
            track_count: response.track_count(),
        }
    }
}

/// HAL links exposed by a music track representation.
///
/// The `self` link addresses the track, the `album` link the album that holds
/// it, and the `medium` link the medium that sequences it (`API-034`,
/// `API-037`).
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct TrackLinks {
    /// Link to the album the track belongs to.
    pub album: Link,
    /// Link to the medium the track is placed in.
    pub medium: Link,
    /// Link to the track resource itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

impl TrackLinks {
    /// Build the links of a track identified by its album, medium and index.
    #[must_use]
    pub fn for_track(album_id: NumericID, medium_index: u8, track_index: u8) -> Self {
        Self {
            album: Link::new(&format!("/api/v1/library/music/album/{album_id}")),
            medium: Link::new(&format!(
                "/api/v1/library/music/album/{album_id}/medium/{medium_index}"
            )),
            self_link: Link::new(&format!(
                "/api/v1/library/music/album/{album_id}/medium/{medium_index}/track/{track_index}"
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

/// One music track of an album medium.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct TrackResponse {
    /// Identifier of the album the track belongs to.
    pub album_id: NumericID,
    /// Identifier of the backing file.
    pub file_id: NumericID,
    /// Date of the first release of the recording, in `YYYY-MM-DD` form.
    pub first_release_date: String,
    /// Track index: the position of the track within its medium.
    pub id: u8,
    /// ISRC code of the recording, when set.
    pub isrc_code: Option<String>,
    /// Links to the track, its album and its medium.
    #[serde(rename = "_links")]
    pub links: TrackLinks,
    /// Album-scoped index of the medium the track is placed in.
    pub medium_index: u8,
    /// Name of the recording, when it has one.
    pub name: Option<String>,
    /// Identifier of the shared recording the track places.
    pub recording_id: NumericID,
}

impl From<&ListAlbumTracksItem> for TrackResponse {
    fn from(item: &ListAlbumTracksItem) -> Self {
        Self {
            album_id: item.album_id(),
            file_id: item.file_id(),
            first_release_date: format_date(item.first_release_date()),
            id: item.track_index(),
            isrc_code: item.isrc_code().map(str::to_owned),
            links: TrackLinks::for_track(item.album_id(), item.medium_index(), item.track_index()),
            medium_index: item.medium_index(),
            name: item.name().map(str::to_owned),
            recording_id: item.recording_id(),
        }
    }
}

impl From<GetAlbumTrackResponse> for TrackResponse {
    fn from(response: GetAlbumTrackResponse) -> Self {
        Self {
            album_id: response.album_id(),
            file_id: response.file_id(),
            first_release_date: format_date(response.first_release_date()),
            id: response.track_index(),
            isrc_code: response.isrc_code().map(str::to_owned),
            links: TrackLinks::for_track(
                response.album_id(),
                response.medium_index(),
                response.track_index(),
            ),
            medium_index: response.medium_index(),
            name: response.name().map(str::to_owned),
            recording_id: response.recording_id(),
        }
    }
}

/// HAL links exposed by a film representation.
///
/// The `self` link addresses the film; the video is embedded in the
/// representation and carries no links of its own (`API-037`). A cover is
/// exposed as the `cover` link when the film stores artwork.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct MovieLinks {
    /// Link to the film's cover file, when the film stores artwork.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover: Option<Link>,
    /// Link to the film resource itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

impl MovieLinks {
    /// Build the links of the film identified by `movie_id`.
    #[must_use]
    pub fn for_movie(movie_id: NumericID, cover_file_id: Option<NumericID>) -> Self {
        Self {
            cover: cover_file_id.map(|file_id| Link::new(&asset_image_href(file_id))),
            self_link: Link::new(&format!("/api/v1/library/movie/{movie_id}")),
        }
    }
}

/// The video backing a film or an episode.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct VideoResponse {
    /// Codec of the video.
    pub codec: String,
    /// Duration of the video, in milliseconds.
    pub duration_ms: f64,
    /// Identifier of the backing file.
    pub file_id: NumericID,
    /// Height of the video, in pixels.
    pub height: i64,
    /// Unique identifier of the video.
    pub id: NumericID,
    /// Scan type of the video.
    pub scan_type: ScanType,
    /// Width of the video, in pixels.
    pub width: i64,
}

impl From<&VideoSummary> for VideoResponse {
    fn from(video: &VideoSummary) -> Self {
        Self {
            codec: video.codec().to_owned(),
            duration_ms: video.duration_ms(),
            file_id: video.file_id(),
            height: video.height(),
            id: video.id(),
            scan_type: video.scan_type(),
            width: video.width(),
        }
    }
}

/// A film representation.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct MovieResponse {
    /// Country of origin of the film.
    pub country_of_origin: String,
    /// Identifier of the file holding the film's artwork, when set.
    pub cover_file_id: Option<NumericID>,
    /// Genres of the film, ordered by genre identifier.
    pub genres: Vec<String>,
    /// Unique identifier of the film.
    pub id: NumericID,
    /// Links to the film.
    #[serde(rename = "_links")]
    pub links: MovieLinks,
    /// Name of the film.
    pub name: String,
    /// Plot of the film.
    pub plot: String,
    /// Date of the release of the film, in `YYYY-MM-DD` form.
    pub release_date: String,
    /// Video backing the film.
    pub video: VideoResponse,
}

impl From<&ListMoviesItem> for MovieResponse {
    fn from(item: &ListMoviesItem) -> Self {
        Self {
            cover_file_id: item.cover_file_id(),
            country_of_origin: item.country_of_origin().to_owned(),
            genres: item.genre_ids().to_vec(),
            id: item.id(),
            links: MovieLinks::for_movie(item.id(), item.cover_file_id()),
            name: item.name().to_owned(),
            plot: item.plot().to_owned(),
            release_date: format_date(item.release_date()),
            video: VideoResponse::from(item.video()),
        }
    }
}

impl From<&GetMovieResponseData> for MovieResponse {
    fn from(response: &GetMovieResponseData) -> Self {
        Self {
            cover_file_id: response.cover_file_id(),
            country_of_origin: response.country_of_origin().to_owned(),
            genres: response.genre_ids().to_vec(),
            id: response.id(),
            links: MovieLinks::for_movie(response.id(), response.cover_file_id()),
            name: response.name().to_owned(),
            plot: response.plot().to_owned(),
            release_date: format_date(response.release_date()),
            video: VideoResponse::from(response.video()),
        }
    }
}

/// HAL links exposed by a film collection representation.
///
/// The `self` link addresses the collection view the representation is about,
/// `next` and `prev` navigate that same view when another page exists, and the
/// templated `search` link exposes the collection filters (`API-037`,
/// `API-042`). The active filter and the page size are carried into every link
/// so a filtered or paginated listing stays navigable.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct MovieListLinks {
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

impl MovieListLinks {
    /// Build the links of a film collection page.
    ///
    /// `query` carries the active filter, `limit` and `offset` are the
    /// effective page, `returned` is the number of films in the page and
    /// `total` the number of matching films before pagination.
    #[must_use]
    pub fn for_page(
        query: &ListMoviesQuery,
        limit: usize,
        offset: usize,
        returned: usize,
        total: usize,
    ) -> Self {
        let next = (offset.saturating_add(returned) < total)
            .then(|| Link::new(&movie_page_href(query, limit, offset.saturating_add(limit))));
        let prev = (offset > 0)
            .then(|| Link::new(&movie_page_href(query, limit, offset.saturating_sub(limit))));
        Self {
            next,
            prev,
            search: Link::templated("/api/v1/library/movie{?name,limit,offset}"),
            self_link: Link::new(&movie_page_href(query, limit, offset)),
        }
    }
}

/// Films matching the requested filters.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct MovieListResponse {
    /// The films of the returned page.
    pub items: Vec<MovieResponse>,
    /// Navigation links of the collection.
    #[serde(rename = "_links")]
    pub links: MovieListLinks,
}

/// HAL links exposed by a show representation.
///
/// The `self` link addresses the show; the seasons are embedded and each
/// season carries its own links (`API-037`). A cover is exposed as the `cover`
/// link when the show stores artwork.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct SeriesLinks {
    /// Link to the show's cover file, when the show stores artwork.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover: Option<Link>,
    /// Link to the show resource itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

impl SeriesLinks {
    /// Build the links of the show identified by `series_id`.
    #[must_use]
    pub fn for_series(series_id: NumericID, cover_file_id: Option<NumericID>) -> Self {
        Self {
            cover: cover_file_id.map(|file_id| Link::new(&asset_image_href(file_id))),
            self_link: Link::new(&format!("/api/v1/library/series/{series_id}")),
        }
    }
}

/// HAL links exposed by a season representation.
///
/// The `self` link addresses the season, the `series` link the show that holds
/// it, and the `episodes` link the season's episode collection (`API-034`,
/// `API-037`). A cover is exposed as the `cover` link when the season stores
/// artwork.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct SeriesSeasonLinks {
    /// Link to the season's cover file, when the season stores artwork.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover: Option<Link>,
    /// Link to the episode collection of the season.
    pub episodes: Link,
    /// Link to the season resource itself.
    #[serde(rename = "self")]
    pub self_link: Link,
    /// Link to the show that holds the season.
    pub series: Link,
}

impl SeriesSeasonLinks {
    /// Build the links of a season identified by its show and index.
    #[must_use]
    pub fn for_season(
        series_id: NumericID,
        season_index: u32,
        cover_file_id: Option<NumericID>,
    ) -> Self {
        Self {
            cover: cover_file_id.map(|file_id| Link::new(&asset_image_href(file_id))),
            episodes: Link::new(&format!(
                "/api/v1/library/series/{series_id}/season/{season_index}/episode"
            )),
            self_link: Link::new(&format!(
                "/api/v1/library/series/{series_id}/season/{season_index}"
            )),
            series: Link::new(&format!("/api/v1/library/series/{series_id}")),
        }
    }
}

/// HAL links exposed by an episode representation.
///
/// The `self` link addresses the episode, the `series` link the show that holds
/// it, and the `season` link the season that sequences it (`API-034`,
/// `API-037`).
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct SeriesEpisodeLinks {
    /// Link to the season that sequences the episode.
    pub season: Link,
    /// Link to the episode resource itself.
    #[serde(rename = "self")]
    pub self_link: Link,
    /// Link to the show that holds the episode.
    pub series: Link,
}

impl SeriesEpisodeLinks {
    /// Build the links of an episode identified by its show, season and index.
    #[must_use]
    pub fn for_episode(series_id: NumericID, season_index: u32, episode_index: u32) -> Self {
        Self {
            season: Link::new(&format!(
                "/api/v1/library/series/{series_id}/season/{season_index}"
            )),
            self_link: Link::new(&format!(
                "/api/v1/library/series/{series_id}/season/{season_index}/episode/{episode_index}"
            )),
            series: Link::new(&format!("/api/v1/library/series/{series_id}")),
        }
    }
}

/// One season of a show.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct SeasonResponse {
    /// Identifier of the file holding the season's artwork, when set.
    pub cover_file_id: Option<NumericID>,
    /// Number of episodes the season holds.
    pub episode_count: usize,
    /// Series-scoped index of the season.
    pub index: u32,
    /// Links to the season, its show and its episode collection.
    #[serde(rename = "_links")]
    pub links: SeriesSeasonLinks,
    /// Name of the season, when set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Identifier of the show holding the season.
    pub series_id: NumericID,
}

impl SeasonResponse {
    /// Assemble a season representation from its parts.
    #[must_use]
    pub fn new(
        series_id: NumericID,
        index: u32,
        name: Option<String>,
        episode_count: usize,
        cover_file_id: Option<NumericID>,
    ) -> Self {
        Self {
            cover_file_id,
            episode_count,
            index,
            links: SeriesSeasonLinks::for_season(series_id, index, cover_file_id),
            name,
            series_id,
        }
    }
}

impl From<&GetSeriesSeasonResponse> for SeasonResponse {
    fn from(response: &GetSeriesSeasonResponse) -> Self {
        Self::new(
            response.series_id(),
            response.index(),
            response.name().map(str::to_owned),
            response.episode_count(),
            response.cover_file_id(),
        )
    }
}

/// A show representation: its metadata and season summaries.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct SeriesResponse {
    /// Country of origin of the show.
    pub country_of_origin: String,
    /// Identifier of the file holding the show's artwork, when set.
    pub cover_file_id: Option<NumericID>,
    /// Date of the first airing of the show, in `YYYY-MM-DD` form.
    pub first_air_date: String,
    /// Genres of the show, ordered by genre identifier.
    pub genres: Vec<String>,
    /// Unique identifier of the show.
    pub id: NumericID,
    /// Links to the show.
    #[serde(rename = "_links")]
    pub links: SeriesLinks,
    /// Name of the show.
    pub name: String,
    /// Plot of the show.
    pub plot: String,
    /// Seasons of the show, ordered by index.
    pub seasons: Vec<SeasonResponse>,
}

impl From<&GetSeriesResponseData> for SeriesResponse {
    fn from(response: &GetSeriesResponseData) -> Self {
        Self {
            cover_file_id: response.cover_file_id(),
            country_of_origin: response.country_of_origin().to_owned(),
            first_air_date: format_date(response.first_air_date()),
            genres: response.genre_ids().to_vec(),
            id: response.id(),
            links: SeriesLinks::for_series(response.id(), response.cover_file_id()),
            name: response.name().to_owned(),
            plot: response.plot().to_owned(),
            seasons: response
                .seasons()
                .iter()
                .map(|season| {
                    SeasonResponse::new(
                        response.id(),
                        season.index(),
                        season.name().map(str::to_owned),
                        season.episode_count(),
                        season.cover_file_id(),
                    )
                })
                .collect(),
        }
    }
}

/// One show of a listing: its identifier, name and cover.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct SeriesSummaryResponse {
    /// Identifier of the file holding the show's artwork, when set.
    pub cover_file_id: Option<NumericID>,
    /// Unique identifier of the show.
    pub id: NumericID,
    /// Links to the show.
    #[serde(rename = "_links")]
    pub links: SeriesLinks,
    /// Name of the show.
    pub name: String,
}

impl From<&ListSeriesItem> for SeriesSummaryResponse {
    fn from(item: &ListSeriesItem) -> Self {
        Self {
            cover_file_id: item.cover_file_id(),
            id: item.id(),
            links: SeriesLinks::for_series(item.id(), item.cover_file_id()),
            name: item.name().to_owned(),
        }
    }
}

/// HAL links exposed by a show collection representation.
///
/// The `self` link addresses the collection view the representation is about,
/// `next` and `prev` navigate that same view when another page exists, and the
/// templated `search` link exposes the collection filters (`API-037`,
/// `API-042`).
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct SeriesListLinks {
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

impl SeriesListLinks {
    /// Build the links of a show collection page.
    #[must_use]
    pub fn for_page(
        query: &ListSeriesQuery,
        limit: usize,
        offset: usize,
        returned: usize,
        total: usize,
    ) -> Self {
        let next = (offset.saturating_add(returned) < total).then(|| {
            Link::new(&series_page_href(
                query,
                limit,
                offset.saturating_add(limit),
            ))
        });
        let prev = (offset > 0).then(|| {
            Link::new(&series_page_href(
                query,
                limit,
                offset.saturating_sub(limit),
            ))
        });
        Self {
            next,
            prev,
            search: Link::templated("/api/v1/library/series{?name,limit,offset}"),
            self_link: Link::new(&series_page_href(query, limit, offset)),
        }
    }
}

/// Shows matching the requested filters.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct SeriesListResponse {
    /// The shows of the returned page.
    pub items: Vec<SeriesSummaryResponse>,
    /// Navigation links of the collection.
    #[serde(rename = "_links")]
    pub links: SeriesListLinks,
}

/// An episode representation.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct EpisodeResponse {
    /// Date of the first airing of the episode, in `YYYY-MM-DD` form.
    pub air_date: String,
    /// Season-scoped index of the episode.
    pub id: u32,
    /// Links to the episode, its show and its season.
    #[serde(rename = "_links")]
    pub links: SeriesEpisodeLinks,
    /// Name of the episode.
    pub name: String,
    /// Plot of the episode.
    pub plot: String,
    /// Series-scoped index of the season holding the episode.
    pub season_index: u32,
    /// Identifier of the show holding the episode.
    pub series_id: NumericID,
    /// Video backing the episode.
    pub video: VideoResponse,
}

impl From<&ListSeriesEpisodesItem> for EpisodeResponse {
    fn from(item: &ListSeriesEpisodesItem) -> Self {
        Self {
            air_date: format_date(item.air_date()),
            id: item.id(),
            links: SeriesEpisodeLinks::for_episode(
                item.series_id(),
                item.season_index(),
                item.id(),
            ),
            name: item.name().to_owned(),
            plot: item.plot().to_owned(),
            season_index: item.season_index(),
            series_id: item.series_id(),
            video: VideoResponse::from(item.video()),
        }
    }
}

impl From<&GetSeriesEpisodeResponse> for EpisodeResponse {
    fn from(response: &GetSeriesEpisodeResponse) -> Self {
        Self {
            air_date: format_date(response.air_date()),
            id: response.id(),
            links: SeriesEpisodeLinks::for_episode(
                response.series_id(),
                response.season_index(),
                response.id(),
            ),
            name: response.name().to_owned(),
            plot: response.plot().to_owned(),
            season_index: response.season_index(),
            series_id: response.series_id(),
            video: VideoResponse::from(response.video()),
        }
    }
}

/// Episodes of one season.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct EpisodeListResponse {
    /// The episodes of the season, ordered by index.
    pub items: Vec<EpisodeResponse>,
    /// Navigation links of the collection.
    #[serde(rename = "_links")]
    pub links: SelfLinks,
}

/// HAL links exposed by a music playlist representation.
///
/// The `self` link addresses the canonical playlist; the `tracks` link exposes
/// the alias route that serves the playlist with its embedded tracks
/// (`API-037`, `API-042`). A cover is exposed as the `cover` link when the
/// playlist stores artwork.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct PlaylistLinks {
    /// Link to the playlist's cover file, when the playlist stores artwork.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover: Option<Link>,
    /// Link to the playlist resource itself.
    #[serde(rename = "self")]
    pub self_link: Link,
    /// Link to the playlist's track alias.
    pub tracks: Link,
}

impl PlaylistLinks {
    /// Build the links of the playlist identified by `playlist_id`.
    #[must_use]
    pub fn for_playlist(playlist_id: NumericID, cover_file_id: Option<NumericID>) -> Self {
        Self {
            cover: cover_file_id.map(|file_id| Link::new(&asset_image_href(file_id))),
            self_link: Link::new(&format!("/api/v1/library/music/playlist/{playlist_id}")),
            tracks: Link::new(&format!(
                "/api/v1/library/music/playlist/{playlist_id}/track"
            )),
        }
    }
}

/// HAL links exposed by a playlist track representation.
///
/// The `self` link addresses the track, and the `playlist` link the playlist
/// that sequences it (`API-034`, `API-037`).
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct PlaylistTrackLinks {
    /// Link to the playlist the track belongs to.
    pub playlist: Link,
    /// Link to the track resource itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

impl PlaylistTrackLinks {
    /// Build the links of a track identified by its playlist and index.
    #[must_use]
    pub fn for_track(playlist_id: NumericID, track_index: u32) -> Self {
        Self {
            playlist: Link::new(&format!("/api/v1/library/music/playlist/{playlist_id}")),
            self_link: Link::new(&format!(
                "/api/v1/library/music/playlist/{playlist_id}/track/{track_index}"
            )),
        }
    }
}

/// One track of a music playlist.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct PlaylistTrackResponse {
    /// Identifier of the backing file.
    pub file_id: NumericID,
    /// Date of the first release of the recording, in `YYYY-MM-DD` form.
    pub first_release_date: String,
    /// Zero-based index of the track within the playlist.
    pub id: u32,
    /// ISRC code of the recording, when set.
    pub isrc_code: Option<String>,
    /// Links to the track and its playlist.
    #[serde(rename = "_links")]
    pub links: PlaylistTrackLinks,
    /// Name of the recording, when set.
    pub name: Option<String>,
    /// Identifier of the shared recording the track places.
    pub recording_id: NumericID,
}

impl PlaylistTrackResponse {
    /// Assemble a track representation from its parts.
    #[must_use]
    pub fn new(playlist_id: NumericID, track: &MusicRecording) -> Self {
        let index = track.index().playlist_index().unwrap_or_default();
        Self {
            file_id: track.file_id(),
            first_release_date: format_date(track.first_release_date()),
            id: index,
            isrc_code: track.isrc_code().map(str::to_owned),
            links: PlaylistTrackLinks::for_track(playlist_id, index),
            name: track.name().map(str::to_owned),
            recording_id: track.id(),
        }
    }
}

/// A music playlist representation.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct PlaylistResponse {
    /// Identifier of the file holding the playlist's artwork, when set.
    pub cover_file_id: Option<NumericID>,
    /// Date and time of the creation of the playlist.
    pub created_at: String,
    /// Unique identifier of the playlist.
    pub id: NumericID,
    /// Whether the playlist is visible to every user.
    pub is_public: bool,
    /// Date and time of the last modification of the playlist.
    pub last_modified_at: String,
    /// Links to the playlist and its track alias.
    #[serde(rename = "_links")]
    pub links: PlaylistLinks,
    /// Name of the playlist.
    pub name: String,
    /// Identifier of the owning user.
    pub owner_id: NumericID,
    /// Number of tracks the playlist holds.
    pub track_count: usize,
    /// Tracks of the playlist, ordered by index.
    pub tracks: Vec<PlaylistTrackResponse>,
}

impl PlaylistResponse {
    /// Assemble a playlist representation from a listed playlist item.
    #[must_use]
    pub fn from_listed(item: &ListPlaylistsItem) -> Self {
        Self {
            cover_file_id: item.cover_file_id(),
            created_at: format_timestamp(item.created_at()),
            id: item.id(),
            is_public: item.is_public(),
            last_modified_at: format_timestamp(item.last_modified_at()),
            links: PlaylistLinks::for_playlist(item.id(), item.cover_file_id()),
            name: item.name().to_owned(),
            owner_id: item.user_id(),
            track_count: item.track_count(),
            tracks: Vec::new(),
        }
    }

    /// Assemble a playlist representation from a hydrated playlist.
    #[must_use]
    pub fn new(playlist: &MusicPlaylist) -> Self {
        Self {
            cover_file_id: playlist.cover_file_id(),
            created_at: format_timestamp(playlist.created_at()),
            id: playlist.id(),
            is_public: playlist.is_public(),
            last_modified_at: format_timestamp(playlist.last_modified_at()),
            links: PlaylistLinks::for_playlist(playlist.id(), playlist.cover_file_id()),
            name: playlist.name().to_owned(),
            owner_id: playlist.user_id(),
            track_count: playlist.tracks().len(),
            tracks: playlist
                .tracks()
                .iter()
                .map(|track| PlaylistTrackResponse::new(playlist.id(), track))
                .collect(),
        }
    }
}

/// HAL links exposed by a music playlist collection representation.
///
/// The `self` link addresses the collection view the representation is about,
/// `next` and `prev` navigate that same view when another page exists, and the
/// templated `search` link exposes the collection filters (`API-037`,
/// `API-042`). The active filters and the page size are carried into every link
/// so a filtered or paginated listing stays navigable.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct PlaylistListLinks {
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

impl PlaylistListLinks {
    /// Build the links of a playlist collection page.
    #[must_use]
    pub fn for_page(
        query: &ListPlaylistsQuery,
        limit: usize,
        offset: usize,
        returned: usize,
        total: usize,
    ) -> Self {
        let next = (offset.saturating_add(returned) < total).then(|| {
            Link::new(&playlist_page_href(
                query,
                limit,
                offset.saturating_add(limit),
            ))
        });
        let prev = (offset > 0).then(|| {
            Link::new(&playlist_page_href(
                query,
                limit,
                offset.saturating_sub(limit),
            ))
        });
        Self {
            next,
            prev,
            search: Link::templated(
                "/api/v1/library/music/playlist{?name,is_public,owner_id,limit,offset}",
            ),
            self_link: Link::new(&playlist_page_href(query, limit, offset)),
        }
    }
}

/// Playlists matching the requested filters.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct PlaylistListResponse {
    /// The playlists of the returned page.
    pub items: Vec<PlaylistResponse>,
    /// Navigation links of the collection.
    #[serde(rename = "_links")]
    pub links: PlaylistListLinks,
}

/// Map a create-playlist error to its API error.
impl From<CreatePlaylistError> for ApiError {
    fn from(err: CreatePlaylistError) -> Self {
        match err {
            CreatePlaylistError::InvalidName | CreatePlaylistError::NameTooLong => {
                Self::UnprocessableEntity(err.to_string())
            }
            CreatePlaylistError::NameAlreadyExists => Self::Conflict(err.to_string()),
            CreatePlaylistError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            CreatePlaylistError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a list-playlists error to its API error.
impl From<ListPlaylistsError> for ApiError {
    fn from(err: ListPlaylistsError) -> Self {
        match err {
            ListPlaylistsError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            ListPlaylistsError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a fetch-playlist error to its API error.
impl From<GetPlaylistError> for ApiError {
    fn from(err: GetPlaylistError) -> Self {
        match err {
            GetPlaylistError::NoSuchPlaylist => Self::NotFound(err.to_string()),
            GetPlaylistError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetPlaylistError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a fetch-playlist-track error to its API error.
impl From<GetPlaylistTrackError> for ApiError {
    fn from(err: GetPlaylistTrackError) -> Self {
        match err {
            GetPlaylistTrackError::NoSuchPlaylist | GetPlaylistTrackError::NoSuchTrack => {
                Self::NotFound(err.to_string())
            }
            GetPlaylistTrackError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetPlaylistTrackError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map an update-playlist error to its API error.
impl From<UpdatePlaylistError> for ApiError {
    fn from(err: UpdatePlaylistError) -> Self {
        match err {
            UpdatePlaylistError::Conflict
            | UpdatePlaylistError::NameAlreadyExists
            | UpdatePlaylistError::RecordingAlreadyPresent => Self::Conflict(err.to_string()),
            UpdatePlaylistError::Forbidden => Self::Forbidden(err.to_string()),
            UpdatePlaylistError::InvalidName
            | UpdatePlaylistError::NameTooLong
            | UpdatePlaylistError::NoSuchRecording => Self::UnprocessableEntity(err.to_string()),
            UpdatePlaylistError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            UpdatePlaylistError::NoSuchPlaylist => Self::NotFound(err.to_string()),
            UpdatePlaylistError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a delete-playlist error to its API error.
impl From<DeletePlaylistError> for ApiError {
    fn from(err: DeletePlaylistError) -> Self {
        match err {
            DeletePlaylistError::Forbidden => Self::Forbidden(err.to_string()),
            DeletePlaylistError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            DeletePlaylistError::NoSuchPlaylist => Self::NotFound(err.to_string()),
            DeletePlaylistError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a remove-playlist-track error to its API error.
impl From<RemovePlaylistTrackError> for ApiError {
    fn from(err: RemovePlaylistTrackError) -> Self {
        match err {
            RemovePlaylistTrackError::NoSuchPlaylist | RemovePlaylistTrackError::NoSuchTrack => {
                Self::NotFound(err.to_string())
            }
            RemovePlaylistTrackError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            RemovePlaylistTrackError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a reorder-playlist-tracks error to its API error.
impl From<ReorderPlaylistTracksError> for ApiError {
    fn from(err: ReorderPlaylistTracksError) -> Self {
        match err {
            ReorderPlaylistTracksError::InvalidOrder => Self::UnprocessableEntity(err.to_string()),
            ReorderPlaylistTracksError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            ReorderPlaylistTracksError::NoSuchPlaylist => Self::NotFound(err.to_string()),
            ReorderPlaylistTracksError::Unknown(_) => Self::InternalServerError,
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

/// Map a fetch-album error to its API error.
impl From<GetAlbumError> for ApiError {
    fn from(err: GetAlbumError) -> Self {
        match err {
            GetAlbumError::NoSuchAlbum => Self::NotFound(err.to_string()),
            GetAlbumError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetAlbumError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a fetch-album-medium error to its API error.
impl From<GetAlbumMediumError> for ApiError {
    fn from(err: GetAlbumMediumError) -> Self {
        match err {
            GetAlbumMediumError::NoSuchAlbum | GetAlbumMediumError::NoSuchMedium => {
                Self::NotFound(err.to_string())
            }
            GetAlbumMediumError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetAlbumMediumError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a list-album-tracks error to its API error.
impl From<ListAlbumTracksError> for ApiError {
    fn from(err: ListAlbumTracksError) -> Self {
        match err {
            ListAlbumTracksError::NoSuchAlbum | ListAlbumTracksError::NoSuchMedium => {
                Self::NotFound(err.to_string())
            }
            ListAlbumTracksError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            ListAlbumTracksError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a fetch-album-track error to its API error.
impl From<GetAlbumTrackError> for ApiError {
    fn from(err: GetAlbumTrackError) -> Self {
        match err {
            GetAlbumTrackError::NoSuchAlbum
            | GetAlbumTrackError::NoSuchMedium
            | GetAlbumTrackError::NoSuchTrack => Self::NotFound(err.to_string()),
            GetAlbumTrackError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetAlbumTrackError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a list-films error to its API error.
impl From<ListMoviesError> for ApiError {
    fn from(err: ListMoviesError) -> Self {
        match err {
            ListMoviesError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            ListMoviesError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a fetch-film error to its API error.
impl From<GetMovieError> for ApiError {
    fn from(err: GetMovieError) -> Self {
        match err {
            GetMovieError::NoSuchMovie => Self::NotFound(err.to_string()),
            GetMovieError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetMovieError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a list-shows error to its API error.
impl From<ListSeriesError> for ApiError {
    fn from(err: ListSeriesError) -> Self {
        match err {
            ListSeriesError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            ListSeriesError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a fetch-show error to its API error.
impl From<GetSeriesError> for ApiError {
    fn from(err: GetSeriesError) -> Self {
        match err {
            GetSeriesError::NoSuchSeries => Self::NotFound(err.to_string()),
            GetSeriesError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetSeriesError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a fetch-season error to its API error.
impl From<GetSeriesSeasonError> for ApiError {
    fn from(err: GetSeriesSeasonError) -> Self {
        match err {
            GetSeriesSeasonError::NoSuchSeries | GetSeriesSeasonError::NoSuchSeason => {
                Self::NotFound(err.to_string())
            }
            GetSeriesSeasonError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetSeriesSeasonError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a list-episodes error to its API error.
impl From<ListSeriesEpisodesError> for ApiError {
    fn from(err: ListSeriesEpisodesError) -> Self {
        match err {
            ListSeriesEpisodesError::NoSuchSeries | ListSeriesEpisodesError::NoSuchSeason => {
                Self::NotFound(err.to_string())
            }
            ListSeriesEpisodesError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            ListSeriesEpisodesError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a fetch-episode error to its API error.
impl From<GetSeriesEpisodeError> for ApiError {
    fn from(err: GetSeriesEpisodeError) -> Self {
        match err {
            GetSeriesEpisodeError::NoSuchSeries
            | GetSeriesEpisodeError::NoSuchSeason
            | GetSeriesEpisodeError::NoSuchEpisode => Self::NotFound(err.to_string()),
            GetSeriesEpisodeError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetSeriesEpisodeError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Canonical URI reference of the gallery item identified by `item_id`.
#[must_use]
pub(crate) fn gallery_item_self_href(gallery_id: NumericID, item_id: NumericID) -> String {
    format!("/api/v1/library/gallery/{gallery_id}/item/{item_id}")
}

/// Build the Asset image href of the file identified by `file_id`.
///
/// TODO(add-cover-art): the Asset image route is not served yet; revisit this
/// helper when `/api/v1/asset/image/{file_id}` lands.
#[must_use]
fn asset_image_href(file_id: NumericID) -> String {
    format!("/api/v1/asset/image/{file_id}")
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

/// Build the href of the playlist collection view at `limit`/`offset`,
/// preserving the active filters so a filtered page keeps its navigation
/// (`API-037`).
fn playlist_page_href(query: &ListPlaylistsQuery, limit: usize, offset: usize) -> String {
    let mut href = String::from("/api/v1/library/music/playlist?");
    if let Some(name) = query.name.as_deref() {
        href.push_str("name=");
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
    if let Some(is_public) = query.is_public {
        href.push_str("is_public=");
        href.push_str(if is_public { "true" } else { "false" });
        href.push('&');
    }
    if let Some(owner_id) = query.owner_id {
        href.push_str("owner_id=");
        href.push_str(&owner_id.to_string());
        href.push('&');
    }
    href.push_str("limit=");
    href.push_str(&limit.to_string());
    href.push_str("&offset=");
    href.push_str(&offset.to_string());
    href
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

/// Build the href of the film collection view at `limit`/`offset`, preserving
/// the active filter so a filtered page keeps its navigation (`API-037`).
fn movie_page_href(query: &ListMoviesQuery, limit: usize, offset: usize) -> String {
    let mut href = String::from("/api/v1/library/movie?");
    push_name_filter(&mut href, query.name.as_deref());
    href.push_str("limit=");
    href.push_str(&limit.to_string());
    href.push_str("&offset=");
    href.push_str(&offset.to_string());
    href
}

/// Build the href of the show collection view at `limit`/`offset`, preserving
/// the active filter so a filtered page keeps its navigation (`API-037`).
fn series_page_href(query: &ListSeriesQuery, limit: usize, offset: usize) -> String {
    let mut href = String::from("/api/v1/library/series?");
    push_name_filter(&mut href, query.name.as_deref());
    href.push_str("limit=");
    href.push_str(&limit.to_string());
    href.push_str("&offset=");
    href.push_str(&offset.to_string());
    href
}

/// Append the percent-encoded `name` filter to `href`, when one is active.
///
/// The unreserved character set keeps the value inside the query string, so it
/// cannot break out of it.
fn push_name_filter(href: &mut String, name: Option<&str>) {
    let Some(filter) = name else {
        return;
    };
    href.push_str("name=");
    for byte in filter.bytes() {
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

/// Parse a `u32` season index from a path segment.
pub(crate) fn parse_season_index(value: &str) -> Result<u32, ApiError> {
    value.parse::<u32>().map_err(|_| {
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "season_index",
            reason = "invalid_index",
            "rejected a malformed season index"
        );
        ApiError::BadRequest("invalid season index".to_owned())
    })
}

/// Parse a show identifier from a path segment.
pub(crate) fn parse_series_id(value: &str) -> Result<NumericID, ApiError> {
    value.parse::<NumericID>().map_err(|_| {
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "series_id",
            reason = "invalid_identifier",
            "rejected a malformed show identifier"
        );
        ApiError::BadRequest("invalid show identifier".to_owned())
    })
}

/// Parse a `u8` music index from a path segment.
///
/// `field` is the declared security-event field and `message` the stable,
/// handler-owned rejection message; the malformed value and the parser detail
/// never reach the log (`OBS-003`, `OBS-006`).
pub(crate) fn parse_index(
    value: &str,
    field: &'static str,
    message: &'static str,
) -> Result<u8, ApiError> {
    value.parse::<u8>().map_err(|_| {
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = field,
            reason = "invalid_index",
            "rejected a malformed music index"
        );
        ApiError::BadRequest(message.to_owned())
    })
}

/// Parse a playlist identifier from a path segment.
pub(crate) fn parse_playlist_id(value: &str) -> Result<NumericID, ApiError> {
    value.parse::<NumericID>().map_err(|_| {
        // The malformed value and the parser detail never reach the log
        // (`OBS-003`); only the classification and its declared fields do
        // (`OBS-006`).
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "playlist_id",
            reason = "invalid_identifier",
            "rejected a malformed music playlist identifier"
        );
        ApiError::BadRequest("invalid music playlist identifier".to_owned())
    })
}

/// Parse a zero-based playlist track index from a path segment.
pub(crate) fn parse_playlist_track_index(value: &str) -> Result<u32, ApiError> {
    value.parse::<u32>().map_err(|_| {
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "track_index",
            reason = "invalid_index",
            "rejected a malformed playlist track index"
        );
        ApiError::BadRequest("invalid playlist track index".to_owned())
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
        .route("/music/album/{album_id}", get(get_album))
        .route(
            "/music/album/{album_id}/medium/{medium_index}",
            get(get_album_medium),
        )
        .route(
            "/music/album/{album_id}/medium/{medium_index}/track",
            get(get_album_track_list),
        )
        .route(
            "/music/album/{album_id}/medium/{medium_index}/track/{track_index}",
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

    let movie: Router<AppState> = Router::new()
        .route("/movie", get(get_movie_list))
        .route("/movie/{movie_id}", get(get_movie));

    let series: Router<AppState> = Router::new()
        .route("/series", get(get_series_list))
        .route("/series/{series_id}", get(get_series))
        .route(
            "/series/{series_id}/season/{season_index}",
            get(get_series_season),
        )
        .route(
            "/series/{series_id}/season/{season_index}/episode",
            get(get_series_episode_list),
        )
        .route(
            "/series/{series_id}/season/{season_index}/episode/{episode_index}",
            get(get_series_episode),
        );

    let playlist: Router<AppState> = Router::new()
        .route(
            "/music/playlist",
            get(get_playlist_list).post(post_playlist),
        )
        .route(
            "/music/playlist/{playlist_id}",
            get(get_playlist)
                .patch(patch_playlist)
                .delete(delete_playlist),
        )
        .route(
            "/music/playlist/{playlist_id}/track",
            get(get_playlist_track_list).put(put_playlist_track),
        )
        .route(
            "/music/playlist/{playlist_id}/track/{track_index}",
            get(get_playlist_track).delete(delete_playlist_track),
        );

    gallery
        .merge(album)
        .merge(movie)
        .merge(playlist)
        .merge(series)
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

    use super::asset_image_href;
    use super::library_routes;
    use crate::application::port::create_gallery::CreateGalleryResponse;
    use crate::application::port::create_gallery::CreateGalleryUseCase;
    use crate::application::port::create_gallery::MockCreateGalleryUseCase;
    use crate::application::port::delete_gallery::DeleteGalleryUseCase;
    use crate::application::port::delete_gallery::MockDeleteGalleryUseCase;
    use crate::application::port::get_gallery::GetGalleryResponse;
    use crate::application::port::get_gallery::GetGalleryUseCase;
    use crate::application::port::get_gallery::MockGetGalleryUseCase;
    use crate::application::port::get_movie::GetMovieResponse;
    use crate::application::port::get_movie::GetMovieUseCase;
    use crate::application::port::get_movie::MockGetMovieUseCase;
    use crate::application::port::get_series::GetSeriesResponse;
    use crate::application::port::get_series::GetSeriesUseCase;
    use crate::application::port::get_series::MockGetSeriesUseCase;
    use crate::application::port::get_series_episode::GetSeriesEpisodeResponse;
    use crate::application::port::get_series_episode::GetSeriesEpisodeUseCase;
    use crate::application::port::get_series_episode::MockGetSeriesEpisodeUseCase;
    use crate::application::port::get_series_season::GetSeriesSeasonResponse;
    use crate::application::port::get_series_season::GetSeriesSeasonUseCase;
    use crate::application::port::get_series_season::MockGetSeriesSeasonUseCase;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::list_movies::ListMoviesResponse;
    use crate::application::port::list_movies::ListMoviesUseCase;
    use crate::application::port::list_movies::MockListMoviesUseCase;
    use crate::application::port::list_series::ListSeriesResponse;
    use crate::application::port::list_series::ListSeriesUseCase;
    use crate::application::port::list_series::MockListSeriesUseCase;
    use crate::application::port::list_series_episodes::ListSeriesEpisodesResponse;
    use crate::application::port::list_series_episodes::ListSeriesEpisodesUseCase;
    use crate::application::port::list_series_episodes::MockListSeriesEpisodesUseCase;
    use crate::application::port::video_summary::VideoSummary;
    use crate::domain::port::token_provider::TokenProvider as _;
    use crate::infrastructure::inbound::rest::app_state::AppState;
    use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;
    use crate::test_helpers::TEST_JWT_SECRET;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::sample_episode;
    use crate::test_helpers::sample_movie;
    use crate::test_helpers::video_date;

    /// A fixed instant the fixtures timestamp with.
    fn ts() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 6)
            .unwrap_or_default()
            .and_hms_opt(12, 0, 0)
            .unwrap_or_default()
    }

    #[tokio::test]
    async fn asset_image_href_addresses_the_asset_image_route() {
        // Act
        let href = asset_image_href(42);

        // Assert
        assert_eq!(
            href, "/api/v1/asset/image/42",
            "the cover link must name the Asset image route"
        );
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

    /// Drive one route through the real router and return the response.
    async fn reach(state: &AppState, uri: &str) -> Result<Response, Box<dyn Error>> {
        let request = Request::builder()
            .method("GET")
            .uri(uri)
            .header(header::AUTHORIZATION, format!("Bearer {}", bearer().await?))
            .body(Body::empty())?;
        Ok(library_routes(state).oneshot(request).await?)
    }

    #[tokio::test]
    async fn library_routes_movie_list_route_reaches_handler() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListMoviesUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Ok(ListMoviesResponse::new(Vec::new(), 0)) }));
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_list_movies()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn ListMoviesUseCase>);
        let state = app_state_with_library(Arc::new(factory))?;

        // Act
        let response = reach(&state, "/movie").await?;

        // Assert
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "the film collection route must reach its handler"
        );
        Ok(())
    }

    #[tokio::test]
    async fn library_routes_movie_route_reaches_handler() -> Result<(), Box<dyn Error>> {
        // Arrange
        let movie = sample_movie(3)?;
        let response_data = GetMovieResponse::new(
            movie.id(),
            movie.name().to_owned(),
            movie.plot().to_owned(),
            movie.country_of_origin().to_owned(),
            movie.release_date(),
            movie.genre_ids().to_vec(),
            None,
            VideoSummary::from(movie.video()),
        );
        let mut use_case = MockGetMovieUseCase::new();
        use_case.expect_execute().times(1).return_once(move |_| {
            let copy = response_data.clone();
            Box::pin(async move { Ok(copy) })
        });
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_get_movie()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn GetMovieUseCase>);
        let state = app_state_with_library(Arc::new(factory))?;

        // Act
        let response = reach(&state, "/movie/3").await?;

        // Assert
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "the film route must reach its handler"
        );
        Ok(())
    }

    #[tokio::test]
    async fn library_routes_series_list_route_reaches_handler() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListSeriesUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Ok(ListSeriesResponse::new(Vec::new(), 0)) }));
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_list_series()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn ListSeriesUseCase>);
        let state = app_state_with_library(Arc::new(factory))?;

        // Act
        let response = reach(&state, "/series").await?;

        // Assert
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "the show collection route must reach its handler"
        );
        Ok(())
    }

    #[tokio::test]
    async fn library_routes_series_route_reaches_handler() -> Result<(), Box<dyn Error>> {
        // Arrange
        let response_data = GetSeriesResponse::new(
            3,
            "Twin Peaks".to_owned(),
            "A plot".to_owned(),
            "US".to_owned(),
            video_date(),
            Vec::new(),
            None,
            Vec::new(),
        );
        let mut use_case = MockGetSeriesUseCase::new();
        use_case.expect_execute().times(1).return_once(move |_| {
            let copy = response_data.clone();
            Box::pin(async move { Ok(copy) })
        });
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_get_series()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn GetSeriesUseCase>);
        let state = app_state_with_library(Arc::new(factory))?;

        // Act
        let response = reach(&state, "/series/3").await?;

        // Assert
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "the show route must reach its handler"
        );
        Ok(())
    }

    #[tokio::test]
    async fn library_routes_series_season_route_reaches_handler() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetSeriesSeasonUseCase::new();
        use_case.expect_execute().times(1).return_once(|_| {
            Box::pin(async { Ok(GetSeriesSeasonResponse::new(3, 1, None, 0, None)) })
        });
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_get_series_season()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn GetSeriesSeasonUseCase>);
        let state = app_state_with_library(Arc::new(factory))?;

        // Act
        let response = reach(&state, "/series/3/season/1").await?;

        // Assert
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "the season route must reach its handler"
        );
        Ok(())
    }

    #[tokio::test]
    async fn library_routes_series_episode_list_route_reaches_handler() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut use_case = MockListSeriesEpisodesUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Ok(ListSeriesEpisodesResponse::new(Vec::new())) }));
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_list_series_episodes()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn ListSeriesEpisodesUseCase>);
        let state = app_state_with_library(Arc::new(factory))?;

        // Act
        let response = reach(&state, "/series/3/season/1/episode").await?;

        // Assert
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "the episode collection route must reach its handler"
        );
        Ok(())
    }

    #[tokio::test]
    async fn library_routes_series_episode_route_reaches_handler() -> Result<(), Box<dyn Error>> {
        // Arrange
        let episode = sample_episode(1, 11)?;
        let response_data = GetSeriesEpisodeResponse::new(
            episode.index().value(),
            3,
            1,
            episode.name().to_owned(),
            episode.plot().to_owned(),
            episode.air_date(),
            VideoSummary::from(episode.video()),
        );
        let mut use_case = MockGetSeriesEpisodeUseCase::new();
        use_case.expect_execute().times(1).return_once(move |_| {
            let copy = response_data.clone();
            Box::pin(async move { Ok(copy) })
        });
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_get_series_episode()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn GetSeriesEpisodeUseCase>);
        let state = app_state_with_library(Arc::new(factory))?;

        // Act
        let response = reach(&state, "/series/3/season/1/episode/1").await?;

        // Assert
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "the episode route must reach its handler"
        );
        Ok(())
    }
}
