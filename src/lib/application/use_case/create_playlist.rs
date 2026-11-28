use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use tracing::warn;

use crate::application::authorization::authorize;
use crate::application::cover_is_viewable;
use crate::application::port::{
    CreatePlaylistCommand, CreatePlaylistError, CreatePlaylistResponse, CreatePlaylistUseCase,
};
use crate::domain::model::{
    Action, ChildKind, MusicPlaylist, ResourceKind, ResourceRef, Visibility,
};
use crate::domain::port::{
    IdentityUnitOfWork, LibraryUnitOfWork, MusicPlaylistFilter, MusicPlaylistRepository as _,
    NewResource, RepositoryError, ResourceRepository as _, UnitOfWork as _, UnitOfWorkFactory,
    UserFilter, UserRepository as _, UserUnitOfWork,
};
use crate::domain::service::{NamePolicy, NamePolicyError};

/// Use case implementation for creating a playlist.
pub(crate) struct CreatePlaylist<F> {
    /// Factory opening the unit of work wrapping the creation.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> CreatePlaylist<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> CreatePlaylistUseCase for CreatePlaylist<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
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
                .begin_write()
                .await
                .map_err(|error| CreatePlaylistError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
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

                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::PlaylistCollection, 0),
                    Action::CreateChild,
                    Some(ChildKind::MusicPlaylist),
                )
                .await
                .map_err(|_| CreatePlaylistError::Forbidden)?;

                let name =
                    NamePolicy::validate(command.name().to_owned()).map_err(
                        |error| match error {
                            NamePolicyError::Empty => {
                                warn!(
                                    target: "security",
                                    event = "input_validation_failed",
                                    field = %"name",
                                    reason = %"empty",
                                    "rejected a client-supplied value"
                                );
                                CreatePlaylistError::InvalidName
                            }
                            NamePolicyError::SurroundingWhitespace => {
                                warn!(
                                    target: "security",
                                    event = "input_validation_failed",
                                    field = %"name",
                                    reason = %"surrounding_whitespace",
                                    "rejected a client-supplied value"
                                );
                                CreatePlaylistError::InvalidName
                            }
                            NamePolicyError::TooLong => {
                                warn!(
                                    target: "security",
                                    event = "input_validation_failed",
                                    field = %"name",
                                    reason = %"too_long",
                                    "rejected a client-supplied value"
                                );
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
                    command.cover_image_id(),
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

                if let Some(cover_image_id) = command.cover_image_id() {
                    if !cover_is_viewable(&mut unit_of_work, &caller, Some(cover_image_id)).await {
                        return Err(CreatePlaylistError::InvalidCover);
                    }
                    let backed = unit_of_work
                        .music_playlists()
                        .search(&MusicPlaylistFilter {
                            cover_image_id: Some(cover_image_id),
                            ..MusicPlaylistFilter::default()
                        })
                        .await
                        .map_err(|error| CreatePlaylistError::Unknown(error.into()))?;
                    if !backed.is_empty() {
                        return Err(CreatePlaylistError::InvalidCover);
                    }
                }

                let created = unit_of_work
                    .music_playlists()
                    .create(playlist)
                    .await
                    .map_err(|error| match error {
                        // A concurrent insert won the race for the per-owner
                        // unique name (`STY-RUST-014`).
                        RepositoryError::AlreadyExist => CreatePlaylistError::NameAlreadyExists,
                        RepositoryError::Conflict | RepositoryError::DataIntegrityViolation => {
                            CreatePlaylistError::InvalidCover
                        }
                        // Every other variant maps to `Unknown`; naming them
                        // keeps a new port variant a compile error here.
                        RepositoryError::ConcurrencyConflict
                        | RepositoryError::OperationFailed
                        | RepositoryError::Timeout
                        | RepositoryError::Unavailable
                        | RepositoryError::Unknown(_) => CreatePlaylistError::Unknown(error.into()),
                    })?;

                unit_of_work
                    .resources()
                    .create(NewResource {
                        kind: ResourceKind::MusicPlaylist,
                        entity_id: created.id(),
                        owner_id: Some(command.caller_id()),
                        auth_parent: Some(ResourceRef::new(ResourceKind::Library, 0)),
                        visibility: if created.is_public() {
                            Visibility::Public
                        } else {
                            Visibility::Private
                        },
                    })
                    .await
                    .map_err(|error| CreatePlaylistError::Unknown(error.into()))?;

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
    use super::*;
    use crate::domain::model::{Role, User};

    use crate::domain::port::{MockMusicPlaylistRepository, MockUserRepository};

    use crate::domain::service::MAX_NAME_LENGTH;

    use crate::test_helpers::*;
    use chrono::NaiveDateTime;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    type UseCase = CreatePlaylist<TestFactory>;

    fn playlist(id: i64, user_id: i64, name: &str, is_public: bool) -> MusicPlaylist {
        MusicPlaylist::new(
            id,
            name.to_owned(),
            NaiveDateTime::default(),
            NaiveDateTime::default(),
            user_id,
            is_public,
            None,
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

    #[tokio::test]
    async fn create_playlist_creates_a_private_playlist_by_default() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository, caller_id: i64| -> Result<(), Box<dyn Error>> {
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
        })(&mut users, 1)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        let stored = playlist(9, 1, "Mix", false);
        (move |playlists: &mut MockMusicPlaylistRepository| {
            playlists
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        })(&mut playlists);
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
            .execute(CreatePlaylistCommand::new(1, "Mix".to_owned(), false, None))
            .await?;

        // Assert
        assert_eq!(response.playlist().id(), 9);
        assert!(!response.playlist().is_public());
        assert!(response.playlist().tracks().is_empty());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_playlist_surrounding_whitespace_returns_invalid_name()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository, caller_id: i64| -> Result<(), Box<dyn Error>> {
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
        })(&mut users, 1)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        playlists.expect_search().times(0);
        playlists.expect_create().times(0);
        let harness = harness_with(users, playlists);
        let use_case: UseCase = CreatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreatePlaylistCommand::new(
                1,
                " Mix ".to_owned(),
                false,
                None,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(CreatePlaylistError::InvalidName)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_playlist_duplicate_name_returns_already_exists() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository, caller_id: i64| -> Result<(), Box<dyn Error>> {
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
        })(&mut users, 1)?;
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
            .execute(CreatePlaylistCommand::new(1, "Mix".to_owned(), false, None))
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
        (move |users: &mut MockUserRepository, caller_id: i64| -> Result<(), Box<dyn Error>> {
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
        })(&mut users, 1)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        (move |playlists: &mut MockMusicPlaylistRepository| {
            playlists
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        })(&mut playlists);
        playlists
            .expect_create()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::AlreadyExist) }));
        let harness = harness_with(users, playlists);
        let use_case: UseCase = CreatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreatePlaylistCommand::new(1, "Mix".to_owned(), false, None))
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
        (move |users: &mut MockUserRepository, caller_id: i64| -> Result<(), Box<dyn Error>> {
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
        })(&mut users, 1)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        playlists.expect_create().times(0);
        let harness = harness_with(users, playlists);
        let use_case: UseCase = CreatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreatePlaylistCommand::new(1, "   ".to_owned(), false, None))
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
        (move |users: &mut MockUserRepository, caller_id: i64| -> Result<(), Box<dyn Error>> {
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
        })(&mut users, 1)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        playlists.expect_create().times(0);
        let harness = harness_with(users, playlists);
        let use_case: UseCase = CreatePlaylist::new(Arc::clone(&harness.factory));
        let name = "x".repeat(MAX_NAME_LENGTH.saturating_add(1));

        // Act
        let result = use_case
            .execute(CreatePlaylistCommand::new(1, name, false, None))
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
            .execute(CreatePlaylistCommand::new(
                999,
                "Mix".to_owned(),
                false,
                None,
            ))
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
        (move |users: &mut MockUserRepository, caller_id: i64| -> Result<(), Box<dyn Error>> {
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
        })(&mut users, 1)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        (move |playlists: &mut MockMusicPlaylistRepository| {
            playlists
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        })(&mut playlists);
        playlists
            .expect_create()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, playlists);
        let use_case: UseCase = CreatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreatePlaylistCommand::new(1, "Mix".to_owned(), false, None))
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
        (move |users: &mut MockUserRepository, caller_id: i64| -> Result<(), Box<dyn Error>> {
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
        })(&mut users, 1)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        let stored = playlist(9, 1, "Mix", false);
        (move |playlists: &mut MockMusicPlaylistRepository| {
            playlists
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        })(&mut playlists);
        playlists.expect_create().times(1).return_once(move |_| {
            let found = stored.clone();
            Box::pin(async move { Ok(found) })
        });
        let harness = harness_with(users, playlists);
        harness.commit_fails.store(true, Ordering::SeqCst);
        let use_case: UseCase = CreatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreatePlaylistCommand::new(1, "Mix".to_owned(), false, None))
            .await;

        // Assert
        assert!(matches!(result, Err(CreatePlaylistError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_playlist_stores_a_viewable_cover() -> Result<(), Box<dyn Error>> {
        // Arrange: the caller names an image no playlist backs yet.
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository, caller_id: i64| -> Result<(), Box<dyn Error>> {
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
        })(&mut users, 1)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        let stored = MusicPlaylist::new(
            9,
            "Mix".to_owned(),
            NaiveDateTime::default(),
            NaiveDateTime::default(),
            1,
            false,
            Some(19),
            Vec::new(),
        );
        playlists
            .expect_search()
            .times(2)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        playlists
            .expect_create()
            .times(1)
            .withf(|pending| pending.cover_image_id() == Some(19))
            .return_once(move |_| {
                let found = stored.clone();
                Box::pin(async move { Ok(found) })
            });
        let harness = harness_with(users, playlists);
        let use_case: UseCase = CreatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(CreatePlaylistCommand::new(
                1,
                "Mix".to_owned(),
                false,
                Some(19),
            ))
            .await?;

        // Assert
        assert_eq!(response.playlist().cover_image_id(), Some(19));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_playlist_unknown_cover_returns_invalid_cover() -> Result<(), Box<dyn Error>> {
        // Arrange: the named identifier resolves to no image.
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository, caller_id: i64| -> Result<(), Box<dyn Error>> {
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
        })(&mut users, 1)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        playlists
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        playlists.expect_create().times(0);
        let harness = harness_with(users, playlists).with_unknown_images();
        let use_case: UseCase = CreatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreatePlaylistCommand::new(
                1,
                "Mix".to_owned(),
                false,
                Some(19),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(CreatePlaylistError::InvalidCover)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_playlist_cover_backed_by_another_playlist_returns_invalid_cover()
    -> Result<(), Box<dyn Error>> {
        // Arrange: another playlist already carries the named image.
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository, caller_id: i64| -> Result<(), Box<dyn Error>> {
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
        })(&mut users, 1)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        playlists
            .expect_search()
            .times(1)
            .withf(|filter| filter.name.is_some())
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let backer = playlist(3, 2, "Theirs", false);
        playlists
            .expect_search()
            .times(1)
            .withf(|filter| filter.cover_image_id == Some(19))
            .return_once(move |_| {
                let found = backer.clone();
                Box::pin(async move { Ok(vec![found]) })
            });
        playlists.expect_create().times(0);
        let harness = harness_with(users, playlists);
        let use_case: UseCase = CreatePlaylist::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreatePlaylistCommand::new(
                1,
                "Mix".to_owned(),
                false,
                Some(19),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(CreatePlaylistError::InvalidCover)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
