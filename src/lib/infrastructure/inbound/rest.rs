pub mod api_error;
pub mod app_state;
pub mod hal;
pub mod handler;
pub mod middleware;

use handler::asset::get_upload::__path_get_upload;
use handler::asset::post_upload::__path_post_upload;
use handler::asset::post_upload_complete::__path_post_upload_complete;
use handler::asset::put_upload_chunk::__path_put_upload_chunk;
use handler::get_health::__path_get_health;
use handler::identity::get_permission::__path_get_permission;
use handler::identity::patch_credential::__path_patch_credential;
use handler::identity::post_login::__path_post_login;
use handler::identity::post_register::__path_post_register;
use handler::identity::put_permission::__path_put_permission;
use handler::library::delete_gallery::__path_delete_gallery;
use handler::library::delete_gallery_item::__path_delete_gallery_item;
use handler::library::delete_playlist::__path_delete_playlist;
use handler::library::delete_playlist_track::__path_delete_playlist_track;
use handler::library::get_album::__path_get_album;
use handler::library::get_album_list::__path_get_album_list;
use handler::library::get_album_medium::__path_get_album_medium;
use handler::library::get_album_track::__path_get_album_track;
use handler::library::get_album_track_list::__path_get_album_track_list;
use handler::library::get_gallery::__path_get_gallery;
use handler::library::get_gallery_item::__path_get_gallery_item;
use handler::library::get_gallery_item_list::__path_get_gallery_item_list;
use handler::library::get_gallery_list::__path_get_gallery_list;
use handler::library::get_movie::__path_get_movie;
use handler::library::get_movie_list::__path_get_movie_list;
use handler::library::get_playlist::__path_get_playlist;
use handler::library::get_playlist_list::__path_get_playlist_list;
use handler::library::get_playlist_track::__path_get_playlist_track;
use handler::library::get_playlist_track_list::__path_get_playlist_track_list;
use handler::library::get_series::__path_get_series;
use handler::library::get_series_episode::__path_get_series_episode;
use handler::library::get_series_episode_list::__path_get_series_episode_list;
use handler::library::get_series_list::__path_get_series_list;
use handler::library::get_series_season::__path_get_series_season;
use handler::library::patch_playlist::__path_patch_playlist;
use handler::library::post_gallery::__path_post_gallery;
use handler::library::post_gallery_item::__path_post_gallery_item;
use handler::library::post_playlist::__path_post_playlist;
use handler::library::put_playlist_track::__path_put_playlist_track;
use handler::user::get_me::__path_get_me;
use handler::user::get_user::__path_get_user;
use handler::user::get_user_list::__path_get_user_list;
use handler::user::patch_user::__path_patch_user;

use crate::domain::model::user::Role;
use crate::domain::model::video::ScanType;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::hal::Link;
use crate::infrastructure::inbound::rest::hal::SelfLinks;
use crate::infrastructure::inbound::rest::handler::asset::UploadSessionLinks;
use crate::infrastructure::inbound::rest::handler::asset::UploadSessionResponse;
use crate::infrastructure::inbound::rest::handler::asset::post_upload::PostUploadRequest;
use crate::infrastructure::inbound::rest::handler::asset::post_upload::PostUploadResponse;
use crate::infrastructure::inbound::rest::handler::asset::post_upload_complete::PostUploadCompleteResponse;
use crate::infrastructure::inbound::rest::handler::asset::put_upload_chunk::PutUploadChunkRequest;
use crate::infrastructure::inbound::rest::handler::identity::PermissionGrantResponse;
use crate::infrastructure::inbound::rest::handler::identity::PermissionOverrideRequest;
use crate::infrastructure::inbound::rest::handler::identity::PermissionRequest;
use crate::infrastructure::inbound::rest::handler::identity::PermissionResource;
use crate::infrastructure::inbound::rest::handler::identity::PermissionResponse;
use crate::infrastructure::inbound::rest::handler::identity::patch_credential::PatchCredentialRequest;
use crate::infrastructure::inbound::rest::handler::identity::post_login::LoginRequest;
use crate::infrastructure::inbound::rest::handler::identity::post_login::LoginResponse;
use crate::infrastructure::inbound::rest::handler::identity::post_register::RegisterRequest;
use crate::infrastructure::inbound::rest::handler::identity::post_register::RegisterResponse;
use crate::infrastructure::inbound::rest::handler::library::AlbumLinks;
use crate::infrastructure::inbound::rest::handler::library::AlbumListLinks;
use crate::infrastructure::inbound::rest::handler::library::AlbumListResponse;
use crate::infrastructure::inbound::rest::handler::library::AlbumResponse;
use crate::infrastructure::inbound::rest::handler::library::EpisodeListResponse;
use crate::infrastructure::inbound::rest::handler::library::EpisodeResponse;
use crate::infrastructure::inbound::rest::handler::library::GalleryItemDetailResponse;
use crate::infrastructure::inbound::rest::handler::library::GalleryItemResponse;
use crate::infrastructure::inbound::rest::handler::library::GalleryItemType;
use crate::infrastructure::inbound::rest::handler::library::GalleryLinks;
use crate::infrastructure::inbound::rest::handler::library::GalleryResponse;
use crate::infrastructure::inbound::rest::handler::library::MediumLinks;
use crate::infrastructure::inbound::rest::handler::library::MediumResponse;
use crate::infrastructure::inbound::rest::handler::library::MovieLinks;
use crate::infrastructure::inbound::rest::handler::library::MovieListLinks;
use crate::infrastructure::inbound::rest::handler::library::MovieListResponse;
use crate::infrastructure::inbound::rest::handler::library::MovieResponse;
use crate::infrastructure::inbound::rest::handler::library::PlaylistLinks;
use crate::infrastructure::inbound::rest::handler::library::PlaylistListLinks;
use crate::infrastructure::inbound::rest::handler::library::PlaylistListResponse;
use crate::infrastructure::inbound::rest::handler::library::PlaylistResponse;
use crate::infrastructure::inbound::rest::handler::library::PlaylistTrackLinks;
use crate::infrastructure::inbound::rest::handler::library::PlaylistTrackResponse;
use crate::infrastructure::inbound::rest::handler::library::SeasonResponse;
use crate::infrastructure::inbound::rest::handler::library::SeriesEpisodeLinks;
use crate::infrastructure::inbound::rest::handler::library::SeriesLinks;
use crate::infrastructure::inbound::rest::handler::library::SeriesListLinks;
use crate::infrastructure::inbound::rest::handler::library::SeriesListResponse;
use crate::infrastructure::inbound::rest::handler::library::SeriesResponse;
use crate::infrastructure::inbound::rest::handler::library::SeriesSeasonLinks;
use crate::infrastructure::inbound::rest::handler::library::SeriesSummaryResponse;
use crate::infrastructure::inbound::rest::handler::library::TrackLinks;
use crate::infrastructure::inbound::rest::handler::library::TrackListResponse;
use crate::infrastructure::inbound::rest::handler::library::TrackResponse;
use crate::infrastructure::inbound::rest::handler::library::VideoResponse;
use crate::infrastructure::inbound::rest::handler::library::get_gallery_list::GalleryListLinks;
use crate::infrastructure::inbound::rest::handler::library::get_gallery_list::GalleryListResponse;
use crate::infrastructure::inbound::rest::handler::library::patch_playlist::PatchPlaylistRequest;
use crate::infrastructure::inbound::rest::handler::library::post_gallery::PostGalleryRequest;
use crate::infrastructure::inbound::rest::handler::library::post_gallery_item::PostGalleryItemRequest;
use crate::infrastructure::inbound::rest::handler::library::post_playlist::PostPlaylistRequest;
use crate::infrastructure::inbound::rest::handler::library::put_playlist_track::ReorderPlaylistTracksRequest;
use crate::infrastructure::inbound::rest::handler::user::GetUserResponse;
use crate::infrastructure::inbound::rest::handler::user::get_me::GetMeResponse;
use crate::infrastructure::inbound::rest::handler::user::get_user_list::GetUserListResponse;
use crate::infrastructure::inbound::rest::handler::user::get_user_list::UserListLinks;
use crate::infrastructure::inbound::rest::handler::user::patch_user::PatchUserRequest;
use crate::infrastructure::inbound::rest::handler::user::patch_user::PatchUserResponse;

use utoipa::openapi::OpenApi;
use utoipa::openapi::security::Http;
use utoipa::openapi::security::HttpAuthScheme;
use utoipa::openapi::security::SecurityScheme;

/// Add the `bearer_auth` security scheme referenced by the protected
/// endpoints.
struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
            );
        }
    }
}
/// Root `OpenAPI` aggregation for the Mnemorium HTTP API.
#[derive(utoipa::OpenApi)]
#[openapi(
    info(
        title = "Mnemorium API",
        version = env!("CARGO_PKG_VERSION"),
        description = "HTTP API of the Mnemorium service"
    ),
    servers(
        (url = "http://0.0.0.0:4080/api/v1", description = "Local development server")
    ),
    paths(get_health, get_upload, patch_credential, post_login, post_register, post_upload, post_upload_complete, put_upload_chunk, get_me, get_permission, get_user, get_user_list, patch_user, put_permission, delete_gallery, delete_gallery_item, delete_playlist, delete_playlist_track, get_album, get_album_list, get_album_medium, get_album_track, get_album_track_list, get_gallery, get_gallery_item, get_gallery_item_list, get_gallery_list, get_movie, get_movie_list, get_playlist, get_playlist_list, get_playlist_track, get_playlist_track_list, get_series, get_series_episode, get_series_episode_list, get_series_list, get_series_season, patch_playlist, post_gallery, post_gallery_item, post_playlist, put_playlist_track),
    components(schemas(AlbumLinks, AlbumListLinks, AlbumListResponse, AlbumResponse, EpisodeListResponse, EpisodeResponse, ErrorBody, GalleryItemDetailResponse, GalleryItemResponse, GalleryItemType, GalleryLinks, GalleryListLinks, GalleryListResponse, GalleryResponse, GetMeResponse, GetUserListResponse, GetUserResponse, Link, LoginRequest, LoginResponse, MediumLinks, MediumResponse, MovieLinks, MovieListLinks, MovieListResponse, MovieResponse, PatchCredentialRequest, PatchPlaylistRequest, PatchUserRequest, PatchUserResponse, PermissionGrantResponse, PermissionOverrideRequest, PermissionRequest, PermissionResource, PermissionResponse, PlaylistLinks, PlaylistListLinks, PlaylistListResponse, PlaylistResponse, PlaylistTrackLinks, PlaylistTrackResponse, PostGalleryItemRequest, PostGalleryRequest, PostPlaylistRequest, PostUploadCompleteResponse, PostUploadRequest, PostUploadResponse, PutUploadChunkRequest, RegisterRequest, RegisterResponse, ReorderPlaylistTracksRequest, Role, ScanType, SeasonResponse, SelfLinks, SeriesEpisodeLinks, SeriesLinks, SeriesListLinks, SeriesListResponse, SeriesResponse, SeriesSeasonLinks, SeriesSummaryResponse, TrackLinks, TrackListResponse, TrackResponse, UploadSessionLinks, UploadSessionResponse, UserListLinks, VideoResponse)),
    tags(
        (name = "system", description = "System-level endpoints"),
        (name = "asset", description = "Asset bounded context"),
        (name = "identity", description = "Identity bounded context"),
        (name = "library", description = "Library bounded context"),
        (name = "user", description = "User bounded context")
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;
