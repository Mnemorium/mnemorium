#![allow(
    clippy::struct_field_names,
    reason = "a port item names its fields after the identifiers the API and SQL sections mandate, and mockall names a mock's fields after the trait methods"
)]

/// The add gallery item use-case port.
pub(crate) mod add_gallery_item;
/// The apply movie metadata use-case port.
pub(crate) mod apply_movie;
/// The apply music album use-case port.
pub(crate) mod apply_music_album;
/// The apply series metadata use-case port.
pub(crate) mod apply_series;
/// The apply series episode metadata use-case port.
pub(crate) mod apply_series_episode;
/// The asset use-case factory port.
pub(crate) mod asset_use_case_factory;
/// The begin upload use-case port.
pub(crate) mod begin_upload;
/// The cancel upload use-case port.
pub(crate) mod cancel_upload;
/// The complete upload use-case port.
pub(crate) mod complete_upload;
/// The create gallery use-case port.
pub(crate) mod create_gallery;
/// The create movie use-case port.
pub(crate) mod create_movie;
/// The create music album use-case port.
pub(crate) mod create_music_album;
/// The create music group use-case port.
pub(crate) mod create_music_group;
/// The create playlist use-case port.
pub(crate) mod create_playlist;
/// The create series use-case port.
pub(crate) mod create_series;
/// The delete gallery use-case port.
pub(crate) mod delete_gallery;
/// The delete gallery item use-case port.
pub(crate) mod delete_gallery_item;
/// The delete playlist use-case port.
pub(crate) mod delete_playlist;
/// The ensure storage directories use-case port.
pub(crate) mod ensure_storage_directories;
/// The get album use-case port.
pub(crate) mod get_album;
/// The get album medium use-case port.
pub(crate) mod get_album_medium;
/// The get album track use-case port.
pub(crate) mod get_album_track;
/// The get asset content use-case port.
pub(crate) mod get_asset_content;
/// The get current user use-case port.
pub(crate) mod get_current_user;
/// The get file use-case port.
pub(crate) mod get_file;
/// The get gallery use-case port.
pub(crate) mod get_gallery;
/// The get gallery item use-case port.
pub(crate) mod get_gallery_item;
/// The get image use-case port.
pub(crate) mod get_image;
/// The get movie use-case port.
pub(crate) mod get_movie;
/// The get music group use-case port.
pub(crate) mod get_music_group;
/// The get permission use-case port.
pub(crate) mod get_permission;
/// The get playlist use-case port.
pub(crate) mod get_playlist;
/// The get playlist track use-case port.
pub(crate) mod get_playlist_track;
/// The get series use-case port.
pub(crate) mod get_series;
/// The get series episode use-case port.
pub(crate) mod get_series_episode;
/// The get series season use-case port.
pub(crate) mod get_series_season;
/// The get upload use-case port.
pub(crate) mod get_upload;
/// The get user use-case port.
pub(crate) mod get_user;
/// The identify music use-case port.
pub(crate) mod identify_music;
/// The identify video use-case port.
pub(crate) mod identify_video;
/// The identity use-case factory port.
pub(crate) mod identity_use_case_factory;
/// The ingest media use-case port.
pub(crate) mod ingest_media;
/// The initialize root admin use-case port.
pub(crate) mod initialize_root_admin;
/// The library use-case factory port.
pub(crate) mod library_use_case_factory;
/// The list album tracks use-case port.
pub(crate) mod list_album_tracks;
/// The list albums use-case port.
pub(crate) mod list_albums;
/// The list galleries use-case port.
pub(crate) mod list_galleries;
/// The list movies use-case port.
pub(crate) mod list_movies;
/// The list music groups use-case port.
pub(crate) mod list_music_groups;
/// The list playlists use-case port.
pub(crate) mod list_playlists;
/// The list series use-case port.
pub(crate) mod list_series;
/// The list series episodes use-case port.
pub(crate) mod list_series_episodes;
/// The list users use-case port.
pub(crate) mod list_users;
/// The load configuration use-case port.
pub(crate) mod load_configuration;
/// The load state use-case port.
pub(crate) mod load_state;
/// The login user use-case port.
pub(crate) mod login_user;
/// The media change source use-case port.
pub(crate) mod media_change_source;
/// The patch album track use-case port.
pub(crate) mod patch_album_track;
/// The patch credential use-case port.
pub(crate) mod patch_credential;
/// The put album medium use-case port.
pub(crate) mod put_album_medium;
/// The put album track use-case port.
pub(crate) mod put_album_track;
/// The put series episode use-case port.
pub(crate) mod put_series_episode;
/// The put series season use-case port.
pub(crate) mod put_series_season;
/// The reap uploads use-case port.
pub(crate) mod reap_uploads;
/// The register user use-case port.
pub(crate) mod register_user;
/// The remove playlist track use-case port.
pub(crate) mod remove_playlist_track;
/// The reorder playlist tracks use-case port.
pub(crate) mod reorder_playlist_tracks;
/// The replace permission use-case port.
pub(crate) mod replace_permission;
/// The resolve music metadata use-case port.
pub(crate) mod resolve_music_metadata;
/// The resolve video metadata use-case port.
pub(crate) mod resolve_video_metadata;
/// The update movie use-case port.
pub(crate) mod update_movie;
/// The update music album use-case port.
pub(crate) mod update_music_album;
/// The update music group use-case port.
pub(crate) mod update_music_group;
/// The update playlist use-case port.
pub(crate) mod update_playlist;
/// The update series use-case port.
pub(crate) mod update_series;
/// The update series episode use-case port.
pub(crate) mod update_series_episode;
/// The update series season use-case port.
pub(crate) mod update_series_season;
/// The update user use-case port.
pub(crate) mod update_user;
/// The user use-case factory port.
pub(crate) mod user_use_case_factory;
/// The write upload chunk use-case port.
pub(crate) mod write_upload_chunk;
