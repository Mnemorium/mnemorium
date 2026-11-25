//! The one cover-art rule every use case that writes a cover calls.
//!
//! A cover names an image through its backing file's resource, and the caller
//! must be able to view that file: referencing another caller's artwork is the
//! same violation whether the reference is stored on a gallery item, a playlist
//! or a catalogue row. The check mirrors the read decision in `get_image`, so a
//! caller may attach exactly the artwork it could render.

use crate::application::authorization::authorize;
use crate::domain::NumericID;
use crate::domain::model::{Action, ResourceKind, ResourceRef, User};
use crate::domain::port::{
    IdentityUnitOfWork, ImageFilter, ImageRepository as _, LibraryUnitOfWork,
};

/// Return whether `cover_image_id`, when set, names an image whose backing file
/// `caller` may view.
///
/// A write that names no cover passes without a lookup, so a metadata update
/// that leaves the artwork alone never depends on the caller's view of it. An
/// unknown image, an unreadable one and a storage failure all answer `false`;
/// the caller maps that to its own cover-conflict variant.
pub(crate) async fn is_viewable<U>(
    unit_of_work: &mut U,
    caller: &User,
    cover_image_id: Option<NumericID>,
) -> bool
where
    U: IdentityUnitOfWork + LibraryUnitOfWork,
{
    let Some(identifier) = cover_image_id else {
        return true;
    };
    let found = match unit_of_work
        .images()
        .search(&ImageFilter {
            image_id: Some(identifier),
            ..ImageFilter::default()
        })
        .await
    {
        Ok(found) => found.into_iter().next(),
        Err(_) => return false,
    };
    let Some(image) = found else {
        return false;
    };
    authorize(
        &mut unit_of_work.resources(),
        caller,
        ResourceRef::new(ResourceKind::File, image.file_id()),
        Action::View,
        None,
    )
    .await
    .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    use rstest::fixture;

    use crate::domain::model::{Orientation, Role};
    use crate::domain::port::{MockImageRepository, MockResourceRepository};
    use crate::test_helpers::TestUnitOfWork;
    use std::error::Error;

    /// A caller that owns the resources the shared mock returns.
    #[fixture]
    fn caller() -> Result<User, Box<dyn Error>> {
        Ok(User::try_new(
            1,
            "alice".to_owned(),
            None,
            1,
            Role::Standard,
        )?)
    }

    /// An image backed by `file_id`.
    fn image(
        id: NumericID,
        file_id: NumericID,
    ) -> Result<crate::domain::model::Image, Box<dyn Error>> {
        Ok(crate::domain::model::Image::try_new(
            id,
            "cover.png".to_owned(),
            10,
            10,
            Orientation::Square,
            chrono::NaiveDate::default(),
            file_id,
            None,
        )?)
    }

    #[tokio::test]
    async fn an_absent_cover_needs_no_lookup() -> Result<(), Box<dyn Error>> {
        // Arrange: a unit of work whose image repository must never be queried.
        let mut unit_of_work = TestUnitOfWork::default();
        let mut images = MockImageRepository::new();
        images.expect_search().times(0);
        unit_of_work.images = images;

        // Act
        let viewable = is_viewable(&mut unit_of_work, &caller()?, None).await;

        // Assert
        assert!(viewable);
        Ok(())
    }

    #[tokio::test]
    async fn a_readable_cover_passes() -> Result<(), Box<dyn Error>> {
        // Arrange: the image exists and the caller owns its backing file.
        let mut unit_of_work = TestUnitOfWork::default();
        let stored = image(19, 7)?;
        let mut images = MockImageRepository::new();
        images
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored]) }));
        unit_of_work.images = images;

        // Act
        let viewable = is_viewable(&mut unit_of_work, &caller()?, Some(19)).await;

        // Assert
        assert!(viewable);
        Ok(())
    }

    #[tokio::test]
    async fn an_unknown_cover_fails() -> Result<(), Box<dyn Error>> {
        // Arrange: the identifier resolves to no image.
        let mut unit_of_work = TestUnitOfWork::default();
        let mut images = MockImageRepository::new();
        images
            .expect_search()
            .times(1)
            .return_once(|_| Box::pin(async { Ok(Vec::new()) }));
        unit_of_work.images = images;

        // Act
        let viewable = is_viewable(&mut unit_of_work, &caller()?, Some(19)).await;

        // Assert
        assert!(!viewable);
        Ok(())
    }

    #[tokio::test]
    async fn an_unreadable_cover_fails() -> Result<(), Box<dyn Error>> {
        // Arrange: the image exists but its backing file is concealed.
        let mut unit_of_work = TestUnitOfWork::default();
        let stored = image(19, 7)?;
        let mut images = MockImageRepository::new();
        images
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored]) }));
        unit_of_work.images = images;
        let mut resources = MockResourceRepository::new();
        resources
            .expect_search()
            .times(1)
            .return_once(|_, _| Box::pin(async { Ok(Vec::new()) }));
        unit_of_work.resources = resources;

        // Act
        let viewable = is_viewable(&mut unit_of_work, &caller()?, Some(19)).await;

        // Assert
        assert!(!viewable);
        Ok(())
    }

    #[tokio::test]
    async fn an_image_lookup_failure_fails() -> Result<(), Box<dyn Error>> {
        // Arrange: the image repository reports a storage failure.
        let mut unit_of_work = TestUnitOfWork::default();
        let mut images = MockImageRepository::new();
        images.expect_search().times(1).return_once(|_| {
            Box::pin(async { Err(crate::domain::port::RepositoryError::OperationFailed) })
        });
        unit_of_work.images = images;

        // Act
        let viewable = is_viewable(&mut unit_of_work, &caller()?, Some(19)).await;

        // Assert
        assert!(!viewable);
        Ok(())
    }
}
