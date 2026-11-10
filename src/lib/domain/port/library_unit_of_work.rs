use crate::domain::port::{
    AudioRepository, GalleryRepository, ImageRepository, MovieRepository, MusicAlbumRepository,
    MusicGroupRepository, MusicPlaylistRepository, SeriesRepository, UnitOfWork, VideoRepository,
};

/// Library bounded-context view over a [`UnitOfWork`].
pub(crate) trait LibraryUnitOfWork: UnitOfWork {
    /// Repository persisting audio rows.
    fn audios(&mut self) -> impl AudioRepository + '_;

    /// Repository persisting galleries and their items.
    fn galleries(&mut self) -> impl GalleryRepository + '_;

    /// Repository persisting images.
    fn images(&mut self) -> impl ImageRepository + '_;

    /// Repository querying films and their videos.
    fn movies(&mut self) -> impl MovieRepository + '_;

    /// Repository querying music albums and their tracks.
    fn music_albums(&mut self) -> impl MusicAlbumRepository + '_;

    /// Repository persisting music groups.
    fn music_groups(&mut self) -> impl MusicGroupRepository + '_;

    /// Repository persisting music playlists and their tracks.
    fn music_playlists(&mut self) -> impl MusicPlaylistRepository + '_;

    /// Repository querying shows, their seasons and their episodes.
    fn series(&mut self) -> impl SeriesRepository + '_;

    /// Repository persisting videos.
    fn videos(&mut self) -> impl VideoRepository + '_;
}
