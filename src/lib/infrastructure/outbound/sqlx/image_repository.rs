use sqlx::{QueryBuilder, Sqlite, Transaction};

use crate::domain::model::image::{Image, Orientation};
use crate::domain::port::error::RepositoryError;
use crate::domain::port::image_repository::{ImageFilter, ImageRepository};

use super::error_mapping::stored_data_invalid;
use super::model::image::{Image as SqlxImage, Orientation as SqlxOrientation};
use super::model::scope::{parse_scope, scope_token};
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
        .bind(image.scope().map(scope_token))
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
        .map(|value| parse_scope(value).ok_or_else(|| stored_data_invalid("image_scope")))
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
    use crate::domain::model::scope::Scope;
    use crate::infrastructure::outbound::sqlx::test_helpers::begin_transaction;
    use chrono::NaiveDate;
    use rstest::rstest;
    use std::error::Error;

    /// Open a migrated in-memory database and begin one transaction.
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
            "INSERT INTO file (file_id, path, user_id, is_public, mime_type_id, size_bytes, modified_at, origin, integrity_hash)
             VALUES (?1, ?2, ?3, 0, ?4, 1024, '2026-10-06 00:00:00', 'UPLOADED', ?5)",
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
            Some(Scope::Library),
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
        assert_eq!(persisted.scope(), Some(Scope::Library));
        assert_eq!(found.and_then(|image| image.scope()), Some(Scope::Library));
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
}
