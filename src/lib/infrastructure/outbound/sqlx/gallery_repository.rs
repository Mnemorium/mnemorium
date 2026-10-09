use chrono::NaiveDateTime;
use sqlx::QueryBuilder;
use sqlx::Sqlite;
use sqlx::Transaction;

use crate::domain::alias::NumericID;
use crate::domain::model::gallery::Gallery;
use crate::domain::model::gallery_item::GalleryItem;
use crate::domain::model::gallery_item::GalleryItemMedia;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::gallery_repository::GalleryFilter;
use crate::domain::port::gallery_repository::GalleryItemDetail;
use crate::domain::port::gallery_repository::GalleryItemFilter;
use crate::domain::port::gallery_repository::GalleryRepository;

use super::model::gallery::Gallery as SqlxGallery;
use super::model::gallery_item::GalleryItem as SqlxGalleryItem;
use super::model::gallery_item::GalleryItemDetail as SqlxGalleryItemDetail;

/// Repository persisting galleries and their items, backed by a `SQLite`
/// transaction.
pub struct SqlxGalleryRepository<'transaction> {
    /// Transaction the repository reads from and writes to.
    transaction: &'transaction mut Transaction<'static, Sqlite>,
}

impl<'transaction> SqlxGalleryRepository<'transaction> {
    /// Create a new repository bound to `transaction`.
    #[must_use]
    pub fn new(transaction: &'transaction mut Transaction<'static, Sqlite>) -> Self {
        Self { transaction }
    }
}

impl GalleryRepository for SqlxGalleryRepository<'_> {
    async fn add_item(&mut self, item: GalleryItem) -> Result<GalleryItem, RepositoryError> {
        let (image_id, video_id) = match item.media() {
            GalleryItemMedia::Image(image) => (Some(image), None),
            GalleryItemMedia::Video(video) => (None, Some(video)),
        };
        let row = sqlx::query_as::<_, SqlxGalleryItem>(
            "INSERT INTO gallery_item (gallery_id, image_id, video_id, item_index, added_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            RETURNING
                gallery_item_id,
                gallery_id,
                image_id,
                video_id,
                item_index,
                added_at",
        )
        .bind(item.gallery_id())
        .bind(image_id)
        .bind(video_id)
        .bind(item.item_index())
        .bind(item.added_at())
        .fetch_one(&mut **self.transaction)
        .await?;

        domain_item(
            row.gallery_item_id,
            row.gallery_id,
            row.image_id,
            row.video_id,
            row.item_index,
            row.added_at,
        )
    }

    async fn create(&mut self, gallery: Gallery) -> Result<Gallery, RepositoryError> {
        let row = sqlx::query_as::<_, SqlxGallery>(
            "INSERT INTO gallery (name, created_at, last_modified_at, is_public, user_id)
            VALUES (?1, ?2, ?3, ?4, ?5)
            RETURNING
                gallery_id,
                name,
                created_at,
                last_modified_at,
                is_public,
                user_id",
        )
        .bind(gallery.name())
        .bind(gallery.created_at())
        .bind(gallery.last_modified_at())
        .bind(gallery.is_public())
        .bind(gallery.user_id())
        .fetch_one(&mut **self.transaction)
        .await?;

        domain_gallery(row)
    }

    async fn delete(&mut self, id: NumericID) -> Result<bool, RepositoryError> {
        let result = sqlx::query("DELETE FROM gallery WHERE gallery_id = ?")
            .bind(id)
            .execute(&mut **self.transaction)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn delete_item(&mut self, id: NumericID) -> Result<bool, RepositoryError> {
        let result = sqlx::query("DELETE FROM gallery_item WHERE gallery_item_id = ?")
            .bind(id)
            .execute(&mut **self.transaction)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn next_item_index(&mut self, gallery_id: NumericID) -> Result<i64, RepositoryError> {
        let next = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(MAX(item_index) + 1, 0)
            FROM gallery_item
            WHERE gallery_id = ?",
        )
        .bind(gallery_id)
        .fetch_one(&mut **self.transaction)
        .await?;

        Ok(next)
    }

    async fn save(&mut self, gallery: Gallery) -> Result<Gallery, RepositoryError> {
        let row = sqlx::query_as::<_, SqlxGallery>(
            "INSERT INTO gallery (gallery_id, name, created_at, last_modified_at, is_public, user_id)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT (gallery_id) DO UPDATE SET
                name = excluded.name,
                created_at = excluded.created_at,
                last_modified_at = excluded.last_modified_at,
                is_public = excluded.is_public,
                user_id = excluded.user_id
            RETURNING
                gallery_id,
                name,
                created_at,
                last_modified_at,
                is_public,
                user_id",
        )
        .bind(gallery.gallery_id())
        .bind(gallery.name())
        .bind(gallery.created_at())
        .bind(gallery.last_modified_at())
        .bind(gallery.is_public())
        .bind(gallery.user_id())
        .fetch_one(&mut **self.transaction)
        .await?;

        domain_gallery(row)
    }

    async fn search(&mut self, filter: &GalleryFilter) -> Result<Vec<Gallery>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT
                gallery_id,
                name,
                created_at,
                last_modified_at,
                is_public,
                user_id
            FROM gallery",
        );
        let mut first = true;

        if let Some(id) = filter.id {
            push_filter(&mut first, &mut builder, "gallery_id = ", id);
        }
        if let Some(is_public) = filter.is_public {
            push_filter(&mut first, &mut builder, "is_public = ", is_public);
        }
        if let Some(name) = filter.name.as_deref() {
            push_filter(&mut first, &mut builder, "name = ", name);
        }
        if let Some(user_id) = filter.user_id {
            push_filter(&mut first, &mut builder, "user_id = ", user_id);
        }

        // A stable order makes the caller's offset/limit pagination deterministic
        // (`ListGalleries` pages the returned collection in memory).
        builder.push(" ORDER BY gallery_id");

        let rows = builder
            .build_query_as::<SqlxGallery>()
            .fetch_all(&mut **self.transaction)
            .await?;

        rows.into_iter().map(domain_gallery).collect()
    }

    async fn search_items(
        &mut self,
        filter: &GalleryItemFilter,
    ) -> Result<Vec<GalleryItemDetail>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT
                gallery_item.gallery_item_id,
                gallery_item.gallery_id,
                gallery_item.image_id,
                gallery_item.video_id,
                gallery_item.item_index,
                gallery_item.added_at,
                COALESCE(image.file_id, video.file_id) AS file_id,
                COALESCE(image.name, file.path) AS name
            FROM gallery_item
            LEFT JOIN image ON image.image_id = gallery_item.image_id
            LEFT JOIN video ON video.video_id = gallery_item.video_id
            LEFT JOIN file ON file.file_id = COALESCE(image.file_id, video.file_id)",
        );
        let mut first = true;

        if let Some(gallery_id) = filter.gallery_id {
            push_filter(
                &mut first,
                &mut builder,
                "gallery_item.gallery_id = ",
                gallery_id,
            );
        }
        if let Some(id) = filter.id {
            push_filter(
                &mut first,
                &mut builder,
                "gallery_item.gallery_item_id = ",
                id,
            );
        }
        if let Some(media) = filter.media {
            match media {
                GalleryItemMedia::Image(image) => {
                    push_filter(&mut first, &mut builder, "gallery_item.image_id = ", image);
                }
                GalleryItemMedia::Video(video) => {
                    push_filter(&mut first, &mut builder, "gallery_item.video_id = ", video);
                }
            }
        }

        builder.push(" ORDER BY gallery_item.item_index");

        let rows = builder
            .build_query_as::<SqlxGalleryItemDetail>()
            .fetch_all(&mut **self.transaction)
            .await?;

        rows.into_iter()
            .map(|row| {
                let item = domain_item(
                    row.gallery_item_id,
                    row.gallery_id,
                    row.image_id,
                    row.video_id,
                    row.item_index,
                    row.added_at,
                )?;
                Ok(GalleryItemDetail::new(item, row.file_id, row.name))
            })
            .collect()
    }
}

/// Append a `WHERE`/`AND`-separated query filter condition to `builder`.
fn push_filter<'value, T>(
    first: &mut bool,
    builder: &mut QueryBuilder<Sqlite>,
    clause: &'static str,
    value: T,
) where
    T: sqlx::Encode<'value, Sqlite> + sqlx::Type<Sqlite>,
{
    if *first {
        builder.push(" WHERE ");
        *first = false;
    } else {
        builder.push(" AND ");
    }
    builder.push(clause);
    builder.push_bind(value);
}

/// Map a persisted gallery row back to the domain model.
fn domain_gallery(row: SqlxGallery) -> Result<Gallery, RepositoryError> {
    Gallery::try_new(
        row.gallery_id,
        row.user_id,
        row.name,
        row.is_public,
        row.created_at,
        row.last_modified_at,
    )
    .map_err(|_| RepositoryError::DataIntegrityViolation)
}

/// Map persisted gallery-item columns back to the domain model.
fn domain_item(
    gallery_item_id: NumericID,
    gallery_id: NumericID,
    image_id: Option<NumericID>,
    video_id: Option<NumericID>,
    item_index: i64,
    added_at: NaiveDateTime,
) -> Result<GalleryItem, RepositoryError> {
    GalleryItem::try_new(
        gallery_item_id,
        gallery_id,
        image_id,
        video_id,
        item_index,
        added_at,
    )
    .map_err(|_| RepositoryError::DataIntegrityViolation)
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use chrono::NaiveDateTime;
    use sqlx::Sqlite;
    use sqlx::Transaction;
    use sqlx::sqlite::SqlitePoolOptions;

    use crate::domain::model::gallery::Gallery;
    use crate::domain::model::gallery_item::GalleryItem;
    use crate::domain::model::gallery_item::GalleryItemMedia;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::gallery_repository::GalleryFilter;
    use crate::domain::port::gallery_repository::GalleryItemFilter;
    use crate::domain::port::gallery_repository::GalleryRepository as _;

    use super::SqlxGalleryRepository;

    /// Open a migrated in-memory database and begin one transaction.
    async fn begin_transaction() -> Result<Transaction<'static, Sqlite>, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        pool.begin().await
    }

    /// Seed a credential and a standard user so foreign keys resolve.
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
        Ok(())
    }

    /// Seed a file owned by `user_id` with a per-file unique digest.
    async fn seed_file(
        transaction: &mut Transaction<'static, Sqlite>,
        file_id: i64,
        user_id: i64,
        path: &str,
        mime_type_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO file (file_id, path, user_id, is_public, mime_type_id, integrity_hash)
             VALUES (?1, ?2, ?3, 0, ?4, ?5)",
        )
        .bind(file_id)
        .bind(path)
        .bind(user_id)
        .bind(mime_type_id)
        .bind(format!("{file_id:064x}"))
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Seed an image backed by `file_id`.
    async fn seed_image(
        transaction: &mut Transaction<'static, Sqlite>,
        image_id: i64,
        file_id: i64,
        name: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO image (image_id, name, width_px, height_px, orientation, created_at, file_id)
             VALUES (?1, ?2, 10, 10, 'SQUARE', '2026-10-06', ?3)",
        )
        .bind(image_id)
        .bind(name)
        .bind(file_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Seed a video backed by `file_id`.
    async fn seed_video(
        transaction: &mut Transaction<'static, Sqlite>,
        video_id: i64,
        file_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO video (
                video_id, duration_ms, codec, frame_count, width, height, color_id, scan_type, file_id
             )
             VALUES (?1, 1000.0, 'H264', 30, 10, 10, 'YCbCr', 'PROGRESSIVE', ?2)",
        )
        .bind(video_id)
        .bind(file_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Seed a private gallery directly, to reach constraint violations the
    /// repository API cannot produce.
    async fn seed_gallery(
        transaction: &mut Transaction<'static, Sqlite>,
        gallery_id: i64,
        user_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO gallery (gallery_id, name, created_at, last_modified_at, is_public, user_id)
             VALUES (?1, ?2, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 0, ?3)",
        )
        .bind(gallery_id)
        .bind(format!("gallery {gallery_id}"))
        .bind(user_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Build a domain gallery.
    fn gallery(
        gallery_id: i64,
        user_id: Option<i64>,
        name: &str,
        is_public: bool,
    ) -> Result<Gallery, Box<dyn Error>> {
        let now = NaiveDateTime::default();
        Ok(Gallery::try_new(
            gallery_id,
            user_id,
            name.to_owned(),
            is_public,
            now,
            now,
        )?)
    }

    /// Build a domain gallery item referencing `media`.
    fn item(
        gallery_item_id: i64,
        gallery_id: i64,
        media: GalleryItemMedia,
        item_index: i64,
    ) -> Result<GalleryItem, Box<dyn Error>> {
        let (image_id, video_id) = match media {
            GalleryItemMedia::Image(id) => (Some(id), None),
            GalleryItemMedia::Video(id) => (None, Some(id)),
        };
        Ok(GalleryItem::try_new(
            gallery_item_id,
            gallery_id,
            image_id,
            video_id,
            item_index,
            NaiveDateTime::default(),
        )?)
    }

    #[tokio::test]
    async fn create_assigns_final_identifier_and_round_trips() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let pending = gallery(0, Some(1), "Holidays", false)?;

        // Act
        let mut repository = SqlxGalleryRepository::new(&mut transaction);
        let persisted = repository.create(pending).await?;
        let found = repository
            .search(&GalleryFilter {
                id: Some(persisted.gallery_id()),
                ..GalleryFilter::default()
            })
            .await?;

        // Assert
        assert_ne!(
            persisted.gallery_id(),
            0,
            "a new gallery must receive a real identifier"
        );
        assert_eq!(persisted.name(), "Holidays");
        assert_eq!(persisted.user_id(), Some(1));
        assert!(!persisted.is_public());
        assert_eq!(found.first(), Some(&persisted));
        Ok(())
    }

    #[tokio::test]
    async fn create_rejects_duplicate_name_for_the_same_owner() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxGalleryRepository::new(&mut transaction);
        repository
            .create(gallery(0, Some(1), "Holidays", false)?)
            .await?;

        // Act
        let duplicate = repository
            .create(gallery(0, Some(1), "Holidays", true)?)
            .await;

        // Assert: uq_gallery_user_id_name rejects a second name for the owner.
        assert!(matches!(duplicate, Err(RepositoryError::AlreadyExist)));
        Ok(())
    }

    #[tokio::test]
    async fn create_allows_the_same_name_for_different_owners() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_user(&mut transaction, 2).await?;
        let mut repository = SqlxGalleryRepository::new(&mut transaction);
        let first = repository
            .create(gallery(0, Some(1), "Holidays", false)?)
            .await?;

        // Act
        let second = repository
            .create(gallery(0, Some(2), "Holidays", false)?)
            .await?;

        // Assert
        assert_ne!(first.gallery_id(), second.gallery_id());
        assert_eq!(second.name(), "Holidays");
        Ok(())
    }

    #[tokio::test]
    async fn save_persists_last_modified_at() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxGalleryRepository::new(&mut transaction);
        let mut gallery = repository
            .create(gallery(0, Some(1), "Holidays", false)?)
            .await?;
        let touched = NaiveDateTime::parse_from_str("2030-01-02 03:04:05", "%Y-%m-%d %H:%M:%S")?;

        // Act
        gallery.touch(touched);
        let saved = repository.save(gallery).await?;
        let found = repository
            .search(&GalleryFilter {
                id: Some(saved.gallery_id()),
                ..GalleryFilter::default()
            })
            .await?;

        // Assert
        assert_eq!(saved.last_modified_at(), touched);
        assert_eq!(found.first().map(Gallery::last_modified_at), Some(touched));
        Ok(())
    }

    #[tokio::test]
    async fn save_touches_default_gallery_without_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxGalleryRepository::new(&mut transaction);
        let mut default = repository
            .search(&GalleryFilter {
                id: Some(0),
                ..GalleryFilter::default()
            })
            .await?
            .into_iter()
            .next()
            .ok_or("the default gallery must be seeded")?;
        let touched = NaiveDateTime::parse_from_str("2031-05-06 07:08:09", "%Y-%m-%d %H:%M:%S")?;

        // Act
        default.touch(touched);
        let saved = repository.save(default).await?;

        // Assert
        assert_eq!(saved.gallery_id(), 0);
        assert_eq!(saved.last_modified_at(), touched);
        Ok(())
    }

    #[tokio::test]
    async fn save_renaming_default_gallery_returns_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxGalleryRepository::new(&mut transaction);
        let mut default = repository
            .search(&GalleryFilter {
                id: Some(0),
                ..GalleryFilter::default()
            })
            .await?
            .into_iter()
            .next()
            .ok_or("the default gallery must be seeded")?;

        // Act
        default.set_name("Renamed".to_owned())?;
        let result = repository.save(default).await;

        // Assert
        assert!(matches!(result, Err(RepositoryError::Conflict)));
        Ok(())
    }

    #[tokio::test]
    async fn delete_reports_matched_and_missing_galleries() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxGalleryRepository::new(&mut transaction);
        let created = repository
            .create(gallery(0, Some(1), "Holidays", false)?)
            .await?;

        // Act
        let deleted = repository.delete(created.gallery_id()).await?;
        let missing = repository.delete(created.gallery_id()).await?;

        // Assert
        assert!(deleted, "an existing gallery must be deleted");
        assert!(
            !missing,
            "a missing gallery must report false, not an error"
        );
        Ok(())
    }

    #[tokio::test]
    async fn delete_default_gallery_returns_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxGalleryRepository::new(&mut transaction);

        // Act
        let result = repository.delete(0).await;

        // Assert
        assert!(matches!(result, Err(RepositoryError::Conflict)));
        Ok(())
    }

    #[tokio::test]
    async fn delete_cascades_gallery_items() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 1, 1, "a.png", "image/png").await?;
        seed_image(&mut transaction, 1, 1, "a.png").await?;
        let mut repository = SqlxGalleryRepository::new(&mut transaction);
        let created = repository
            .create(gallery(0, Some(1), "Holidays", false)?)
            .await?;
        repository
            .add_item(item(
                0,
                created.gallery_id(),
                GalleryItemMedia::Image(1),
                0,
            )?)
            .await?;

        // Act
        let deleted = repository.delete(created.gallery_id()).await?;
        let items = repository
            .search_items(&GalleryItemFilter {
                gallery_id: Some(created.gallery_id()),
                ..GalleryItemFilter::default()
            })
            .await?;

        // Assert
        assert!(deleted);
        assert!(
            items.is_empty(),
            "deleting a gallery must cascade its items"
        );
        Ok(())
    }

    #[tokio::test]
    async fn search_filters_by_name_public_and_owner() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_user(&mut transaction, 2).await?;
        let mut repository = SqlxGalleryRepository::new(&mut transaction);
        repository
            .create(gallery(0, Some(1), "Trip", true)?)
            .await?;
        // A second owner may reuse the name: uniqueness is per owner.
        repository
            .create(gallery(0, Some(2), "Trip", false)?)
            .await?;
        repository
            .create(gallery(0, Some(1), "Other", true)?)
            .await?;

        // Act
        let by_name = repository
            .search(&GalleryFilter {
                name: Some("Trip".to_owned()),
                ..GalleryFilter::default()
            })
            .await?;
        let by_public = repository
            .search(&GalleryFilter {
                is_public: Some(true),
                ..GalleryFilter::default()
            })
            .await?;
        let by_owner = repository
            .search(&GalleryFilter {
                user_id: Some(2),
                ..GalleryFilter::default()
            })
            .await?;
        let all = repository.search(&GalleryFilter::default()).await?;

        // Assert
        assert_eq!(by_name.len(), 2);
        assert_eq!(by_name.first().map(Gallery::name), Some("Trip"));
        // The default gallery is public, plus the two explicitly public ones.
        assert_eq!(by_public.len(), 3);
        assert!(by_public.iter().all(Gallery::is_public));
        assert_eq!(by_owner.len(), 1);
        assert_eq!(by_owner.first().and_then(Gallery::user_id), Some(2));
        // The default gallery is public, ownerless and seeded (PERS-005).
        assert_eq!(all.len(), 4);
        Ok(())
    }

    #[tokio::test]
    async fn search_returns_galleries_ordered_by_identifier() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxGalleryRepository::new(&mut transaction);
        let second = repository
            .create(gallery(0, Some(1), "Second", false)?)
            .await?;
        let third = repository
            .create(gallery(0, Some(1), "Third", false)?)
            .await?;

        // Act
        let found = repository
            .search(&GalleryFilter {
                user_id: Some(1),
                ..GalleryFilter::default()
            })
            .await?;

        // Assert
        let ids: Vec<_> = found.iter().map(Gallery::gallery_id).collect();
        assert_eq!(ids, vec![second.gallery_id(), third.gallery_id()]);
        Ok(())
    }

    #[tokio::test]
    async fn search_returns_empty_for_unknown_name() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxGalleryRepository::new(&mut transaction);

        // Act
        let found = repository
            .search(&GalleryFilter {
                name: Some("absent".to_owned()),
                ..GalleryFilter::default()
            })
            .await?;

        // Assert
        assert!(found.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn search_filters_by_name_and_owner_conjunctively() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_user(&mut transaction, 2).await?;
        let mut repository = SqlxGalleryRepository::new(&mut transaction);
        repository
            .create(gallery(0, Some(1), "Holidays", false)?)
            .await?;
        repository
            .create(gallery(0, Some(2), "Holidays", false)?)
            .await?;
        repository
            .create(gallery(0, Some(1), "Other", false)?)
            .await?;

        // Act
        let by_name_and_owner = repository
            .search(&GalleryFilter {
                name: Some("Holidays".to_owned()),
                user_id: Some(1),
                ..GalleryFilter::default()
            })
            .await?;

        // Assert: both conditions must hold, so only the caller's own row matches.
        assert_eq!(by_name_and_owner.len(), 1);
        assert_eq!(
            by_name_and_owner.first().and_then(Gallery::user_id),
            Some(1)
        );
        assert_eq!(
            by_name_and_owner.first().map(Gallery::name),
            Some("Holidays")
        );
        Ok(())
    }

    #[tokio::test]
    async fn default_gallery_is_seeded_public_and_ownerless() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxGalleryRepository::new(&mut transaction);

        // Act
        let found = repository
            .search(&GalleryFilter {
                id: Some(0),
                ..GalleryFilter::default()
            })
            .await?;

        // Assert
        let default = found.first().ok_or("the default gallery must be seeded")?;
        assert_eq!(default.name(), "Default");
        assert!(default.is_public());
        assert_eq!(default.user_id(), None);
        assert!(default.is_system());
        Ok(())
    }

    #[tokio::test]
    async fn add_item_assigns_identifier_and_round_trips() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "a.png", "image/png").await?;
        seed_image(&mut transaction, 5, 10, "a.png").await?;
        let mut repository = SqlxGalleryRepository::new(&mut transaction);
        let created = repository
            .create(gallery(0, Some(1), "Holidays", false)?)
            .await?;

        // Act
        let persisted = repository
            .add_item(item(
                0,
                created.gallery_id(),
                GalleryItemMedia::Image(5),
                0,
            )?)
            .await?;
        let details = repository
            .search_items(&GalleryItemFilter {
                gallery_id: Some(created.gallery_id()),
                ..GalleryItemFilter::default()
            })
            .await?;

        // Assert
        assert_ne!(
            persisted.gallery_item_id(),
            0,
            "a new item must receive a real identifier"
        );
        assert_eq!(persisted.media(), GalleryItemMedia::Image(5));
        assert_eq!(persisted.item_index(), 0);
        let detail = details.first().ok_or("the item must be found")?;
        assert_eq!(detail.file_id(), 10);
        assert_eq!(detail.name(), "a.png");
        Ok(())
    }

    #[tokio::test]
    async fn add_item_rejects_duplicate_index_within_a_gallery() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "a.png", "image/png").await?;
        seed_file(&mut transaction, 11, 1, "b.png", "image/png").await?;
        seed_image(&mut transaction, 5, 10, "a.png").await?;
        seed_image(&mut transaction, 6, 11, "b.png").await?;
        let mut repository = SqlxGalleryRepository::new(&mut transaction);
        let created = repository
            .create(gallery(0, Some(1), "Holidays", false)?)
            .await?;
        repository
            .add_item(item(
                0,
                created.gallery_id(),
                GalleryItemMedia::Image(5),
                0,
            )?)
            .await?;

        // Act
        let result = repository
            .add_item(item(
                0,
                created.gallery_id(),
                GalleryItemMedia::Image(6),
                0,
            )?)
            .await;

        // Assert
        assert!(matches!(result, Err(RepositoryError::AlreadyExist)));
        Ok(())
    }

    #[tokio::test]
    async fn add_item_rejects_media_already_assigned() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "a.png", "image/png").await?;
        seed_image(&mut transaction, 5, 10, "a.png").await?;
        let mut repository = SqlxGalleryRepository::new(&mut transaction);
        let first = repository
            .create(gallery(0, Some(1), "First", false)?)
            .await?;
        let second = repository
            .create(gallery(0, Some(1), "Second", false)?)
            .await?;
        repository
            .add_item(item(0, first.gallery_id(), GalleryItemMedia::Image(5), 0)?)
            .await?;

        // Act
        let result = repository
            .add_item(item(0, second.gallery_id(), GalleryItemMedia::Image(5), 0)?)
            .await;

        // Assert: the partial unique index keeps a medium in one gallery only
        // (PERS-019).
        assert!(matches!(result, Err(RepositoryError::AlreadyExist)));
        Ok(())
    }

    #[tokio::test]
    async fn add_item_missing_gallery_returns_data_integrity_violation()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "a.png", "image/png").await?;
        seed_image(&mut transaction, 5, 10, "a.png").await?;
        let mut repository = SqlxGalleryRepository::new(&mut transaction);

        // Act
        let result = repository
            .add_item(item(0, 999, GalleryItemMedia::Image(5), 0)?)
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn add_item_missing_media_returns_data_integrity_violation() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxGalleryRepository::new(&mut transaction);
        let created = repository
            .create(gallery(0, Some(1), "Holidays", false)?)
            .await?;

        // Act
        let result = repository
            .add_item(item(
                0,
                created.gallery_id(),
                GalleryItemMedia::Image(999),
                0,
            )?)
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn search_items_names_images_and_videos() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "files/photo.png", "image/png").await?;
        seed_file(&mut transaction, 11, 1, "files/clip.mp4", "video/mp4").await?;
        seed_image(&mut transaction, 5, 10, "photo.png").await?;
        seed_video(&mut transaction, 6, 11).await?;
        let mut repository = SqlxGalleryRepository::new(&mut transaction);
        let created = repository
            .create(gallery(0, Some(1), "Holidays", false)?)
            .await?;
        repository
            .add_item(item(
                0,
                created.gallery_id(),
                GalleryItemMedia::Image(5),
                0,
            )?)
            .await?;
        repository
            .add_item(item(
                0,
                created.gallery_id(),
                GalleryItemMedia::Video(6),
                1,
            )?)
            .await?;

        // Act
        let details = repository
            .search_items(&GalleryItemFilter {
                gallery_id: Some(created.gallery_id()),
                ..GalleryItemFilter::default()
            })
            .await?;

        // Assert
        assert_eq!(details.len(), 2);
        let image = details.first().ok_or("the image item must be found")?;
        assert_eq!(image.item().media(), GalleryItemMedia::Image(5));
        assert_eq!(image.file_id(), 10);
        assert_eq!(image.name(), "photo.png");
        let video = details.get(1).ok_or("the video item must be found")?;
        assert_eq!(video.item().media(), GalleryItemMedia::Video(6));
        assert_eq!(video.file_id(), 11);
        assert_eq!(video.name(), "files/clip.mp4");
        Ok(())
    }

    #[tokio::test]
    async fn search_items_filters_by_gallery_identifier_and_media() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "files/photo.png", "image/png").await?;
        seed_image(&mut transaction, 5, 10, "photo.png").await?;
        let mut repository = SqlxGalleryRepository::new(&mut transaction);
        let first = repository
            .create(gallery(0, Some(1), "First", false)?)
            .await?;
        let second = repository
            .create(gallery(0, Some(1), "Second", false)?)
            .await?;
        let stored = repository
            .add_item(item(0, first.gallery_id(), GalleryItemMedia::Image(5), 0)?)
            .await?;
        let reassignment = repository
            .add_item(item(0, second.gallery_id(), GalleryItemMedia::Image(5), 0)?)
            .await;

        // Act
        let by_gallery = repository
            .search_items(&GalleryItemFilter {
                gallery_id: Some(first.gallery_id()),
                ..GalleryItemFilter::default()
            })
            .await?;
        let by_id = repository
            .search_items(&GalleryItemFilter {
                id: Some(stored.gallery_item_id()),
                ..GalleryItemFilter::default()
            })
            .await?;
        let by_media = repository
            .search_items(&GalleryItemFilter {
                media: Some(GalleryItemMedia::Image(5)),
                ..GalleryItemFilter::default()
            })
            .await?;

        // Assert
        assert!(matches!(reassignment, Err(RepositoryError::AlreadyExist)));
        assert_eq!(by_gallery.len(), 1);
        assert_eq!(
            by_gallery
                .first()
                .map(|detail| detail.item().gallery_item_id()),
            Some(stored.gallery_item_id())
        );
        assert_eq!(by_id.len(), 1);
        // The second add failed the partial unique index, so the medium stayed
        // in the first gallery only.
        assert_eq!(by_media.len(), 1);
        assert_eq!(
            by_media.first().map(|detail| detail.item().gallery_id()),
            Some(first.gallery_id())
        );
        Ok(())
    }

    #[tokio::test]
    async fn search_items_returns_empty_for_unknown_gallery() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxGalleryRepository::new(&mut transaction);

        // Act
        let details = repository
            .search_items(&GalleryItemFilter {
                gallery_id: Some(999),
                ..GalleryItemFilter::default()
            })
            .await?;

        // Assert
        assert!(details.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn delete_item_reports_matched_and_missing_items() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "a.png", "image/png").await?;
        seed_image(&mut transaction, 5, 10, "a.png").await?;
        let mut repository = SqlxGalleryRepository::new(&mut transaction);
        let created = repository
            .create(gallery(0, Some(1), "Holidays", false)?)
            .await?;
        let stored = repository
            .add_item(item(
                0,
                created.gallery_id(),
                GalleryItemMedia::Image(5),
                0,
            )?)
            .await?;

        // Act
        let deleted = repository.delete_item(stored.gallery_item_id()).await?;
        let missing = repository.delete_item(stored.gallery_item_id()).await?;

        // Assert
        assert!(deleted, "an existing item must be deleted");
        assert!(!missing, "a missing item must report false, not an error");
        Ok(())
    }

    #[tokio::test]
    async fn next_item_index_grows_with_the_highest_index() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "a.png", "image/png").await?;
        seed_image(&mut transaction, 5, 10, "a.png").await?;
        let mut repository = SqlxGalleryRepository::new(&mut transaction);
        let created = repository
            .create(gallery(0, Some(1), "Holidays", false)?)
            .await?;
        let empty = repository.next_item_index(created.gallery_id()).await?;
        repository
            .add_item(item(
                0,
                created.gallery_id(),
                GalleryItemMedia::Image(5),
                4,
            )?)
            .await?;

        // Act
        let after = repository.next_item_index(created.gallery_id()).await?;

        // Assert
        assert_eq!(empty, 0, "an empty gallery's next index must be zero");
        assert_eq!(after, 5, "the next index must follow the highest one");
        Ok(())
    }

    #[tokio::test]
    async fn gallery_item_rejects_both_media() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "a.png", "image/png").await?;
        seed_file(&mut transaction, 11, 1, "b.mp4", "video/mp4").await?;
        seed_image(&mut transaction, 5, 10, "a.png").await?;
        seed_video(&mut transaction, 6, 11).await?;
        seed_gallery(&mut transaction, 1, 1).await?;

        // Act
        let result = sqlx::query(
            "INSERT INTO gallery_item (gallery_item_id, gallery_id, image_id, video_id, item_index)
             VALUES (1, 1, 5, 6, 0)",
        )
        .execute(&mut *transaction)
        .await
        .map_err(RepositoryError::from);

        // Assert: chk_gallery_item_one_media (PERS-018).
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn gallery_item_rejects_neither_media() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_gallery(&mut transaction, 1, 1).await?;

        // Act
        let result = sqlx::query(
            "INSERT INTO gallery_item (gallery_item_id, gallery_id, item_index)
             VALUES (1, 1, 0)",
        )
        .execute(&mut *transaction)
        .await
        .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn gallery_item_rejects_negative_index() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "a.png", "image/png").await?;
        seed_image(&mut transaction, 5, 10, "a.png").await?;
        seed_gallery(&mut transaction, 1, 1).await?;

        // Act
        let result = sqlx::query(
            "INSERT INTO gallery_item (gallery_item_id, gallery_id, image_id, item_index)
             VALUES (1, 1, 5, -1)",
        )
        .execute(&mut *transaction)
        .await
        .map_err(RepositoryError::from);

        // Assert: chk_gallery_item_item_index.
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }
}
