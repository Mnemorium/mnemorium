/// The add gallery item use case implementation.
pub(crate) mod add_gallery_item;
/// The apply music album use case implementation.
pub(crate) mod apply_music_album;
pub(crate) mod asset_access;
/// The begin upload use case implementation.
pub(crate) mod begin_upload;
/// The complete upload use case implementation.
pub(crate) mod complete_upload;
/// The create gallery use case implementation.
pub(crate) mod create_gallery;
/// The create music album use case implementation.
pub(crate) mod create_music_album;
/// The create music group use case implementation.
pub(crate) mod create_music_group;
/// The create playlist use case implementation.
pub(crate) mod create_playlist;
/// The delete gallery use case implementation.
pub(crate) mod delete_gallery;
/// The delete gallery item use case implementation.
pub(crate) mod delete_gallery_item;
/// The delete playlist use case implementation.
pub(crate) mod delete_playlist;
/// The ensure storage directories use case implementation.
pub(crate) mod ensure_storage_directories;
/// The get album use case implementation.
pub(crate) mod get_album;
/// The get album medium use case implementation.
pub(crate) mod get_album_medium;
/// The get album track use case implementation.
pub(crate) mod get_album_track;
/// The get asset content use case implementation.
pub(crate) mod get_asset_content;
/// The get current user use case implementation.
pub(crate) mod get_current_user;
/// The get file use case implementation.
pub(crate) mod get_file;
/// The get gallery use case implementation.
pub(crate) mod get_gallery;
/// The get gallery item use case implementation.
pub(crate) mod get_gallery_item;
/// The get image use case implementation.
pub(crate) mod get_image;
/// The get movie use case implementation.
pub(crate) mod get_movie;
/// The get music group use case implementation.
pub(crate) mod get_music_group;
/// The get permission use case implementation.
pub(crate) mod get_permission;
/// The get playlist use case implementation.
pub(crate) mod get_playlist;
/// The get playlist track use case implementation.
pub(crate) mod get_playlist_track;
/// The get series use case implementation.
pub(crate) mod get_series;
/// The get series episode use case implementation.
pub(crate) mod get_series_episode;
/// The get series season use case implementation.
pub(crate) mod get_series_season;
/// The get upload use case implementation.
pub(crate) mod get_upload;
/// The get user use case implementation.
pub(crate) mod get_user;
/// The identify music use case implementation.
pub(crate) mod identify_music;
/// The identify video use case implementation.
pub(crate) mod identify_video;
/// The ingest media use case implementation.
pub(crate) mod ingest_media;
/// The initialize root admin use case implementation.
pub(crate) mod initialize_root_admin;
/// The list album tracks use case implementation.
pub(crate) mod list_album_tracks;
/// The list albums use case implementation.
pub(crate) mod list_albums;
/// The list galleries use case implementation.
pub(crate) mod list_galleries;
/// The list movies use case implementation.
pub(crate) mod list_movies;
/// The list music groups use case implementation.
pub(crate) mod list_music_groups;
/// The list playlists use case implementation.
pub(crate) mod list_playlists;
/// The list series use case implementation.
pub(crate) mod list_series;
/// The list series episodes use case implementation.
pub(crate) mod list_series_episodes;
/// The list users use case implementation.
pub(crate) mod list_users;
/// The load configuration use case implementation.
pub(crate) mod load_configuration;
/// The load state use case implementation.
pub(crate) mod load_state;
/// The login user use case implementation.
pub(crate) mod login_user;
/// The patch album track use case implementation.
pub(crate) mod patch_album_track;
/// The patch credential use case implementation.
pub(crate) mod patch_credential;
/// The put album medium use case implementation.
pub(crate) mod put_album_medium;
/// The put album track use case implementation.
pub(crate) mod put_album_track;
/// The register user use case implementation.
pub(crate) mod register_user;
/// The remove playlist track use case implementation.
pub(crate) mod remove_playlist_track;
/// The reorder playlist tracks use case implementation.
pub(crate) mod reorder_playlist_tracks;
/// The replace permission use case implementation.
pub(crate) mod replace_permission;
/// The resolve music metadata use case implementation.
pub(crate) mod resolve_music_metadata;
/// The update music album use case implementation.
pub(crate) mod update_music_album;
/// The update music group use case implementation.
pub(crate) mod update_music_group;
/// The update playlist use case implementation.
pub(crate) mod update_playlist;
/// The update user use case implementation.
pub(crate) mod update_user;
pub(crate) mod upload_layout;
pub(crate) mod upload_session;
/// The write upload chunk use case implementation.
pub(crate) mod write_upload_chunk;

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
