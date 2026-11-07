/// The library context endpoints.
pub(crate) mod delete_gallery;
/// The library context endpoints.
pub(crate) mod delete_gallery_item;
/// The library context endpoints.
pub(crate) mod delete_playlist;
/// The library context endpoints.
pub(crate) mod delete_playlist_track;
/// The library context endpoints.
pub(crate) mod get_album;
/// The library context endpoints.
pub(crate) mod get_album_list;
/// The library context endpoints.
pub(crate) mod get_album_medium;
/// The library context endpoints.
pub(crate) mod get_album_track;
/// The library context endpoints.
pub(crate) mod get_album_track_list;
/// The library context endpoints.
pub(crate) mod get_gallery;
/// The library context endpoints.
pub(crate) mod get_gallery_item;
/// The library context endpoints.
pub(crate) mod get_gallery_item_list;
/// The library context endpoints.
pub(crate) mod get_gallery_list;
/// The library context endpoints.
pub(crate) mod get_movie;
/// The library context endpoints.
pub(crate) mod get_movie_identify;
/// The library context endpoints.
pub(crate) mod get_movie_identify_metadata;
/// The library context endpoints.
pub(crate) mod get_movie_list;
/// The library context endpoints.
pub(crate) mod get_music_group;
/// The library context endpoints.
pub(crate) mod get_music_group_list;
/// The library context endpoints.
pub(crate) mod get_music_identify;
/// The library context endpoints.
pub(crate) mod get_music_identify_metadata;
/// The library context endpoints.
pub(crate) mod get_playlist;
/// The library context endpoints.
pub(crate) mod get_playlist_list;
/// The library context endpoints.
pub(crate) mod get_playlist_track;
/// The library context endpoints.
pub(crate) mod get_playlist_track_list;
/// The library context endpoints.
pub(crate) mod get_series;
/// The library context endpoints.
pub(crate) mod get_series_episode;
/// The library context endpoints.
pub(crate) mod get_series_episode_list;
/// The library context endpoints.
pub(crate) mod get_series_identify;
/// The library context endpoints.
pub(crate) mod get_series_identify_metadata;
/// The library context endpoints.
pub(crate) mod get_series_list;
/// The library context endpoints.
pub(crate) mod get_series_season;
/// The library context endpoints.
pub(crate) mod patch_album;
/// The library context endpoints.
pub(crate) mod patch_album_track;
/// The library context endpoints.
pub(crate) mod patch_movie;
/// The library context endpoints.
pub(crate) mod patch_music_group;
/// The library context endpoints.
pub(crate) mod patch_playlist;
/// The library context endpoints.
pub(crate) mod patch_series;
/// The library context endpoints.
pub(crate) mod patch_series_episode;
/// The library context endpoints.
pub(crate) mod patch_series_season;
/// The library context endpoints.
pub(crate) mod post_album;
/// The library context endpoints.
pub(crate) mod post_album_apply;
/// The library context endpoints.
pub(crate) mod post_gallery;
/// The library context endpoints.
pub(crate) mod post_gallery_item;
/// The library context endpoints.
pub(crate) mod post_movie;
/// The library context endpoints.
pub(crate) mod post_movie_apply;
/// The library context endpoints.
pub(crate) mod post_music_group;
/// The library context endpoints.
pub(crate) mod post_playlist;
/// The library context endpoints.
pub(crate) mod post_series;
/// The library context endpoints.
pub(crate) mod post_series_apply;
/// The library context endpoints.
pub(crate) mod post_series_episode_apply;
/// The library context endpoints.
pub(crate) mod put_album_medium;
/// The library context endpoints.
pub(crate) mod put_album_track;
/// The library context endpoints.
pub(crate) mod put_playlist_track;
/// The library context endpoints.
pub(crate) mod put_series_episode;
/// The library context endpoints.
pub(crate) mod put_series_season;

use axum::middleware;
use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use tracing::warn;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::application::field_update::FieldUpdate;
use crate::application::port::add_gallery_item::AddGalleryItemResponse;
use crate::application::port::apply_movie::ApplyMovieError;
use crate::application::port::apply_music_album::ApplyMusicAlbumError;
use crate::application::port::apply_series::ApplySeriesError;
use crate::application::port::apply_series_episode::ApplySeriesEpisodeError;
use crate::application::port::create_gallery::CreateGalleryResponse;
use crate::application::port::create_movie::CreateMovieError;
use crate::application::port::create_music_album::CreateMusicAlbumError;
use crate::application::port::create_music_group::CreateMusicGroupError;
use crate::application::port::create_playlist::CreatePlaylistError;
use crate::application::port::create_series::CreateSeriesError;
use crate::application::port::delete_playlist::DeletePlaylistError;
use crate::application::port::get_album::{
    GetAlbumError, GetAlbumResponse as GetAlbumResponseData,
};
use crate::application::port::get_album_medium::{GetAlbumMediumError, GetAlbumMediumResponse};
use crate::application::port::get_album_track::{GetAlbumTrackError, GetAlbumTrackResponse};
use crate::application::port::get_gallery::{
    GalleryItemSummary, GetGalleryResponse as GetGalleryResponseData,
};
use crate::application::port::get_gallery_item::GetGalleryItemResponse;
use crate::application::port::get_movie::{
    GetMovieError, GetMovieResponse as GetMovieResponseData,
};
use crate::application::port::get_music_group::{
    GetMusicGroupError, GetMusicGroupResponse as GetMusicGroupResponseData,
};
use crate::application::port::get_playlist::GetPlaylistError;
use crate::application::port::get_playlist_track::GetPlaylistTrackError;
use crate::application::port::get_series::{
    GetSeriesError, GetSeriesResponse as GetSeriesResponseData,
};
use crate::application::port::get_series_episode::{
    GetSeriesEpisodeError, GetSeriesEpisodeResponse,
};
use crate::application::port::get_series_season::{GetSeriesSeasonError, GetSeriesSeasonResponse};
use crate::application::port::identify_music::IdentifyMusicError;
use crate::application::port::identify_video::IdentifyVideoError;
use crate::application::port::list_album_tracks::{ListAlbumTracksError, ListAlbumTracksItem};
use crate::application::port::list_albums::{ListAlbumsError, ListAlbumsItem};
use crate::application::port::list_galleries::ListGalleriesItem;
use crate::application::port::list_movies::{ListMoviesError, ListMoviesItem};
use crate::application::port::list_music_groups::{ListMusicGroupsError, ListMusicGroupsItem};
use crate::application::port::list_playlists::{ListPlaylistsError, ListPlaylistsItem};
use crate::application::port::list_series::{ListSeriesError, ListSeriesItem};
use crate::application::port::list_series_episodes::{
    ListSeriesEpisodesError, ListSeriesEpisodesItem,
};
use crate::application::port::patch_album_track::PatchAlbumTrackError;
use crate::application::port::put_album_medium::PutAlbumMediumError;
use crate::application::port::put_album_track::PutAlbumTrackError;
use crate::application::port::put_series_episode::PutSeriesEpisodeError;
use crate::application::port::put_series_season::PutSeriesSeasonError;
use crate::application::port::remove_playlist_track::RemovePlaylistTrackError;
use crate::application::port::reorder_playlist_tracks::ReorderPlaylistTracksError;
use crate::application::port::resolve_music_metadata::ResolveMusicMetadataError;
use crate::application::port::resolve_video_metadata::ResolveVideoMetadataError;
use crate::application::port::update_movie::UpdateMovieError;
use crate::application::port::update_music_album::UpdateMusicAlbumError;
use crate::application::port::update_music_group::UpdateMusicGroupError;
use crate::application::port::update_playlist::UpdatePlaylistError;
use crate::application::port::update_series::UpdateSeriesError;
use crate::application::port::update_series_episode::UpdateSeriesEpisodeError;
use crate::application::port::update_series_season::UpdateSeriesSeasonError;
use crate::domain::alias::NumericID;
use crate::domain::model::episode::Episode;
use crate::domain::model::external_ref::ExternalRef;
use crate::domain::model::gallery_item::GalleryItemMedia;
use crate::domain::model::media_metadata::MediaMetadata;
use crate::domain::model::media_query::VideoKind;
use crate::domain::model::movie::Movie;
use crate::domain::model::music_album::{MusicAlbum, ProductionType};
use crate::domain::model::music_metadata::{ArtistCredit, MediumFormat, MediumSummary};
use crate::domain::model::music_playlist::MusicPlaylist;
use crate::domain::model::music_recording::MusicRecording;
use crate::domain::model::series::Series;
use crate::domain::model::video::{ScanType, Video};
use crate::domain::model::video_candidate::Candidate;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::{Link, SelfLinks};
use crate::infrastructure::inbound::rest::handler::asset::asset_image_href;
use crate::infrastructure::inbound::rest::handler::library::get_album_list::ListAlbumsQuery;
use crate::infrastructure::inbound::rest::handler::library::get_movie_list::ListMoviesQuery;
use crate::infrastructure::inbound::rest::handler::library::get_music_group_list::ListMusicGroupsQuery;
use crate::infrastructure::inbound::rest::handler::library::get_playlist_list::ListPlaylistsQuery;
use crate::infrastructure::inbound::rest::handler::library::get_series_list::ListSeriesQuery;
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
pub(crate) const DATE_FORMAT: &str = "%Y-%m-%d";

/// HAL links exposed by a gallery representation.
///
/// The `self` link addresses the gallery; the `items` link exposes the alias
/// path where its items are served.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct GalleryLinks {
    /// Link to the item collection of the gallery.
    pub items: Link,
    /// Link to the gallery resource itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

impl GalleryLinks {
    /// Build the links of the gallery identified by `gallery_id`.
    #[must_use]
    fn for_gallery(gallery_id: NumericID) -> Self {
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
pub(crate) enum GalleryItemType {
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
    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        match value {
            "image" => Some(Self::Image),
            "video" => Some(Self::Video),
            _ => None,
        }
    }

    /// Build the medium reference of `media_id` for this kind.
    #[must_use]
    pub(crate) fn media(self, media_id: NumericID) -> GalleryItemMedia {
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
pub(crate) struct GalleryItemResponse {
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
pub(crate) struct GalleryItemDetailResponse {
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
pub(crate) struct GalleryResponse {
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
/// representation and each medium carries its own links. A cover is exposed as
/// the `cover` link when the album stores artwork, and the group the album
/// belongs to is exposed as the `group` link.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct AlbumLinks {
    /// Link to the album's cover image, when the album stores artwork.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover: Option<Link>,
    /// Link to the music group the album belongs to.
    pub group: Link,
    /// Link to the album resource itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

impl AlbumLinks {
    /// Build the links of the album identified by `album_id`.
    #[must_use]
    fn for_album(
        album_id: NumericID,
        cover_image_id: Option<NumericID>,
        music_group_id: NumericID,
    ) -> Self {
        Self {
            cover: cover_image_id.map(|image_id| Link::new(&asset_image_href(image_id))),
            group: Link::new(&format!("/api/v1/library/music/group/{music_group_id}")),
            self_link: Link::new(&format!("/api/v1/library/music/album/{album_id}")),
        }
    }
}

/// HAL links exposed by a music medium representation.
///
/// The `self` link addresses the medium, the `album` link the album that holds
/// it, and the `tracks` link the medium's track collection.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct MediumLinks {
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
    fn for_medium(album_id: NumericID, medium_index: u8) -> Self {
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
pub(crate) struct MediumResponse {
    /// Album-scoped index of the medium.
    pub index: u8,
    /// Links to the medium, its album and its track collection.
    #[serde(rename = "_links")]
    pub links: MediumLinks,
    /// Stored type of the medium (`CD` or `DIGITAL`).
    #[serde(rename = "type")]
    pub medium_type: String,
    /// Number of recordings the medium sequences.
    pub track_count: usize,
}

impl MediumResponse {
    /// Assemble a medium representation from its parts.
    #[must_use]
    pub(crate) fn new(
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
/// templated `search` link exposes the collection filters. The active filter
/// and the page size are carried into every link so a filtered or paginated
/// listing stays navigable.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct AlbumListLinks {
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
    pub(crate) fn for_page(
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
pub(crate) struct AlbumListResponse {
    /// The albums of the returned page.
    pub items: Vec<AlbumResponse>,
    /// Navigation links of the collection.
    #[serde(rename = "_links")]
    pub links: AlbumListLinks,
}

/// A music album representation.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct AlbumResponse {
    /// Identifier of the image holding the album's artwork, when set.
    pub cover_image_id: Option<NumericID>,
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
    /// Date of the release of the album, in `YYYY-MM-DD` form, or `null` when
    /// the provider reports none.
    pub release_date: Option<String>,
    /// Number of tracks the album contains.
    pub track_count: usize,
}

impl From<&ListAlbumsItem> for AlbumResponse {
    fn from(item: &ListAlbumsItem) -> Self {
        Self {
            cover_image_id: item.cover_image_id(),
            genres: item.genres().to_vec(),
            id: item.album_id(),
            links: AlbumLinks::for_album(
                item.album_id(),
                item.cover_image_id(),
                item.music_group_id(),
            ),
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
            release_date: item.release_date().map(format_date),
            track_count: item.track_count(),
        }
    }
}

impl From<&GetAlbumResponseData> for AlbumResponse {
    fn from(response: &GetAlbumResponseData) -> Self {
        Self {
            cover_image_id: response.cover_image_id(),
            genres: response.genres().to_vec(),
            id: response.album_id(),
            links: AlbumLinks::for_album(
                response.album_id(),
                response.cover_image_id(),
                response.music_group_id(),
            ),
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
            release_date: response.release_date().map(format_date),
            track_count: response.track_count(),
        }
    }
}

impl From<&MusicAlbum> for AlbumResponse {
    fn from(album: &MusicAlbum) -> Self {
        let track_count = album
            .mediums()
            .iter()
            .map(|medium| medium.tracks().len())
            .sum();
        Self {
            cover_image_id: album.cover_image_id(),
            genres: album.genre_ids().to_vec(),
            id: album.id(),
            links: AlbumLinks::for_album(
                album.id(),
                album.cover_image_id(),
                album.music_group_id(),
            ),
            mediums: album
                .mediums()
                .iter()
                .map(|medium| {
                    MediumResponse::new(
                        album.id(),
                        medium.index().index(),
                        medium.index().as_wire().to_owned(),
                        medium.tracks().len(),
                    )
                })
                .collect(),
            name: album.name().to_owned(),
            production_type: album
                .production_type()
                .map(ProductionType::as_wire)
                .map(str::to_owned),
            release_date: album.release_date().map(format_date),
            track_count,
        }
    }
}

impl MediumResponse {
    /// Build the representation of the medium at `medium_index` of `album`.
    #[must_use]
    pub(crate) fn from_album(album: &MusicAlbum, medium_index: u8) -> Option<Self> {
        album
            .mediums()
            .iter()
            .find(|medium| medium.index().index() == medium_index)
            .map(|medium| {
                Self::new(
                    album.id(),
                    medium_index,
                    medium.index().as_wire().to_owned(),
                    medium.tracks().len(),
                )
            })
    }
}

impl TrackResponse {
    /// Build the representation of the track at `(medium_index, track_index)`
    /// of `album`.
    #[must_use]
    pub(crate) fn from_album(
        album: &MusicAlbum,
        medium_index: u8,
        track_index: u8,
    ) -> Option<Self> {
        let medium = album
            .mediums()
            .iter()
            .find(|medium| medium.index().index() == medium_index)?;
        let recording = medium
            .tracks()
            .iter()
            .find(|track| track.index().medium_index() == Some(track_index))?;
        Some(Self {
            album_id: album.id(),
            file_id: recording.file_id(),
            id: track_index,
            isrc_code: recording.isrc_code().map(str::to_owned),
            links: TrackLinks::for_track(album.id(), medium_index, track_index),
            medium_index,
            name: recording.name().map(str::to_owned),
            recording_id: recording.id(),
        })
    }
}

/// HAL links exposed by a music group representation.
///
/// The `self` link addresses the group and the `cover` link its artwork, when
/// the group stores one.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct MusicGroupLinks {
    /// Link to the group's cover image, when the group stores artwork.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover: Option<Link>,
    /// Link to the group resource itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

impl MusicGroupLinks {
    /// Build the links of the group identified by `group_id`.
    #[must_use]
    fn for_group(group_id: NumericID, cover_image_id: Option<NumericID>) -> Self {
        Self {
            cover: cover_image_id.map(|image_id| Link::new(&asset_image_href(image_id))),
            self_link: Link::new(&format!("/api/v1/library/music/group/{group_id}")),
        }
    }
}

/// A music group representation.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct MusicGroupResponse {
    /// Identifier of the image holding the group's artwork, when set.
    pub cover_image_id: Option<NumericID>,
    /// Unique identifier of the group.
    pub id: NumericID,
    /// Links to the group and its cover.
    #[serde(rename = "_links")]
    pub links: MusicGroupLinks,
    /// Stage name of the group.
    pub name: String,
}

impl MusicGroupResponse {
    /// Assemble a group representation from its parts.
    #[must_use]
    pub(crate) fn new(
        group_id: NumericID,
        name: String,
        cover_image_id: Option<NumericID>,
    ) -> Self {
        Self {
            cover_image_id,
            id: group_id,
            links: MusicGroupLinks::for_group(group_id, cover_image_id),
            name,
        }
    }
}

impl From<&ListMusicGroupsItem> for MusicGroupResponse {
    fn from(item: &ListMusicGroupsItem) -> Self {
        Self::new(
            item.group_id(),
            item.name().to_owned(),
            item.cover_image_id(),
        )
    }
}

impl From<&GetMusicGroupResponseData> for MusicGroupResponse {
    fn from(response: &GetMusicGroupResponseData) -> Self {
        Self::new(
            response.group_id(),
            response.name().to_owned(),
            response.cover_image_id(),
        )
    }
}

/// HAL links exposed by a music group collection representation.
///
/// The `self` link addresses the collection view the representation is about,
/// `next` and `prev` navigate that same view when another page exists, and the
/// templated `search` link exposes the collection filters. The active filter
/// and the page size are carried into every link so a filtered or paginated
/// listing stays navigable.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct MusicGroupListLinks {
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

impl MusicGroupListLinks {
    /// Build the links of a music group collection page.
    ///
    /// `query` carries the active filter, `limit` and `offset` are the
    /// effective page, `returned` is the number of groups in the page and
    /// `total` the number of matching groups before pagination.
    #[must_use]
    pub(crate) fn for_page(
        query: &ListMusicGroupsQuery,
        limit: usize,
        offset: usize,
        returned: usize,
        total: usize,
    ) -> Self {
        let next = (offset.saturating_add(returned) < total)
            .then(|| Link::new(&group_page_href(query, limit, offset.saturating_add(limit))));
        let prev = (offset > 0)
            .then(|| Link::new(&group_page_href(query, limit, offset.saturating_sub(limit))));
        Self {
            next,
            prev,
            search: Link::templated("/api/v1/library/music/group{?name,limit,offset}"),
            self_link: Link::new(&group_page_href(query, limit, offset)),
        }
    }
}

/// Music groups matching the requested filters.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct MusicGroupListResponse {
    /// The groups of the returned page.
    pub items: Vec<MusicGroupResponse>,
    /// Navigation links of the collection.
    #[serde(rename = "_links")]
    pub links: MusicGroupListLinks,
}

/// HAL links exposed by a music track representation.
///
/// The `self` link addresses the track, the `album` link the album that holds
/// it, and the `medium` link the medium that sequences it.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct TrackLinks {
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
    fn for_track(album_id: NumericID, medium_index: u8, track_index: u8) -> Self {
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
pub(crate) struct TrackListResponse {
    /// The tracks of the album.
    pub items: Vec<TrackResponse>,
    /// Navigation links of the collection.
    #[serde(rename = "_links")]
    pub links: SelfLinks,
}

/// One music track of an album medium.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct TrackResponse {
    /// Identifier of the album the track belongs to.
    pub album_id: NumericID,
    /// Identifier of the backing file.
    pub file_id: NumericID,
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

/// One name a provider credits for an album, a release or a recording.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct ArtistCreditResponse {
    /// Phrase the provider places after the name, when it reports one.
    pub join_phrase: Option<String>,
    /// Name the provider credits.
    pub name: String,
}

impl ArtistCreditResponse {
    /// Build the representation of an artist credit.
    #[must_use]
    pub(crate) fn new(credit: &ArtistCredit) -> Self {
        Self {
            join_phrase: credit.join_phrase().map(str::to_owned),
            name: credit.name().to_owned(),
        }
    }
}

/// Opaque reference to one item at a metadata provider.
///
/// The reference is provider-tagged: an identifier one provider issued means
/// nothing to another, so every reference names the provider that owns it.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct MusicReferenceResponse {
    /// Identifier the provider issued.
    pub id: String,
    /// Provider that issued the identifier (`musicbrainz`).
    pub provider: String,
}

impl MusicReferenceResponse {
    /// Build the representation of an external reference.
    #[must_use]
    pub(crate) fn new(reference: &ExternalRef) -> Self {
        Self {
            id: reference.id().to_owned(),
            provider: reference.provider().as_str().to_owned(),
        }
    }
}

/// One medium of an identified release.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct MusicMediumResponse {
    /// Format the provider reports (`CD`, or the provider's own name for it).
    pub format: String,
    /// Album-scoped position of the medium, starting at one.
    pub index: u32,
    /// Number of tracks the medium carries.
    pub track_count: u32,
}

impl MusicMediumResponse {
    /// Build the representation of a medium summary.
    #[must_use]
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "the arms borrow the format out of a non-Copy enum matched by reference"
    )]
    pub(crate) fn new(medium: &MediumSummary) -> Self {
        let format = match medium.format() {
            MediumFormat::Cd => "CD".to_owned(),
            MediumFormat::Other(name) => name.clone(),
        };
        Self {
            format,
            index: medium.index(),
            track_count: medium.track_count(),
        }
    }
}

/// HAL links exposed by a film representation.
///
/// The `self` link addresses the film; the video is embedded in the
/// representation and carries no links of its own. A cover is exposed as the
/// `cover` link when the film stores artwork.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct MovieLinks {
    /// Link to the film's cover image, when the film stores artwork.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover: Option<Link>,
    /// Link to the film resource itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

impl MovieLinks {
    /// Build the links of the film identified by `movie_id`.
    #[must_use]
    fn for_movie(movie_id: NumericID, cover_image_id: Option<NumericID>) -> Self {
        Self {
            cover: cover_image_id.map(|image_id| Link::new(&asset_image_href(image_id))),
            self_link: Link::new(&format!("/api/v1/library/movie/{movie_id}")),
        }
    }
}

/// The video backing a film or an episode.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct VideoResponse {
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

impl From<&Video> for VideoResponse {
    fn from(video: &Video) -> Self {
        Self {
            codec: video.codec().to_owned(),
            duration_ms: video.duration_ms(),
            file_id: video.file_id(),
            height: video.height(),
            id: video.video_id(),
            scan_type: video.scan_type(),
            width: video.width(),
        }
    }
}

/// An opaque provider reference.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct VideoReferenceResponse {
    /// Identifier the provider issued for the item.
    pub id: String,
    /// Provider that issued the identifier.
    pub provider: String,
}

impl VideoReferenceResponse {
    /// Build the representation of `reference`.
    #[must_use]
    pub(crate) fn new(reference: &ExternalRef) -> Self {
        Self {
            id: reference.id().to_owned(),
            provider: reference.provider().as_str().to_owned(),
        }
    }
}

/// One video candidate the provider reported, shaped by its kind.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
#[non_exhaustive]
pub(crate) enum VideoCandidateResponse {
    /// A film.
    Movie {
        /// Opaque reference the provider issued.
        reference: VideoReferenceResponse,
        /// Title the provider reports.
        title: String,
        /// Release year, when the provider reports one.
        year: Option<u32>,
    },
    /// A show.
    Series {
        /// Opaque reference the provider issued.
        reference: VideoReferenceResponse,
        /// Title the provider reports.
        title: String,
        /// First-air year, when the provider reports one.
        year: Option<u32>,
    },
}

impl From<&Candidate> for VideoCandidateResponse {
    fn from(candidate: &Candidate) -> Self {
        let reference = VideoReferenceResponse::new(candidate.reference());
        let title = candidate.title().to_owned();
        let year = candidate.year();
        match candidate.kind() {
            VideoKind::Movie => Self::Movie {
                reference,
                title,
                year,
            },
            VideoKind::Series => Self::Series {
                reference,
                title,
                year,
            },
        }
    }
}

/// Candidates the configured provider reported, in its order.
///
/// `available` tells a configured provider apart from no provider at all: an
/// empty `items` list is a search that ran and found nothing, while
/// `available: false` is a surface the system cannot offer.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct VideoCandidateListResponse {
    /// Whether a video metadata provider is configured.
    pub available: bool,
    /// Candidates the provider reported, in its order.
    pub items: Vec<VideoCandidateResponse>,
    /// Link to the search view the representation reports.
    #[serde(rename = "_links")]
    pub links: SelfLinks,
}

impl VideoCandidateListResponse {
    /// Build the representation of a completed search over `href`.
    #[must_use]
    pub(crate) fn new(href: &str, candidates: &[Candidate]) -> Self {
        Self {
            links: SelfLinks::new(href),
            available: true,
            items: candidates
                .iter()
                .map(VideoCandidateResponse::from)
                .collect(),
        }
    }

    /// Build the representation of a surface with no configured provider.
    #[must_use]
    pub(crate) fn unavailable(href: &str) -> Self {
        Self {
            links: SelfLinks::new(href),
            available: false,
            items: Vec::new(),
        }
    }
}

/// The provider-independent metadata document of an identified video work.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
#[non_exhaustive]
pub(crate) enum VideoMetadataResponse {
    /// One episode.
    Episode {
        /// Date the episode first aired, in `YYYY-MM-DD` form, when known.
        air_date: Option<String>,
        /// Index of the episode within its season.
        episode: u32,
        /// Plot summary the provider reports.
        plot: String,
        /// Index of the season within the show.
        season: u32,
        /// Title the provider reports.
        title: String,
    },
    /// One film.
    Movie {
        /// Country of origin as an uppercase ISO 3166-1 alpha-2 code, when known.
        country_of_origin: Option<String>,
        /// Genres from the shared vocabulary.
        genres: Vec<String>,
        /// Plot summary the provider reports.
        plot: String,
        /// Date the film was released, in `YYYY-MM-DD` form, when known.
        release_date: Option<String>,
        /// Title the provider reports.
        title: String,
    },
    /// One show.
    Series {
        /// Country of origin as an uppercase ISO 3166-1 alpha-2 code, when known.
        country_of_origin: Option<String>,
        /// Date the show first aired, in `YYYY-MM-DD` form, when known.
        first_air_date: Option<String>,
        /// Genres from the shared vocabulary.
        genres: Vec<String>,
        /// Plot summary the provider reports.
        plot: String,
        /// Title the provider reports.
        title: String,
    },
}

impl From<&MediaMetadata> for VideoMetadataResponse {
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "the arms borrow the metadata out of a non-Copy enum matched by reference"
    )]
    fn from(metadata: &MediaMetadata) -> Self {
        match metadata {
            MediaMetadata::Episode {
                air_date,
                episode,
                plot,
                season,
                title,
                ..
            } => Self::Episode {
                air_date: air_date.map(format_date),
                episode: *episode,
                plot: plot.clone(),
                season: *season,
                title: title.clone(),
            },
            MediaMetadata::Movie {
                country_of_origin,
                genres,
                plot,
                release_date,
                title,
                ..
            } => Self::Movie {
                country_of_origin: country_of_origin.clone(),
                genres: genres
                    .iter()
                    .map(|genre| genre.as_str().to_owned())
                    .collect(),
                plot: plot.clone(),
                release_date: release_date.map(format_date),
                title: title.clone(),
            },
            MediaMetadata::Series {
                country_of_origin,
                first_air_date,
                genres,
                plot,
                title,
                ..
            } => Self::Series {
                country_of_origin: country_of_origin.clone(),
                first_air_date: first_air_date.map(format_date),
                genres: genres
                    .iter()
                    .map(|genre| genre.as_str().to_owned())
                    .collect(),
                plot: plot.clone(),
                title: title.clone(),
            },
        }
    }
}

/// The metadata document of one identified work, when one is available.
///
/// `available` tells a configured provider apart from no provider at all, so a
/// client does not read an unconfigured system as a provider that could not
/// resolve the reference.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct VideoMetadataDocumentResponse {
    /// Whether a video metadata provider is configured.
    pub available: bool,
    /// Link to the resolve view the representation reports.
    #[serde(rename = "_links")]
    pub links: SelfLinks,
    /// The resolved document, absent when no provider is configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<VideoMetadataResponse>,
}

impl VideoMetadataDocumentResponse {
    /// Build the representation of a resolved document over `href`.
    #[must_use]
    pub(crate) fn new(href: &str, metadata: &MediaMetadata) -> Self {
        Self {
            links: SelfLinks::new(href),
            available: true,
            metadata: Some(VideoMetadataResponse::from(metadata)),
        }
    }

    /// Build the representation of a surface with no configured provider.
    #[must_use]
    pub(crate) fn unavailable(href: &str) -> Self {
        Self {
            links: SelfLinks::new(href),
            available: false,
            metadata: None,
        }
    }
}

/// A provider reference in a video metadata apply body.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct VideoApplyReferenceRequest {
    /// Identifier the provider issued.
    #[schema(example = "27205")]
    pub id: String,
    /// Provider that issued the identifier (`tmdb` or `omdb`).
    #[schema(example = "tmdb")]
    pub provider: String,
}

/// Payload to apply external metadata to a video catalogue entity.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct VideoApplyRequest {
    /// Reference to the provider work.
    pub reference: VideoApplyReferenceRequest,
}

/// A film representation.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct MovieResponse {
    /// Country of origin of the film, when known.
    pub country_of_origin: Option<String>,
    /// Identifier of the image holding the film's artwork, when set.
    pub cover_image_id: Option<NumericID>,
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
    /// Date of the release of the film, in `YYYY-MM-DD` form, when known.
    pub release_date: Option<String>,
    /// Video backing the film.
    pub video: VideoResponse,
}

impl From<&ListMoviesItem> for MovieResponse {
    fn from(item: &ListMoviesItem) -> Self {
        Self {
            cover_image_id: item.cover_image_id(),
            country_of_origin: item.country_of_origin().map(str::to_owned),
            genres: item.genre_ids().to_vec(),
            id: item.id(),
            links: MovieLinks::for_movie(item.id(), item.cover_image_id()),
            name: item.name().to_owned(),
            plot: item.plot().to_owned(),
            release_date: item.release_date().map(format_date),
            video: VideoResponse::from(item.video()),
        }
    }
}

impl From<&GetMovieResponseData> for MovieResponse {
    fn from(response: &GetMovieResponseData) -> Self {
        Self {
            cover_image_id: response.cover_image_id(),
            country_of_origin: response.country_of_origin().map(str::to_owned),
            genres: response.genre_ids().to_vec(),
            id: response.id(),
            links: MovieLinks::for_movie(response.id(), response.cover_image_id()),
            name: response.name().to_owned(),
            plot: response.plot().to_owned(),
            release_date: response.release_date().map(format_date),
            video: VideoResponse::from(response.video()),
        }
    }
}

impl From<&Movie> for MovieResponse {
    fn from(movie: &Movie) -> Self {
        Self {
            cover_image_id: movie.cover_image_id(),
            country_of_origin: movie.country_of_origin().map(str::to_owned),
            genres: movie.genre_ids().to_vec(),
            id: movie.id(),
            links: MovieLinks::for_movie(movie.id(), movie.cover_image_id()),
            name: movie.name().to_owned(),
            plot: movie.plot().to_owned(),
            release_date: movie.release_date().map(format_date),
            video: VideoResponse::from(movie.video()),
        }
    }
}

/// HAL links exposed by a film collection representation.
///
/// The `self` link addresses the collection view the representation is about,
/// `next` and `prev` navigate that same view when another page exists, and the
/// templated `search` link exposes the collection filters. The active filter
/// and the page size are carried into every link so a filtered or paginated
/// listing stays navigable.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct MovieListLinks {
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
    pub(crate) fn for_page(
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
pub(crate) struct MovieListResponse {
    /// The films of the returned page.
    pub items: Vec<MovieResponse>,
    /// Navigation links of the collection.
    #[serde(rename = "_links")]
    pub links: MovieListLinks,
}

/// HAL links exposed by a show representation.
///
/// The `self` link addresses the show; the seasons are embedded and each
/// season carries its own links. A cover is exposed as the `cover` link when
/// the show stores artwork.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct SeriesLinks {
    /// Link to the show's cover image, when the show stores artwork.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover: Option<Link>,
    /// Link to the show resource itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

impl SeriesLinks {
    /// Build the links of the show identified by `series_id`.
    #[must_use]
    fn for_series(series_id: NumericID, cover_image_id: Option<NumericID>) -> Self {
        Self {
            cover: cover_image_id.map(|image_id| Link::new(&asset_image_href(image_id))),
            self_link: Link::new(&format!("/api/v1/library/series/{series_id}")),
        }
    }
}

/// HAL links exposed by a season representation.
///
/// The `self` link addresses the season, the `series` link the show that holds
/// it, and the `episodes` link the season's episode collection. A cover is
/// exposed as the `cover` link when the season stores artwork.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct SeriesSeasonLinks {
    /// Link to the season's cover image, when the season stores artwork.
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
    fn for_season(
        series_id: NumericID,
        season_index: u32,
        cover_image_id: Option<NumericID>,
    ) -> Self {
        Self {
            cover: cover_image_id.map(|image_id| Link::new(&asset_image_href(image_id))),
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
/// it, and the `season` link the season that sequences it.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct SeriesEpisodeLinks {
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
    fn for_episode(series_id: NumericID, season_index: u32, episode_index: u32) -> Self {
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
pub(crate) struct SeasonResponse {
    /// Identifier of the image holding the season's artwork, when set.
    pub cover_image_id: Option<NumericID>,
    /// Number of episodes the season holds.
    pub episode_count: usize,
    /// Series-scoped index of the season.
    pub index: u32,
    /// Links to the season, its show and its episode collection.
    #[serde(rename = "_links")]
    pub links: SeriesSeasonLinks,
    /// Name of the season, when set.
    pub name: Option<String>,
    /// Identifier of the show holding the season.
    pub series_id: NumericID,
}

impl SeasonResponse {
    /// Assemble a season representation from its parts.
    #[must_use]
    pub(crate) fn new(
        series_id: NumericID,
        index: u32,
        name: Option<String>,
        episode_count: usize,
        cover_image_id: Option<NumericID>,
    ) -> Self {
        Self {
            cover_image_id,
            episode_count,
            index,
            links: SeriesSeasonLinks::for_season(series_id, index, cover_image_id),
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
            response.cover_image_id(),
        )
    }
}

/// A show representation: its metadata and season summaries.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct SeriesResponse {
    /// Country of origin of the show, when known.
    pub country_of_origin: Option<String>,
    /// Identifier of the image holding the show's artwork, when set.
    pub cover_image_id: Option<NumericID>,
    /// Date of the first airing of the show, in `YYYY-MM-DD` form, when known.
    pub first_air_date: Option<String>,
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
            cover_image_id: response.cover_image_id(),
            country_of_origin: response.country_of_origin().map(str::to_owned),
            first_air_date: response.first_air_date().map(format_date),
            genres: response.genre_ids().to_vec(),
            id: response.id(),
            links: SeriesLinks::for_series(response.id(), response.cover_image_id()),
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
                        season.cover_image_id(),
                    )
                })
                .collect(),
        }
    }
}

impl From<&Series> for SeriesResponse {
    fn from(series: &Series) -> Self {
        Self {
            cover_image_id: series.cover_image_id(),
            country_of_origin: series.country_of_origin().map(str::to_owned),
            first_air_date: series.first_air_date().map(format_date),
            genres: series.genre_ids().to_vec(),
            id: series.id(),
            links: SeriesLinks::for_series(series.id(), series.cover_image_id()),
            name: series.name().to_owned(),
            plot: series.plot().to_owned(),
            seasons: series
                .seasons()
                .iter()
                .map(|season| {
                    SeasonResponse::new(
                        series.id(),
                        season.index().value(),
                        season.name().map(str::to_owned),
                        season.episodes().len(),
                        season.cover_image_id(),
                    )
                })
                .collect(),
        }
    }
}

/// One show of a listing: its identifier, name and cover.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct SeriesSummaryResponse {
    /// Identifier of the image holding the show's artwork, when set.
    pub cover_image_id: Option<NumericID>,
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
            cover_image_id: item.cover_image_id(),
            id: item.id(),
            links: SeriesLinks::for_series(item.id(), item.cover_image_id()),
            name: item.name().to_owned(),
        }
    }
}

/// HAL links exposed by a show collection representation.
///
/// The `self` link addresses the collection view the representation is about,
/// `next` and `prev` navigate that same view when another page exists, and the
/// templated `search` link exposes the collection filters.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct SeriesListLinks {
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
    pub(crate) fn for_page(
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
pub(crate) struct SeriesListResponse {
    /// The shows of the returned page.
    pub items: Vec<SeriesSummaryResponse>,
    /// Navigation links of the collection.
    #[serde(rename = "_links")]
    pub links: SeriesListLinks,
}

/// An episode representation.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct EpisodeResponse {
    /// Date of the first airing of the episode, in `YYYY-MM-DD` form, when
    /// known.
    pub air_date: Option<String>,
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
            air_date: item.air_date().map(format_date),
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
            air_date: response.air_date().map(format_date),
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

impl EpisodeResponse {
    /// Assemble the representation of `episode` at the addressed show and
    /// season.
    #[must_use]
    pub(crate) fn from_episode(series_id: NumericID, season_index: u32, episode: &Episode) -> Self {
        let episode_index = episode.index().value();
        Self {
            air_date: episode.air_date().map(format_date),
            id: episode_index,
            links: SeriesEpisodeLinks::for_episode(series_id, season_index, episode_index),
            name: episode.name().to_owned(),
            plot: episode.plot().to_owned(),
            season_index,
            series_id,
            video: VideoResponse::from(episode.video()),
        }
    }
}

/// Episodes of one season.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct EpisodeListResponse {
    /// The episodes of the season, ordered by index.
    pub items: Vec<EpisodeResponse>,
    /// Navigation links of the collection.
    #[serde(rename = "_links")]
    pub links: SelfLinks,
}

/// HAL links exposed by a music playlist representation.
///
/// The `self` link addresses the canonical playlist; the `tracks` link exposes
/// the alias route that serves the playlist with its embedded tracks. A cover
/// is exposed as the `cover` link when the playlist stores artwork.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PlaylistLinks {
    /// Link to the playlist's cover image, when the playlist stores artwork.
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
    fn for_playlist(playlist_id: NumericID, cover_image_id: Option<NumericID>) -> Self {
        Self {
            cover: cover_image_id.map(|image_id| Link::new(&asset_image_href(image_id))),
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
/// that sequences it.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PlaylistTrackLinks {
    /// Link to the playlist the track belongs to.
    pub playlist: Link,
    /// Link to the track resource itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

impl PlaylistTrackLinks {
    /// Build the links of a track identified by its playlist and index.
    #[must_use]
    fn for_track(playlist_id: NumericID, track_index: u32) -> Self {
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
pub(crate) struct PlaylistTrackResponse {
    /// Identifier of the backing file.
    pub file_id: NumericID,
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
    pub(crate) fn new(playlist_id: NumericID, track: &MusicRecording) -> Self {
        let index = track.index().playlist_index().unwrap_or_default();
        Self {
            file_id: track.file_id(),
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
pub(crate) struct PlaylistResponse {
    /// Identifier of the image holding the playlist's artwork, when set.
    pub cover_image_id: Option<NumericID>,
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
    pub(crate) fn from_listed(item: &ListPlaylistsItem) -> Self {
        Self {
            cover_image_id: item.cover_image_id(),
            created_at: format_timestamp(item.created_at()),
            id: item.id(),
            is_public: item.is_public(),
            last_modified_at: format_timestamp(item.last_modified_at()),
            links: PlaylistLinks::for_playlist(item.id(), item.cover_image_id()),
            name: item.name().to_owned(),
            owner_id: item.user_id(),
            track_count: item.track_count(),
            tracks: Vec::new(),
        }
    }

    /// Assemble a playlist representation from a hydrated playlist.
    #[must_use]
    pub(crate) fn new(playlist: &MusicPlaylist) -> Self {
        Self {
            cover_image_id: playlist.cover_image_id(),
            created_at: format_timestamp(playlist.created_at()),
            id: playlist.id(),
            is_public: playlist.is_public(),
            last_modified_at: format_timestamp(playlist.last_modified_at()),
            links: PlaylistLinks::for_playlist(playlist.id(), playlist.cover_image_id()),
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
/// templated `search` link exposes the collection filters. The active filters
/// and the page size are carried into every link so a filtered or paginated
/// listing stays navigable.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PlaylistListLinks {
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
    pub(crate) fn for_page(
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
pub(crate) struct PlaylistListResponse {
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
            RemovePlaylistTrackError::Forbidden => Self::Forbidden(err.to_string()),
            RemovePlaylistTrackError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            RemovePlaylistTrackError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a reorder-playlist-tracks error to its API error.
impl From<ReorderPlaylistTracksError> for ApiError {
    fn from(err: ReorderPlaylistTracksError) -> Self {
        match err {
            ReorderPlaylistTracksError::Forbidden => Self::Forbidden(err.to_string()),
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

/// Map a create-album error to its API error.
impl From<CreateMusicAlbumError> for ApiError {
    fn from(err: CreateMusicAlbumError) -> Self {
        match err {
            CreateMusicAlbumError::Conflict => Self::Conflict(err.to_string()),
            CreateMusicAlbumError::Forbidden => Self::Forbidden(err.to_string()),
            CreateMusicAlbumError::InvalidGenre
            | CreateMusicAlbumError::InvalidName
            | CreateMusicAlbumError::NameTooLong
            | CreateMusicAlbumError::NoSuchGroup => Self::UnprocessableEntity(err.to_string()),
            CreateMusicAlbumError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            CreateMusicAlbumError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map an update-album error to its API error.
impl From<UpdateMusicAlbumError> for ApiError {
    fn from(err: UpdateMusicAlbumError) -> Self {
        match err {
            UpdateMusicAlbumError::Conflict => Self::Conflict(err.to_string()),
            UpdateMusicAlbumError::Forbidden => Self::Forbidden(err.to_string()),
            UpdateMusicAlbumError::InvalidGenre
            | UpdateMusicAlbumError::InvalidName
            | UpdateMusicAlbumError::NameTooLong
            | UpdateMusicAlbumError::NoSuchGroup => Self::UnprocessableEntity(err.to_string()),
            UpdateMusicAlbumError::NoSuchAlbum => Self::NotFound(err.to_string()),
            UpdateMusicAlbumError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            UpdateMusicAlbumError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a put-medium error to its API error.
impl From<PutAlbumMediumError> for ApiError {
    fn from(err: PutAlbumMediumError) -> Self {
        match err {
            PutAlbumMediumError::Conflict => Self::Conflict(err.to_string()),
            PutAlbumMediumError::DefaultAlbum | PutAlbumMediumError::InvalidMediumType => {
                Self::UnprocessableEntity(err.to_string())
            }
            PutAlbumMediumError::Forbidden => Self::Forbidden(err.to_string()),
            PutAlbumMediumError::NoSuchAlbum => Self::NotFound(err.to_string()),
            PutAlbumMediumError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            PutAlbumMediumError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a put-track error to its API error.
impl From<PutAlbumTrackError> for ApiError {
    fn from(err: PutAlbumTrackError) -> Self {
        match err {
            PutAlbumTrackError::Conflict => Self::Conflict(err.to_string()),
            PutAlbumTrackError::DefaultAlbum | PutAlbumTrackError::NoSuchRecording => {
                Self::UnprocessableEntity(err.to_string())
            }
            PutAlbumTrackError::Forbidden => Self::Forbidden(err.to_string()),
            PutAlbumTrackError::NoSuchAlbum | PutAlbumTrackError::NoSuchMedium => {
                Self::NotFound(err.to_string())
            }
            PutAlbumTrackError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            PutAlbumTrackError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a patch-track error to its API error.
impl From<PatchAlbumTrackError> for ApiError {
    fn from(err: PatchAlbumTrackError) -> Self {
        match err {
            PatchAlbumTrackError::Conflict | PatchAlbumTrackError::IsrcAlreadyExists => {
                Self::Conflict(err.to_string())
            }
            PatchAlbumTrackError::DefaultAlbum
            | PatchAlbumTrackError::InvalidIsrc
            | PatchAlbumTrackError::InvalidName => Self::UnprocessableEntity(err.to_string()),
            PatchAlbumTrackError::Forbidden => Self::Forbidden(err.to_string()),
            PatchAlbumTrackError::NoSuchAlbum
            | PatchAlbumTrackError::NoSuchMedium
            | PatchAlbumTrackError::NoSuchTrack => Self::NotFound(err.to_string()),
            PatchAlbumTrackError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            PatchAlbumTrackError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map an apply-album error to its API error.
impl From<ApplyMusicAlbumError> for ApiError {
    fn from(err: ApplyMusicAlbumError) -> Self {
        match err {
            ApplyMusicAlbumError::Conflict => Self::Conflict(err.to_string()),
            ApplyMusicAlbumError::DefaultAlbum
            | ApplyMusicAlbumError::InvalidPlacement
            | ApplyMusicAlbumError::InvalidReference
            | ApplyMusicAlbumError::UnknownProvider => Self::UnprocessableEntity(err.to_string()),
            ApplyMusicAlbumError::Forbidden => Self::Forbidden(err.to_string()),
            ApplyMusicAlbumError::NoSuchAlbum | ApplyMusicAlbumError::UnknownReference => {
                Self::NotFound(err.to_string())
            }
            ApplyMusicAlbumError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            ApplyMusicAlbumError::ProviderUnavailable
            | ApplyMusicAlbumError::ProviderFailure
            | ApplyMusicAlbumError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map an applied-film error to its API error.
impl From<ApplyMovieError> for ApiError {
    fn from(err: ApplyMovieError) -> Self {
        match err {
            ApplyMovieError::Conflict => Self::Conflict(err.to_string()),
            ApplyMovieError::Forbidden => Self::Forbidden(err.to_string()),
            ApplyMovieError::InvalidReference => Self::UnprocessableEntity(err.to_string()),
            ApplyMovieError::NoSuchMovie | ApplyMovieError::UnknownReference => {
                Self::NotFound(err.to_string())
            }
            ApplyMovieError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            ApplyMovieError::ProviderUnavailable
            | ApplyMovieError::ProviderFailure
            | ApplyMovieError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map an applied-show error to its API error.
impl From<ApplySeriesError> for ApiError {
    fn from(err: ApplySeriesError) -> Self {
        match err {
            ApplySeriesError::Conflict => Self::Conflict(err.to_string()),
            ApplySeriesError::Forbidden => Self::Forbidden(err.to_string()),
            ApplySeriesError::InvalidReference => Self::UnprocessableEntity(err.to_string()),
            ApplySeriesError::NoSuchSeries | ApplySeriesError::UnknownReference => {
                Self::NotFound(err.to_string())
            }
            ApplySeriesError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            ApplySeriesError::ProviderUnavailable
            | ApplySeriesError::ProviderFailure
            | ApplySeriesError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map an applied-episode error to its API error.
impl From<ApplySeriesEpisodeError> for ApiError {
    fn from(err: ApplySeriesEpisodeError) -> Self {
        match err {
            ApplySeriesEpisodeError::Conflict => Self::Conflict(err.to_string()),
            ApplySeriesEpisodeError::Forbidden => Self::Forbidden(err.to_string()),
            ApplySeriesEpisodeError::InvalidReference
            | ApplySeriesEpisodeError::PositionMismatch => {
                Self::UnprocessableEntity(err.to_string())
            }
            ApplySeriesEpisodeError::NoSuchEpisode
            | ApplySeriesEpisodeError::NoSuchSeason
            | ApplySeriesEpisodeError::NoSuchSeries
            | ApplySeriesEpisodeError::UnknownReference => Self::NotFound(err.to_string()),
            ApplySeriesEpisodeError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            ApplySeriesEpisodeError::ProviderUnavailable
            | ApplySeriesEpisodeError::ProviderFailure
            | ApplySeriesEpisodeError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a create-music-group error to its API error.
impl From<CreateMusicGroupError> for ApiError {
    fn from(err: CreateMusicGroupError) -> Self {
        match err {
            CreateMusicGroupError::Forbidden => Self::Forbidden(err.to_string()),
            CreateMusicGroupError::InvalidCover | CreateMusicGroupError::NameAlreadyExists => {
                Self::Conflict(err.to_string())
            }
            CreateMusicGroupError::InvalidName | CreateMusicGroupError::NameTooLong => {
                Self::UnprocessableEntity(err.to_string())
            }
            CreateMusicGroupError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            CreateMusicGroupError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a fetch-music-group error to its API error.
impl From<GetMusicGroupError> for ApiError {
    fn from(err: GetMusicGroupError) -> Self {
        match err {
            GetMusicGroupError::NoSuchGroup => Self::NotFound(err.to_string()),
            GetMusicGroupError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetMusicGroupError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a list-music-groups error to its API error.
impl From<ListMusicGroupsError> for ApiError {
    fn from(err: ListMusicGroupsError) -> Self {
        match err {
            ListMusicGroupsError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            ListMusicGroupsError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map an update-music-group error to its API error.
impl From<UpdateMusicGroupError> for ApiError {
    fn from(err: UpdateMusicGroupError) -> Self {
        match err {
            UpdateMusicGroupError::Conflict
            | UpdateMusicGroupError::InvalidCover
            | UpdateMusicGroupError::NameAlreadyExists => Self::Conflict(err.to_string()),
            UpdateMusicGroupError::Forbidden => Self::Forbidden(err.to_string()),
            UpdateMusicGroupError::InvalidName | UpdateMusicGroupError::NameTooLong => {
                Self::UnprocessableEntity(err.to_string())
            }
            UpdateMusicGroupError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            UpdateMusicGroupError::NoSuchGroup => Self::NotFound(err.to_string()),
            UpdateMusicGroupError::Unknown(_) => Self::InternalServerError,
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

/// Map a music identification error to its API error.
impl From<IdentifyMusicError> for ApiError {
    fn from(err: IdentifyMusicError) -> Self {
        match err {
            IdentifyMusicError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            IdentifyMusicError::NotPermitted => Self::NotFound(err.to_string()),
            // The handler reports an unconfigured provider as an unavailable
            // document before the conversion, so reaching this arm is a fault.
            IdentifyMusicError::Unavailable | IdentifyMusicError::Unknown(_) => {
                Self::InternalServerError
            }
        }
    }
}

/// Map a music metadata resolution error to its API error.
impl From<ResolveMusicMetadataError> for ApiError {
    fn from(err: ResolveMusicMetadataError) -> Self {
        match err {
            ResolveMusicMetadataError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            ResolveMusicMetadataError::NotPermitted
            | ResolveMusicMetadataError::UnknownProvider => Self::NotFound(err.to_string()),
            // The handler reports an unconfigured provider as an unavailable
            // document before the conversion, so reaching this arm is a fault.
            ResolveMusicMetadataError::Unavailable | ResolveMusicMetadataError::Unknown(_) => {
                Self::InternalServerError
            }
        }
    }
}

/// Map a video identification error to its API error.
impl From<IdentifyVideoError> for ApiError {
    fn from(err: IdentifyVideoError) -> Self {
        match err {
            IdentifyVideoError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            IdentifyVideoError::NotPermitted => Self::NotFound(err.to_string()),
            // The handler reports an unconfigured provider as an unavailable
            // document before the conversion, so reaching this arm is a fault.
            IdentifyVideoError::Unavailable | IdentifyVideoError::Unknown(_) => {
                Self::InternalServerError
            }
        }
    }
}

/// Map a video metadata resolution error to its API error.
impl From<ResolveVideoMetadataError> for ApiError {
    fn from(err: ResolveVideoMetadataError) -> Self {
        match err {
            ResolveVideoMetadataError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            ResolveVideoMetadataError::NotPermitted => Self::NotFound(err.to_string()),
            ResolveVideoMetadataError::KindMismatch
            | ResolveVideoMetadataError::PositionMismatch => {
                Self::UnprocessableEntity(err.to_string())
            }
            // The handler reports an unconfigured provider as an unavailable
            // document before the conversion, so reaching this arm is a fault.
            ResolveVideoMetadataError::Unavailable | ResolveVideoMetadataError::Unknown(_) => {
                Self::InternalServerError
            }
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

/// Map a create-film error to its API error.
impl From<CreateMovieError> for ApiError {
    fn from(err: CreateMovieError) -> Self {
        match err {
            CreateMovieError::Conflict => Self::Conflict(err.to_string()),
            CreateMovieError::Forbidden => Self::Forbidden(err.to_string()),
            CreateMovieError::InvalidCountry
            | CreateMovieError::InvalidGenre
            | CreateMovieError::InvalidName
            | CreateMovieError::NameTooLong
            | CreateMovieError::NoSuchVideo => Self::UnprocessableEntity(err.to_string()),
            CreateMovieError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            CreateMovieError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map an update-film error to its API error.
impl From<UpdateMovieError> for ApiError {
    fn from(err: UpdateMovieError) -> Self {
        match err {
            UpdateMovieError::Conflict => Self::Conflict(err.to_string()),
            UpdateMovieError::Forbidden => Self::Forbidden(err.to_string()),
            UpdateMovieError::InvalidCountry
            | UpdateMovieError::InvalidGenre
            | UpdateMovieError::InvalidName
            | UpdateMovieError::NameTooLong => Self::UnprocessableEntity(err.to_string()),
            UpdateMovieError::NoSuchMovie => Self::NotFound(err.to_string()),
            UpdateMovieError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            UpdateMovieError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a create-show error to its API error.
impl From<CreateSeriesError> for ApiError {
    fn from(err: CreateSeriesError) -> Self {
        match err {
            CreateSeriesError::Conflict => Self::Conflict(err.to_string()),
            CreateSeriesError::Forbidden => Self::Forbidden(err.to_string()),
            CreateSeriesError::InvalidCountry
            | CreateSeriesError::InvalidGenre
            | CreateSeriesError::InvalidName
            | CreateSeriesError::NameTooLong => Self::UnprocessableEntity(err.to_string()),
            CreateSeriesError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            CreateSeriesError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map an update-show error to its API error.
impl From<UpdateSeriesError> for ApiError {
    fn from(err: UpdateSeriesError) -> Self {
        match err {
            UpdateSeriesError::Conflict => Self::Conflict(err.to_string()),
            UpdateSeriesError::Forbidden => Self::Forbidden(err.to_string()),
            UpdateSeriesError::InvalidCountry
            | UpdateSeriesError::InvalidGenre
            | UpdateSeriesError::InvalidName
            | UpdateSeriesError::NameTooLong => Self::UnprocessableEntity(err.to_string()),
            UpdateSeriesError::NoSuchSeries => Self::NotFound(err.to_string()),
            UpdateSeriesError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            UpdateSeriesError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a write-season error to its API error.
impl From<PutSeriesSeasonError> for ApiError {
    fn from(err: PutSeriesSeasonError) -> Self {
        match err {
            PutSeriesSeasonError::Conflict => Self::Conflict(err.to_string()),
            PutSeriesSeasonError::Forbidden => Self::Forbidden(err.to_string()),
            PutSeriesSeasonError::NoSuchSeries => Self::NotFound(err.to_string()),
            PutSeriesSeasonError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            PutSeriesSeasonError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map an update-season error to its API error.
impl From<UpdateSeriesSeasonError> for ApiError {
    fn from(err: UpdateSeriesSeasonError) -> Self {
        match err {
            UpdateSeriesSeasonError::Conflict => Self::Conflict(err.to_string()),
            UpdateSeriesSeasonError::Forbidden => Self::Forbidden(err.to_string()),
            UpdateSeriesSeasonError::NoSuchSeason | UpdateSeriesSeasonError::NoSuchSeries => {
                Self::NotFound(err.to_string())
            }
            UpdateSeriesSeasonError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            UpdateSeriesSeasonError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map a write-episode error to its API error.
impl From<PutSeriesEpisodeError> for ApiError {
    fn from(err: PutSeriesEpisodeError) -> Self {
        match err {
            PutSeriesEpisodeError::Conflict => Self::Conflict(err.to_string()),
            PutSeriesEpisodeError::Forbidden => Self::Forbidden(err.to_string()),
            PutSeriesEpisodeError::InvalidName
            | PutSeriesEpisodeError::NameTooLong
            | PutSeriesEpisodeError::NoSuchVideo => Self::UnprocessableEntity(err.to_string()),
            PutSeriesEpisodeError::NoSuchSeason | PutSeriesEpisodeError::NoSuchSeries => {
                Self::NotFound(err.to_string())
            }
            PutSeriesEpisodeError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            PutSeriesEpisodeError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Map an update-episode error to its API error.
impl From<UpdateSeriesEpisodeError> for ApiError {
    fn from(err: UpdateSeriesEpisodeError) -> Self {
        match err {
            UpdateSeriesEpisodeError::Forbidden => Self::Forbidden(err.to_string()),
            UpdateSeriesEpisodeError::InvalidName | UpdateSeriesEpisodeError::NameTooLong => {
                Self::UnprocessableEntity(err.to_string())
            }
            UpdateSeriesEpisodeError::NoSuchEpisode
            | UpdateSeriesEpisodeError::NoSuchSeason
            | UpdateSeriesEpisodeError::NoSuchSeries => Self::NotFound(err.to_string()),
            UpdateSeriesEpisodeError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            UpdateSeriesEpisodeError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Canonical URI reference of the gallery item identified by `item_id`.
#[must_use]
fn gallery_item_self_href(gallery_id: NumericID, item_id: NumericID) -> String {
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

/// Build the href of the music group collection view at `limit`/`offset`,
/// preserving the active filter so a filtered page keeps its navigation
/// (`API-037`).
fn group_page_href(query: &ListMusicGroupsQuery, limit: usize, offset: usize) -> String {
    let mut href = String::from("/api/v1/library/music/group?");
    push_name_filter(&mut href, query.name.as_deref());
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

/// Parse a music group identifier from a path segment.
pub(crate) fn parse_group_id(value: &str) -> Result<NumericID, ApiError> {
    value.parse::<NumericID>().map_err(|_| {
        // The malformed value and the parser detail never reach the log
        // (`OBS-003`); only the classification and its declared fields do
        // (`OBS-006`).
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "group_id",
            reason = "invalid_identifier",
            "rejected a malformed music group identifier"
        );
        ApiError::BadRequest("invalid music group identifier".to_owned())
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

/// Parse a `u32` episode index from a path segment.
pub(crate) fn parse_episode_index(value: &str) -> Result<u32, ApiError> {
    value.parse::<u32>().map_err(|_| {
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "episode_index",
            reason = "invalid_index",
            "rejected a malformed episode index"
        );
        ApiError::BadRequest("invalid episode index".to_owned())
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

/// Parse a `u8` music index from a path segment, reporting a value outside the
/// 8-bit range as unprocessable rather than malformed.
///
/// The write routes admit an index the domain models as `u8`, so a syntactically
/// valid index above the range is a well-formed request the server cannot
/// process (`422`), while a non-numeric segment stays a malformed path (`400`).
pub(crate) fn parse_write_index(
    value: &str,
    field: &'static str,
    message: &'static str,
) -> Result<u8, ApiError> {
    let parsed = value.parse::<u32>().map_err(|_| {
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = field,
            reason = "invalid_index",
            "rejected a malformed music index"
        );
        ApiError::BadRequest(message.to_owned())
    })?;
    u8::try_from(parsed).map_err(|_| {
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = field,
            reason = "index_out_of_range",
            "rejected an out-of-range music index"
        );
        ApiError::UnprocessableEntity(message.to_owned())
    })
}

/// Deserialize a field that distinguishes an absent value from `null`.
///
/// The field's `default` covers absence; this maps a body value to `Set` and an
/// explicit `null` to `Clear`, so a `Clear` reaches the use case only when the
/// client sent `null`.
pub(crate) fn deserialize_field_update<'de, D, T>(
    deserializer: D,
) -> Result<FieldUpdate<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
        .map(|value| value.map_or(FieldUpdate::Clear, FieldUpdate::Set))
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

/// Register the routes of the Library bounded context.
///
/// The handlers carry their own paths, so this only groups them; protection is
/// applied by [`library_routes`].
pub(crate) fn library_router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(
            get_album_list::get_album_list,
            post_album::post_album
        ))
        .routes(routes!(get_album::get_album, patch_album::patch_album))
        .routes(routes!(
            get_album_medium::get_album_medium,
            put_album_medium::put_album_medium
        ))
        .routes(routes!(get_album_track_list::get_album_track_list))
        .routes(routes!(
            get_album_track::get_album_track,
            put_album_track::put_album_track,
            patch_album_track::patch_album_track
        ))
        .routes(routes!(post_album_apply::post_album_apply))
        .routes(routes!(
            get_music_group_list::get_music_group_list,
            post_music_group::post_music_group
        ))
        .routes(routes!(
            get_music_group::get_music_group,
            patch_music_group::patch_music_group
        ))
        .routes(routes!(
            get_gallery_list::get_gallery_list,
            post_gallery::post_gallery
        ))
        .routes(routes!(
            get_gallery::get_gallery,
            delete_gallery::delete_gallery
        ))
        .routes(routes!(
            get_gallery_item_list::get_gallery_item_list,
            post_gallery_item::post_gallery_item
        ))
        .routes(routes!(
            get_gallery_item::get_gallery_item,
            delete_gallery_item::delete_gallery_item
        ))
        .routes(routes!(
            get_movie_list::get_movie_list,
            post_movie::post_movie
        ))
        .routes(routes!(get_movie::get_movie, patch_movie::patch_movie))
        .routes(routes!(post_movie_apply::post_movie_apply))
        .routes(routes!(get_movie_identify::get_movie_identify))
        .routes(routes!(
            get_movie_identify_metadata::get_movie_identify_metadata
        ))
        .routes(routes!(get_music_identify::get_music_identify))
        .routes(routes!(
            get_music_identify_metadata::get_music_identify_metadata
        ))
        .routes(routes!(
            get_series_list::get_series_list,
            post_series::post_series
        ))
        .routes(routes!(get_series::get_series, patch_series::patch_series))
        .routes(routes!(post_series_apply::post_series_apply))
        .routes(routes!(get_series_identify::get_series_identify))
        .routes(routes!(
            get_series_identify_metadata::get_series_identify_metadata
        ))
        .routes(routes!(
            get_series_season::get_series_season,
            put_series_season::put_series_season,
            patch_series_season::patch_series_season
        ))
        .routes(routes!(get_series_episode_list::get_series_episode_list))
        .routes(routes!(
            get_series_episode::get_series_episode,
            put_series_episode::put_series_episode,
            patch_series_episode::patch_series_episode
        ))
        .routes(routes!(
            post_series_episode_apply::post_series_episode_apply
        ))
        .routes(routes!(
            get_playlist_list::get_playlist_list,
            post_playlist::post_playlist
        ))
        .routes(routes!(
            get_playlist::get_playlist,
            patch_playlist::patch_playlist,
            delete_playlist::delete_playlist
        ))
        .routes(routes!(
            get_playlist_track_list::get_playlist_track_list,
            put_playlist_track::put_playlist_track
        ))
        .routes(routes!(
            get_playlist_track::get_playlist_track,
            delete_playlist_track::delete_playlist_track
        ))
}

/// Routes of the Library bounded context.
///
/// Every route requires an authenticated caller.
pub(crate) fn library_routes(state: &AppState) -> OpenApiRouter<()> {
    library_router()
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            authenticate::<JwtTokenProvider>,
        ))
        .with_state(state.clone())
}
