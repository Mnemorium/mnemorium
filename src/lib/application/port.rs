#![allow(
    clippy::struct_field_names,
    reason = "a port item names its fields after the identifiers the API and SQL sections mandate, and mockall names a mock's fields after the trait methods"
)]

/// The add gallery item use-case port.
mod add_gallery_item;
/// The apply movie metadata use-case port.
mod apply_movie;
/// The apply music album use-case port.
mod apply_music_album;
/// The apply series metadata use-case port.
mod apply_series;
/// The apply series episode metadata use-case port.
mod apply_series_episode;
/// The asset use-case factory port.
mod asset_use_case_factory;
/// The begin upload use-case port.
mod begin_upload;
/// The cancel upload use-case port.
mod cancel_upload;
/// The complete upload use-case port.
mod complete_upload;
/// The create gallery use-case port.
mod create_gallery;
/// The create movie use-case port.
mod create_movie;
/// The create music album use-case port.
mod create_music_album;
/// The create music group use-case port.
mod create_music_group;
/// The create playlist use-case port.
mod create_playlist;
/// The create series use-case port.
mod create_series;
/// The delete gallery use-case port.
mod delete_gallery;
/// The delete gallery item use-case port.
mod delete_gallery_item;
/// The delete playlist use-case port.
mod delete_playlist;
/// The delete user use-case port.
mod delete_user;
/// The ensure storage directories use-case port.
mod ensure_storage_directories;
/// The get album use-case port.
mod get_album;
/// The get album medium use-case port.
mod get_album_medium;
/// The get album track use-case port.
mod get_album_track;
/// The get asset content use-case port.
mod get_asset_content;
/// The get audio use-case port.
mod get_audio;
/// The get current user use-case port.
mod get_current_user;
/// The get file use-case port.
mod get_file;
/// The get gallery use-case port.
mod get_gallery;
/// The get gallery item use-case port.
mod get_gallery_item;
/// The get image use-case port.
mod get_image;
/// The get movie use-case port.
mod get_movie;
/// The get music group use-case port.
mod get_music_group;
/// The get permission summary use-case port.
mod get_permission_summary;
/// The get playlist use-case port.
mod get_playlist;
/// The get playlist track use-case port.
mod get_playlist_track;
/// The get resource permission use-case port.
mod get_resource_permission;
/// The get series use-case port.
mod get_series;
/// The get series episode use-case port.
mod get_series_episode;
/// The get series season use-case port.
mod get_series_season;
/// The get upload use-case port.
mod get_upload;
/// The get user use-case port.
mod get_user;
/// The get video use-case port.
mod get_video;
/// The identify music use-case port.
mod identify_music;
/// The identify video use-case port.
mod identify_video;
/// The identity use-case factory port.
mod identity_use_case_factory;
/// The ingest media use-case port.
mod ingest_media;
/// The initialize root admin use-case port.
mod initialize_root_admin;
/// The library use-case factory port.
mod library_use_case_factory;
/// The list album tracks use-case port.
mod list_album_tracks;
/// The list albums use-case port.
mod list_albums;
/// The list galleries use-case port.
mod list_galleries;
/// The list movies use-case port.
mod list_movies;
/// The list music groups use-case port.
mod list_music_groups;
/// The list playlists use-case port.
mod list_playlists;
/// The list series use-case port.
mod list_series;
/// The list series episodes use-case port.
mod list_series_episodes;
/// The list users use-case port.
mod list_users;
/// The load configuration use-case port.
mod load_configuration;
/// The load state use-case port.
mod load_state;
/// The login user use-case port.
mod login_user;
/// The media change source use-case port.
mod media_change_source;
/// The patch album track use-case port.
mod patch_album_track;
/// The patch credential use-case port.
mod patch_credential;
/// The put album medium use-case port.
mod put_album_medium;
/// The put album track use-case port.
mod put_album_track;
/// The put resource permission use-case port.
mod put_resource_permission;
/// The put series episode use-case port.
mod put_series_episode;
/// The put series season use-case port.
mod put_series_season;
/// The reap uploads use-case port.
mod reap_uploads;
/// The regenerate token use-case port.
mod regenerate_token;
/// The register user use-case port.
mod register_user;
/// The remove playlist track use-case port.
mod remove_playlist_track;
/// The reorder playlist tracks use-case port.
mod reorder_playlist_tracks;
/// The resolve music metadata use-case port.
mod resolve_music_metadata;
/// The resolve video metadata use-case port.
mod resolve_video_metadata;
/// The revoke share use-case port.
mod revoke_share;
/// The share resource use-case port.
mod share_resource;
/// The update movie use-case port.
mod update_movie;
/// The update music album use-case port.
mod update_music_album;
/// The update music group use-case port.
mod update_music_group;
/// The update playlist use-case port.
mod update_playlist;
/// The update series use-case port.
mod update_series;
/// The update series episode use-case port.
mod update_series_episode;
/// The update series season use-case port.
mod update_series_season;
/// The update user use-case port.
mod update_user;
/// The user use-case factory port.
mod user_use_case_factory;
/// The write upload chunk use-case port.
mod write_upload_chunk;

pub(crate) use self::add_gallery_item::{
    AddGalleryItemCommand, AddGalleryItemError, AddGalleryItemResponse, AddGalleryItemUseCase,
};
pub(crate) use self::apply_movie::{
    ApplyMovieCommand, ApplyMovieError, ApplyMovieResponse, ApplyMovieUseCase,
};
pub(crate) use self::apply_music_album::{
    AlbumPlacement, ApplyMusicAlbumCommand, ApplyMusicAlbumError, ApplyMusicAlbumResponse,
    ApplyMusicAlbumUseCase,
};
pub(crate) use self::apply_series::{
    ApplySeriesCommand, ApplySeriesError, ApplySeriesResponse, ApplySeriesUseCase,
};
pub(crate) use self::apply_series_episode::{
    ApplySeriesEpisodeCommand, ApplySeriesEpisodeError, ApplySeriesEpisodeResponse,
    ApplySeriesEpisodeUseCase,
};
pub(crate) use self::asset_use_case_factory::AssetUseCaseFactory;
#[cfg(test)]
pub(crate) use self::asset_use_case_factory::MockAssetUseCaseFactory;
pub(crate) use self::begin_upload::{
    BeginUploadCommand, BeginUploadError, BeginUploadResponse, BeginUploadUseCase,
};
pub(crate) use self::cancel_upload::{CancelUploadCommand, CancelUploadError, CancelUploadUseCase};
pub(crate) use self::complete_upload::{
    CompleteUploadCommand, CompleteUploadError, CompleteUploadResponse, CompleteUploadUseCase,
};
pub(crate) use self::create_gallery::{
    CreateGalleryCommand, CreateGalleryError, CreateGalleryResponse, CreateGalleryUseCase,
};
pub(crate) use self::create_movie::{
    CreateMovieCommand, CreateMovieError, CreateMovieResponse, CreateMovieUseCase,
};
pub(crate) use self::create_music_album::{
    CreateMusicAlbumCommand, CreateMusicAlbumError, CreateMusicAlbumResponse,
    CreateMusicAlbumUseCase,
};
pub(crate) use self::create_music_group::{
    CreateMusicGroupCommand, CreateMusicGroupError, CreateMusicGroupResponse,
    CreateMusicGroupUseCase,
};
pub(crate) use self::create_playlist::{
    CreatePlaylistCommand, CreatePlaylistError, CreatePlaylistResponse, CreatePlaylistUseCase,
};
pub(crate) use self::create_series::{
    CreateSeriesCommand, CreateSeriesError, CreateSeriesResponse, CreateSeriesUseCase,
};
pub(crate) use self::delete_gallery::{
    DeleteGalleryCommand, DeleteGalleryError, DeleteGalleryUseCase,
};
pub(crate) use self::delete_gallery_item::{
    DeleteGalleryItemCommand, DeleteGalleryItemError, DeleteGalleryItemUseCase,
};
pub(crate) use self::delete_playlist::{
    DeletePlaylistCommand, DeletePlaylistError, DeletePlaylistUseCase,
};
pub(crate) use self::delete_user::{DeleteUserCommand, DeleteUserError, DeleteUserUseCase};
pub(crate) use self::ensure_storage_directories::{
    EnsureStorageDirectoriesCommand, EnsureStorageDirectoriesError,
    EnsureStorageDirectoriesResponse, EnsureStorageDirectoriesUseCase,
};
pub(crate) use self::get_album::{
    GetAlbumCommand, GetAlbumError, GetAlbumMediumItem, GetAlbumResponse, GetAlbumUseCase,
};
pub(crate) use self::get_album_medium::{
    GetAlbumMediumCommand, GetAlbumMediumError, GetAlbumMediumResponse, GetAlbumMediumUseCase,
};
pub(crate) use self::get_album_track::{
    GetAlbumTrackCommand, GetAlbumTrackError, GetAlbumTrackResponse, GetAlbumTrackUseCase,
};
pub(crate) use self::get_asset_content::{
    AssetTarget, ContentOutcome, GetAssetContentCommand, GetAssetContentError,
    GetAssetContentResponse, GetAssetContentUseCase,
};
pub(crate) use self::get_audio::{
    GetAudioCommand, GetAudioError, GetAudioResponse, GetAudioUseCase,
};
pub(crate) use self::get_current_user::{
    GetCurrentUserCommand, GetCurrentUserError, GetCurrentUserResponse, GetCurrentUserUseCase,
};
pub(crate) use self::get_file::{GetFileCommand, GetFileError, GetFileResponse, GetFileUseCase};
pub(crate) use self::get_gallery::{
    GalleryItemSummary, GetGalleryCommand, GetGalleryError, GetGalleryResponse, GetGalleryUseCase,
};
pub(crate) use self::get_gallery_item::{
    GetGalleryItemCommand, GetGalleryItemError, GetGalleryItemResponse, GetGalleryItemUseCase,
};
pub(crate) use self::get_image::{
    GetImageCommand, GetImageError, GetImageResponse, GetImageUseCase,
};
pub(crate) use self::get_movie::{
    GetMovieCommand, GetMovieError, GetMovieResponse, GetMovieUseCase,
};
pub(crate) use self::get_music_group::{
    GetMusicGroupCommand, GetMusicGroupError, GetMusicGroupResponse, GetMusicGroupUseCase,
};
pub(crate) use self::get_permission_summary::{
    GetPermissionSummaryCommand, GetPermissionSummaryUseCase, GrantedResource,
    PermissionDecisionQuery, PermissionError, PermissionSummary,
};
pub(crate) use self::get_playlist::{
    GetPlaylistCommand, GetPlaylistError, GetPlaylistResponse, GetPlaylistUseCase,
};
pub(crate) use self::get_playlist_track::{
    GetPlaylistTrackCommand, GetPlaylistTrackError, GetPlaylistTrackResponse,
    GetPlaylistTrackUseCase,
};
pub(crate) use self::get_resource_permission::{
    GetResourcePermissionCommand, GetResourcePermissionUseCase, ResourcePermission,
};
pub(crate) use self::get_series::{
    GetSeriesCommand, GetSeriesError, GetSeriesResponse, GetSeriesSeasonItem, GetSeriesUseCase,
};
pub(crate) use self::get_series_episode::{
    GetSeriesEpisodeCommand, GetSeriesEpisodeError, GetSeriesEpisodeResponse,
    GetSeriesEpisodeUseCase,
};
pub(crate) use self::get_series_season::{
    GetSeriesSeasonCommand, GetSeriesSeasonError, GetSeriesSeasonResponse, GetSeriesSeasonUseCase,
};
pub(crate) use self::get_upload::{
    GetUploadCommand, GetUploadError, GetUploadResponse, GetUploadUseCase,
};
pub(crate) use self::get_user::{GetUserCommand, GetUserError, GetUserResponse, GetUserUseCase};
pub(crate) use self::get_video::{
    GetVideoCommand, GetVideoError, GetVideoResponse, GetVideoUseCase,
};
pub(crate) use self::identify_music::{
    IdentifyMusicCommand, IdentifyMusicError, IdentifyMusicResponse, IdentifyMusicUseCase,
};
pub(crate) use self::identify_video::{
    IdentifyVideoCommand, IdentifyVideoError, IdentifyVideoResponse, IdentifyVideoUseCase,
};
pub(crate) use self::identity_use_case_factory::IdentityUseCaseFactory;
#[cfg(test)]
pub(crate) use self::identity_use_case_factory::MockIdentityUseCaseFactory;
#[cfg(test)]
pub(crate) use self::ingest_media::MockIngestMediaUseCase;
pub(crate) use self::ingest_media::{
    IngestMediaCommand, IngestMediaError, IngestMediaResponse, IngestMediaUseCase, IngestRequest,
};
pub(crate) use self::initialize_root_admin::{
    InitializeRootAdminCommand, InitializeRootAdminError, InitializeRootAdminResponse,
    InitializeRootAdminUseCase,
};
pub(crate) use self::library_use_case_factory::LibraryUseCaseFactory;
#[cfg(test)]
pub(crate) use self::library_use_case_factory::MockLibraryUseCaseFactory;
pub(crate) use self::list_album_tracks::{
    ListAlbumTracksCommand, ListAlbumTracksError, ListAlbumTracksItem, ListAlbumTracksResponse,
    ListAlbumTracksUseCase,
};
pub(crate) use self::list_albums::{
    ListAlbumsCommand, ListAlbumsError, ListAlbumsItem, ListAlbumsMediumItem, ListAlbumsResponse,
    ListAlbumsUseCase,
};
pub(crate) use self::list_galleries::{
    ListGalleriesCommand, ListGalleriesError, ListGalleriesItem, ListGalleriesResponse,
    ListGalleriesUseCase,
};
pub(crate) use self::list_movies::{
    ListMoviesCommand, ListMoviesError, ListMoviesItem, ListMoviesResponse, ListMoviesUseCase,
};
pub(crate) use self::list_music_groups::{
    ListMusicGroupsCommand, ListMusicGroupsError, ListMusicGroupsItem, ListMusicGroupsResponse,
    ListMusicGroupsUseCase,
};
pub(crate) use self::list_playlists::{
    ListPlaylistsCommand, ListPlaylistsError, ListPlaylistsItem, ListPlaylistsResponse,
    ListPlaylistsUseCase,
};
pub(crate) use self::list_series::{
    ListSeriesCommand, ListSeriesError, ListSeriesItem, ListSeriesResponse, ListSeriesUseCase,
};
pub(crate) use self::list_series_episodes::{
    ListSeriesEpisodesCommand, ListSeriesEpisodesError, ListSeriesEpisodesItem,
    ListSeriesEpisodesResponse, ListSeriesEpisodesUseCase,
};
pub(crate) use self::list_users::{
    ListUsersCommand, ListUsersError, ListUsersResponse, ListUsersUseCase,
};
pub(crate) use self::load_configuration::{
    LoadConfigurationError, LoadConfigurationResponse, LoadConfigurationUseCase,
};
pub(crate) use self::load_state::{
    DATABASE_PATH, JWT_SECRET_VARIABLE, LoadStateError, LoadStateResponse, LoadStateUseCase,
    PEPPER_VARIABLE, SECRET_LENGTH, STATE_PATH, StateSeedSource, StateSeeds,
};
#[cfg(test)]
pub(crate) use self::login_user::MockLoginUserUseCase;
pub(crate) use self::login_user::{LoginUserCommand, LoginUserError, LoginUserUseCase};
#[cfg(test)]
pub(crate) use self::media_change_source::MockMediaChangeSource;
pub(crate) use self::media_change_source::{
    MediaChange, MediaChangeSource, MediaChangeSourceError,
};
pub(crate) use self::patch_album_track::{
    PatchAlbumTrackCommand, PatchAlbumTrackError, PatchAlbumTrackResponse, PatchAlbumTrackUseCase,
};
pub(crate) use self::patch_credential::{
    PatchCredentialCommand, PatchCredentialError, PatchCredentialUseCase,
};
pub(crate) use self::put_album_medium::{
    PutAlbumMediumCommand, PutAlbumMediumError, PutAlbumMediumResponse, PutAlbumMediumUseCase,
};
pub(crate) use self::put_album_track::{
    PutAlbumTrackCommand, PutAlbumTrackError, PutAlbumTrackResponse, PutAlbumTrackUseCase,
};
pub(crate) use self::put_resource_permission::{
    GrantRequest, PutResourcePermissionCommand, PutResourcePermissionUseCase,
};
pub(crate) use self::put_series_episode::{
    PutSeriesEpisodeCommand, PutSeriesEpisodeError, PutSeriesEpisodeResponse,
    PutSeriesEpisodeUseCase,
};
pub(crate) use self::put_series_season::{
    PutSeriesSeasonCommand, PutSeriesSeasonError, PutSeriesSeasonResponse, PutSeriesSeasonUseCase,
};
#[cfg(test)]
pub(crate) use self::reap_uploads::MockReapUploadsUseCase;
pub(crate) use self::reap_uploads::{
    ReapUploadsCommand, ReapUploadsError, ReapUploadsResponse, ReapUploadsUseCase,
};
#[cfg(test)]
pub(crate) use self::regenerate_token::MockRegenerateTokenUseCase;
pub(crate) use self::regenerate_token::{
    RegenerateTokenCommand, RegenerateTokenError, RegenerateTokenUseCase,
};
pub(crate) use self::register_user::{
    RegisterUserCommand, RegisterUserError, RegisterUserResponse, RegisterUserUseCase,
};
pub(crate) use self::remove_playlist_track::{
    RemovePlaylistTrackCommand, RemovePlaylistTrackError, RemovePlaylistTrackUseCase,
};
pub(crate) use self::reorder_playlist_tracks::{
    ReorderPlaylistTracksCommand, ReorderPlaylistTracksError, ReorderPlaylistTracksResponse,
    ReorderPlaylistTracksUseCase,
};
pub(crate) use self::resolve_music_metadata::{
    ResolveMusicMetadataCommand, ResolveMusicMetadataError, ResolveMusicMetadataResponse,
    ResolveMusicMetadataUseCase,
};
pub(crate) use self::resolve_video_metadata::{
    ResolveVideoMetadataCommand, ResolveVideoMetadataError, ResolveVideoMetadataResponse,
    ResolveVideoMetadataUseCase,
};
pub(crate) use self::revoke_share::{RevokeShareCommand, RevokeShareUseCase};
pub(crate) use self::share_resource::{ShareResourceCommand, ShareResourceUseCase};
pub(crate) use self::update_movie::{
    UpdateMovieCommand, UpdateMovieError, UpdateMovieResponse, UpdateMovieUseCase,
};
pub(crate) use self::update_music_album::{
    UpdateMusicAlbumCommand, UpdateMusicAlbumError, UpdateMusicAlbumResponse,
    UpdateMusicAlbumUseCase,
};
pub(crate) use self::update_music_group::{
    UpdateMusicGroupCommand, UpdateMusicGroupError, UpdateMusicGroupResponse,
    UpdateMusicGroupUseCase,
};
pub(crate) use self::update_playlist::{
    UpdatePlaylistCommand, UpdatePlaylistError, UpdatePlaylistResponse, UpdatePlaylistUseCase,
};
pub(crate) use self::update_series::{
    UpdateSeriesCommand, UpdateSeriesError, UpdateSeriesResponse, UpdateSeriesUseCase,
};
pub(crate) use self::update_series_episode::{
    UpdateSeriesEpisodeCommand, UpdateSeriesEpisodeError, UpdateSeriesEpisodeResponse,
    UpdateSeriesEpisodeUseCase,
};
pub(crate) use self::update_series_season::{
    UpdateSeriesSeasonCommand, UpdateSeriesSeasonError, UpdateSeriesSeasonResponse,
    UpdateSeriesSeasonUseCase,
};
pub(crate) use self::update_user::{
    UpdateUserCommand, UpdateUserError, UpdateUserResponse, UpdateUserUseCase,
};
#[cfg(test)]
pub(crate) use self::user_use_case_factory::MockUserUseCaseFactory;
pub(crate) use self::user_use_case_factory::UserUseCaseFactory;
pub(crate) use self::write_upload_chunk::{
    WriteUploadChunkCommand, WriteUploadChunkError, WriteUploadChunkResponse,
    WriteUploadChunkUseCase,
};
