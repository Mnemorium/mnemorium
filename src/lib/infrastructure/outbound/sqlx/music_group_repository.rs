use sqlx::{QueryBuilder, Sqlite, Transaction};

use crate::domain::alias::NumericID;
use crate::domain::model::music_group::MusicGroup;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::music_group_repository::{MusicGroupFilter, MusicGroupRepository};

use super::error_mapping::stored_data_invalid;
use super::model::music_group::MusicGroupRow;
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

    async fn get(&mut self, id: NumericID) -> Result<Option<MusicGroup>, RepositoryError> {
        let row = sqlx::query_as::<_, MusicGroupRow>(
            "SELECT music_group_id, stage_name, cover_image_id
             FROM music_group
             WHERE music_group_id = ?",
        )
        .bind(id)
        .fetch_optional(&mut **self.transaction)
        .await?;

        row.map(domain_group).transpose()
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

    async fn update(&mut self, group: MusicGroup) -> Result<Option<MusicGroup>, RepositoryError> {
        let row = sqlx::query_as::<_, MusicGroupRow>(
            "UPDATE music_group
             SET stage_name = ?1, cover_image_id = ?2
             WHERE music_group_id = ?3
             RETURNING music_group_id, stage_name, cover_image_id",
        )
        .bind(group.stage_name())
        .bind(group.cover_image_id())
        .bind(group.id())
        .fetch_optional(&mut **self.transaction)
        .await?;

        row.map(domain_group).transpose()
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
    use sqlx::sqlite::SqlitePoolOptions;
    use std::error::Error;

    /// Open a migrated in-memory database and begin one transaction.
    async fn begin_transaction() -> Result<Transaction<'static, Sqlite>, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        pool.begin().await
    }

    /// Seed a credential and a standard user so the image's file resolves.
    async fn seed_user(
        transaction: &mut Transaction<'static, Sqlite>,
        user_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO credential (credential_id, password_hash) VALUES (?1, ?2)")
            .bind(user_id)
            .bind(format!("hash-{user_id}"))
            .execute(&mut **transaction)
            .await?;
        sqlx::query(
            "INSERT INTO user (user_id, role, username, email, credential_id)
             VALUES (?1, 'STANDARD', ?2, NULL, ?1)",
        )
        .bind(user_id)
        .bind(format!("user{user_id}"))
        .execute(&mut **transaction)
        .await?;
        sqlx::query(
            "INSERT INTO file (file_id, path, user_id, is_public, mime_type_id, size_bytes, modified_at, origin, integrity_hash)
             VALUES (?1, ?2, ?3, 0, 'image/png', 1024, '2026-10-06 00:00:00', 'UPLOADED', ?4)",
        )
        .bind(user_id)
        .bind(format!("cover-{user_id}.png"))
        .bind(user_id)
        .bind(format!("{user_id:064x}"))
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Seed an image, unattached or already scoped.
    async fn seed_image(
        transaction: &mut Transaction<'static, Sqlite>,
        image_id: i64,
        file_id: i64,
        scope: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO image (
                image_id, name, width_px, height_px, orientation, created_at, scope, file_id
             )
             VALUES (?1, ?2, 10, 10, 'SQUARE', '2026-10-06', ?3, ?4)",
        )
        .bind(image_id)
        .bind(format!("cover-{image_id}.png"))
        .bind(scope)
        .bind(file_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

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
        seed_user(&mut transaction, 1).await?;
        seed_image(&mut transaction, 1, 1, None).await?;
        let mut repository = SqlxMusicGroupRepository::new(&mut transaction);

        // Act
        let plain = repository.create(group(0, "The Group", None)?).await?;
        let covered = repository.create(group(0, "The Covered", Some(1))?).await?;
        let read = repository.get(covered.id()).await?;

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
        let found = repository.get(999_999_999).await?;

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
        let found = repository.get(0).await?;

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
        seed_user(&mut transaction, 1).await?;
        seed_image(&mut transaction, 1, 1, None).await?;
        let mut repository = SqlxMusicGroupRepository::new(&mut transaction);
        let created = repository.create(group(0, "The Group", None)?).await?;

        // Act
        let renamed = repository
            .update(group(created.id(), "The Renamed", Some(1))?)
            .await?;

        // Assert
        assert_eq!(
            renamed.as_ref().map(MusicGroup::stage_name),
            Some("The Renamed")
        );
        assert_eq!(
            renamed.as_ref().and_then(MusicGroup::cover_image_id),
            Some(1)
        );
        assert_eq!(repository.get(created.id()).await?, renamed);
        Ok(())
    }

    #[tokio::test]
    async fn update_unknown_group_returns_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxMusicGroupRepository::new(&mut transaction);

        // Act
        let updated = repository
            .update(group(999_999_999, "The Group", None)?)
            .await?;

        // Assert
        assert_eq!(updated, None, "a missing group is a value, not an error");
        Ok(())
    }

    #[tokio::test]
    async fn update_of_the_unknown_group_returns_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxMusicGroupRepository::new(&mut transaction);

        // Act: the seeded group 0 is protected by its update trigger.
        let result = repository.update(group(0, "Renamed", None)?).await;

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
        seed_user(&mut transaction, 1).await?;
        seed_image(&mut transaction, 1, 1, Some("USER")).await?;
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
        seed_user(&mut transaction, 1).await?;
        seed_image(&mut transaction, 1, 1, Some("USER")).await?;
        let mut repository = SqlxMusicGroupRepository::new(&mut transaction);
        let created = repository.create(group(0, "The Group", None)?).await?;

        // Act
        let result = repository
            .update(group(created.id(), "The Group", Some(1))?)
            .await;

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::Conflict)),
            "a library cover must reject a USER image on update too"
        );
        Ok(())
    }
}
