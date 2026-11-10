use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tracing::warn;

use crate::application::port::put_album_medium::{
    PutAlbumMediumCommand, PutAlbumMediumError, PutAlbumMediumResponse, PutAlbumMediumUseCase,
};
use crate::domain::model::music_medium::MediumIndex;
use crate::domain::model::permission::Action;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::music_album_repository::{MusicAlbumFilter, MusicAlbumRepository as _};
use crate::domain::port::permission_repository::{PermissionFilter, PermissionRepository as _};
use crate::domain::port::unit_of_work::{UnitOfWork as _, UnitOfWorkFactory};
use crate::domain::port::user_repository::{UserFilter, UserRepository as _};
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::service::authorization::{Authorization, Decision};

/// Use case implementation for creating or retyping a music medium.
pub(crate) struct PutAlbumMedium<F> {
    /// Factory opening the unit of work wrapping the write.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> PutAlbumMedium<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> PutAlbumMediumUseCase for PutAlbumMedium<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: PutAlbumMediumCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PutAlbumMediumResponse, PutAlbumMediumError>>
                + Send
                + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| PutAlbumMediumError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| PutAlbumMediumError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(PutAlbumMediumError::NoSuchCaller)?;

                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| PutAlbumMediumError::Unknown(error.into()))?;

                let album = unit_of_work
                    .music_albums()
                    .search(&MusicAlbumFilter {
                        id: Some(command.album_id()),
                        ..MusicAlbumFilter::default()
                    })
                    .await
                    .map_err(|error| PutAlbumMediumError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(PutAlbumMediumError::NoSuchAlbum)?;

                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::LibraryRead, &album)
                {
                    warn!(
                        target: "security",
                        event = "authz_failed",
                        actor = caller.id(),
                        action = %denial.action().verb(),
                        target = %denial.action().target(),
                        "denied an action"
                    );
                    return Err(PutAlbumMediumError::NoSuchAlbum);
                }
                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::LibraryManage, &album)
                {
                    warn!(
                        target: "security",
                        event = "authz_failed",
                        actor = caller.id(),
                        action = %denial.action().verb(),
                        target = %denial.action().target(),
                        "denied an action"
                    );
                    return Err(if denial.conceals() {
                        PutAlbumMediumError::NoSuchAlbum
                    } else {
                        PutAlbumMediumError::Forbidden
                    });
                }

                if album.id() == 0 {
                    return Err(PutAlbumMediumError::DefaultAlbum);
                }

                let medium = MediumIndex::from_wire(command.medium_type(), command.medium_index())
                    .ok_or_else(|| {
                        warn!(
                            target: "security",
                            event = "input_validation_failed",
                            field = %"medium_type",
                            reason = %"invalid_value",
                            "rejected a client-supplied value"
                        );
                        PutAlbumMediumError::InvalidMediumType
                    })?;

                let existed = album
                    .mediums()
                    .iter()
                    .any(|stored| stored.index().index() == command.medium_index());

                let stored_medium = unit_of_work
                    .music_albums()
                    .put_medium(album.id(), medium)
                    .await
                    .map_err(|error| {
                        if matches!(error, RepositoryError::Conflict) {
                            PutAlbumMediumError::Conflict
                        } else {
                            PutAlbumMediumError::Unknown(error.into())
                        }
                    })?
                    .ok_or(PutAlbumMediumError::NoSuchAlbum)?;

                let stored = unit_of_work
                    .music_albums()
                    .search(&MusicAlbumFilter {
                        id: Some(album.id()),
                        ..MusicAlbumFilter::default()
                    })
                    .await
                    .map_err(|error| PutAlbumMediumError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(PutAlbumMediumError::NoSuchAlbum)?;

                Ok(PutAlbumMediumResponse::new(
                    stored,
                    stored_medium.index().index(),
                    !existed,
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| PutAlbumMediumError::Unknown(error.into()))?;
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
    use crate::domain::alias::NumericID;
    use crate::domain::model::music_album::MusicAlbum;
    use crate::domain::model::music_medium::MusicMedium;
    use crate::domain::model::user::{Role, User};
    use crate::domain::port::music_album_repository::MockMusicAlbumRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::*;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = PutAlbumMedium<TestFactory>;

    /// Expect the caller load to find an administrator.
    fn expect_admin(
        users: &mut MockUserRepository,
        caller_id: NumericID,
    ) -> Result<(), Box<dyn Error>> {
        let caller = User::try_new(caller_id, "admin".to_owned(), None, caller_id, Role::Admin)?;
        users
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(caller_id))
            .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
        Ok(())
    }

    /// Build a Library test harness around the given repository.
    fn harness_with(
        users: MockUserRepository,
        albums: MockMusicAlbumRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            music_albums: albums,
            permissions: no_permissions(),
            users,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
    }

    /// Expect the album load to find `album`.
    fn expect_album(albums: &mut MockMusicAlbumRepository, album: MusicAlbum) {
        let album_id = album.id();
        albums
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(album_id))
            .return_once(move |_| {
                let found = album.clone();
                Box::pin(async move { Ok(vec![found]) })
            });
    }

    /// The album with a `CD` medium at index 1.
    fn album_with_cd(id: NumericID) -> Result<MusicAlbum, Box<dyn Error>> {
        sample_album(id, vec![MusicMedium::new(MediumIndex::Cd(1), Vec::new())])
    }

    #[tokio::test]
    async fn put_album_medium_creates_the_medium() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, sample_album(7, Vec::new())?);
        albums
            .expect_put_medium()
            .times(1)
            .withf(|album_id, medium| *album_id == 7 && *medium == MediumIndex::Cd(1))
            .returning(|_, medium| {
                let written = MusicMedium::new(medium, Vec::new());
                Box::pin(async move { Ok(Some(written)) })
            });
        let stored = album_with_cd(7)?;
        albums.expect_search().times(1).return_once(move |_| {
            let found = stored.clone();
            Box::pin(async move { Ok(vec![found]) })
        });
        let harness = harness_with(users, albums);
        let use_case: UseCase = PutAlbumMedium::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(PutAlbumMediumCommand::new(1, 7, 1, "CD".to_owned()))
            .await?;

        // Assert
        assert!(response.created());
        assert_eq!(response.medium_index(), 1);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn put_album_medium_retypes_the_medium() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, album_with_cd(7)?);
        albums.expect_put_medium().times(1).returning(|_, medium| {
            let written = MusicMedium::new(medium, Vec::new());
            Box::pin(async move { Ok(Some(written)) })
        });
        let stored = sample_album(
            7,
            vec![MusicMedium::new(MediumIndex::Digital(1), Vec::new())],
        )?;
        albums.expect_search().times(1).return_once(move |_| {
            let found = stored.clone();
            Box::pin(async move { Ok(vec![found]) })
        });
        let harness = harness_with(users, albums);
        let use_case: UseCase = PutAlbumMedium::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(PutAlbumMediumCommand::new(1, 7, 1, "DIGITAL".to_owned()))
            .await?;

        // Assert
        assert!(!response.created());
        Ok(())
    }

    #[tokio::test]
    async fn put_album_medium_rejects_an_unknown_type() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, sample_album(7, Vec::new())?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = PutAlbumMedium::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(PutAlbumMediumCommand::new(1, 7, 1, "VINYL".to_owned()))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(PutAlbumMediumError::InvalidMediumType)
        ));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn put_album_medium_rejects_the_default_album() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, sample_album(0, Vec::new())?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = PutAlbumMedium::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(PutAlbumMediumCommand::new(1, 0, 1, "CD".to_owned()))
            .await;

        // Assert
        assert!(matches!(result, Err(PutAlbumMediumError::DefaultAlbum)));
        Ok(())
    }

    #[tokio::test]
    async fn put_album_medium_unknown_album_returns_no_such_album() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        albums
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, albums);
        let use_case: UseCase = PutAlbumMedium::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(PutAlbumMediumCommand::new(1, 404, 1, "CD".to_owned()))
            .await;

        // Assert
        assert!(matches!(result, Err(PutAlbumMediumError::NoSuchAlbum)));
        Ok(())
    }

    #[tokio::test]
    async fn put_album_medium_protected_medium_is_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange: the stored medium refuses the write.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, sample_album(7, Vec::new())?);
        albums
            .expect_put_medium()
            .times(1)
            .returning(|_, _| Box::pin(async { Err(RepositoryError::Conflict) }));
        let harness = harness_with(users, albums);
        let use_case: UseCase = PutAlbumMedium::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(PutAlbumMediumCommand::new(1, 7, 1, "CD".to_owned()))
            .await;

        // Assert
        assert!(matches!(result, Err(PutAlbumMediumError::Conflict)));
        Ok(())
    }

    #[tokio::test]
    async fn put_album_medium_standard_user_is_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, sample_album(7, Vec::new())?);
        let harness = harness_with(users, albums);
        let use_case: UseCase = PutAlbumMedium::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(PutAlbumMediumCommand::new(1, 7, 1, "CD".to_owned()))
            .await;

        // Assert
        assert!(matches!(result, Err(PutAlbumMediumError::Forbidden)));
        Ok(())
    }
}
