use std::sync::Arc;

use tokio::sync::Semaphore;

use crate::application::port::{
    AddGalleryItemUseCase, ApplyMovieUseCase, ApplyMusicAlbumUseCase, ApplySeriesEpisodeUseCase,
    ApplySeriesUseCase, CreateGalleryUseCase, CreateMovieUseCase, CreateMusicAlbumUseCase,
    CreateMusicGroupUseCase, CreatePlaylistUseCase, CreateSeriesUseCase, DeleteGalleryItemUseCase,
    DeleteGalleryUseCase, DeletePlaylistUseCase, GetAlbumMediumUseCase, GetAlbumTrackUseCase,
    GetAlbumUseCase, GetGalleryItemUseCase, GetGalleryUseCase, GetMovieUseCase,
    GetMusicGroupUseCase, GetPlaylistTrackUseCase, GetPlaylistUseCase, GetSeriesEpisodeUseCase,
    GetSeriesSeasonUseCase, GetSeriesUseCase, IdentifyMusicUseCase, IdentifyVideoUseCase,
    IngestMediaUseCase, LibraryUseCaseFactory, ListAlbumTracksUseCase, ListAlbumsUseCase,
    ListGalleriesUseCase, ListMoviesUseCase, ListMusicGroupsUseCase, ListPlaylistsUseCase,
    ListSeriesEpisodesUseCase, ListSeriesUseCase, PatchAlbumTrackUseCase, PutAlbumMediumUseCase,
    PutAlbumTrackUseCase, PutSeriesEpisodeUseCase, PutSeriesSeasonUseCase,
    RemovePlaylistTrackUseCase, ReorderPlaylistTracksUseCase, ResolveMusicMetadataUseCase,
    ResolveVideoMetadataUseCase, UpdateMovieUseCase, UpdateMusicAlbumUseCase,
    UpdateMusicGroupUseCase, UpdatePlaylistUseCase, UpdateSeriesEpisodeUseCase,
    UpdateSeriesSeasonUseCase, UpdateSeriesUseCase,
};
use crate::application::use_case::{
    AddGalleryItem, ApplyMovie, ApplyMusicAlbum, ApplySeries, ApplySeriesEpisode, CreateGallery,
    CreateMovie, CreateMusicAlbum, CreateMusicGroup, CreatePlaylist, CreateSeries, DeleteGallery,
    DeleteGalleryItem, DeletePlaylist, GetAlbum, GetAlbumMedium, GetAlbumTrack, GetGallery,
    GetGalleryItem, GetMovie, GetMusicGroup, GetPlaylist, GetPlaylistTrack, GetSeries,
    GetSeriesEpisode, GetSeriesSeason, IdentifyMusic, IdentifyVideo, IngestMedia, ListAlbumTracks,
    ListAlbums, ListGalleries, ListMovies, ListMusicGroups, ListPlaylists, ListSeries,
    ListSeriesEpisodes, PatchAlbumTrack, PutAlbumMedium, PutAlbumTrack, PutSeriesEpisode,
    PutSeriesSeason, RemovePlaylistTrack, ReorderPlaylistTracks, ResolveMusicMetadata,
    ResolveVideoMetadata, UpdateMovie, UpdateMusicAlbum, UpdateMusicGroup, UpdatePlaylist,
    UpdateSeries, UpdateSeriesEpisode, UpdateSeriesSeason, media_root,
};
use crate::domain::model::Configuration;
use crate::infrastructure::outbound::LocalMediaProbe;
use crate::infrastructure::outbound::file_system::{FileSystemStorage, LocalMediaSource};
use crate::infrastructure::outbound::infer::InferContentTypeSniffer;
use crate::infrastructure::outbound::metadata::{
    FailoverVideoMetadataProvider, configured_links, configured_music_provider,
};
use crate::infrastructure::outbound::sqlx::SqlxUnitOfWorkFactory;

/// Builds the Library use cases.
///
/// The accessors build the stateless file storage and media adapters at the
/// point of use, so the media-library use cases compose their paths from the
/// fixed storage root the Asset use cases write to. The probe permit pool is a
/// startup value the composition root captured; each accessor builds the probe
/// adapter afresh from it and stores neither.
pub(crate) struct RuntimeLibraryUseCaseFactory {
    /// Immutable configuration the metadata chain is resolved from.
    configuration: Arc<Configuration>,
    /// Permit pool bounding concurrent probes, shared with the upload path.
    probe_permits: Arc<Semaphore>,
    /// Factory opening the unit of work wrapping the Library use cases.
    unit_of_work_factory: Arc<SqlxUnitOfWorkFactory>,
}

impl RuntimeLibraryUseCaseFactory {
    /// Create a new factory.
    #[must_use]
    pub(crate) fn new(
        unit_of_work_factory: Arc<SqlxUnitOfWorkFactory>,
        probe_permits: Arc<Semaphore>,
        configuration: Arc<Configuration>,
    ) -> Self {
        Self {
            configuration,
            probe_permits,
            unit_of_work_factory,
        }
    }
}

impl LibraryUseCaseFactory for RuntimeLibraryUseCaseFactory {
    fn add_gallery_item(&self) -> Arc<dyn AddGalleryItemUseCase> {
        Arc::new(AddGalleryItem::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn apply_movie(&self) -> Arc<dyn ApplyMovieUseCase> {
        // The chain is configuration-derived, so the accessor rebuilds it from
        // the immutable configuration on every call (STY-RUST-062).
        let links = configured_links(self.configuration.library().metadata().video());
        let provider = if links.is_empty() {
            None
        } else {
            Some(Arc::new(FailoverVideoMetadataProvider::new(links)))
        };
        Arc::new(ApplyMovie::new(
            Arc::clone(&self.unit_of_work_factory),
            provider,
        ))
    }
    fn apply_music_album(&self) -> Arc<dyn ApplyMusicAlbumUseCase> {
        // The provider is configuration-derived, so the accessor rebuilds it from
        // the immutable configuration on every call (STY-RUST-062).
        let provider = configured_music_provider(self.configuration.library().metadata().music())
            .map(Arc::new);
        Arc::new(ApplyMusicAlbum::new(
            Arc::clone(&self.unit_of_work_factory),
            provider,
        ))
    }

    fn apply_series(&self) -> Arc<dyn ApplySeriesUseCase> {
        // The chain is configuration-derived, so the accessor rebuilds it from
        // the immutable configuration on every call (STY-RUST-062).
        let links = configured_links(self.configuration.library().metadata().video());
        let provider = if links.is_empty() {
            None
        } else {
            Some(Arc::new(FailoverVideoMetadataProvider::new(links)))
        };
        Arc::new(ApplySeries::new(
            Arc::clone(&self.unit_of_work_factory),
            provider,
        ))
    }

    fn apply_series_episode(&self) -> Arc<dyn ApplySeriesEpisodeUseCase> {
        // The chain is configuration-derived, so the accessor rebuilds it from
        // the immutable configuration on every call (STY-RUST-062).
        let links = configured_links(self.configuration.library().metadata().video());
        let provider = if links.is_empty() {
            None
        } else {
            Some(Arc::new(FailoverVideoMetadataProvider::new(links)))
        };
        Arc::new(ApplySeriesEpisode::new(
            Arc::clone(&self.unit_of_work_factory),
            provider,
        ))
    }

    fn create_gallery(&self) -> Arc<dyn CreateGalleryUseCase> {
        Arc::new(CreateGallery::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn create_movie(&self) -> Arc<dyn CreateMovieUseCase> {
        Arc::new(CreateMovie::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn create_music_album(&self) -> Arc<dyn CreateMusicAlbumUseCase> {
        Arc::new(CreateMusicAlbum::new(Arc::clone(
            &self.unit_of_work_factory,
        )))
    }

    fn create_music_group(&self) -> Arc<dyn CreateMusicGroupUseCase> {
        Arc::new(CreateMusicGroup::new(Arc::clone(
            &self.unit_of_work_factory,
        )))
    }

    fn create_playlist(&self) -> Arc<dyn CreatePlaylistUseCase> {
        Arc::new(CreatePlaylist::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn create_series(&self) -> Arc<dyn CreateSeriesUseCase> {
        Arc::new(CreateSeries::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn delete_gallery(&self) -> Arc<dyn DeleteGalleryUseCase> {
        // The file storage adapter is stateless and rebuilt per accessor.
        Arc::new(DeleteGallery::new(
            Arc::clone(&self.unit_of_work_factory),
            Arc::new(FileSystemStorage::new()),
        ))
    }

    fn delete_gallery_item(&self) -> Arc<dyn DeleteGalleryItemUseCase> {
        // The file storage adapter is stateless and rebuilt per accessor.
        Arc::new(DeleteGalleryItem::new(
            Arc::clone(&self.unit_of_work_factory),
            Arc::new(FileSystemStorage::new()),
        ))
    }

    fn delete_playlist(&self) -> Arc<dyn DeletePlaylistUseCase> {
        Arc::new(DeletePlaylist::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn get_album(&self) -> Arc<dyn GetAlbumUseCase> {
        Arc::new(GetAlbum::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn get_album_medium(&self) -> Arc<dyn GetAlbumMediumUseCase> {
        Arc::new(GetAlbumMedium::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn get_album_track(&self) -> Arc<dyn GetAlbumTrackUseCase> {
        Arc::new(GetAlbumTrack::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn get_gallery(&self) -> Arc<dyn GetGalleryUseCase> {
        Arc::new(GetGallery::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn get_gallery_item(&self) -> Arc<dyn GetGalleryItemUseCase> {
        Arc::new(GetGalleryItem::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn get_movie(&self) -> Arc<dyn GetMovieUseCase> {
        Arc::new(GetMovie::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn get_music_group(&self) -> Arc<dyn GetMusicGroupUseCase> {
        Arc::new(GetMusicGroup::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn get_playlist(&self) -> Arc<dyn GetPlaylistUseCase> {
        Arc::new(GetPlaylist::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn get_playlist_track(&self) -> Arc<dyn GetPlaylistTrackUseCase> {
        Arc::new(GetPlaylistTrack::new(Arc::clone(
            &self.unit_of_work_factory,
        )))
    }

    fn get_series(&self) -> Arc<dyn GetSeriesUseCase> {
        Arc::new(GetSeries::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn get_series_episode(&self) -> Arc<dyn GetSeriesEpisodeUseCase> {
        Arc::new(GetSeriesEpisode::new(Arc::clone(
            &self.unit_of_work_factory,
        )))
    }

    fn get_series_season(&self) -> Arc<dyn GetSeriesSeasonUseCase> {
        Arc::new(GetSeriesSeason::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn identify_music(&self) -> Arc<dyn IdentifyMusicUseCase> {
        // The provider is configuration-derived, so the accessor rebuilds it from
        // the immutable configuration on every call (STY-RUST-062).
        let provider = configured_music_provider(self.configuration.library().metadata().music())
            .map(Arc::new);
        Arc::new(IdentifyMusic::new(
            Arc::clone(&self.unit_of_work_factory),
            provider,
        ))
    }

    fn identify_video(&self) -> Arc<dyn IdentifyVideoUseCase> {
        // The chain is configuration-derived, so the accessor rebuilds it from
        // the immutable configuration on every call (STY-RUST-062).
        let links = configured_links(self.configuration.library().metadata().video());
        let provider = if links.is_empty() {
            None
        } else {
            Some(Arc::new(FailoverVideoMetadataProvider::new(links)))
        };
        Arc::new(IdentifyVideo::new(
            Arc::clone(&self.unit_of_work_factory),
            provider,
        ))
    }

    fn ingest_media(&self) -> Arc<dyn IngestMediaUseCase> {
        Arc::new(IngestMedia::new(
            Arc::clone(&self.unit_of_work_factory),
            Arc::new(LocalMediaSource::new(media_root())),
            Arc::new(InferContentTypeSniffer::new()),
            Arc::new(LocalMediaProbe::with_permits(Arc::clone(
                &self.probe_permits,
            ))),
        ))
    }

    fn list_album_tracks(&self) -> Arc<dyn ListAlbumTracksUseCase> {
        Arc::new(ListAlbumTracks::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn list_albums(&self) -> Arc<dyn ListAlbumsUseCase> {
        Arc::new(ListAlbums::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn list_galleries(&self) -> Arc<dyn ListGalleriesUseCase> {
        Arc::new(ListGalleries::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn list_movies(&self) -> Arc<dyn ListMoviesUseCase> {
        Arc::new(ListMovies::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn list_music_groups(&self) -> Arc<dyn ListMusicGroupsUseCase> {
        Arc::new(ListMusicGroups::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn list_playlists(&self) -> Arc<dyn ListPlaylistsUseCase> {
        Arc::new(ListPlaylists::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn list_series(&self) -> Arc<dyn ListSeriesUseCase> {
        Arc::new(ListSeries::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn list_series_episodes(&self) -> Arc<dyn ListSeriesEpisodesUseCase> {
        Arc::new(ListSeriesEpisodes::new(Arc::clone(
            &self.unit_of_work_factory,
        )))
    }

    fn patch_album_track(&self) -> Arc<dyn PatchAlbumTrackUseCase> {
        Arc::new(PatchAlbumTrack::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn put_album_medium(&self) -> Arc<dyn PutAlbumMediumUseCase> {
        Arc::new(PutAlbumMedium::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn put_album_track(&self) -> Arc<dyn PutAlbumTrackUseCase> {
        Arc::new(PutAlbumTrack::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn put_series_episode(&self) -> Arc<dyn PutSeriesEpisodeUseCase> {
        Arc::new(PutSeriesEpisode::new(Arc::clone(
            &self.unit_of_work_factory,
        )))
    }

    fn put_series_season(&self) -> Arc<dyn PutSeriesSeasonUseCase> {
        Arc::new(PutSeriesSeason::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn remove_playlist_track(&self) -> Arc<dyn RemovePlaylistTrackUseCase> {
        Arc::new(RemovePlaylistTrack::new(Arc::clone(
            &self.unit_of_work_factory,
        )))
    }

    fn reorder_playlist_tracks(&self) -> Arc<dyn ReorderPlaylistTracksUseCase> {
        Arc::new(ReorderPlaylistTracks::new(Arc::clone(
            &self.unit_of_work_factory,
        )))
    }

    fn resolve_music_metadata(&self) -> Arc<dyn ResolveMusicMetadataUseCase> {
        // The provider is configuration-derived, so the accessor rebuilds it from
        // the immutable configuration on every call (STY-RUST-062).
        let provider = configured_music_provider(self.configuration.library().metadata().music())
            .map(Arc::new);
        Arc::new(ResolveMusicMetadata::new(
            Arc::clone(&self.unit_of_work_factory),
            provider,
        ))
    }

    fn resolve_video_metadata(&self) -> Arc<dyn ResolveVideoMetadataUseCase> {
        // The chain is configuration-derived, so the accessor rebuilds it from
        // the immutable configuration on every call (STY-RUST-062).
        let links = configured_links(self.configuration.library().metadata().video());
        let provider = if links.is_empty() {
            None
        } else {
            Some(Arc::new(FailoverVideoMetadataProvider::new(links)))
        };
        Arc::new(ResolveVideoMetadata::new(
            Arc::clone(&self.unit_of_work_factory),
            provider,
        ))
    }

    fn update_movie(&self) -> Arc<dyn UpdateMovieUseCase> {
        Arc::new(UpdateMovie::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn update_music_album(&self) -> Arc<dyn UpdateMusicAlbumUseCase> {
        Arc::new(UpdateMusicAlbum::new(Arc::clone(
            &self.unit_of_work_factory,
        )))
    }

    fn update_music_group(&self) -> Arc<dyn UpdateMusicGroupUseCase> {
        Arc::new(UpdateMusicGroup::new(Arc::clone(
            &self.unit_of_work_factory,
        )))
    }

    fn update_playlist(&self) -> Arc<dyn UpdatePlaylistUseCase> {
        Arc::new(UpdatePlaylist::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn update_series(&self) -> Arc<dyn UpdateSeriesUseCase> {
        Arc::new(UpdateSeries::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn update_series_episode(&self) -> Arc<dyn UpdateSeriesEpisodeUseCase> {
        Arc::new(UpdateSeriesEpisode::new(Arc::clone(
            &self.unit_of_work_factory,
        )))
    }

    fn update_series_season(&self) -> Arc<dyn UpdateSeriesSeasonUseCase> {
        Arc::new(UpdateSeriesSeason::new(Arc::clone(
            &self.unit_of_work_factory,
        )))
    }
}
