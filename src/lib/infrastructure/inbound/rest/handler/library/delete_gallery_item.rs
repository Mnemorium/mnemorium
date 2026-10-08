use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;

use crate::application::port::delete_gallery_item::DeleteGalleryItemCommand;
use crate::application::port::delete_gallery_item::DeleteGalleryItemError;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::library::get_gallery::parse_gallery_id;
use crate::infrastructure::inbound::rest::handler::library::get_gallery_item::parse_item_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Map a delete-gallery-item error to its API error.
impl From<DeleteGalleryItemError> for ApiError {
    fn from(err: DeleteGalleryItemError) -> Self {
        match err {
            DeleteGalleryItemError::Forbidden => Self::Forbidden(err.to_string()),
            DeleteGalleryItemError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            DeleteGalleryItemError::NoSuchGallery | DeleteGalleryItemError::NoSuchItem => {
                Self::NotFound(err.to_string())
            }
            DeleteGalleryItemError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Delete one item of a gallery by its identifiers.
///
/// Any authenticated caller may delete an item of a public gallery; an item of
/// a private gallery may be deleted by the gallery owner and administrators
/// only. The item is scoped to the gallery named in the path. Deletion is
/// permanent and removes the medium and its backing file.
#[utoipa::path(
    delete,
    operation_id = "delete_gallery_item",
    path = "/library/gallery/{id}/item/{item_id}",
    tag = "library",
    params(
        ("id" = NumericID, Path, description = "Identifier of the gallery the item belongs to"),
        ("item_id" = NumericID, Path, description = "Identifier of the item to delete"),
    ),
    responses(
        (
            status = NO_CONTENT,
            description = "Item deleted",
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid gallery or item identifier"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
        ),
        (
            status = FORBIDDEN,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Caller is not allowed to delete items of the gallery"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown gallery or item"
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unexpected error"
        ),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "Delete a gallery item"
)]
pub async fn delete_gallery_item(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    Path((gallery_id_value, item_id_value)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let gallery_id = parse_gallery_id(&gallery_id_value)?;
    let item_id = parse_item_id(&item_id_value)?;
    state
        .library_use_case_factory()
        .delete_gallery_item()
        .execute(DeleteGalleryItemCommand::new(
            caller.user_id(),
            gallery_id,
            item_id,
        ))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::response::Response;
    use axum::routing::delete;
    use mockall::predicate::eq;
    use tower::ServiceExt as _;

    use super::delete_gallery_item;
    use crate::application::port::delete_gallery_item::DeleteGalleryItemCommand;
    use crate::application::port::delete_gallery_item::DeleteGalleryItemError;
    use crate::application::port::delete_gallery_item::DeleteGalleryItemUseCase;
    use crate::application::port::delete_gallery_item::MockDeleteGalleryItemUseCase;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;

    /// Send a DELETE to `/api/v1/library/gallery/{id}/item/{item_id}` on behalf
    /// of `caller_id`, injecting the caller identifier the way the auth
    /// middleware does.
    async fn send(
        use_case: MockDeleteGalleryItemUseCase,
        caller_id: NumericID,
        id: &str,
        item_id: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_delete_gallery_item()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn DeleteGalleryItemUseCase>);

        let mut request = Request::builder()
            .method("DELETE")
            .uri(format!("/api/v1/library/gallery/{id}/item/{item_id}"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/gallery/{id}/item/{item_id}",
                delete(delete_gallery_item),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    /// Make the mocked use case fail with `error`.
    fn expect_error(use_case: &mut MockDeleteGalleryItemUseCase, error: DeleteGalleryItemError) {
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));
    }

    #[tokio::test]
    async fn delete_gallery_item_existing_item_returns_no_content() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockDeleteGalleryItemUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(DeleteGalleryItemCommand::new(1, 3, 42)))
            .return_once(|_| Box::pin(async { Ok(()) }));

        // Act
        let response = send(use_case, 1, "3", "42").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_forbidden_returns_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockDeleteGalleryItemUseCase::new();
        expect_error(&mut use_case, DeleteGalleryItemError::Forbidden);

        // Act
        let response = send(use_case, 1, "3", "42").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the caller is not allowed to delete items of this gallery")
        );
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_unknown_item_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockDeleteGalleryItemUseCase::new();
        expect_error(&mut use_case, DeleteGalleryItemError::NoSuchItem);

        // Act
        let response = send(use_case, 1, "3", "42").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_unknown_caller_returns_unauthorized() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut use_case = MockDeleteGalleryItemUseCase::new();
        expect_error(&mut use_case, DeleteGalleryItemError::NoSuchCaller);

        // Act
        let response = send(use_case, 999, "3", "42").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_dependency_failure_returns_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockDeleteGalleryItemUseCase::new();
        expect_error(
            &mut use_case,
            DeleteGalleryItemError::Unknown(anyhow::anyhow!("boom")),
        );

        // Act
        let response = send(use_case, 1, "3", "42").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_invalid_item_id_returns_bad_request() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = MockDeleteGalleryItemUseCase::new();

        // Act
        let response = send(use_case, 1, "3", "abc").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("invalid item identifier")
        );
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_item_invalid_gallery_id_returns_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockDeleteGalleryItemUseCase::new();

        // Act
        let response = send(use_case, 1, "abc", "42").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("invalid gallery identifier")
        );
        Ok(())
    }
}
