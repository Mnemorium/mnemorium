use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::get_album::GetAlbumCommand;
use crate::application::port::get_album::GetAlbumError;
use crate::application::port::get_album::GetAlbumMediumItem;
use crate::application::port::get_album::GetAlbumResponse;
use crate::application::port::get_album::GetAlbumUseCase;
use crate::application::security_event;
use crate::domain::model::permission::Action;
use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::music_album_repository::MusicAlbumFilter;
use crate::domain::port::music_album_repository::MusicAlbumRepository as _;
use crate::domain::port::permission_repository::PermissionFilter;
use crate::domain::port::permission_repository::PermissionRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::service::authorization::Authorization;
use crate::domain::service::authorization::Decision;

/// Use case implementation for fetching one music album.
pub struct GetAlbum<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> GetAlbum<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> GetAlbumUseCase for GetAlbum<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: GetAlbumCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetAlbumResponse, GetAlbumError>> + Send + 'future>>
    {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| GetAlbumError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| GetAlbumError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetAlbumError::NoSuchCaller)?;

                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| GetAlbumError::Unknown(error.into()))?;

                let album = unit_of_work
                    .music_albums()
                    .search(&MusicAlbumFilter {
                        id: Some(command.album_id()),
                        ..MusicAlbumFilter::default()
                    })
                    .await
                    .map_err(|error| GetAlbumError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetAlbumError::NoSuchAlbum)?;
                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::CatalogRead, &album)
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(GetAlbumError::NoSuchAlbum);
                }

                let mediums: Vec<_> = album
                    .mediums()
                    .iter()
                    .map(|medium| {
                        GetAlbumMediumItem::new(
                            medium.index().index(),
                            medium.index().as_wire().to_owned(),
                            medium.tracks().len(),
                        )
                    })
                    .collect();
                let track_count = mediums.iter().map(GetAlbumMediumItem::track_count).sum();

                Ok(GetAlbumResponse::new(
                    album.id(),
                    album.name().to_owned(),
                    album.release_date(),
                    album.production_type(),
                    album.genre_ids().to_vec(),
                    track_count,
                    album.cover_file_id(),
                    mediums,
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| GetAlbumError::Unknown(error.into()))?;
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

    use crate::application::port::get_album::GetAlbumCommand;
    use crate::application::port::get_album::GetAlbumError;
    use crate::application::port::get_album::GetAlbumMediumItem;
    use crate::application::port::get_album::GetAlbumUseCase as _;
    use crate::domain::model::music_album::MusicAlbum;
    use crate::domain::model::music_album::ProductionType;
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
    use crate::test_helpers::no_permissions;

    use super::GetAlbum;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = GetAlbum<TestFactory>;

    /// Build one `CD` medium sequencing one recording.
    fn medium(medium_index: u8) -> Result<MusicMedium, Box<dyn Error>> {
        let recording = MusicRecording::try_new(
            900,
            Some("So What".to_owned()),
            album_release_date(),
            None,
            1,
            101,
            None,
            TrackIndex::MediumIndex(0),
        )?;
        Ok(MusicMedium::new(
            MediumIndex::Cd(medium_index),
            vec![recording],
        ))
    }

    /// Build an album holding two `CD` mediums.
    fn album(album_id: i64) -> Result<MusicAlbum, Box<dyn Error>> {
        Ok(MusicAlbum::try_new(
            album_id,
            format!("Album {album_id}"),
            album_release_date(),
            Some(ProductionType::Studio),
            vec!["JAZZ".to_owned()],
            None,
            None,
            vec![medium(1)?, medium(2)?],
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
    async fn get_album_returns_the_album_with_its_mediums() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album(7)?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = GetAlbum::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(GetAlbumCommand::new(1, 7)).await?;

        // Assert
        assert_eq!(response.album_id(), 7);
        assert_eq!(response.name(), "Album 7");
        assert_eq!(response.release_date(), album_release_date());
        assert_eq!(response.production_type(), Some(ProductionType::Studio));
        assert_eq!(response.genres(), ["JAZZ"]);
        assert_eq!(response.track_count(), 2);
        assert_eq!(response.mediums().len(), 2);
        assert_eq!(
            response
                .mediums()
                .first()
                .map(GetAlbumMediumItem::medium_index),
            Some(1)
        );
        assert_eq!(
            response
                .mediums()
                .first()
                .map(GetAlbumMediumItem::medium_type),
            Some("CD")
        );
        assert_eq!(
            response
                .mediums()
                .first()
                .map(GetAlbumMediumItem::track_count),
            Some(1)
        );
        assert_eq!(
            response
                .mediums()
                .get(1)
                .map(GetAlbumMediumItem::medium_index),
            Some(2)
        );
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_album_without_medium_returns_an_empty_medium_list() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, empty_album(7)?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = GetAlbum::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(GetAlbumCommand::new(1, 7)).await?;

        // Assert
        assert_eq!(response.album_id(), 7);
        assert!(response.mediums().is_empty());
        assert_eq!(response.track_count(), 0);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_album_carries_the_cover_of_an_album_with_artwork() -> Result<(), Box<dyn Error>> {
        // Arrange: an album whose artwork is file 19.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let covered = MusicAlbum::try_new(
            7,
            "Album 7".to_owned(),
            album_release_date(),
            None,
            Vec::new(),
            None,
            Some(19),
            Vec::new(),
        )?;
        assert_eq!(
            covered.cover_file_id(),
            Some(19),
            "an album built with artwork must report the identifier"
        );
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, covered);
        let harness = harness_with(users, albums);
        let use_case: UseCase = GetAlbum::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(GetAlbumCommand::new(1, 7)).await?;

        // Assert
        assert_eq!(response.cover_file_id(), Some(19));
        Ok(())
    }

    #[tokio::test]
    async fn get_album_unknown_album_returns_no_such_album() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        albums
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, albums);
        let use_case: UseCase = GetAlbum::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetAlbumCommand::new(1, 999)).await;

        // Assert
        assert!(matches!(result, Err(GetAlbumError::NoSuchAlbum)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_album_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, MockMusicAlbumRepository::new());
        let use_case: UseCase = GetAlbum::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetAlbumCommand::new(999, 7)).await;

        // Assert
        assert!(matches!(result, Err(GetAlbumError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_album_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        albums
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, albums);
        let use_case: UseCase = GetAlbum::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetAlbumCommand::new(1, 7)).await;

        // Assert
        assert!(matches!(result, Err(GetAlbumError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_album_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album(7)?);
        let harness = harness_with(users, albums);
        harness.commit_fails.store(true, Ordering::SeqCst);
        let use_case: UseCase = GetAlbum::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetAlbumCommand::new(1, 7)).await;

        // Assert
        assert!(matches!(result, Err(GetAlbumError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }
}
