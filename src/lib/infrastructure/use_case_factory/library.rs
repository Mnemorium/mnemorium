use std::sync::Arc;

use tokio::sync::Semaphore;

use crate::application::port::add_gallery_item::AddGalleryItemUseCase;
use crate::application::port::create_gallery::CreateGalleryUseCase;
use crate::application::port::create_playlist::CreatePlaylistUseCase;
use crate::application::port::delete_gallery::DeleteGalleryUseCase;
use crate::application::port::delete_gallery_item::DeleteGalleryItemUseCase;
use crate::application::port::delete_playlist::DeletePlaylistUseCase;
use crate::application::port::get_album::GetAlbumUseCase;
use crate::application::port::get_album_medium::GetAlbumMediumUseCase;
use crate::application::port::get_album_track::GetAlbumTrackUseCase;
use crate::application::port::get_gallery::GetGalleryUseCase;
use crate::application::port::get_gallery_item::GetGalleryItemUseCase;
use crate::application::port::get_movie::GetMovieUseCase;
use crate::application::port::get_playlist::GetPlaylistUseCase;
use crate::application::port::get_playlist_track::GetPlaylistTrackUseCase;
use crate::application::port::get_series::GetSeriesUseCase;
use crate::application::port::get_series_episode::GetSeriesEpisodeUseCase;
use crate::application::port::get_series_season::GetSeriesSeasonUseCase;
use crate::application::port::identify_video::IdentifyVideoUseCase;
use crate::application::port::ingest_media::IngestMediaUseCase;
use crate::application::port::library_use_case_factory::LibraryUseCaseFactory;
use crate::application::port::list_album_tracks::ListAlbumTracksUseCase;
use crate::application::port::list_albums::ListAlbumsUseCase;
use crate::application::port::list_galleries::ListGalleriesUseCase;
use crate::application::port::list_movies::ListMoviesUseCase;
use crate::application::port::list_playlists::ListPlaylistsUseCase;
use crate::application::port::list_series::ListSeriesUseCase;
use crate::application::port::list_series_episodes::ListSeriesEpisodesUseCase;
use crate::application::port::remove_playlist_track::RemovePlaylistTrackUseCase;
use crate::application::port::reorder_playlist_tracks::ReorderPlaylistTracksUseCase;
use crate::application::port::update_playlist::UpdatePlaylistUseCase;
use crate::application::use_case::add_gallery_item::AddGalleryItem;
use crate::application::use_case::create_gallery::CreateGallery;
use crate::application::use_case::create_playlist::CreatePlaylist;
use crate::application::use_case::delete_gallery::DeleteGallery;
use crate::application::use_case::delete_gallery_item::DeleteGalleryItem;
use crate::application::use_case::delete_playlist::DeletePlaylist;
use crate::application::use_case::get_album::GetAlbum;
use crate::application::use_case::get_album_medium::GetAlbumMedium;
use crate::application::use_case::get_album_track::GetAlbumTrack;
use crate::application::use_case::get_gallery::GetGallery;
use crate::application::use_case::get_gallery_item::GetGalleryItem;
use crate::application::use_case::get_movie::GetMovie;
use crate::application::use_case::get_playlist::GetPlaylist;
use crate::application::use_case::get_playlist_track::GetPlaylistTrack;
use crate::application::use_case::get_series::GetSeries;
use crate::application::use_case::get_series_episode::GetSeriesEpisode;
use crate::application::use_case::get_series_season::GetSeriesSeason;
use crate::application::use_case::identify_video::IdentifyVideo;
use crate::application::use_case::ingest_media::IngestMedia;
use crate::application::use_case::list_album_tracks::ListAlbumTracks;
use crate::application::use_case::list_albums::ListAlbums;
use crate::application::use_case::list_galleries::ListGalleries;
use crate::application::use_case::list_movies::ListMovies;
use crate::application::use_case::list_playlists::ListPlaylists;
use crate::application::use_case::list_series::ListSeries;
use crate::application::use_case::list_series_episodes::ListSeriesEpisodes;
use crate::application::use_case::remove_playlist_track::RemovePlaylistTrack;
use crate::application::use_case::reorder_playlist_tracks::ReorderPlaylistTracks;
use crate::application::use_case::update_playlist::UpdatePlaylist;
use crate::application::use_case::upload_layout::media_root;
use crate::domain::model::configuration::Configuration;
use crate::infrastructure::outbound::file_system::file_storage::FileSystemStorage;
use crate::infrastructure::outbound::file_system::media_source::LocalMediaSource;
use crate::infrastructure::outbound::infer::content_type_sniffer::InferContentTypeSniffer;
use crate::infrastructure::outbound::media_probe::LocalMediaProbe;
use crate::infrastructure::outbound::metadata::configured_links;
use crate::infrastructure::outbound::metadata::failover::FailoverVideoMetadataProvider;
use crate::infrastructure::outbound::sqlx::unit_of_work::SqlxUnitOfWorkFactory;

/// Builds the Library use cases.
///
/// The accessors build the stateless file storage and media adapters at the
/// point of use, so the media-library use cases compose their paths from the
/// fixed storage root the Asset use cases write to. The probe permit pool is a
/// startup value the composition root captured; each accessor builds the probe
/// adapter afresh from it and stores neither.
pub struct RuntimeLibraryUseCaseFactory {
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
    pub fn new(
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

    fn create_gallery(&self) -> Arc<dyn CreateGalleryUseCase> {
        Arc::new(CreateGallery::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn create_playlist(&self) -> Arc<dyn CreatePlaylistUseCase> {
        Arc::new(CreatePlaylist::new(Arc::clone(&self.unit_of_work_factory)))
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

    fn identify_video(&self) -> Arc<dyn IdentifyVideoUseCase> {
        // The chain is configuration-derived, so the accessor rebuilds it from
        // the immutable configuration on every call (STY-RUST-062).
        let links = configured_links(self.configuration.library().metadata().video());
        let provider = if links.is_empty() {
            None
        } else {
            Some(Arc::new(FailoverVideoMetadataProvider::new(links)))
        };
        Arc::new(IdentifyVideo::new(provider))
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

    fn update_playlist(&self) -> Arc<dyn UpdatePlaylistUseCase> {
        Arc::new(UpdatePlaylist::new(Arc::clone(&self.unit_of_work_factory)))
    }
}
