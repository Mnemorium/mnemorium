use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::domain::port::{
    AssetUnitOfWork, AudioRepository, CredentialRepository, FileRepository, GalleryRepository,
    IdentityUnitOfWork, ImageRepository, LibraryUnitOfWork, MimeTypeRepository, MovieRepository,
    MusicAlbumRepository, MusicGroupRepository, MusicPlaylistRepository, PermissionRepository,
    SeriesRepository, UnitOfWork, UnitOfWorkError, UnitOfWorkFactory, UploadRepository,
    UserRepository, UserUnitOfWork, VideoRepository,
};
use crate::infrastructure::outbound::sqlx::{
    SqlxAudioRepository, SqlxCredentialRepository, SqlxFileRepository, SqlxGalleryRepository,
    SqlxImageRepository, SqlxMimeTypeRepository, SqlxMovieRepository, SqlxMusicAlbumRepository,
    SqlxMusicGroupRepository, SqlxMusicPlaylistRepository, SqlxPermissionRepository,
    SqlxSeriesRepository, SqlxUploadRepository, SqlxUserRepository, SqlxVideoRepository,
};

/// Statement opening a write-intent transaction.
///
/// `BEGIN IMMEDIATE` takes the write lock up front, so a concurrent writer
/// queues on the busy timeout instead of failing on a stale snapshot.
const BEGIN_IMMEDIATE: &str = "BEGIN IMMEDIATE";

/// Unit of work backed by a single `SQLite` transaction.
pub(crate) struct SqlxUnitOfWork {
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
    fn audios(&mut self) -> impl AudioRepository + '_ {
        SqlxAudioRepository::new(self.transaction())
    }

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

    fn music_groups(&mut self) -> impl MusicGroupRepository + '_ {
        SqlxMusicGroupRepository::new(self.transaction())
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
pub(crate) struct SqlxUnitOfWorkFactory {
    /// Connection pool the transactions are acquired from.
    pool: SqlitePool,
}

impl SqlxUnitOfWorkFactory {
    /// Create a new factory bound to `pool`.
    #[must_use]
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl UnitOfWorkFactory for SqlxUnitOfWorkFactory {
    type Uow = SqlxUnitOfWork;

    async fn begin(&self) -> Result<Self::Uow, UnitOfWorkError> {
        let transaction = self.pool.begin().await.map_err(UnitOfWorkError::from)?;
        Ok(SqlxUnitOfWork { transaction })
    }

    async fn begin_write(&self) -> Result<Self::Uow, UnitOfWorkError> {
        let transaction = self
            .pool
            .begin_with(BEGIN_IMMEDIATE)
            .await
            .map_err(UnitOfWorkError::from)?;
        Ok(SqlxUnitOfWork { transaction })
    }
}
