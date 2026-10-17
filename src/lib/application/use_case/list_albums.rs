use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::list_albums::ListAlbumsCommand;
use crate::application::port::list_albums::ListAlbumsError;
use crate::application::port::list_albums::ListAlbumsItem;
use crate::application::port::list_albums::ListAlbumsResponse;
use crate::application::port::list_albums::ListAlbumsUseCase;
use crate::application::use_case::library_access;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::music_album_repository::MusicAlbumFilter;
use crate::domain::port::music_album_repository::MusicAlbumRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;

/// Use case implementation for listing the music albums the caller may see.
pub struct ListAlbums<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> ListAlbums<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> ListAlbumsUseCase for ListAlbums<F>
where
    F: UnitOfWorkFactory,
    F::Uow: LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: ListAlbumsCommand,
    ) -> Pin<Box<dyn Future<Output = Result<ListAlbumsResponse, ListAlbumsError>> + Send + 'future>>
    {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| ListAlbumsError::Unknown(error.into()))?;

            let result = async {
                library_access::load_caller(&mut unit_of_work, command.caller_id())
                    .await
                    .map_err(|error| ListAlbumsError::Unknown(error.into()))?
                    .ok_or(ListAlbumsError::NoSuchCaller)?;

                let albums = unit_of_work
                    .music_albums()
                    .search(&MusicAlbumFilter {
                        name: command.name().map(str::to_owned),
                        ..MusicAlbumFilter::default()
                    })
                    .await
                    .map_err(|error| ListAlbumsError::Unknown(error.into()))?;

                let total = albums.len();
                let offset = command.offset().unwrap_or(0);
                let page = albums
                    .into_iter()
                    .skip(offset)
                    .take(command.limit().unwrap_or(usize::MAX))
                    .map(|album| {
                        ListAlbumsItem::new(
                            album.id(),
                            album.name().to_owned(),
                            album.release_date(),
                            album.production_type(),
                            album.genre_ids().to_vec(),
                            album
                                .mediums()
                                .iter()
                                .map(|medium| medium.tracks().len())
                                .sum(),
                        )
                    })
                    .collect();

                Ok(ListAlbumsResponse::new(page, total))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| ListAlbumsError::Unknown(error.into()))?;
                    Ok(value)
                }
                Err(error) => {
                    if unit_of_work.rollback().await.is_err() {
                        // The unit-of-work adapter owns the rollback-failure log (OBS-002).
                    }
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

    use chrono::NaiveDate;

    use crate::application::port::list_albums::ListAlbumsCommand;
    use crate::application::port::list_albums::ListAlbumsError;
    use crate::application::port::list_albums::ListAlbumsItem;
    use crate::application::port::list_albums::ListAlbumsUseCase as _;
    use crate::domain::model::music_album::MusicAlbum;
    use crate::domain::model::music_album::ProductionType;
    use crate::domain::model::music_medium::MusicMedium;
    use crate::domain::model::music_recording::MusicRecording;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::music_album_repository::MockMusicAlbumRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::GalleryFactoryHarness;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::TestUnitOfWork;
    use crate::test_helpers::album_release_date;

    use super::ListAlbums;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = ListAlbums<TestFactory>;

    /// Build one album holding `album_id` tracks, named `Album {album_id}`.
    fn album(album_id: i64) -> Result<MusicAlbum, Box<dyn Error>> {
        let tracks = (0..album_id)
            .map(|index| {
                Ok(MusicRecording::try_new(
                    index,
                    Some(format!("Track {index}")),
                    album_release_date(),
                    None,
                    index,
                    index,
                    None,
                )?)
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
        Ok(MusicAlbum::try_new(
            album_id,
            format!("Album {album_id}"),
            album_release_date(),
            Some(ProductionType::Studio),
            vec!["JAZZ".to_owned()],
            None,
            vec![MusicMedium::new(album_id, Some(1), "CD".to_owned(), tracks)],
        )?)
    }

    /// Build one album holding no medium, named `name`.
    fn empty_album(album_id: i64, name: &str) -> Result<MusicAlbum, Box<dyn Error>> {
        Ok(MusicAlbum::try_new(
            album_id,
            name.to_owned(),
            NaiveDate::default(),
            None,
            Vec::new(),
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

    #[tokio::test]
    async fn list_albums_returns_every_album_of_the_page() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        let first = album(1)?;
        let second = album(2)?;
        albums
            .expect_search()
            .times(1)
            .withf(|filter| filter.id.is_none() && filter.name.is_none())
            .return_once(move |_| {
                let found = vec![first.clone(), second.clone()];
                Box::pin(async move { Ok(found) })
            });
        let harness = harness_with(users, albums);
        let use_case: UseCase = ListAlbums::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(ListAlbumsCommand::new(1, None, None, None))
            .await?;

        // Assert
        assert_eq!(response.total(), 2);
        assert_eq!(
            response.albums().first(),
            Some(&ListAlbumsItem::new(
                1,
                "Album 1".to_owned(),
                album_release_date(),
                Some(ProductionType::Studio),
                vec!["JAZZ".to_owned()],
                1,
            ))
        );
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn list_albums_forwards_the_filter_and_paginates() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let first = empty_album(1, "Kind 1")?;
        let second = empty_album(2, "Kind 2")?;
        let third = empty_album(3, "Kind 3")?;
        let mut albums = MockMusicAlbumRepository::new();
        albums
            .expect_search()
            .times(1)
            .withf(|filter| filter.name.as_deref() == Some("Kind"))
            .return_once(move |_| Box::pin(async move { Ok(vec![first, second, third]) }));
        let harness = harness_with(users, albums);
        let use_case: UseCase = ListAlbums::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(ListAlbumsCommand::new(
                1,
                Some("Kind".to_owned()),
                Some(1),
                Some(1),
            ))
            .await?;

        // Assert
        assert_eq!(response.total(), 3);
        assert_eq!(
            response.albums().first().map(ListAlbumsItem::album_id),
            Some(2)
        );
        Ok(())
    }

    #[tokio::test]
    async fn list_albums_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, MockMusicAlbumRepository::new());
        let use_case: UseCase = ListAlbums::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ListAlbumsCommand::new(999, None, None, None))
            .await;

        // Assert
        assert!(matches!(result, Err(ListAlbumsError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn list_albums_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        albums
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, albums);
        let use_case: UseCase = ListAlbums::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ListAlbumsCommand::new(1, None, None, None))
            .await;

        // Assert
        assert!(matches!(result, Err(ListAlbumsError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn list_albums_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        albums
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, albums);
        harness.commit_fails.store(true, Ordering::SeqCst);
        let use_case: UseCase = ListAlbums::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ListAlbumsCommand::new(1, None, None, None))
            .await;

        // Assert
        assert!(matches!(result, Err(ListAlbumsError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }
}
