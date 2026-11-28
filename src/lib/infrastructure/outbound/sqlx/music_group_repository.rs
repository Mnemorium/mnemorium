use sqlx::{QueryBuilder, Sqlite, Transaction};

use crate::domain::model::MusicGroup;
use crate::domain::port::{MusicGroupFilter, MusicGroupRepository, RepositoryError};

use super::error_mapping::stored_data_invalid;
use super::model::MusicGroupRow;

use super::query_filter::push_filter;

/// Repository persisting music groups, backed by a `SQLite` transaction.
pub(crate) struct SqlxMusicGroupRepository<'transaction> {
    /// Transaction the repository reads from and writes to.
    transaction: &'transaction mut Transaction<'static, Sqlite>,
}

impl<'transaction> SqlxMusicGroupRepository<'transaction> {
    /// Create a new repository bound to `transaction`.
    #[must_use]
    pub(crate) fn new(transaction: &'transaction mut Transaction<'static, Sqlite>) -> Self {
        Self { transaction }
    }
}

impl MusicGroupRepository for SqlxMusicGroupRepository<'_> {
    async fn create(&mut self, group: MusicGroup) -> Result<MusicGroup, RepositoryError> {
        let row = sqlx::query_as::<_, MusicGroupRow>(
            "INSERT INTO music_group (stage_name, cover_image_id)
             VALUES (?1, ?2)
             RETURNING music_group_id, stage_name, cover_image_id",
        )
        .bind(group.stage_name())
        .bind(group.cover_image_id())
        .fetch_one(&mut **self.transaction)
        .await?;

        domain_group(row)
    }
    async fn save(&mut self, group: MusicGroup) -> Result<MusicGroup, RepositoryError> {
        let row = sqlx::query_as::<_, MusicGroupRow>(
            "INSERT INTO music_group (
                music_group_id, stage_name, cover_image_id
             )
             VALUES (?1, ?2, ?3)
             ON CONFLICT (music_group_id) DO UPDATE SET
                stage_name = excluded.stage_name,
                cover_image_id = excluded.cover_image_id
             RETURNING music_group_id, stage_name, cover_image_id",
        )
        .bind(group.id())
        .bind(group.stage_name())
        .bind(group.cover_image_id())
        .fetch_one(&mut **self.transaction)
        .await?;

        domain_group(row)
    }
    async fn search(
        &mut self,
        filter: &MusicGroupFilter,
    ) -> Result<Vec<MusicGroup>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT music_group_id, stage_name, cover_image_id FROM music_group",
        );
        let mut first = true;

        if let Some(id) = filter.id {
            push_filter(&mut first, &mut builder, "music_group_id = ", id);
        }
        if let Some(name) = filter.stage_name.as_deref() {
            push_filter(&mut first, &mut builder, "stage_name = ", name);
        }

        // A stable group order makes the caller's offset/limit pagination
        // deterministic (`ListMusicGroups` pages in memory).
        builder.push(" ORDER BY music_group_id");

        let rows = builder
            .build_query_as::<MusicGroupRow>()
            .fetch_all(&mut **self.transaction)
            .await?;

        rows.into_iter().map(domain_group).collect()
    }
}

/// Assemble the domain group held by `row`.
fn domain_group(row: MusicGroupRow) -> Result<MusicGroup, RepositoryError> {
    MusicGroup::try_new(row.music_group_id, row.stage_name, row.cover_image_id)
        .map_err(|_| stored_data_invalid("invalid_music_group"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::NumericID;
    use crate::infrastructure::outbound::sqlx::test_helpers::*;

    use crate::infrastructure::outbound::sqlx::test_helpers::begin_transaction;
    use std::error::Error;

    /// Build a group named `name` with an optional cover.
    fn group(
        id: NumericID,
        name: &str,
        cover_image_id: Option<NumericID>,
    ) -> Result<MusicGroup, Box<dyn Error>> {
        Ok(MusicGroup::try_new(id, name.to_owned(), cover_image_id)?)
    }

    #[tokio::test]
    async fn create_round_trips_name_and_cover() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user_v3(&mut transaction, 1).await?;
        seed_image_v3(&mut transaction, 1, 1, None).await?;
        let mut repository = SqlxMusicGroupRepository::new(&mut transaction);

        // Act
        let plain = repository.create(group(0, "The Group", None)?).await?;
        let covered = repository.create(group(0, "The Covered", Some(1))?).await?;
        let read = repository
            .search(&MusicGroupFilter {
                id: Some(covered.id()),
                ..MusicGroupFilter::default()
            })
            .await?
            .into_iter()
            .next();

        // Assert: the insert assigns identifiers and stores name and cover.
        assert_ne!(plain.id(), 0, "the insert must assign an identifier");
        assert_eq!(plain.stage_name(), "The Group");
        assert_eq!(plain.cover_image_id(), None);
        assert_eq!(read, Some(covered));
        assert_eq!(
            read.as_ref().and_then(MusicGroup::cover_image_id),
            Some(1),
            "the cover survives the round trip"
        );
        Ok(())
    }

    #[tokio::test]
    async fn create_duplicate_name_returns_already_exist() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxMusicGroupRepository::new(&mut transaction);
        repository.create(group(0, "The Group", None)?).await?;

        // Act
        let result = repository.create(group(0, "The Group", None)?).await;

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::AlreadyExist)),
            "a duplicate stage name must be refused"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_unknown_group_returns_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxMusicGroupRepository::new(&mut transaction);

        // Act
        let found = repository
            .search(&MusicGroupFilter {
                id: Some(999_999_999),
                ..MusicGroupFilter::default()
            })
            .await?
            .into_iter()
            .next();

        // Assert
        assert_eq!(found, None, "a missing group is a value, not an error");
        Ok(())
    }

    #[tokio::test]
    async fn get_the_default_group_returns_it() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxMusicGroupRepository::new(&mut transaction);

        // Act
        let found = repository
            .search(&MusicGroupFilter {
                id: Some(0),
                ..MusicGroupFilter::default()
            })
            .await?
            .into_iter()
            .next();

        // Assert
        assert_eq!(
            found.map(|group| group.stage_name().to_owned()),
            Some("Unknown".to_owned()),
            "the seeded default group is readable like any other"
        );
        Ok(())
    }

    #[tokio::test]
    async fn a_created_group_can_be_an_albums_group() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxMusicGroupRepository::new(&mut transaction);
        let created = repository.create(group(0, "The Group", None)?).await?;

        // Act: an album names the created group.
        sqlx::query(
            "INSERT INTO music_album (music_album_id, name, release_date, music_group_id)
             VALUES (1, 'The Album', '1990-04-08', ?1)",
        )
        .bind(created.id())
        .execute(&mut *transaction)
        .await?;

        // Assert
        let stored: i64 =
            sqlx::query_scalar("SELECT music_group_id FROM music_album WHERE music_album_id = 1")
                .fetch_one(&mut *transaction)
                .await?;
        assert_eq!(
            stored,
            created.id(),
            "an album can name a created group as its group"
        );
        Ok(())
    }

    #[tokio::test]
    async fn search_filters_by_name() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxMusicGroupRepository::new(&mut transaction);
        repository.create(group(0, "The Group", None)?).await?;
        repository.create(group(0, "Another Group", None)?).await?;

        // Act
        let matches = repository
            .search(&MusicGroupFilter {
                stage_name: Some("The Group".to_owned()),
                ..MusicGroupFilter::default()
            })
            .await?;

        // Assert
        assert_eq!(matches.len(), 1);
        assert_eq!(
            matches.first().map(MusicGroup::stage_name),
            Some("The Group")
        );
        Ok(())
    }

    #[tokio::test]
    async fn update_round_trips_name_and_cover() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user_v3(&mut transaction, 1).await?;
        seed_image_v3(&mut transaction, 1, 1, None).await?;
        let mut repository = SqlxMusicGroupRepository::new(&mut transaction);
        let created = repository.create(group(0, "The Group", None)?).await?;

        // Act
        let renamed = repository
            .save(group(created.id(), "The Renamed", Some(1))?)
            .await?;

        // Assert
        assert_eq!(renamed.stage_name(), "The Renamed");
        assert_eq!(renamed.cover_image_id(), Some(1));
        assert_eq!(
            repository
                .search(&MusicGroupFilter {
                    id: Some(created.id()),
                    ..MusicGroupFilter::default()
                })
                .await?,
            vec![renamed]
        );
        Ok(())
    }

    #[tokio::test]
    async fn save_unknown_group_inserts_it() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxMusicGroupRepository::new(&mut transaction);

        // Act
        let saved = repository
            .save(group(999_999_999, "The Group", None)?)
            .await?;

        // Assert: save upserts, so an unknown identifier is inserted.
        assert_eq!(saved.id(), 999_999_999);
        assert_eq!(saved.stage_name(), "The Group");
        Ok(())
    }

    #[tokio::test]
    async fn save_of_the_default_group_returns_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxMusicGroupRepository::new(&mut transaction);

        // Act: the seeded group 0 is protected by its update trigger.
        let result = repository.save(group(0, "Renamed", None)?).await;

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::Conflict)),
            "the default group cannot be renamed"
        );
        Ok(())
    }

    #[tokio::test]
    async fn create_rejects_a_user_cover_image() -> Result<(), Box<dyn Error>> {
        // Arrange: image 1 already belongs to a user.
        let mut transaction = begin_transaction().await?;
        seed_user_v3(&mut transaction, 1).await?;
        seed_image_v3(&mut transaction, 1, 1, Some("USER")).await?;
        let mut repository = SqlxMusicGroupRepository::new(&mut transaction);

        // Act
        let result = repository.create(group(0, "The Group", Some(1))?).await;

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::Conflict)),
            "a library cover must reject a USER image"
        );
        let stored: i64 =
            sqlx::query_scalar("SELECT count(*) FROM music_group WHERE stage_name = 'The Group'")
                .fetch_one(&mut *transaction)
                .await?;
        assert_eq!(stored, 0, "a rejected cover must store nothing");
        Ok(())
    }

    #[tokio::test]
    async fn update_rejects_a_user_cover_image() -> Result<(), Box<dyn Error>> {
        // Arrange: a group without artwork and image 1 owned by a user.
        let mut transaction = begin_transaction().await?;
        seed_user_v3(&mut transaction, 1).await?;
        seed_image_v3(&mut transaction, 1, 1, Some("USER")).await?;
        let mut repository = SqlxMusicGroupRepository::new(&mut transaction);
        let created = repository.create(group(0, "The Group", None)?).await?;

        // Act
        let result = repository
            .save(group(created.id(), "The Group", Some(1))?)
            .await;

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::Conflict)),
            "a library cover must reject a USER image on update too"
        );
        Ok(())
    }

    #[tokio::test]
    async fn music_group_accepts_a_cover_image() -> Result<(), Box<dyn Error>> {
        // Arrange: image 1 starts unattached.
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;

        // Act
        sqlx::query(
            "INSERT INTO music_group (music_group_id, stage_name, cover_image_id)
             VALUES (1, 'The Group', 1)",
        )
        .execute(&pool)
        .await?;

        // Assert
        let cover: Option<i64> =
            sqlx::query_scalar("SELECT cover_image_id FROM music_group WHERE music_group_id = 1")
                .fetch_one(&pool)
                .await?;
        let scope: Option<String> =
            sqlx::query_scalar("SELECT scope FROM image WHERE image_id = 1")
                .fetch_one(&pool)
                .await?;
        assert_eq!(cover, Some(1));
        assert_eq!(
            scope.as_deref(),
            Some("LIBRARY"),
            "a group cover must mark its image LIBRARY"
        );
        Ok(())
    }

    #[tokio::test]
    async fn music_group_rejects_a_missing_cover_image() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;

        // Act: image 99 does not exist.
        let result = sqlx::query(
            "INSERT INTO music_group (music_group_id, stage_name, cover_image_id)
             VALUES (1, 'The Group', 99)",
        )
        .execute(&pool)
        .await
        .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }
}
