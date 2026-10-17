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
                            album.genres().to_vec(),
                            album.track_count(),
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
    use crate::domain::model::music_album::ProductionType;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::file_repository::MockFileRepository;
    use crate::domain::port::gallery_repository::MockGalleryRepository;
    use crate::domain::port::image_repository::MockImageRepository;
    use crate::domain::port::music_album_repository::MockMusicAlbumRepository;
    use crate::domain::port::music_album_repository::MusicAlbumSummary;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::domain::port::video_repository::MockVideoRepository;
    use crate::test_helpers::GalleryFactoryHarness;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::gallery_factory;

    use super::ListAlbums;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = ListAlbums<TestFactory>;

    /// Build a domain release date.
    fn release_date() -> Result<NaiveDate, Box<dyn Error>> {
        NaiveDate::from_ymd_opt(1959, 8, 17).ok_or_else(|| "invalid fixture date".into())
    }

    /// Build one album summary with a generated name.
    fn album(album_id: i64) -> Result<MusicAlbumSummary, Box<dyn Error>> {
        Ok(MusicAlbumSummary::new(
            album_id,
            None,
            format!("Album {album_id}"),
            release_date()?,
            Some(ProductionType::Studio),
            vec!["JAZZ".to_owned()],
            usize::try_from(album_id).unwrap_or_default(),
        ))
    }

    /// Build a Library test harness around the given mocks.
    fn harness_with(
        users: MockUserRepository,
        albums: MockMusicAlbumRepository,
    ) -> GalleryFactoryHarness {
        gallery_factory(
            MockGalleryRepository::new(),
            MockImageRepository::new(),
            MockVideoRepository::new(),
            users,
            MockFileRepository::new(),
            albums,
        )
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
                release_date()?,
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
        let mut albums = MockMusicAlbumRepository::new();
        albums
            .expect_search()
            .times(1)
            .withf(|filter| filter.name.as_deref() == Some("Kind"))
            .return_once(|_| {
                Box::pin(async {
                    Ok(vec![
                        MusicAlbumSummary::new(
                            1,
                            None,
                            "Kind 1".to_owned(),
                            NaiveDate::default(),
                            None,
                            Vec::new(),
                            0,
                        ),
                        MusicAlbumSummary::new(
                            2,
                            None,
                            "Kind 2".to_owned(),
                            NaiveDate::default(),
                            None,
                            Vec::new(),
                            0,
                        ),
                        MusicAlbumSummary::new(
                            3,
                            None,
                            "Kind 3".to_owned(),
                            NaiveDate::default(),
                            None,
                            Vec::new(),
                            0,
                        ),
                    ])
                })
            });
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
