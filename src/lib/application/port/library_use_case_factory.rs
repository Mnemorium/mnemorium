use std::sync::Arc;

use crate::application::port::{
    AddGalleryItemUseCase, ApplyMovieUseCase, ApplyMusicAlbumUseCase, ApplySeriesEpisodeUseCase,
    ApplySeriesUseCase, CreateGalleryUseCase, CreateMovieUseCase, CreateMusicAlbumUseCase,
    CreateMusicGroupUseCase, CreatePlaylistUseCase, CreateSeriesUseCase, DeleteGalleryItemUseCase,
    DeleteGalleryUseCase, DeletePlaylistUseCase, GetAlbumMediumUseCase, GetAlbumTrackUseCase,
    GetAlbumUseCase, GetGalleryItemUseCase, GetGalleryUseCase, GetMovieUseCase,
    GetMusicGroupUseCase, GetPlaylistTrackUseCase, GetPlaylistUseCase, GetSeriesEpisodeUseCase,
    GetSeriesSeasonUseCase, GetSeriesUseCase, IdentifyMusicUseCase, IdentifyVideoUseCase,
    IngestMediaUseCase, ListAlbumTracksUseCase, ListAlbumsUseCase, ListGalleriesUseCase,
    ListMoviesUseCase, ListMusicGroupsUseCase, ListPlaylistsUseCase, ListSeriesEpisodesUseCase,
    ListSeriesUseCase, PatchAlbumTrackUseCase, PutAlbumMediumUseCase, PutAlbumTrackUseCase,
    PutSeriesEpisodeUseCase, PutSeriesSeasonUseCase, RemovePlaylistTrackUseCase,
    ReorderPlaylistTracksUseCase, ResolveMusicMetadataUseCase, ResolveVideoMetadataUseCase,
    UpdateMovieUseCase, UpdateMusicAlbumUseCase, UpdateMusicGroupUseCase, UpdatePlaylistUseCase,
    UpdateSeriesEpisodeUseCase, UpdateSeriesSeasonUseCase, UpdateSeriesUseCase,
};

/// Builds the use cases of the Library bounded context on demand.
///
/// The inbound layer depends on this port instead of a pre-built use case, so
/// that every request gets a use case bound to the current dependencies.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait LibraryUseCaseFactory: Send + Sync {
    /// Build the use case adding one of the caller's media to a gallery.
    fn add_gallery_item(&self) -> Arc<dyn AddGalleryItemUseCase>;

    /// Build the use case applying external metadata to a film.
    fn apply_movie(&self) -> Arc<dyn ApplyMovieUseCase>;

    /// Build the use case applying external metadata to a music album.
    fn apply_music_album(&self) -> Arc<dyn ApplyMusicAlbumUseCase>;

    /// Build the use case applying external metadata to a show.
    fn apply_series(&self) -> Arc<dyn ApplySeriesUseCase>;

    /// Build the use case applying external metadata to one episode.
    fn apply_series_episode(&self) -> Arc<dyn ApplySeriesEpisodeUseCase>;

    /// Build the use case creating a gallery.
    fn create_gallery(&self) -> Arc<dyn CreateGalleryUseCase>;

    /// Build the use case creating a film from a video.
    fn create_movie(&self) -> Arc<dyn CreateMovieUseCase>;

    /// Build the use case creating a music album.
    fn create_music_album(&self) -> Arc<dyn CreateMusicAlbumUseCase>;

    /// Build the use case creating a music group.
    fn create_music_group(&self) -> Arc<dyn CreateMusicGroupUseCase>;

    /// Build the use case creating a playlist.
    fn create_playlist(&self) -> Arc<dyn CreatePlaylistUseCase>;

    /// Build the use case creating a show.
    fn create_series(&self) -> Arc<dyn CreateSeriesUseCase>;

    /// Build the use case deleting a gallery and its items.
    fn delete_gallery(&self) -> Arc<dyn DeleteGalleryUseCase>;

    /// Build the use case deleting one item of a gallery.
    fn delete_gallery_item(&self) -> Arc<dyn DeleteGalleryItemUseCase>;

    /// Build the use case deleting a playlist and its tracks.
    fn delete_playlist(&self) -> Arc<dyn DeletePlaylistUseCase>;

    /// Build the use case fetching one music album and its mediums.
    fn get_album(&self) -> Arc<dyn GetAlbumUseCase>;

    /// Build the use case fetching one medium of a music album.
    fn get_album_medium(&self) -> Arc<dyn GetAlbumMediumUseCase>;

    /// Build the use case fetching one track of a music album medium.
    fn get_album_track(&self) -> Arc<dyn GetAlbumTrackUseCase>;

    /// Build the use case fetching one gallery and its items.
    fn get_gallery(&self) -> Arc<dyn GetGalleryUseCase>;

    /// Build the use case fetching one item of a gallery.
    fn get_gallery_item(&self) -> Arc<dyn GetGalleryItemUseCase>;

    /// Build the use case fetching one film.
    fn get_movie(&self) -> Arc<dyn GetMovieUseCase>;

    /// Build the use case fetching one music group.
    fn get_music_group(&self) -> Arc<dyn GetMusicGroupUseCase>;

    /// Build the use case fetching one playlist and its tracks.
    fn get_playlist(&self) -> Arc<dyn GetPlaylistUseCase>;

    /// Build the use case fetching one track of a playlist.
    fn get_playlist_track(&self) -> Arc<dyn GetPlaylistTrackUseCase>;

    /// Build the use case fetching one show and its season summaries.
    fn get_series(&self) -> Arc<dyn GetSeriesUseCase>;

    /// Build the use case fetching one episode of a season.
    fn get_series_episode(&self) -> Arc<dyn GetSeriesEpisodeUseCase>;

    /// Build the use case fetching one season of a show.
    fn get_series_season(&self) -> Arc<dyn GetSeriesSeasonUseCase>;

    /// Build the internal use case identifying music from a query.
    fn identify_music(&self) -> Arc<dyn IdentifyMusicUseCase>;

    /// Build the internal use case identifying a video from its name.
    fn identify_video(&self) -> Arc<dyn IdentifyVideoUseCase>;

    /// Build the use case reconciling the library root with the registered
    /// files.
    fn ingest_media(&self) -> Arc<dyn IngestMediaUseCase>;

    /// Build the use case listing the tracks of a music album.
    fn list_album_tracks(&self) -> Arc<dyn ListAlbumTracksUseCase>;

    /// Build the use case listing the music albums the caller may see.
    fn list_albums(&self) -> Arc<dyn ListAlbumsUseCase>;

    /// Build the use case listing the galleries the caller may see.
    fn list_galleries(&self) -> Arc<dyn ListGalleriesUseCase>;

    /// Build the use case listing the films the caller may see.
    fn list_movies(&self) -> Arc<dyn ListMoviesUseCase>;

    /// Build the use case listing the music groups the caller may see.
    fn list_music_groups(&self) -> Arc<dyn ListMusicGroupsUseCase>;

    /// Build the use case listing the playlists the caller may see.
    fn list_playlists(&self) -> Arc<dyn ListPlaylistsUseCase>;

    /// Build the use case listing the shows the caller may see.
    fn list_series(&self) -> Arc<dyn ListSeriesUseCase>;

    /// Build the use case listing the episodes of a season.
    fn list_series_episodes(&self) -> Arc<dyn ListSeriesEpisodesUseCase>;

    /// Build the use case editing the shared recording behind a track.
    fn patch_album_track(&self) -> Arc<dyn PatchAlbumTrackUseCase>;

    /// Build the use case creating or retyping a music medium.
    fn put_album_medium(&self) -> Arc<dyn PutAlbumMediumUseCase>;

    /// Build the use case placing a recording at a medium track address.
    fn put_album_track(&self) -> Arc<dyn PutAlbumTrackUseCase>;

    /// Build the use case creating or replacing a season episode.
    fn put_series_episode(&self) -> Arc<dyn PutSeriesEpisodeUseCase>;

    /// Build the use case creating or replacing a show season.
    fn put_series_season(&self) -> Arc<dyn PutSeriesSeasonUseCase>;

    /// Build the use case removing one track of a playlist.
    fn remove_playlist_track(&self) -> Arc<dyn RemovePlaylistTrackUseCase>;

    /// Build the use case reordering the tracks of a playlist.
    fn reorder_playlist_tracks(&self) -> Arc<dyn ReorderPlaylistTracksUseCase>;

    /// Build the internal use case resolving one music target's metadata.
    fn resolve_music_metadata(&self) -> Arc<dyn ResolveMusicMetadataUseCase>;

    /// Build the internal use case resolving one video target's metadata.
    fn resolve_video_metadata(&self) -> Arc<dyn ResolveVideoMetadataUseCase>;

    /// Build the use case updating a film.
    fn update_movie(&self) -> Arc<dyn UpdateMovieUseCase>;

    /// Build the use case updating a music album.
    fn update_music_album(&self) -> Arc<dyn UpdateMusicAlbumUseCase>;

    /// Build the use case updating a music group.
    fn update_music_group(&self) -> Arc<dyn UpdateMusicGroupUseCase>;

    /// Build the use case updating a playlist.
    fn update_playlist(&self) -> Arc<dyn UpdatePlaylistUseCase>;

    /// Build the use case updating a show.
    fn update_series(&self) -> Arc<dyn UpdateSeriesUseCase>;

    /// Build the use case updating a season episode.
    fn update_series_episode(&self) -> Arc<dyn UpdateSeriesEpisodeUseCase>;

    /// Build the use case updating a show season.
    fn update_series_season(&self) -> Arc<dyn UpdateSeriesSeasonUseCase>;
}
