use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::list_playlists::ListPlaylistsCommand;
use crate::application::port::list_playlists::ListPlaylistsError;
use crate::application::port::list_playlists::ListPlaylistsItem;
use crate::application::port::list_playlists::ListPlaylistsResponse;
use crate::application::port::list_playlists::ListPlaylistsUseCase;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::music_playlist_repository::MusicPlaylistFilter;
use crate::domain::port::music_playlist_repository::MusicPlaylistRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::service::library_access::LibraryAccess;

/// Use case implementation for listing the playlists the caller may see.
pub struct ListPlaylists<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> ListPlaylists<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> ListPlaylistsUseCase for ListPlaylists<F>
where
    F: UnitOfWorkFactory,
    F::Uow: LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: ListPlaylistsCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ListPlaylistsResponse, ListPlaylistsError>> + Send + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| ListPlaylistsError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| ListPlaylistsError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(ListPlaylistsError::NoSuchCaller)?;

                let mut playlists = unit_of_work
                    .music_playlists()
                    .search(&MusicPlaylistFilter {
                        is_public: command.is_public(),
                        name: command.name().map(str::to_owned),
                        user_id: command.owner_id(),
                        ..MusicPlaylistFilter::default()
                    })
                    .await
                    .map_err(|error| ListPlaylistsError::Unknown(error.into()))?;
                playlists.retain(|playlist| LibraryAccess::can_read_playlist(playlist, &caller));

                let total = playlists.len();
                let offset = command.offset().unwrap_or(0);
                let page = playlists
                    .into_iter()
                    .skip(offset)
                    .take(command.limit().unwrap_or(usize::MAX))
                    .map(|playlist| {
                        ListPlaylistsItem::new(
                            playlist.id(),
                            playlist.user_id(),
                            playlist.name().to_owned(),
                            playlist.is_public(),
                            playlist.created_at(),
                            playlist.last_modified_at(),
                            playlist.tracks().len(),
                            playlist.cover_file_id(),
                        )
                    })
                    .collect();

                Ok(ListPlaylistsResponse::new(page, total))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| ListPlaylistsError::Unknown(error.into()))?;
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

    use chrono::NaiveDate;
    use chrono::NaiveDateTime;

    use crate::application::port::list_playlists::ListPlaylistsCommand;
    use crate::application::port::list_playlists::ListPlaylistsError;
    use crate::application::port::list_playlists::ListPlaylistsItem;
    use crate::application::port::list_playlists::ListPlaylistsUseCase as _;
    use crate::domain::model::music_playlist::MusicPlaylist;
    use crate::domain::model::music_recording::MusicRecording;
    use crate::domain::model::music_recording::TrackIndex;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::music_playlist_repository::MockMusicPlaylistRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::GalleryFactoryHarness;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::TestUnitOfWork;

    use super::ListPlaylists;

    type UseCase = ListPlaylists<TestFactory>;

    /// Build a playlist with `track_count` tracks.
    fn playlist(
        id: i64,
        user_id: i64,
        name: &str,
        is_public: bool,
        track_count: u32,
    ) -> Result<MusicPlaylist, Box<dyn Error>> {
        let tracks = (0..track_count)
            .map(|index| {
                Ok(MusicRecording::try_new(
                    i64::from(index).saturating_add(100),
                    Some(format!("track {index}")),
                    NaiveDate::from_ymd_opt(2026, 10, 20).ok_or("invalid fixture date")?,
                    None,
                    i64::from(index),
                    i64::from(index),
                    None,
                    TrackIndex::PlaylistIndex(index),
                )?)
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
        Ok(MusicPlaylist::new(
            id,
            name.to_owned(),
            NaiveDateTime::default(),
            NaiveDateTime::default(),
            user_id,
            is_public,
            None,
            tracks,
        ))
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
    async fn list_playlists_standard_caller_sees_public_and_owned_only()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        let public = playlist(2, 3, "Public", true, 2)?;
        let own = playlist(3, 4, "Mine", false, 1)?;
        let other_private = playlist(5, 9, "Other", false, 0)?;
        playlists.expect_search().times(1).return_once(move |_| {
            let found = vec![public.clone(), own.clone(), other_private.clone()];
            Box::pin(async move { Ok(found) })
        });
        let harness = harness_with(users, playlists);
        let use_case: UseCase = ListPlaylists::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(ListPlaylistsCommand::new(4, None, None, None, None, None))
            .await?;

        // Assert
        assert_eq!(response.total(), 2);
        assert_eq!(
            response.playlists(),
            &[
                ListPlaylistsItem::new(
                    2,
                    3,
                    "Public".to_owned(),
                    true,
                    NaiveDateTime::default(),
                    NaiveDateTime::default(),
                    2,
                    None,
                ),
                ListPlaylistsItem::new(
                    3,
                    4,
                    "Mine".to_owned(),
                    false,
                    NaiveDateTime::default(),
                    NaiveDateTime::default(),
                    1,
                    None,
                ),
            ]
        );
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn list_playlists_carries_the_cover_of_a_playlist_with_artwork()
    -> Result<(), Box<dyn Error>> {
        // Arrange: one covered playlist beside one without artwork.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let covered = MusicPlaylist::new(
            1,
            "Covered".to_owned(),
            NaiveDateTime::default(),
            NaiveDateTime::default(),
            4,
            true,
            Some(19),
            Vec::new(),
        );
        assert_eq!(
            covered.cover_file_id(),
            Some(19),
            "a playlist built with artwork must report the identifier"
        );
        let uncovered = playlist(2, 4, "Plain", true, 0)?;
        assert_eq!(
            uncovered.cover_file_id(),
            None,
            "a playlist built without artwork must report none"
        );
        let mut playlists = MockMusicPlaylistRepository::new();
        playlists.expect_search().times(1).return_once(move |_| {
            let found = vec![covered.clone(), uncovered.clone()];
            Box::pin(async move { Ok(found) })
        });
        let harness = harness_with(users, playlists);
        let use_case: UseCase = ListPlaylists::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(ListPlaylistsCommand::new(4, None, None, None, None, None))
            .await?;

        // Assert
        assert_eq!(
            response
                .playlists()
                .first()
                .and_then(ListPlaylistsItem::cover_file_id),
            Some(19)
        );
        assert_eq!(
            response
                .playlists()
                .get(1)
                .and_then(ListPlaylistsItem::cover_file_id),
            None
        );
        Ok(())
    }

    #[tokio::test]
    async fn list_playlists_root_admin_sees_every_playlist() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 0, Role::Admin)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        let public = playlist(2, 3, "Public", true, 0)?;
        let private = playlist(3, 4, "Private", false, 0)?;
        playlists.expect_search().times(1).return_once(move |_| {
            let found = vec![public.clone(), private.clone()];
            Box::pin(async move { Ok(found) })
        });
        let harness = harness_with(users, playlists);
        let use_case: UseCase = ListPlaylists::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(ListPlaylistsCommand::new(0, None, None, None, None, None))
            .await?;

        // Assert
        assert_eq!(response.total(), 2);
        Ok(())
    }

    #[tokio::test]
    async fn list_playlists_forwards_filters_and_paginates() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 4, Role::Standard)?;
        let first = playlist(1, 4, "Trip A", true, 0)?;
        let second = playlist(2, 4, "Trip B", true, 0)?;
        let third = playlist(3, 4, "Trip C", true, 0)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        playlists
            .expect_search()
            .times(1)
            .withf(|filter| {
                filter.name.as_deref() == Some("Trip")
                    && filter.is_public == Some(true)
                    && filter.user_id == Some(4)
            })
            .return_once(move |_| {
                let found = vec![first.clone(), second.clone(), third.clone()];
                Box::pin(async move { Ok(found) })
            });
        let harness = harness_with(users, playlists);
        let use_case: UseCase = ListPlaylists::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(ListPlaylistsCommand::new(
                4,
                Some("Trip".to_owned()),
                Some(true),
                Some(4),
                Some(1),
                Some(1),
            ))
            .await?;

        // Assert
        assert_eq!(response.total(), 3);
        assert_eq!(
            response.playlists().first().map(ListPlaylistsItem::id),
            Some(2)
        );
        Ok(())
    }

    #[tokio::test]
    async fn list_playlists_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, MockMusicPlaylistRepository::new());
        let use_case: UseCase = ListPlaylists::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ListPlaylistsCommand::new(999, None, None, None, None, None))
            .await;

        // Assert
        assert!(matches!(result, Err(ListPlaylistsError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn list_playlists_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        playlists
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, playlists);
        let use_case: UseCase = ListPlaylists::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ListPlaylistsCommand::new(1, None, None, None, None, None))
            .await;

        // Assert
        assert!(matches!(result, Err(ListPlaylistsError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn list_playlists_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1, Role::Standard)?;
        let mut playlists = MockMusicPlaylistRepository::new();
        playlists
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, playlists);
        harness.commit_fails.store(true, Ordering::SeqCst);
        let use_case: UseCase = ListPlaylists::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ListPlaylistsCommand::new(1, None, None, None, None, None))
            .await;

        // Assert
        assert!(matches!(result, Err(ListPlaylistsError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }
}
