/// The audio repository adapter.
pub(crate) mod audio_repository;
/// The credential repository adapter.
pub(crate) mod credential_repository;
pub(crate) mod error_mapping;
/// The file repository adapter.
pub(crate) mod file_repository;
/// The gallery repository adapter.
pub(crate) mod gallery_repository;
/// The image repository adapter.
pub(crate) mod image_repository;
/// The mime type repository adapter.
pub(crate) mod mime_type_repository;
/// The SQL row models the repositories decode into.
pub(crate) mod model;
/// The movie repository adapter.
pub(crate) mod movie_repository;
/// The music album repository adapter.
pub(crate) mod music_album_repository;
/// The music group repository adapter.
pub(crate) mod music_group_repository;
/// The music playlist repository adapter.
pub(crate) mod music_playlist_repository;
/// The permission repository adapter.
pub(crate) mod permission_repository;
/// The shared SQL filter and sort clause builders.
pub(crate) mod query_filter;
/// The series repository adapter.
pub(crate) mod series_repository;
/// The pool initialiser and its migrations.
pub(crate) mod sqlite3;
/// The `SQLite` unit of work and its factory.
pub(crate) mod unit_of_work;
/// The upload repository adapter.
pub(crate) mod upload_repository;
/// The user repository adapter.
pub(crate) mod user_repository;
/// The video repository adapter.
pub(crate) mod video_repository;
