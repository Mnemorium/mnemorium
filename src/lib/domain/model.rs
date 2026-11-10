/// The artwork domain model.
mod artwork;
/// The asset domain model.
mod asset;
/// The asset scope domain model.
mod asset_scope;
/// The audio domain model.
mod audio;
/// The configuration domain model.
mod configuration;
/// The credential domain model.
mod credential;
/// The episode domain model.
mod episode;
/// The external reference domain model.
mod external_ref;
/// The file domain model.
mod file;
/// The gallery domain model.
mod gallery;
/// The gallery item domain model.
mod gallery_item;
/// The genre domain model.
mod genre;
/// The image domain model.
mod image;
/// The instance state domain model.
mod instance_state;
/// The integrity hash domain model.
mod integrity_hash;
/// The media metadata domain model.
mod media_metadata;
/// The media query domain model.
mod media_query;
/// The mime type domain model.
mod mime_type;
/// The movie domain model.
mod movie;
/// The music album domain model.
mod music_album;
/// The music candidate domain model.
mod music_candidate;
/// The music group domain model.
mod music_group;
/// The music medium domain model.
mod music_medium;
/// The music metadata domain model.
mod music_metadata;
/// The music playlist domain model.
mod music_playlist;
/// The music query domain model.
mod music_query;
/// The music recording domain model.
mod music_recording;
mod partial_configuration;
/// The permission domain model.
mod permission;
/// The season domain model.
mod season;
/// The series domain model.
mod series;
/// The upload domain model.
mod upload;
/// The user domain model.
mod user;
/// The video domain model.
mod video;
/// The video candidate domain model.
mod video_candidate;

pub(crate) use self::artwork::Artwork;
pub(crate) use self::asset::AssetContext;
pub(crate) use self::asset_scope::AssetScope;
#[cfg(test)]
pub(crate) use self::audio::AudioError;
pub(crate) use self::audio::{Audio, ProbedAudio};
pub(crate) use self::configuration::{
    Asset, AssetUpload, ClientIpHeader, Configuration, Jwt, Library, LibraryMetadata,
    LibraryStartup, Logging, MusicProvider, MusicProviders, Persistence, ProviderId, RateLimit,
    RateLimitError, Rotation, Security, Sqlite3, VideoProvider, VideoProviders,
    parse_trusted_proxies,
};
pub(crate) use self::credential::Credential;
#[cfg(test)]
pub(crate) use self::credential::CredentialError;
pub(crate) use self::episode::{Episode, EpisodeIndex};
pub(crate) use self::external_ref::ExternalRef;
#[cfg(test)]
pub(crate) use self::file::FileError;
pub(crate) use self::file::{File, Origin};
pub(crate) use self::gallery::Gallery;
pub(crate) use self::gallery_item::{GalleryItem, GalleryItemMedia};
pub(crate) use self::genre::Genre;
pub(crate) use self::image::{Image, Orientation, ProbedImage};
#[cfg(test)]
pub(crate) use self::instance_state::InstanceStateError;
pub(crate) use self::instance_state::{InstanceState, SECRET_HEX_LENGTH};
pub(crate) use self::integrity_hash::{DIGEST_HEX_LENGTH, IntegrityHash, IntegrityHashError};
pub(crate) use self::media_metadata::MediaMetadata;
#[cfg(test)]
pub(crate) use self::media_query::MediaQueryError;
pub(crate) use self::media_query::{EpisodePosition, MediaQuery, VideoKind};
pub(crate) use self::mime_type::MimeType;
pub(crate) use self::movie::Movie;
pub(crate) use self::music_album::{MusicAlbum, ProductionType};
pub(crate) use self::music_candidate::{MusicCandidate, MusicTarget};
pub(crate) use self::music_group::MusicGroup;
pub(crate) use self::music_medium::{MediumIndex, MusicMedium};
pub(crate) use self::music_metadata::{
    ArtistCredit, Edition, MediumFormat, MediumSummary, MusicMetadata, ReleaseStatus,
};
pub(crate) use self::music_playlist::{MusicPlaylist, MusicPlaylistError};
#[cfg(test)]
pub(crate) use self::music_query::MusicQueryError;
pub(crate) use self::music_query::{MusicKind, MusicQuery};
pub(crate) use self::music_recording::{MusicRecording, MusicRecordingError, TrackIndex};
pub(crate) use self::partial_configuration::{
    PartialAsset, PartialAssetUpload, PartialConfiguration, PartialJwt, PartialLibrary,
    PartialLogging, PartialPersistence, PartialRateLimit, PartialSecurity, PartialSqlite3,
};
#[cfg(test)]
pub(crate) use self::partial_configuration::{
    PartialLibraryMetadata, PartialLibraryStartup, PartialMusicProvider, PartialMusicProviders,
    PartialVideoMetadata, PartialVideoProvider,
};
pub(crate) use self::permission::{
    ACTIONS, Action, Global, Permission, PermissionError, PermissionPreset, Protected,
    ResourceKind, ResourceRef, RootAccess, Scope,
};
pub(crate) use self::season::{Season, SeasonIndex};
pub(crate) use self::series::Series;
pub(crate) use self::upload::{ChunkBitmap, MAX_TOTAL_CHUNKS, Upload, UploadError};
pub(crate) use self::user::{Role, User, UserError};
pub(crate) use self::video::{ProbedVideo, ScanType, Video};
pub(crate) use self::video_candidate::{Candidate, Target};
