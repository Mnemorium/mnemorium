use sqlx::Sqlite;
use sqlx::SqlitePool;
use sqlx::Transaction;

use crate::domain::port::asset_unit_of_work::AssetUnitOfWork;
use crate::domain::port::credential_repository::CredentialRepository;
use crate::domain::port::error::UnitOfWorkError;
use crate::domain::port::file_repository::FileRepository;
use crate::domain::port::gallery_repository::GalleryRepository;
use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
use crate::domain::port::image_repository::ImageRepository;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::mime_type_repository::MimeTypeRepository;
use crate::domain::port::movie_repository::MovieRepository;
use crate::domain::port::music_album_repository::MusicAlbumRepository;
use crate::domain::port::music_playlist_repository::MusicPlaylistRepository;
use crate::domain::port::permission_repository::PermissionRepository;
use crate::domain::port::series_repository::SeriesRepository;
use crate::domain::port::unit_of_work::UnitOfWork;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::upload_repository::UploadRepository;
use crate::domain::port::user_repository::UserRepository;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::port::video_repository::VideoRepository;
use crate::infrastructure::outbound::sqlx::credential_repository::SqlxCredentialRepository;
use crate::infrastructure::outbound::sqlx::file_repository::SqlxFileRepository;
use crate::infrastructure::outbound::sqlx::gallery_repository::SqlxGalleryRepository;
use crate::infrastructure::outbound::sqlx::image_repository::SqlxImageRepository;
use crate::infrastructure::outbound::sqlx::mime_type_repository::SqlxMimeTypeRepository;
use crate::infrastructure::outbound::sqlx::movie_repository::SqlxMovieRepository;
use crate::infrastructure::outbound::sqlx::music_album_repository::SqlxMusicAlbumRepository;
use crate::infrastructure::outbound::sqlx::music_playlist_repository::SqlxMusicPlaylistRepository;
use crate::infrastructure::outbound::sqlx::permission_repository::SqlxPermissionRepository;
use crate::infrastructure::outbound::sqlx::series_repository::SqlxSeriesRepository;
use crate::infrastructure::outbound::sqlx::upload_repository::SqlxUploadRepository;
use crate::infrastructure::outbound::sqlx::user_repository::SqlxUserRepository;
use crate::infrastructure::outbound::sqlx::video_repository::SqlxVideoRepository;

/// Unit of work backed by a single `SQLite` transaction.
pub struct SqlxUnitOfWork {
    /// Transaction shared by every repository the unit of work exposes.
    transaction: Transaction<'static, Sqlite>,
}

impl SqlxUnitOfWork {
    /// Borrow the transaction backing this unit of work.
    fn transaction(&mut self) -> &mut Transaction<'static, Sqlite> {
        &mut self.transaction
    }
}

impl AssetUnitOfWork for SqlxUnitOfWork {
    fn files(&mut self) -> impl FileRepository + '_ {
        SqlxFileRepository::new(self.transaction())
    }

    fn mime_types(&mut self) -> impl MimeTypeRepository + '_ {
        SqlxMimeTypeRepository::new(self.transaction())
    }

    fn uploads(&mut self) -> impl UploadRepository + '_ {
        SqlxUploadRepository::new(self.transaction())
    }
}

impl IdentityUnitOfWork for SqlxUnitOfWork {
    fn credentials(&mut self) -> impl CredentialRepository + '_ {
        SqlxCredentialRepository::new(self.transaction())
    }

    fn permissions(&mut self) -> impl PermissionRepository + '_ {
        SqlxPermissionRepository::new(self.transaction())
    }
}

impl LibraryUnitOfWork for SqlxUnitOfWork {
    fn galleries(&mut self) -> impl GalleryRepository + '_ {
        SqlxGalleryRepository::new(self.transaction())
    }

    fn images(&mut self) -> impl ImageRepository + '_ {
        SqlxImageRepository::new(self.transaction())
    }

    fn movies(&mut self) -> impl MovieRepository + '_ {
        SqlxMovieRepository::new(self.transaction())
    }

    fn music_albums(&mut self) -> impl MusicAlbumRepository + '_ {
        SqlxMusicAlbumRepository::new(self.transaction())
    }

    fn music_playlists(&mut self) -> impl MusicPlaylistRepository + '_ {
        SqlxMusicPlaylistRepository::new(self.transaction())
    }

    fn series(&mut self) -> impl SeriesRepository + '_ {
        SqlxSeriesRepository::new(self.transaction())
    }

    fn videos(&mut self) -> impl VideoRepository + '_ {
        SqlxVideoRepository::new(self.transaction())
    }
}

impl UserUnitOfWork for SqlxUnitOfWork {
    fn users(&mut self) -> impl UserRepository + '_ {
        SqlxUserRepository::new(self.transaction())
    }
}

impl UnitOfWork for SqlxUnitOfWork {
    async fn commit(self) -> Result<(), UnitOfWorkError> {
        let Self { transaction } = self;
        transaction.commit().await.map_err(UnitOfWorkError::from)
    }

    async fn rollback(self) -> Result<(), UnitOfWorkError> {
        let Self { transaction } = self;
        transaction.rollback().await.map_err(UnitOfWorkError::from)
    }
}

/// Factory opening [`SqlxUnitOfWork`] instances from a `SQLite` pool.
pub struct SqlxUnitOfWorkFactory {
    /// Connection pool the transactions are acquired from.
    pool: SqlitePool,
}

impl SqlxUnitOfWorkFactory {
    /// Create a new factory bound to `pool`.
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl UnitOfWorkFactory for SqlxUnitOfWorkFactory {
    type Uow = SqlxUnitOfWork;

    async fn begin(&self) -> Result<Self::Uow, UnitOfWorkError> {
        let transaction = self.pool.begin().await.map_err(UnitOfWorkError::from)?;
        Ok(SqlxUnitOfWork { transaction })
    }
}
