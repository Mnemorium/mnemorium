use sqlx::{QueryBuilder, Sqlite, Transaction};

use crate::domain::model::{Image, Orientation};
use crate::domain::port::{ImageFilter, ImageRepository, RepositoryError};

use super::error_mapping::stored_data_invalid;
use super::model::{
    Image as SqlxImage, Orientation as SqlxOrientation, asset_scope_token, parse_asset_scope,
};

use super::query_filter::push_filter;

/// Repository persisting images, backed by a `SQLite` transaction.
pub(crate) struct SqlxImageRepository<'transaction> {
    /// Transaction the repository reads from and writes to.
    transaction: &'transaction mut Transaction<'static, Sqlite>,
}

impl<'transaction> SqlxImageRepository<'transaction> {
    /// Create a new repository bound to `transaction`.
    #[must_use]
    pub(crate) fn new(transaction: &'transaction mut Transaction<'static, Sqlite>) -> Self {
        Self { transaction }
    }
}

impl ImageRepository for SqlxImageRepository<'_> {
    async fn create(&mut self, image: Image) -> Result<Image, RepositoryError> {
        let row = sqlx::query_as::<_, SqlxImage>(
            "INSERT INTO image (name, width_px, height_px, orientation, created_at, file_id, scope)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            RETURNING
                image_id,
                name,
                width_px,
                height_px,
                orientation,
                created_at,
                file_id,
                scope",
        )
        .bind(image.name())
        .bind(image.width_px())
        .bind(image.height_px())
        .bind(sqlx_orientation(image.orientation()))
        .bind(image.created_at())
        .bind(image.file_id())
        .bind(image.scope().map(asset_scope_token))
        .fetch_one(&mut **self.transaction)
        .await?;

        domain_image(row)
    }

    async fn delete(&mut self, filter: &ImageFilter) -> Result<bool, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new("DELETE FROM image");
        let mut first = true;

        if let Some(file_id) = filter.file_id {
            push_filter(&mut first, &mut builder, "file_id = ", file_id);
        }
        if let Some(id) = filter.image_id {
            push_filter(&mut first, &mut builder, "image_id = ", id);
        }
        if let Some(name) = filter.name.as_deref() {
            push_filter(&mut first, &mut builder, "name = ", name);
        }
        if let Some(orientation) = filter.orientation {
            push_filter(
                &mut first,
                &mut builder,
                "orientation = ",
                sqlx_orientation(orientation),
            );
        }

        let result = builder.build().execute(&mut **self.transaction).await?;
        Ok(result.rows_affected() > 0)
    }

    async fn search(&mut self, filter: &ImageFilter) -> Result<Vec<Image>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT
                image_id,
                name,
                width_px,
                height_px,
                orientation,
                created_at,
                file_id,
                scope
            FROM image",
        );
        let mut first = true;

        if let Some(file_id) = filter.file_id {
            push_filter(&mut first, &mut builder, "file_id = ", file_id);
        }
        if let Some(id) = filter.image_id {
            push_filter(&mut first, &mut builder, "image_id = ", id);
        }
        if let Some(name) = filter.name.as_deref() {
            push_filter(&mut first, &mut builder, "name = ", name);
        }
        if let Some(orientation) = filter.orientation {
            push_filter(
                &mut first,
                &mut builder,
                "orientation = ",
                sqlx_orientation(orientation),
            );
        }

        let rows = builder
            .build_query_as::<SqlxImage>()
            .fetch_all(&mut **self.transaction)
            .await?;

        rows.into_iter().map(domain_image).collect()
    }
}

/// Map the domain orientation to its persisted representation.
fn sqlx_orientation(orientation: Orientation) -> SqlxOrientation {
    match orientation {
        Orientation::Landscape => SqlxOrientation::Landscape,
        Orientation::Portrait => SqlxOrientation::Portrait,
        Orientation::Square => SqlxOrientation::Square,
    }
}

/// Map a persisted image row back to the domain model.
fn domain_image(row: SqlxImage) -> Result<Image, RepositoryError> {
    let orientation = match row.orientation {
        SqlxOrientation::Landscape => Orientation::Landscape,
        SqlxOrientation::Portrait => Orientation::Portrait,
        SqlxOrientation::Square => Orientation::Square,
    };
    let scope = row
        .scope
        .as_deref()
        .map(|value| parse_asset_scope(value).ok_or_else(|| stored_data_invalid("image_scope")))
        .transpose()?;

    Image::try_new(
        row.image_id,
        row.name,
        row.width_px,
        row.height_px,
        orientation,
        row.created_at,
        row.file_id,
        scope,
    )
    .map_err(|_| RepositoryError::DataIntegrityViolation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::AssetScope;
    use crate::infrastructure::outbound::sqlx::test_helpers::*;

    use crate::infrastructure::outbound::sqlx::test_helpers::begin_transaction;
    use chrono::NaiveDate;
    use rstest::rstest;
    use std::error::Error;

    /// Build a domain image.
    fn image(
        image_id: i64,
        file_id: i64,
        name: &str,
        orientation: Orientation,
    ) -> Result<Image, Box<dyn Error>> {
        Ok(Image::try_new(
            image_id,
            name.to_owned(),
            640,
            480,
            orientation,
            NaiveDate::from_ymd_opt(2026, 10, 6).ok_or("invalid fixture date")?,
            file_id,
            None,
        )?)
    }

    #[tokio::test]
    async fn create_assigns_identifier_and_round_trips() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "files/photo.png", "image/png").await?;
        let pending = image(0, 10, "photo.png", Orientation::Landscape)?;

        // Act
        let mut repository = SqlxImageRepository::new(&mut transaction);
        let persisted = repository.create(pending).await?;
        let found = repository
            .search(&ImageFilter {
                image_id: Some(persisted.image_id()),
                ..ImageFilter::default()
            })
            .await?
            .into_iter()
            .next();

        // Assert
        assert_ne!(
            persisted.image_id(),
            0,
            "a new image must receive a real identifier"
        );
        assert_eq!(persisted.name(), "photo.png");
        assert_eq!(persisted.orientation(), Orientation::Landscape);
        assert_eq!(persisted.file_id(), 10);
        assert_eq!(found, Some(persisted));
        Ok(())
    }

    #[tokio::test]
    async fn create_round_trips_a_stored_scope() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "files/photo.png", "image/png").await?;
        let pending = Image::try_new(
            0,
            "photo.png".to_owned(),
            640,
            480,
            Orientation::Landscape,
            NaiveDate::from_ymd_opt(2026, 10, 6).ok_or("invalid fixture date")?,
            10,
            Some(AssetScope::Library),
        )?;

        // Act
        let mut repository = SqlxImageRepository::new(&mut transaction);
        let persisted = repository.create(pending).await?;
        let found = repository
            .search(&ImageFilter {
                image_id: Some(persisted.image_id()),
                ..ImageFilter::default()
            })
            .await?
            .into_iter()
            .next();

        // Assert
        assert_eq!(persisted.scope(), Some(AssetScope::Library));
        assert_eq!(
            found.and_then(|image| image.scope()),
            Some(AssetScope::Library)
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_missing_image_returns_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;

        // Act
        let mut repository = SqlxImageRepository::new(&mut transaction);
        let missing = repository
            .search(&ImageFilter {
                image_id: Some(999),
                ..ImageFilter::default()
            })
            .await?
            .into_iter()
            .next();

        // Assert
        assert!(missing.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn create_duplicate_file_returns_already_exist() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "files/photo.png", "image/png").await?;
        let mut repository = SqlxImageRepository::new(&mut transaction);
        repository
            .create(image(0, 10, "photo.png", Orientation::Landscape)?)
            .await?;

        // Act
        let result = repository
            .create(image(0, 10, "other.png", Orientation::Portrait)?)
            .await;

        // Assert: uq_image_file_id, one image per file.
        assert!(matches!(result, Err(RepositoryError::AlreadyExist)));
        Ok(())
    }

    #[tokio::test]
    async fn create_missing_file_returns_data_integrity_violation() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;

        // Act
        let mut repository = SqlxImageRepository::new(&mut transaction);
        let result = repository
            .create(image(0, 999, "photo.png", Orientation::Landscape)?)
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn delete_reports_matched_and_missing_rows() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "files/photo.png", "image/png").await?;
        let mut repository = SqlxImageRepository::new(&mut transaction);
        let stored = repository
            .create(image(0, 10, "photo.png", Orientation::Square)?)
            .await?;

        // Act
        let deleted = repository
            .delete(&ImageFilter {
                image_id: Some(stored.image_id()),
                ..ImageFilter::default()
            })
            .await?;
        let missing = repository
            .delete(&ImageFilter {
                image_id: Some(stored.image_id()),
                ..ImageFilter::default()
            })
            .await?;

        // Assert
        assert!(deleted);
        assert!(!missing);
        Ok(())
    }

    #[tokio::test]
    async fn search_filters_by_file_name_and_orientation() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "files/wide.png", "image/png").await?;
        seed_file(&mut transaction, 11, 1, "files/tall.png", "image/png").await?;
        let mut repository = SqlxImageRepository::new(&mut transaction);
        let wide = repository
            .create(image(0, 10, "wide.png", Orientation::Landscape)?)
            .await?;
        repository
            .create(image(0, 11, "tall.png", Orientation::Portrait)?)
            .await?;

        // Act
        let by_file = repository
            .search(&ImageFilter {
                file_id: Some(10),
                ..ImageFilter::default()
            })
            .await?;
        let by_name = repository
            .search(&ImageFilter {
                name: Some("tall.png".to_owned()),
                ..ImageFilter::default()
            })
            .await?;
        let by_orientation = repository
            .search(&ImageFilter {
                orientation: Some(Orientation::Landscape),
                ..ImageFilter::default()
            })
            .await?;
        let all = repository.search(&ImageFilter::default()).await?;

        // Assert
        assert_eq!(by_file.first(), Some(&wide));
        assert_eq!(by_name.len(), 1);
        assert_eq!(by_name.first().map(Image::name), Some("tall.png"));
        assert_eq!(by_orientation.len(), 1);
        assert_eq!(all.len(), 2);
        Ok(())
    }

    #[tokio::test]
    async fn delete_nulls_the_cover_and_keeps_the_owner() -> Result<(), Box<dyn Error>> {
        // Arrange: one music group uses the image as its cover.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "files/cover.png", "image/png").await?;
        let stored = {
            let mut repository = SqlxImageRepository::new(&mut transaction);
            repository
                .create(image(0, 10, "cover.png", Orientation::Square)?)
                .await?
        };
        sqlx::query(
            "INSERT INTO music_group (music_group_id, stage_name, cover_image_id)
             VALUES (1, 'Band', ?)",
        )
        .bind(stored.image_id())
        .execute(&mut *transaction)
        .await?;

        // Act
        {
            let mut repository = SqlxImageRepository::new(&mut transaction);
            repository
                .delete(&ImageFilter {
                    image_id: Some(stored.image_id()),
                    ..ImageFilter::default()
                })
                .await?;
        }

        // Assert
        let cover = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT cover_image_id FROM music_group WHERE music_group_id = 1",
        )
        .fetch_one(&mut *transaction)
        .await?;
        let groups = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM music_group WHERE music_group_id = 1",
        )
        .fetch_one(&mut *transaction)
        .await?;
        assert_eq!(cover, None);
        assert_eq!(groups, 1, "the resource carrying the cover is retained");
        Ok(())
    }

    #[rstest]
    #[case::width_not_positive(
        "INSERT INTO image (image_id, name, width_px, height_px, orientation, created_at, file_id)
         VALUES (1, 'a', 0, 10, 'SQUARE', '2026-10-06', 10)"
    )]
    #[case::height_not_positive(
        "INSERT INTO image (image_id, name, width_px, height_px, orientation, created_at, file_id)
         VALUES (1, 'a', 10, 0, 'SQUARE', '2026-10-06', 10)"
    )]
    #[case::orientation_not_known(
        "INSERT INTO image (image_id, name, width_px, height_px, orientation, created_at, file_id)
         VALUES (1, 'a', 10, 10, 'SIDEWAYS', '2026-10-06', 10)"
    )]
    #[case::file_id_null(
        "INSERT INTO image (image_id, name, width_px, height_px, orientation, created_at, file_id)
         VALUES (1, 'a', 10, 10, 'SQUARE', '2026-10-06', NULL)"
    )]
    #[tokio::test]
    async fn raw_insert_violating_a_check_returns_data_integrity_violation(
        #[case] statement: &'static str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "files/photo.png", "image/png").await?;

        // Act
        let result = sqlx::query(statement)
            .execute(&mut *transaction)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::DataIntegrityViolation)),
            "statement must violate a check or foreign key: {statement}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn deleting_image_cascades_its_item() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;
        seed_gallery(&pool, 1).await?;
        sqlx::query(
            "INSERT INTO gallery_item (gallery_item_id, gallery_id, image_id, item_index)
             VALUES (1, 1, 1, 0)",
        )
        .execute(&pool)
        .await?;

        // Act
        sqlx::query("DELETE FROM image WHERE image_id = 1")
            .execute(&pool)
            .await?;

        // Assert
        let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM gallery_item")
            .fetch_one(&pool)
            .await?;
        assert_eq!(remaining, 0, "deleting an image must cascade its item");
        Ok(())
    }

    #[tokio::test]
    async fn library_cover_accepts_a_library_image_on_every_owner() -> Result<(), Box<dyn Error>> {
        // Arrange: image 1 starts unattached.
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;

        // Act
        seed_covered_owners(&pool, 1).await?;

        // Assert: every owner stores the image as its cover and the first
        // attachment marked it LIBRARY.
        let covers: Vec<Option<i64>> = vec![
            sqlx::query_scalar("SELECT cover_image_id FROM movie")
                .fetch_one(&pool)
                .await?,
            sqlx::query_scalar("SELECT cover_image_id FROM series")
                .fetch_one(&pool)
                .await?,
            sqlx::query_scalar("SELECT cover_image_id FROM season")
                .fetch_one(&pool)
                .await?,
            sqlx::query_scalar("SELECT cover_image_id FROM music_album WHERE music_album_id = 1")
                .fetch_one(&pool)
                .await?,
        ];
        let scope: Option<String> =
            sqlx::query_scalar("SELECT scope FROM image WHERE image_id = 1")
                .fetch_one(&pool)
                .await?;
        assert_eq!(
            covers,
            vec![Some(1); 4],
            "every library owner must store the image as its cover"
        );
        assert_eq!(scope.as_deref(), Some("LIBRARY"));
        Ok(())
    }

    #[tokio::test]
    async fn cover_rejects_a_missing_image() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;

        // Act: image 99 does not exist.
        let result = sqlx::query(
            "INSERT INTO movie (
                movie_id, name, plot, country_of_origin, release_date, video_id, cover_image_id
             )
             VALUES (1, 'Film', 'A plot', 'US', '1990-04-08', 1, 99)",
        )
        .execute(&pool)
        .await
        .map_err(RepositoryError::from);

        // Assert: the foreign key refuses the dangling reference.
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn library_cover_rejects_a_user_image() -> Result<(), Box<dyn Error>> {
        // Arrange: a gallery item owns image 1, so it is USER.
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;

        // Act
        seed_gallery_image(&pool, 1).await?;
        let result = sqlx::query(
            "INSERT INTO movie (
                movie_id, name, plot, country_of_origin, release_date, video_id, cover_image_id
             )
             VALUES (1, 'Film', 'A plot', 'US', '1990-04-08', 1, 1)",
        )
        .execute(&pool)
        .await
        .map_err(RepositoryError::from);

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::Conflict)),
            "a library cover must reject a USER image as a conflict"
        );
        let stored: i64 = sqlx::query_scalar("SELECT count(*) FROM movie")
            .fetch_one(&pool)
            .await?;
        assert_eq!(stored, 0, "a rejected cover must store nothing");
        Ok(())
    }

    #[tokio::test]
    async fn cover_update_rejects_a_user_image() -> Result<(), Box<dyn Error>> {
        // Arrange: a film without artwork, and image 1 used as gallery media.
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;
        seed_gallery_image(&pool, 1).await?;
        sqlx::query(
            "INSERT INTO movie (
                movie_id, name, plot, country_of_origin, release_date, video_id
             )
             VALUES (1, 'Film', 'A plot', 'US', '1990-04-08', 1)",
        )
        .execute(&pool)
        .await?;

        // Act
        let result = sqlx::query("UPDATE movie SET cover_image_id = 1 WHERE movie_id = 1")
            .execute(&pool)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::Conflict)),
            "an update to a USER image cover must be rejected as a conflict"
        );
        let stored: Option<i64> = sqlx::query_scalar("SELECT cover_image_id FROM movie")
            .fetch_one(&pool)
            .await?;
        assert_eq!(stored, None, "the rejected update must store nothing");
        Ok(())
    }

    #[tokio::test]
    async fn asset_scope_rejects_an_unknown_value() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;

        // Act
        let result = sqlx::query("UPDATE image SET scope = 'GALLERY' WHERE image_id = 1")
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

    #[tokio::test]
    async fn asset_scope_is_immutable_once_set() -> Result<(), Box<dyn Error>> {
        // Arrange: the first attachment marks image 1 USER.
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;
        seed_gallery_image(&pool, 1).await?;

        // Act
        let result = sqlx::query("UPDATE image SET scope = 'LIBRARY' WHERE image_id = 1")
            .execute(&pool)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::Conflict)),
            "an asset scope must not change once set"
        );
        let scope: Option<String> =
            sqlx::query_scalar("SELECT scope FROM image WHERE image_id = 1")
                .fetch_one(&pool)
                .await?;
        assert_eq!(scope.as_deref(), Some("USER"));
        Ok(())
    }
}
