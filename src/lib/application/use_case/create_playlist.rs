use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use crate::application::port::create_playlist::CreatePlaylistCommand;
use crate::application::port::create_playlist::CreatePlaylistError;
use crate::application::port::create_playlist::CreatePlaylistResponse;
use crate::application::port::create_playlist::CreatePlaylistUseCase;
use crate::application::security_event;
use crate::domain::model::music_playlist::MusicPlaylist;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::music_playlist_repository::MusicPlaylistFilter;
use crate::domain::port::music_playlist_repository::MusicPlaylistRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::service::name_policy::NamePolicy;
use crate::domain::service::name_policy::NamePolicyError;

/// Use case implementation for creating a playlist.
pub struct CreatePlaylist<F> {
    /// Factory opening the unit of work wrapping the creation.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> CreatePlaylist<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> CreatePlaylistUseCase for CreatePlaylist<F>
where
    F: UnitOfWorkFactory,
    F::Uow: LibraryUnitOfWork + UserUnitOfWork,
{
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "RepositoryError is non_exhaustive, so a catch-all arm is required"
    )]
    fn execute<'future>(
        &'future self,
        command: CreatePlaylistCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<CreatePlaylistResponse, CreatePlaylistError>>
                + Send
                + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| CreatePlaylistError::Unknown(error.into()))?;

            let result = async {
                let _caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| CreatePlaylistError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(CreatePlaylistError::NoSuchCaller)?;

                let name =
                    NamePolicy::validate(command.name().to_owned()).map_err(
                        |error| match error {
                            NamePolicyError::Empty => {
                                security_event::input_validation_failed("name", "empty");
                                CreatePlaylistError::InvalidName
                            }
                            NamePolicyError::TooLong => {
                                security_event::input_validation_failed("name", "too_long");
                                CreatePlaylistError::NameTooLong
                            }
                        },
                    )?;
                let now = Utc::now().naive_utc();
                let playlist = MusicPlaylist::new(
                    0,
                    name,
                    now,
                    now,
                    command.caller_id(),
                    command.is_public(),
                    Vec::new(),
                );

                let existing = unit_of_work
                    .music_playlists()
                    .search(&MusicPlaylistFilter {
                        name: Some(playlist.name().to_owned()),
                        user_id: Some(command.caller_id()),
                        ..MusicPlaylistFilter::default()
                    })
                    .await
                    .map_err(|error| CreatePlaylistError::Unknown(error.into()))?;
                if !existing.is_empty() {
                    return Err(CreatePlaylistError::NameAlreadyExists);
                }

                let created = unit_of_work
                    .music_playlists()
                    .create(playlist)
                    .await
                    .map_err(|error| match error {
                        // A concurrent insert won the race for the per-owner
                        // unique name (`STY-RUST-015`).
                        RepositoryError::AlreadyExist => CreatePlaylistError::NameAlreadyExists,
                        other => CreatePlaylistError::Unknown(other.into()),
                    })?;

                Ok(CreatePlaylistResponse::new(created))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| CreatePlaylistError::Unknown(error.into()))?;
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

    use chrono::NaiveDateTime;

    use crate::application::port::create_playlist::CreatePlaylistCommand;
    use crate::application::port::create_playlist::CreatePlaylistError;
    use crate::application::port::create_playlist::CreatePlaylistUseCase as _;
    use crate::domain::model::music_playlist::MusicPlaylist;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::music_playlist_repository::MockMusicPlaylistRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::domain::service::name_policy::MAX_NAME_LENGTH;
    use crate::test_helpers::GalleryFactoryHarness;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::TestUnitOfWork;

    use super::CreatePlaylist;

    type UseCase = CreatePlaylist<TestFactory>;

    fn playlist(id: i64, user_id: i64, name: &str, is_public: bool) -> MusicPlaylist {
        MusicPlaylist::new(
            id,
            name.to_owned(),
            NaiveDateTime::default(),
            NaiveDateTime::default(),
            user_id,
            is_public,
            Vec::new(),
        )
    }

    fn harness_with(
        users: MockUserRepository,
        playlists: MockMusicPlaylistRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            music_playlists: playlists,
            users,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
    }

    fn expect_caller(users: &mut MockUserRepository, caller_id: i64) -> Result<(), Box<dyn Error>> {
        let caller = User::try_new(
            caller_id,
            "caller".to_owned(),
            None,
            caller_id,
            Role::Standard,
        )?;
        users
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(caller_id))
            .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
        Ok(())
    }

    fn expect_no_collision(playlists: &mut MockMusicPlaylistRepository) {
        playlists
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
    }

    #[tokio::test]
    async fn create_playlist_creates_a_private_playlist_by_default() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        let stored = playlist(9, 1, "Mix", false);
        expect_no_collision(&mut playlists);
        playlists
            .expect_create()
            .times(1)
            .withf(|pending| pending.user_id() == 1 && !pending.is_public())
            .return_once(move |_| {
                let found = stored.clone();
                Box::pin(async move { Ok(found) })
            });
        let harness = harness_with(users, playlists);
        let use_case: UseCase = CreatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(CreatePlaylistCommand::new(1, "Mix".to_owned(), false))
            .await?;

        // Assert
        assert_eq!(response.playlist().id(), 9);
        assert!(!response.playlist().is_public());
        assert!(response.playlist().tracks().is_empty());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_playlist_trims_the_name() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let stored = playlist(9, 1, "Mix", false);
        let mut playlists = MockMusicPlaylistRepository::new();
        playlists
            .expect_search()
            .times(1)
            .withf(|filter| filter.name.as_deref() == Some("Mix") && filter.user_id == Some(1))
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        playlists
            .expect_create()
            .times(1)
            .withf(|pending| pending.name() == "Mix")
            .return_once(move |_| {
                let found = stored.clone();
                Box::pin(async move { Ok(found) })
            });
        let harness = harness_with(users, playlists);
        let use_case: UseCase = CreatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(CreatePlaylistCommand::new(1, "  Mix  ".to_owned(), false))
            .await?;

        // Assert
        assert_eq!(response.playlist().name(), "Mix");
        Ok(())
    }

    #[tokio::test]
    async fn create_playlist_duplicate_name_returns_already_exists() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        let existing = playlist(3, 1, "Mix", false);
        playlists
            .expect_search()
            .times(1)
            .withf(|filter| filter.name.as_deref() == Some("Mix") && filter.user_id == Some(1))
            .return_once(move |_| {
                let found = existing.clone();
                Box::pin(async move { Ok(vec![found]) })
            });
        playlists.expect_create().times(0);
        let harness = harness_with(users, playlists);
        let use_case: UseCase = CreatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreatePlaylistCommand::new(1, "Mix".to_owned(), false))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(CreatePlaylistError::NameAlreadyExists)
        ));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_playlist_raced_duplicate_name_returns_already_exists()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_no_collision(&mut playlists);
        playlists
            .expect_create()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::AlreadyExist) }));
        let harness = harness_with(users, playlists);
        let use_case: UseCase = CreatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreatePlaylistCommand::new(1, "Mix".to_owned(), false))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(CreatePlaylistError::NameAlreadyExists)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn create_playlist_empty_name_returns_invalid_name() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        playlists.expect_create().times(0);
        let harness = harness_with(users, playlists);
        let use_case: UseCase = CreatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreatePlaylistCommand::new(1, "   ".to_owned(), false))
            .await;

        // Assert
        assert!(matches!(result, Err(CreatePlaylistError::InvalidName)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_playlist_long_name_returns_name_too_long() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        playlists.expect_create().times(0);
        let harness = harness_with(users, playlists);
        let use_case: UseCase = CreatePlaylist::new(Arc::clone(&harness.factory));
        let name = "x".repeat(MAX_NAME_LENGTH.saturating_add(1));

        // Act
        let result = use_case
            .execute(CreatePlaylistCommand::new(1, name, false))
            .await;

        // Assert
        assert!(matches!(result, Err(CreatePlaylistError::NameTooLong)));
        Ok(())
    }

    #[tokio::test]
    async fn create_playlist_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, MockMusicPlaylistRepository::new());
        let use_case: UseCase = CreatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreatePlaylistCommand::new(999, "Mix".to_owned(), false))
            .await;

        // Assert
        assert!(matches!(result, Err(CreatePlaylistError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_playlist_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        expect_no_collision(&mut playlists);
        playlists
            .expect_create()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, playlists);
        let use_case: UseCase = CreatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreatePlaylistCommand::new(1, "Mix".to_owned(), false))
            .await;

        // Assert
        assert!(matches!(result, Err(CreatePlaylistError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_playlist_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        let stored = playlist(9, 1, "Mix", false);
        expect_no_collision(&mut playlists);
        playlists.expect_create().times(1).return_once(move |_| {
            let found = stored.clone();
            Box::pin(async move { Ok(found) })
        });
        let harness = harness_with(users, playlists);
        harness.commit_fails.store(true, Ordering::SeqCst);
        let use_case: UseCase = CreatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreatePlaylistCommand::new(1, "Mix".to_owned(), false))
            .await;

        // Assert
        assert!(matches!(result, Err(CreatePlaylistError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }
}
