//! Authorization predicates and the caller load shared by the Library use cases.
//!
//! The authorization matrix (`.artifacts/gallery-implementation-plan.md` § 4) is
//! enforced here, in the application layer: the REST handlers only authenticate
//! the caller and map the errors this module's callers return.

use crate::domain::alias::NumericID;
use crate::domain::model::gallery::Gallery;
use crate::domain::model::user::Role;
use crate::domain::model::user::User;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;

/// Return whether `caller` may add `media` owned by `owner_id`.
///
/// Every caller may use their own media; an administrator may use any media
/// (`gallery-implementation-plan.md` § 4, own-media gate).
#[expect(
    clippy::single_call_fn,
    reason = "the predicate is named after the rule it enforces"
)]
pub(crate) fn can_add_media(caller: &User, owner_id: NumericID) -> bool {
    caller.role() == Role::Admin || caller.id() == owner_id
}

/// Return whether `caller` may add items to `gallery`.
#[expect(
    clippy::single_call_fn,
    reason = "the predicate is named after the rule it enforces"
)]
pub(crate) fn can_add_item(gallery: &Gallery, caller: &User) -> bool {
    can_read_gallery(gallery, caller)
}

/// Return whether `caller` may delete `gallery`.
///
/// Gallery deletion is owner-only: an administrator has no override
/// (`gallery-implementation-plan.md` § 4).
#[expect(
    clippy::single_call_fn,
    reason = "the predicate is named after the rule it enforces"
)]
pub(crate) fn can_delete_gallery(gallery: &Gallery, caller: &User) -> bool {
    gallery.is_owned_by(caller.id())
}

/// Return whether `caller` may delete items of `gallery`.
#[expect(
    clippy::single_call_fn,
    reason = "the predicate is named after the rule it enforces"
)]
pub(crate) fn can_delete_item(gallery: &Gallery, caller: &User) -> bool {
    can_read_gallery(gallery, caller)
}

/// Return whether `caller` may read `gallery`.
///
/// Any authenticated caller may read a public gallery; a private gallery is
/// readable by its owner or an administrator
/// (`gallery-implementation-plan.md` § 4).
pub(crate) fn can_read_gallery(gallery: &Gallery, caller: &User) -> bool {
    gallery.is_public() || gallery.is_owned_by(caller.id()) || caller.role() == Role::Admin
}

/// Return whether `gallery` is the seeded, undeletable system gallery.
#[expect(
    clippy::single_call_fn,
    reason = "the predicate is named after the rule it enforces"
)]
pub(crate) fn is_default_gallery(gallery: &Gallery) -> bool {
    gallery.is_system()
}

/// Load the authenticated caller, or `None` when the account no longer exists.
///
/// # Errors
///
/// Returns the repository error when the user table cannot be read.
pub(crate) async fn load_caller<U>(
    unit_of_work: &mut U,
    caller_id: NumericID,
) -> Result<Option<User>, RepositoryError>
where
    U: UserUnitOfWork,
{
    Ok(unit_of_work
        .users()
        .search(&UserFilter {
            id: Some(caller_id),
            ..UserFilter::default()
        })
        .await?
        .into_iter()
        .next())
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::file_repository::MockFileRepository;
    use crate::domain::port::gallery_repository::MockGalleryRepository;
    use crate::domain::port::media_repository::MockMediaRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::gallery_unit_of_work;

    use super::load_caller;

    #[tokio::test]
    async fn load_caller_returns_matching_user() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        let caller = User::try_new(7, "caller".to_owned(), None, 7, Role::Admin)?;
        users
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
        let (mut unit_of_work, _, _) = gallery_unit_of_work(
            MockGalleryRepository::new(),
            MockMediaRepository::new(),
            users,
            MockFileRepository::new(),
        );

        // Act
        let loaded = load_caller(&mut unit_of_work, 7).await?;

        // Assert
        assert_eq!(loaded.map(|found| found.id()), Some(7));
        Ok(())
    }

    #[tokio::test]
    async fn load_caller_returns_none_for_absent_user() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let (mut unit_of_work, _, _) = gallery_unit_of_work(
            MockGalleryRepository::new(),
            MockMediaRepository::new(),
            users,
            MockFileRepository::new(),
        );

        // Act
        let loaded = load_caller(&mut unit_of_work, 999).await?;

        // Assert
        assert!(loaded.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn load_caller_propagates_repository_failure() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let (mut unit_of_work, _, _) = gallery_unit_of_work(
            MockGalleryRepository::new(),
            MockMediaRepository::new(),
            users,
            MockFileRepository::new(),
        );

        // Act
        let result = load_caller(&mut unit_of_work, 7).await;

        // Assert
        assert!(matches!(result, Err(RepositoryError::OperationFailed)));
        Ok(())
    }
}
