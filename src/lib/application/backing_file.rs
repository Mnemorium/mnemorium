//! Keeps a backing file addressable when the reference that named it is removed.
//!
//! A file's resource hangs from the typed asset that names it, so deleting a
//! gallery item cascades through the medium to the file (`fk_resource_auth_parent`).
//! When the file row survives the reference removal, its resource must survive
//! too, so it moves to an orphan resource owned by the uploader: the owner and
//! the root keep reading it (`REQ-AUTHORIZATION-025-C`), and a later upload that
//! dedups to the row finds an addressable file rather than a bare row.

use crate::domain::model::{File, ResourceKind, ResourceRef, Visibility};
use crate::domain::port::{
    IdentityUnitOfWork, NewResource, RepositoryError, ResourceRepository as _,
};

/// Detach `file`'s resource from the typed asset that is about to disappear.
///
/// The resource keeps its owner and becomes parentless. A row whose resource is
/// already missing — a state an earlier defect could leave — gets one recreated,
/// so the guarantee holds for stored data as well as for new removals.
///
/// # Errors
///
/// Returns the repository error when the resource cannot be moved or recreated.
pub(crate) async fn detach_backing_file<U>(
    unit_of_work: &mut U,
    file: &File,
) -> Result<(), RepositoryError>
where
    U: IdentityUnitOfWork,
{
    let address = ResourceRef::new(ResourceKind::File, file.id());
    if unit_of_work
        .resources()
        .reassign_owner(address, Some(file.user_id()), None)
        .await?
    {
        return Ok(());
    }
    match unit_of_work
        .resources()
        .create(NewResource {
            auth_parent: None,
            entity_id: file.id(),
            kind: ResourceKind::File,
            owner_id: Some(file.user_id()),
            visibility: Visibility::Private,
        })
        .await
    {
        Ok(_) | Err(RepositoryError::AlreadyExist) => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::{NaiveDate, NaiveDateTime};
    use rstest::fixture;
    use std::error::Error;

    use crate::domain::model::{IntegrityHash, Origin, Resource};
    use crate::domain::port::MockResourceRepository;
    use crate::test_helpers::TestUnitOfWork;

    /// A stored file owned by `owner`.
    fn file(owner: i64) -> Result<File, Box<dyn Error>> {
        Ok(File::try_new(
            100,
            "image.png".to_owned(),
            owner,
            false,
            "image/png".to_owned(),
            NaiveDate::default(),
            0,
            NaiveDateTime::default(),
            Some(IntegrityHash::try_new("0".repeat(64))?),
            Some(Origin::Uploaded),
        )?)
    }

    /// The resource the recreated file carries.
    #[fixture]
    fn stored_resource() -> Resource {
        Resource::from_stored(
            7,
            ResourceKind::File,
            100,
            Some(4),
            None,
            Visibility::Private,
        )
    }

    #[tokio::test]
    async fn an_existing_file_resource_is_detached() -> Result<(), Box<dyn Error>> {
        // Arrange: the resource moves without being recreated.
        let mut unit_of_work = TestUnitOfWork::default();
        let mut resources = MockResourceRepository::new();
        resources
            .expect_reassign_owner()
            .times(1)
            .withf(|address, owner, parent| {
                address.kind() == ResourceKind::File && *owner == Some(4) && parent.is_none()
            })
            .returning(|_, _, _| Box::pin(async { Ok(true) }));
        resources.expect_create().times(0);
        unit_of_work.resources = resources;

        // Act
        detach_backing_file(&mut unit_of_work, &file(4)?).await?;

        // Assert: the moved resource is enough, so nothing is recreated.
        Ok(())
    }

    #[tokio::test]
    async fn a_missing_file_resource_is_recreated() -> Result<(), Box<dyn Error>> {
        // Arrange: the row survives without a resource.
        let mut unit_of_work = TestUnitOfWork::default();
        let mut resources = MockResourceRepository::new();
        resources
            .expect_reassign_owner()
            .times(1)
            .returning(|_, _, _| Box::pin(async { Ok(false) }));
        resources
            .expect_create()
            .times(1)
            .withf(|pending| {
                pending.kind == ResourceKind::File
                    && pending.entity_id == 100
                    && pending.owner_id == Some(4)
                    && pending.auth_parent.is_none()
                    && pending.visibility == Visibility::Private
            })
            .returning(|_| Box::pin(async { Ok(stored_resource()) }));
        unit_of_work.resources = resources;

        // Act
        detach_backing_file(&mut unit_of_work, &file(4)?).await?;

        // Assert: the recreated resource is parentless and owned by the uploader.
        Ok(())
    }

    #[tokio::test]
    async fn a_concurrent_recreation_is_tolerated() -> Result<(), Box<dyn Error>> {
        // Arrange: another writer created the resource between the move and the create.
        let mut unit_of_work = TestUnitOfWork::default();
        let mut resources = MockResourceRepository::new();
        resources
            .expect_reassign_owner()
            .times(1)
            .returning(|_, _, _| Box::pin(async { Ok(false) }));
        resources
            .expect_create()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::AlreadyExist) }));
        unit_of_work.resources = resources;

        // Act
        detach_backing_file(&mut unit_of_work, &file(4)?).await?;

        // Assert: the existing resource is the one that survives.
        Ok(())
    }

    #[tokio::test]
    async fn a_storage_failure_is_reported() -> Result<(), Box<dyn Error>> {
        // Arrange: the create fails for another reason.
        let mut unit_of_work = TestUnitOfWork::default();
        let mut resources = MockResourceRepository::new();
        resources
            .expect_reassign_owner()
            .times(1)
            .returning(|_, _, _| Box::pin(async { Ok(false) }));
        resources
            .expect_create()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        unit_of_work.resources = resources;

        // Act
        let result = detach_backing_file(&mut unit_of_work, &file(4)?).await;

        // Assert
        assert!(matches!(result, Err(RepositoryError::OperationFailed)));
        Ok(())
    }
}
