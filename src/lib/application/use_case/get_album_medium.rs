use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::get_album_medium::GetAlbumMediumCommand;
use crate::application::port::get_album_medium::GetAlbumMediumError;
use crate::application::port::get_album_medium::GetAlbumMediumResponse;
use crate::application::port::get_album_medium::GetAlbumMediumUseCase;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::music_album_repository::MusicAlbumFilter;
use crate::domain::port::music_album_repository::MusicAlbumRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;

/// Use case implementation for fetching one medium of a music album.
pub struct GetAlbumMedium<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> GetAlbumMedium<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> GetAlbumMediumUseCase for GetAlbumMedium<F>
where
    F: UnitOfWorkFactory,
    F::Uow: LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: GetAlbumMediumCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<GetAlbumMediumResponse, GetAlbumMediumError>>
                + Send
                + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| GetAlbumMediumError::Unknown(error.into()))?;

            let result = async {
                unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| GetAlbumMediumError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetAlbumMediumError::NoSuchCaller)?;

                let album = unit_of_work
                    .music_albums()
                    .search(&MusicAlbumFilter {
                        id: Some(command.album_id()),
                        ..MusicAlbumFilter::default()
                    })
                    .await
                    .map_err(|error| GetAlbumMediumError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetAlbumMediumError::NoSuchAlbum)?;

                // The medium is resolved from the album aggregate the read
                // returned, so a medium of another album is reported as missing.
                let medium = album
                    .mediums()
                    .iter()
                    .find(|medium| medium.index().index() == command.medium_index())
                    .ok_or(GetAlbumMediumError::NoSuchMedium)?;

                Ok(GetAlbumMediumResponse::new(
                    album.id(),
                    medium.index().index(),
                    medium.index().as_wire().to_owned(),
                    medium.tracks().len(),
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| GetAlbumMediumError::Unknown(error.into()))?;
                    Ok(value)
                }
                Err(error) => {
                    // The unit-of-work adapter owns the rollback-failure log (OBS-002).
                    let _rollback = unit_of_work.rollback().await;
                    Err(error)
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use crate::application::port::get_album_medium::GetAlbumMediumCommand;
    use crate::application::port::get_album_medium::GetAlbumMediumError;
    use crate::application::port::get_album_medium::GetAlbumMediumUseCase as _;
    use crate::domain::model::music_album::MusicAlbum;
    use crate::domain::model::music_medium::MediumIndex;
    use crate::domain::model::music_medium::MusicMedium;
    use crate::domain::model::music_recording::MusicRecording;
    use crate::domain::model::music_recording::TrackIndex;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::music_album_repository::MockMusicAlbumRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::GalleryFactoryHarness;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::TestUnitOfWork;
    use crate::test_helpers::album_release_date;

    use super::GetAlbumMedium;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = GetAlbumMedium<TestFactory>;

    /// Build an album holding one `CD` medium at `medium_index`, with one track.
    fn album(album_id: i64, medium_index: u8) -> Result<MusicAlbum, Box<dyn Error>> {
        let recording = MusicRecording::try_new(
            900,
            Some("So What".to_owned()),
            album_release_date(),
            None,
            1,
            101,
            None,
            TrackIndex::MediumIndex(1),
        )?;
        Ok(MusicAlbum::try_new(
            album_id,
            format!("Album {album_id}"),
            album_release_date(),
            None,
            Vec::new(),
            None,
            None,
            vec![MusicMedium::new(
                MediumIndex::Cd(medium_index),
                vec![recording],
            )],
        )?)
    }

    /// Build an album holding no medium.
    #[expect(
        clippy::single_call_fn,
        reason = "the empty-album fixture names the case it builds"
    )]
    fn empty_album(album_id: i64) -> Result<MusicAlbum, Box<dyn Error>> {
        Ok(MusicAlbum::try_new(
            album_id,
            "Empty".to_owned(),
            album_release_date(),
            None,
            Vec::new(),
            None,
            None,
            Vec::new(),
        )?)
    }

    /// Build a Library test harness around the given mocks.
    fn harness_with(
        users: MockUserRepository,
        albums: MockMusicAlbumRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            music_albums: albums,
            users,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
    }

    /// Expect the caller load to find `caller_id`.
    fn expect_caller(
        users: &mut MockUserRepository,
        caller_id: i64,
        role: Role,
    ) -> Result<(), Box<dyn Error>> {
        let caller = User::try_new(caller_id, "caller".to_owned(), None, caller_id, role)?;
        users
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(caller_id))
            .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
        Ok(())
    }

    /// Expect the album lookup to find `album`.
    fn expect_album(albums: &mut MockMusicAlbumRepository, album: MusicAlbum) {
        let album_id = album.id();
        albums
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(album_id))
            .return_once(move |_| {
                let found = vec![album.clone()];
                Box::pin(async move { Ok(found) })
            });
    }

    #[tokio::test]
    async fn get_album_medium_returns_the_scoped_medium() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album(7, 2)?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = GetAlbumMedium::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(GetAlbumMediumCommand::new(1, 7, 2))
            .await?;

        // Assert
        assert_eq!(response.album_id(), 7);
        assert_eq!(response.medium_index(), 2);
        assert_eq!(response.medium_type(), "CD");
        assert_eq!(response.track_count(), 1);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_album_medium_unknown_album_returns_no_such_album() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        albums
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, albums);
        let use_case: UseCase = GetAlbumMedium::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(GetAlbumMediumCommand::new(1, 999, 1))
            .await;

        // Assert
        assert!(matches!(result, Err(GetAlbumMediumError::NoSuchAlbum)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_album_medium_unknown_index_returns_no_such_medium() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album(7, 1)?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = GetAlbumMedium::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetAlbumMediumCommand::new(1, 7, 9)).await;

        // Assert
        assert!(matches!(result, Err(GetAlbumMediumError::NoSuchMedium)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_album_medium_album_without_medium_returns_no_such_medium()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, empty_album(7)?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = GetAlbumMedium::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetAlbumMediumCommand::new(1, 7, 1)).await;

        // Assert
        assert!(matches!(result, Err(GetAlbumMediumError::NoSuchMedium)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_album_medium_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, MockMusicAlbumRepository::new());
        let use_case: UseCase = GetAlbumMedium::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(GetAlbumMediumCommand::new(999, 7, 1))
            .await;

        // Assert
        assert!(matches!(result, Err(GetAlbumMediumError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_album_medium_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        albums
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, albums);
        let use_case: UseCase = GetAlbumMedium::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetAlbumMediumCommand::new(1, 7, 1)).await;

        // Assert
        assert!(matches!(result, Err(GetAlbumMediumError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_album_medium_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album(7, 1)?);
        let harness = harness_with(users, albums);
        harness.commit_fails.store(true, Ordering::SeqCst);
        let use_case: UseCase = GetAlbumMedium::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetAlbumMediumCommand::new(1, 7, 1)).await;

        // Assert
        assert!(matches!(result, Err(GetAlbumMediumError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }
}
