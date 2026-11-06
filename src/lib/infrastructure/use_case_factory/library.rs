use std::sync::Arc;

use tokio::sync::Semaphore;

use crate::application::port::add_gallery_item::AddGalleryItemUseCase;
use crate::application::port::apply_movie::ApplyMovieUseCase;
use crate::application::port::apply_music_album::ApplyMusicAlbumUseCase;
use crate::application::port::apply_series::ApplySeriesUseCase;
use crate::application::port::apply_series_episode::ApplySeriesEpisodeUseCase;
use crate::application::port::create_gallery::CreateGalleryUseCase;
use crate::application::port::create_movie::CreateMovieUseCase;
use crate::application::port::create_music_album::CreateMusicAlbumUseCase;
use crate::application::port::create_music_group::CreateMusicGroupUseCase;
use crate::application::port::create_playlist::CreatePlaylistUseCase;
use crate::application::port::create_series::CreateSeriesUseCase;
use crate::application::port::delete_gallery::DeleteGalleryUseCase;
use crate::application::port::delete_gallery_item::DeleteGalleryItemUseCase;
use crate::application::port::delete_playlist::DeletePlaylistUseCase;
use crate::application::port::get_album::GetAlbumUseCase;
use crate::application::port::get_album_medium::GetAlbumMediumUseCase;
use crate::application::port::get_album_track::GetAlbumTrackUseCase;
use crate::application::port::get_gallery::GetGalleryUseCase;
use crate::application::port::get_gallery_item::GetGalleryItemUseCase;
use crate::application::port::get_movie::GetMovieUseCase;
use crate::application::port::get_music_group::GetMusicGroupUseCase;
use crate::application::port::get_playlist::GetPlaylistUseCase;
use crate::application::port::get_playlist_track::GetPlaylistTrackUseCase;
use crate::application::port::get_series::GetSeriesUseCase;
use crate::application::port::get_series_episode::GetSeriesEpisodeUseCase;
use crate::application::port::get_series_season::GetSeriesSeasonUseCase;
use crate::application::port::identify_music::IdentifyMusicUseCase;
use crate::application::port::identify_video::IdentifyVideoUseCase;
use crate::application::port::ingest_media::IngestMediaUseCase;
use crate::application::port::library_use_case_factory::LibraryUseCaseFactory;
use crate::application::port::list_album_tracks::ListAlbumTracksUseCase;
use crate::application::port::list_albums::ListAlbumsUseCase;
use crate::application::port::list_galleries::ListGalleriesUseCase;
use crate::application::port::list_movies::ListMoviesUseCase;
use crate::application::port::list_music_groups::ListMusicGroupsUseCase;
use crate::application::port::list_playlists::ListPlaylistsUseCase;
use crate::application::port::list_series::ListSeriesUseCase;
use crate::application::port::list_series_episodes::ListSeriesEpisodesUseCase;
use crate::application::port::patch_album_track::PatchAlbumTrackUseCase;
use crate::application::port::put_album_medium::PutAlbumMediumUseCase;
use crate::application::port::put_album_track::PutAlbumTrackUseCase;
use crate::application::port::put_series_episode::PutSeriesEpisodeUseCase;
use crate::application::port::put_series_season::PutSeriesSeasonUseCase;
use crate::application::port::remove_playlist_track::RemovePlaylistTrackUseCase;
use crate::application::port::reorder_playlist_tracks::ReorderPlaylistTracksUseCase;
use crate::application::port::resolve_music_metadata::ResolveMusicMetadataUseCase;
use crate::application::port::resolve_video_metadata::ResolveVideoMetadataUseCase;
use crate::application::port::update_movie::UpdateMovieUseCase;
use crate::application::port::update_music_album::UpdateMusicAlbumUseCase;
use crate::application::port::update_music_group::UpdateMusicGroupUseCase;
use crate::application::port::update_playlist::UpdatePlaylistUseCase;
use crate::application::port::update_series::UpdateSeriesUseCase;
use crate::application::port::update_series_episode::UpdateSeriesEpisodeUseCase;
use crate::application::port::update_series_season::UpdateSeriesSeasonUseCase;
use crate::application::use_case::add_gallery_item::AddGalleryItem;
use crate::application::use_case::apply_movie::ApplyMovie;
use crate::application::use_case::apply_music_album::ApplyMusicAlbum;
use crate::application::use_case::apply_series::ApplySeries;
use crate::application::use_case::apply_series_episode::ApplySeriesEpisode;
use crate::application::use_case::create_gallery::CreateGallery;
use crate::application::use_case::create_movie::CreateMovie;
use crate::application::use_case::create_music_album::CreateMusicAlbum;
use crate::application::use_case::create_music_group::CreateMusicGroup;
use crate::application::use_case::create_playlist::CreatePlaylist;
use crate::application::use_case::create_series::CreateSeries;
use crate::application::use_case::delete_gallery::DeleteGallery;
use crate::application::use_case::delete_gallery_item::DeleteGalleryItem;
use crate::application::use_case::delete_playlist::DeletePlaylist;
use crate::application::use_case::get_album::GetAlbum;
use crate::application::use_case::get_album_medium::GetAlbumMedium;
use crate::application::use_case::get_album_track::GetAlbumTrack;
use crate::application::use_case::get_gallery::GetGallery;
use crate::application::use_case::get_gallery_item::GetGalleryItem;
use crate::application::use_case::get_movie::GetMovie;
use crate::application::use_case::get_music_group::GetMusicGroup;
use crate::application::use_case::get_playlist::GetPlaylist;
use crate::application::use_case::get_playlist_track::GetPlaylistTrack;
use crate::application::use_case::get_series::GetSeries;
use crate::application::use_case::get_series_episode::GetSeriesEpisode;
use crate::application::use_case::get_series_season::GetSeriesSeason;
use crate::application::use_case::identify_music::IdentifyMusic;
use crate::application::use_case::identify_video::IdentifyVideo;
use crate::application::use_case::ingest_media::IngestMedia;
use crate::application::use_case::list_album_tracks::ListAlbumTracks;
use crate::application::use_case::list_albums::ListAlbums;
use crate::application::use_case::list_galleries::ListGalleries;
use crate::application::use_case::list_movies::ListMovies;
use crate::application::use_case::list_music_groups::ListMusicGroups;
use crate::application::use_case::list_playlists::ListPlaylists;
use crate::application::use_case::list_series::ListSeries;
use crate::application::use_case::list_series_episodes::ListSeriesEpisodes;
use crate::application::use_case::patch_album_track::PatchAlbumTrack;
use crate::application::use_case::put_album_medium::PutAlbumMedium;
use crate::application::use_case::put_album_track::PutAlbumTrack;
use crate::application::use_case::put_series_episode::PutSeriesEpisode;
use crate::application::use_case::put_series_season::PutSeriesSeason;
use crate::application::use_case::remove_playlist_track::RemovePlaylistTrack;
use crate::application::use_case::reorder_playlist_tracks::ReorderPlaylistTracks;
use crate::application::use_case::resolve_music_metadata::ResolveMusicMetadata;
use crate::application::use_case::resolve_video_metadata::ResolveVideoMetadata;
use crate::application::use_case::update_movie::UpdateMovie;
use crate::application::use_case::update_music_album::UpdateMusicAlbum;
use crate::application::use_case::update_music_group::UpdateMusicGroup;
use crate::application::use_case::update_playlist::UpdatePlaylist;
use crate::application::use_case::update_series::UpdateSeries;
use crate::application::use_case::update_series_episode::UpdateSeriesEpisode;
use crate::application::use_case::update_series_season::UpdateSeriesSeason;
use crate::application::use_case::upload_layout::media_root;
use crate::domain::model::configuration::Configuration;
use crate::infrastructure::outbound::file_system::file_storage::FileSystemStorage;
use crate::infrastructure::outbound::file_system::media_source::LocalMediaSource;
use crate::infrastructure::outbound::infer::content_type_sniffer::InferContentTypeSniffer;
use crate::infrastructure::outbound::media_probe::LocalMediaProbe;
use crate::infrastructure::outbound::metadata::configured_links;
use crate::infrastructure::outbound::metadata::configured_music_provider;
use crate::infrastructure::outbound::metadata::failover::FailoverVideoMetadataProvider;
use crate::infrastructure::outbound::sqlx::unit_of_work::SqlxUnitOfWorkFactory;

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
