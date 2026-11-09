use sqlx::{QueryBuilder, Sqlite, Transaction};

use crate::domain::model::mime_type::MimeType;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::mime_type_repository::{MimeTypeFilter, MimeTypeRepository};

use super::query_filter::push_filter;

/// Repository querying mime types, backed by a `SQLite` transaction.
pub(crate) struct SqlxMimeTypeRepository<'transaction> {
    /// Transaction the repository reads from.
    transaction: &'transaction mut Transaction<'static, Sqlite>,
}

impl<'transaction> SqlxMimeTypeRepository<'transaction> {
    /// Create a new repository bound to `transaction`.
    #[must_use]
    pub(crate) fn new(transaction: &'transaction mut Transaction<'static, Sqlite>) -> Self {
        Self { transaction }
    }
}

impl MimeTypeRepository for SqlxMimeTypeRepository<'_> {
    async fn search(&mut self, filter: &MimeTypeFilter) -> Result<Vec<MimeType>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new("SELECT mime_type_id FROM mime_type");
        let mut first = true;

        if let Some(mime_type_id) = filter.mime_type_id.as_deref() {
            push_filter(&mut first, &mut builder, "mime_type_id = ", mime_type_id);
        }
        builder.push(" ORDER BY mime_type_id");

        let ids = builder
            .build_query_scalar::<String>()
            .fetch_all(&mut **self.transaction)
            .await?;

        ids.into_iter()
            .map(|id| MimeType::try_new(id).map_err(|_| RepositoryError::DataIntegrityViolation))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::outbound::sqlx::test_helpers::begin_transaction;
    use std::error::Error;

    #[tokio::test]
    async fn search_returns_the_seeded_mime_types() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxMimeTypeRepository::new(&mut transaction);

        // Act
        let found = repository.search(&MimeTypeFilter::default()).await?;

        // Assert: the supported set comes from the seed and includes media.
        assert!(found.iter().any(|item| item.mime_type_id() == "image/png"));
        assert!(found.iter().any(|item| item.mime_type_id() == "video/mp4"));
        assert!(found.iter().any(|item| item.mime_type_id() == "audio/mpeg"));
        Ok(())
    }

    #[tokio::test]
    async fn search_finds_a_seeded_mime_type() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxMimeTypeRepository::new(&mut transaction);

        // Act
        let found = repository
            .search(&MimeTypeFilter {
                mime_type_id: Some("video/mp4".to_owned()),
            })
            .await?;

        // Assert
        assert_eq!(found.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn search_returns_empty_for_an_unknown_mime_type() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxMimeTypeRepository::new(&mut transaction);

        // Act
        let found = repository
            .search(&MimeTypeFilter {
                mime_type_id: Some("application/unknown".to_owned()),
            })
            .await?;

        // Assert
        assert!(found.is_empty());
        Ok(())
    }
}
