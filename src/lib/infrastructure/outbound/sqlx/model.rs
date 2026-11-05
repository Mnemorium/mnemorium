#![allow(
    clippy::struct_field_names,
    reason = "STY-SQL-008 names each primary key `<table>_id`, so the field repeats the struct's name"
)]

/// The audio SQL model.
pub(crate) mod audio;
/// The credential SQL model.
pub(crate) mod credential;
/// The file SQL model.
pub(crate) mod file;
/// The gallery SQL model.
pub(crate) mod gallery;
/// The gallery item SQL model.
pub(crate) mod gallery_item;
/// The image SQL model.
pub(crate) mod image;
/// The movie SQL model.
pub(crate) mod movie;
/// The music album SQL model.
pub(crate) mod music_album;
/// The music group SQL model.
pub(crate) mod music_group;
/// The music playlist SQL model.
pub(crate) mod music_playlist;
/// The permission SQL model.
pub(crate) mod permission;
/// The scope SQL model.
pub(crate) mod scope;
/// The series SQL model.
pub(crate) mod series;
/// The upload SQL model.
pub(crate) mod upload;
/// The user SQL model.
pub(crate) mod user;
/// The video SQL model.
pub(crate) mod video;
