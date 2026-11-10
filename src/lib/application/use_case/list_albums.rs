use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::{
    ListAlbumsCommand, ListAlbumsError, ListAlbumsItem, ListAlbumsMediumItem, ListAlbumsResponse,
    ListAlbumsUseCase,
};
use crate::domain::model::Action;
use crate::domain::port::{
    IdentityUnitOfWork, LibraryUnitOfWork, MusicAlbumFilter, MusicAlbumRepository as _,
    PermissionFilter, PermissionRepository as _, UnitOfWork as _, UnitOfWorkFactory, UserFilter,
    UserRepository as _, UserUnitOfWork,
};
use crate::domain::service::Authorization;

/// Use case implementation for listing the music albums the caller may see.
pub(crate) struct ListAlbums<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> ListAlbums<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> ListAlbumsUseCase for ListAlbums<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
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
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| ListAlbumsError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(ListAlbumsError::NoSuchCaller)?;

                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| ListAlbumsError::Unknown(error.into()))?;

                let mut albums = unit_of_work
                    .music_albums()
                    .search(&MusicAlbumFilter {
                        name: command.name().map(str::to_owned),
                        ..MusicAlbumFilter::default()
                    })
                    .await
                    .map_err(|error| ListAlbumsError::Unknown(error.into()))?;

                albums.retain(|album| {
                    Authorization::decide(&caller, &permissions, Action::LibraryRead, album)
                        .is_allowed()
                });

                let total = albums.len();
                let offset = command.offset().unwrap_or(0);
                let page = albums
                    .into_iter()
                    .skip(offset)
                    .take(command.limit().unwrap_or(usize::MAX))
                    .map(|album| {
                        let mediums: Vec<_> = album
                            .mediums()
                            .iter()
                            .map(|medium| {
                                ListAlbumsMediumItem::new(
                                    medium.index().index(),
                                    medium.index().as_wire().to_owned(),
                                    medium.tracks().len(),
                                )
                            })
                            .collect();
                        let track_count =
                            mediums.iter().map(ListAlbumsMediumItem::track_count).sum();
                        ListAlbumsItem::new(
                            album.id(),
                            album.music_group_id(),
                            album.name().to_owned(),
                            album.release_date(),
                            album.production_type(),
                            album.genre_ids().to_vec(),
                            track_count,
                            album.cover_image_id(),
                            mediums,
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
    use super::*;
    use crate::domain::model::{
        MediumIndex, MusicAlbum, MusicMedium, MusicRecording, ProductionType, Role, TrackIndex,
        User,
    };

    use crate::domain::port::{MockMusicAlbumRepository, MockUserRepository, RepositoryError};

    use crate::test_helpers::*;
    use chrono::NaiveDate;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = ListAlbums<TestFactory>;

    /// Build one `CD` medium sequencing one recording per index.
    fn medium(medium_index: u8, track_count: u8) -> Result<MusicMedium, Box<dyn Error>> {
        let tracks = (0..track_count)
            .map(|index| {
                Ok(MusicRecording::try_new(
                    i64::from(index),
                    Some(format!("Track {index}")),
                    None,
                    i64::from(index),
                    i64::from(index),
                    None,
                    TrackIndex::MediumIndex(index),
                )?)
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
        Ok(MusicMedium::new(MediumIndex::Cd(medium_index), tracks))
    }

    /// Build one album holding `track_count` tracks on one medium, named
    /// `Album {album_id}`.
    fn album(album_id: i64, track_count: u8) -> Result<MusicAlbum, Box<dyn Error>> {
        Ok(MusicAlbum::try_new(
            album_id,
            format!("Album {album_id}"),
            Some(album_release_date()),
            Some(ProductionType::Studio),
            vec!["JAZZ".to_owned()],
            0,
            None,
            None,
            vec![medium(1, track_count)?],
        )?)
    }

    /// Build one album holding no medium, named `name`.
    fn empty_album(album_id: i64, name: &str) -> Result<MusicAlbum, Box<dyn Error>> {
        Ok(MusicAlbum::try_new(
            album_id,
            name.to_owned(),
            Some(NaiveDate::default()),
            None,
            Vec::new(),
            0,
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
            permissions: no_permissions(),
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
    async fn list_albums_returns_every_album_of_the_page_with_its_mediums()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        let first = album(1, 1)?;
        let second = album(2, 2)?;
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
                0,
                "Album 1".to_owned(),
                Some(album_release_date()),
                Some(ProductionType::Studio),
                vec!["JAZZ".to_owned()],
                1,
                None,
                vec![ListAlbumsMediumItem::new(1, "CD".to_owned(), 1)],
            ))
        );
        assert_eq!(
            response.albums().get(1).map(ListAlbumsItem::track_count),
            Some(2)
        );
        assert_eq!(
            response
                .albums()
                .get(1)
                .and_then(|album| album.mediums().first())
                .map(ListAlbumsMediumItem::track_count),
            Some(2),
            "the album's total track count is the sum of its mediums"
        );
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn list_albums_carries_the_cover_of_an_album_with_artwork() -> Result<(), Box<dyn Error>>
    {
        // Arrange: one covered album beside one without artwork.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let covered = MusicAlbum::try_new(
            1,
            "Album 1".to_owned(),
            Some(album_release_date()),
            None,
            Vec::new(),
            0,
            None,
            Some(19),
            Vec::new(),
        )?;
        assert_eq!(
            covered.cover_image_id(),
            Some(19),
            "an album built with artwork must report the identifier"
        );
        let uncovered = empty_album(2, "Album 2")?;
        assert_eq!(
            uncovered.cover_image_id(),
            None,
            "an album built without artwork must report none"
        );
        let mut albums = MockMusicAlbumRepository::new();
        albums
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![covered, uncovered]) }));
        let harness = harness_with(users, albums);
        let use_case: UseCase = ListAlbums::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(ListAlbumsCommand::new(1, None, None, None))
            .await?;

        // Assert
        assert_eq!(
            response
                .albums()
                .first()
                .and_then(ListAlbumsItem::cover_image_id),
            Some(19)
        );
        assert_eq!(
            response
                .albums()
                .get(1)
                .and_then(ListAlbumsItem::cover_image_id),
            None
        );
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
