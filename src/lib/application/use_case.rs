pub mod add_gallery_item;
pub mod begin_upload;
pub mod complete_upload;
pub mod create_gallery;
pub mod create_playlist;
pub mod delete_gallery;
pub mod delete_gallery_item;
pub mod delete_playlist;
pub mod ensure_storage_directories;
pub mod get_album;
pub mod get_album_medium;
pub mod get_album_track;
pub mod get_current_user;
pub mod get_gallery;
pub mod get_gallery_item;
pub mod get_movie;
pub mod get_playlist;
pub mod get_playlist_track;
pub mod get_series;
pub mod get_series_episode;
pub mod get_series_season;
pub mod get_upload;
pub mod get_user;
pub mod initialize_root_admin;
pub mod list_album_tracks;
pub mod list_albums;
pub mod list_galleries;
pub mod list_movies;
pub mod list_playlists;
pub mod list_series;
pub mod list_series_episodes;
pub mod list_users;
pub mod load_configuration;
pub mod load_state;
pub mod login_user;
pub mod patch_credential;
pub mod register_user;
pub mod remove_playlist_track;
pub mod reorder_playlist_tracks;
pub mod update_playlist;
pub mod update_user;
pub(crate) mod upload_layout;
pub(crate) mod upload_session;
pub mod write_upload_chunk;

/// Fixtures shared by the application use-case test modules.
///
/// A fixture declared here is one the use-case tests would otherwise restate in
/// every module that needs it (`STY-RUST-086`). It is an inline module because a
/// test module never lives in its own file (`TEST-001`).
#[cfg(test)]
mod test_fixtures {
    /// Storage root the use cases compose their paths against.
    pub(crate) const ROOT: &str = "/storage";

    /// Token lifetime the test configurations carry, in seconds.
    pub(crate) const TTL_SECONDS: u64 = 3600;
}
