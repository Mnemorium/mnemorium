#![allow(
    clippy::struct_field_names,
    reason = "STY-SQL-008 names each primary key `<table>_id`, so the field repeats the struct's name"
)]

/// The asset scope SQL model.
mod asset_scope;
/// The audio SQL model.
mod audio;
/// The credential SQL model.
mod credential;
/// The file SQL model.
mod file;
/// The gallery SQL model.
mod gallery;
/// The gallery item SQL model.
mod gallery_item;
/// The image SQL model.
mod image;
/// The movie SQL model.
mod movie;
/// The music album SQL model.
mod music_album;
/// The music group SQL model.
mod music_group;
/// The music playlist SQL model.
mod music_playlist;
/// The permission SQL model.
mod permission;
/// The series SQL model.
mod series;
/// The upload SQL model.
mod upload;
/// The user SQL model.
mod user;
/// The video SQL model.
mod video;

pub(crate) use self::asset_scope::{asset_scope_token, parse_asset_scope};
pub(crate) use self::audio::Audio;
pub(crate) use self::credential::Credential;
pub(crate) use self::file::File;
pub(crate) use self::gallery::Gallery;
pub(crate) use self::gallery_item::{GalleryItem, GalleryItemDetail};
pub(crate) use self::image::{Image, Orientation};
pub(crate) use self::movie::MovieRow;
pub(crate) use self::music_album::MusicAlbumRow;
pub(crate) use self::music_group::MusicGroupRow;
pub(crate) use self::music_playlist::MusicPlaylistRow;
pub(crate) use self::permission::{PermissionResourceRow, PermissionRow};
pub(crate) use self::series::SeriesRow;
pub(crate) use self::upload::Upload;
pub(crate) use self::user::{Role, User};
pub(crate) use self::video::{ScanType, Video, parse_scan_type};
