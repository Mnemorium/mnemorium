/// The add gallery item use case implementation.
mod add_gallery_item;
/// The apply movie metadata use case implementation.
mod apply_movie;
/// The apply music album use case implementation.
mod apply_music_album;
/// The apply series metadata use case implementation.
mod apply_series;
/// The apply series episode metadata use case implementation.
mod apply_series_episode;
/// The begin upload use case implementation.
mod begin_upload;
/// The cancel upload use case implementation.
mod cancel_upload;
/// The complete upload use case implementation.
mod complete_upload;
/// The create gallery use case implementation.
mod create_gallery;
/// The create movie use case implementation.
mod create_movie;
/// The create music album use case implementation.
mod create_music_album;
/// The create music group use case implementation.
mod create_music_group;
/// The create playlist use case implementation.
mod create_playlist;
/// The create series use case implementation.
mod create_series;
/// The delete gallery use case implementation.
mod delete_gallery;
/// The delete gallery item use case implementation.
mod delete_gallery_item;
/// The delete playlist use case implementation.
mod delete_playlist;
/// The delete user use case implementation.
mod delete_user;
/// The ensure storage directories use case implementation.
mod ensure_storage_directories;
/// The get album use case implementation.
mod get_album;
/// The get album medium use case implementation.
mod get_album_medium;
/// The get album track use case implementation.
mod get_album_track;
/// The get asset content use case implementation.
mod get_asset_content;
/// The get audio use case implementation.
mod get_audio;
/// The get current user use case implementation.
mod get_current_user;
/// The get file use case implementation.
mod get_file;
/// The get gallery use case implementation.
mod get_gallery;
/// The get gallery item use case implementation.
mod get_gallery_item;
/// The get image use case implementation.
mod get_image;
/// The get movie use case implementation.
mod get_movie;
/// The get music group use case implementation.
mod get_music_group;
/// The get playlist use case implementation.
mod get_playlist;
/// The get playlist track use case implementation.
mod get_playlist_track;
/// The get series use case implementation.
mod get_series;
/// The get series episode use case implementation.
mod get_series_episode;
/// The get series season use case implementation.
mod get_series_season;
/// The get upload use case implementation.
mod get_upload;
/// The get user use case implementation.
mod get_user;
/// The get video use case implementation.
mod get_video;
/// The identify music use case implementation.
mod identify_music;
/// The identify video use case implementation.
mod identify_video;
/// The ingest media use case implementation.
mod ingest_media;
/// The initialize root admin use case implementation.
mod initialize_root_admin;
/// The list album tracks use case implementation.
mod list_album_tracks;
/// The list albums use case implementation.
mod list_albums;
/// The list galleries use case implementation.
mod list_galleries;
/// The list movies use case implementation.
mod list_movies;
/// The list music groups use case implementation.
mod list_music_groups;
/// The list playlists use case implementation.
mod list_playlists;
/// The list series use case implementation.
mod list_series;
/// The list series episodes use case implementation.
mod list_series_episodes;
/// The list users use case implementation.
mod list_users;
/// The load configuration use case implementation.
mod load_configuration;
/// The load state use case implementation.
mod load_state;
/// The login user use case implementation.
mod login_user;
/// The patch album track use case implementation.
mod patch_album_track;
/// The patch credential use case implementation.
mod patch_credential;
/// The permission use case implementations.
mod permission;
/// The put album medium use case implementation.
mod put_album_medium;
/// The put album track use case implementation.
mod put_album_track;
/// The put series episode use case implementation.
mod put_series_episode;
/// The put series season use case implementation.
mod put_series_season;
/// The reap uploads use case implementation.
mod reap_uploads;
/// The regenerate token use case implementation.
mod regenerate_token;
/// The register user use case implementation.
mod register_user;
/// The remove playlist track use case implementation.
mod remove_playlist_track;
/// The reorder playlist tracks use case implementation.
mod reorder_playlist_tracks;
/// The resolve music metadata use case implementation.
mod resolve_music_metadata;
/// The resolve video metadata use case implementation.
mod resolve_video_metadata;
/// The update movie use case implementation.
mod update_movie;
/// The update music album use case implementation.
mod update_music_album;
/// The update music group use case implementation.
mod update_music_group;
/// The update playlist use case implementation.
mod update_playlist;
/// The update series use case implementation.
mod update_series;
/// The update series episode use case implementation.
mod update_series_episode;
/// The update series season use case implementation.
mod update_series_season;
/// The update user use case implementation.
mod update_user;
mod upload_layout;
mod upload_session;
/// The write upload chunk use case implementation.
mod write_upload_chunk;

pub(crate) use self::add_gallery_item::AddGalleryItem;
pub(crate) use self::apply_movie::ApplyMovie;
pub(crate) use self::apply_music_album::ApplyMusicAlbum;
pub(crate) use self::apply_series::ApplySeries;
pub(crate) use self::apply_series_episode::ApplySeriesEpisode;
pub(crate) use self::begin_upload::BeginUpload;
pub(crate) use self::cancel_upload::CancelUpload;
pub(crate) use self::complete_upload::CompleteUpload;
pub(crate) use self::create_gallery::CreateGallery;
pub(crate) use self::create_movie::CreateMovie;
pub(crate) use self::create_music_album::CreateMusicAlbum;
pub(crate) use self::create_music_group::CreateMusicGroup;
pub(crate) use self::create_playlist::CreatePlaylist;
pub(crate) use self::create_series::CreateSeries;
pub(crate) use self::delete_gallery::DeleteGallery;
pub(crate) use self::delete_gallery_item::DeleteGalleryItem;
pub(crate) use self::delete_playlist::DeletePlaylist;
pub(crate) use self::delete_user::DeleteUser;
pub(crate) use self::ensure_storage_directories::EnsureStorageDirectories;
pub(crate) use self::get_album::GetAlbum;
pub(crate) use self::get_album_medium::GetAlbumMedium;
pub(crate) use self::get_album_track::GetAlbumTrack;
pub(crate) use self::get_asset_content::GetAssetContent;
pub(crate) use self::get_audio::GetAudio;
pub(crate) use self::get_current_user::GetCurrentUser;
pub(crate) use self::get_file::GetFile;
pub(crate) use self::get_gallery::GetGallery;
pub(crate) use self::get_gallery_item::GetGalleryItem;
pub(crate) use self::get_image::GetImage;
pub(crate) use self::get_movie::GetMovie;
pub(crate) use self::get_music_group::GetMusicGroup;
pub(crate) use self::get_playlist::GetPlaylist;
pub(crate) use self::get_playlist_track::GetPlaylistTrack;
pub(crate) use self::get_series::GetSeries;
pub(crate) use self::get_series_episode::GetSeriesEpisode;
pub(crate) use self::get_series_season::GetSeriesSeason;
pub(crate) use self::get_upload::GetUpload;
pub(crate) use self::get_user::GetUser;
pub(crate) use self::get_video::GetVideo;
pub(crate) use self::identify_music::IdentifyMusic;
pub(crate) use self::identify_video::IdentifyVideo;
pub(crate) use self::ingest_media::IngestMedia;
pub(crate) use self::initialize_root_admin::InitializeRootAdmin;
pub(crate) use self::list_album_tracks::ListAlbumTracks;
pub(crate) use self::list_albums::ListAlbums;
pub(crate) use self::list_galleries::ListGalleries;
pub(crate) use self::list_movies::ListMovies;
pub(crate) use self::list_music_groups::ListMusicGroups;
pub(crate) use self::list_playlists::ListPlaylists;
pub(crate) use self::list_series::ListSeries;
pub(crate) use self::list_series_episodes::ListSeriesEpisodes;
pub(crate) use self::list_users::ListUsers;
pub(crate) use self::load_configuration::LoadConfiguration;
pub(crate) use self::load_state::LoadState;
pub(crate) use self::login_user::LoginUser;
pub(crate) use self::patch_album_track::PatchAlbumTrack;
pub(crate) use self::patch_credential::PatchCredential;
pub(crate) use self::permission::{
    GetPermissionSummary, GetResourcePermission, PutResourcePermission, RevokeShare, ShareResource,
};
pub(crate) use self::put_album_medium::PutAlbumMedium;
pub(crate) use self::put_album_track::PutAlbumTrack;
pub(crate) use self::put_series_episode::PutSeriesEpisode;
pub(crate) use self::put_series_season::PutSeriesSeason;
pub(crate) use self::reap_uploads::ReapUploads;
pub(crate) use self::regenerate_token::RegenerateToken;
pub(crate) use self::register_user::RegisterUser;
pub(crate) use self::remove_playlist_track::RemovePlaylistTrack;
pub(crate) use self::reorder_playlist_tracks::ReorderPlaylistTracks;
pub(crate) use self::resolve_music_metadata::ResolveMusicMetadata;
pub(crate) use self::resolve_video_metadata::ResolveVideoMetadata;
pub(crate) use self::update_movie::UpdateMovie;
pub(crate) use self::update_music_album::UpdateMusicAlbum;
pub(crate) use self::update_music_group::UpdateMusicGroup;
pub(crate) use self::update_playlist::UpdatePlaylist;
pub(crate) use self::update_series::UpdateSeries;
pub(crate) use self::update_series_episode::UpdateSeriesEpisode;
pub(crate) use self::update_series_season::UpdateSeriesSeason;
pub(crate) use self::update_user::UpdateUser;
pub(crate) use self::upload_layout::{
    MEDIA_ROOT, media_path, media_root, staged_path, uploads_path,
};
pub(crate) use self::upload_session::{caller_file_id, expiry, received_bitmap};
pub(crate) use self::write_upload_chunk::WriteUploadChunk;

/// Fixtures shared by the application use-case test modules.
///
/// A fixture declared here is one the use-case tests would otherwise restate in
/// every module that needs it (`STY-RUST-086`). It is an inline module because a
/// test module never lives in its own file (`TEST-001`).
#[cfg(test)]
mod test_fixtures {
    /// Token lifetime the test configurations carry, in seconds.
    pub(crate) const TTL_SECONDS: u64 = 3600;
}
