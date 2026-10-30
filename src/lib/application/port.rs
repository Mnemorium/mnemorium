#![allow(
    clippy::struct_field_names,
    reason = "a port item names its fields after the identifiers the API and SQL sections mandate, and mockall names a mock's fields after the trait methods"
)]

pub mod add_gallery_item;
pub mod asset_use_case_factory;
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
pub mod get_permission;
pub mod get_playlist;
pub mod get_playlist_track;
pub mod get_series;
pub mod get_series_episode;
pub mod get_series_season;
pub mod get_upload;
pub mod get_user;
pub mod identify_video;
pub mod identity_use_case_factory;
pub mod ingest_media;
pub mod initialize_root_admin;
pub mod library_use_case_factory;
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
pub mod media_change_source;
pub mod patch_credential;
pub mod register_user;
pub mod remove_playlist_track;
pub mod reorder_playlist_tracks;
pub mod replace_permission;
pub mod update_playlist;
pub mod update_user;
pub mod user_use_case_factory;
pub mod video_summary;
pub mod write_upload_chunk;
