use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;

use crate::application::port::delete_gallery::DeleteGalleryCommand;
use crate::application::port::delete_gallery::DeleteGalleryError;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::library::get_gallery::parse_gallery_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Map a delete-gallery error to its API error.
impl From<DeleteGalleryError> for ApiError {
    fn from(err: DeleteGalleryError) -> Self {
        match err {
            DeleteGalleryError::DefaultGallery => Self::Conflict(err.to_string()),
            DeleteGalleryError::Forbidden => Self::Forbidden(err.to_string()),
            DeleteGalleryError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            DeleteGalleryError::NoSuchGallery => Self::NotFound(err.to_string()),
            DeleteGalleryError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Delete a gallery and its items by its identifier.
///
/// Only the owner of the gallery may delete it; administrators have no
/// override. The seeded default gallery (identifier `0`) is never deletable and
/// answers `409`. Deletion is permanent.
#[utoipa::path(
    delete,
    operation_id = "delete_gallery",
    path = "/library/gallery/{id}",
    tag = "library",
    params(
        ("id" = NumericID, Path, description = "Identifier of the gallery to delete"),
    ),
    responses(
        (
            status = NO_CONTENT,
            description = "Gallery deleted",
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid gallery identifier"
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
            description = "Caller is not the owner of the gallery"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown gallery"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The default gallery cannot be deleted"
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
    summary = "Delete a gallery"
)]
pub async fn delete_gallery(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let gallery_id = parse_gallery_id(&id)?;
    state
        .library_use_case_factory()
        .delete_gallery()
        .execute(DeleteGalleryCommand::new(caller.user_id(), gallery_id))
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

    use super::delete_gallery;
    use crate::application::port::delete_gallery::DeleteGalleryCommand;
    use crate::application::port::delete_gallery::DeleteGalleryError;
    use crate::application::port::delete_gallery::DeleteGalleryUseCase;
    use crate::application::port::delete_gallery::MockDeleteGalleryUseCase;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;

    /// Send a DELETE to `/api/v1/library/gallery/{id}` on behalf of `caller_id`,
    /// injecting the caller identifier the way the auth middleware does.
    async fn send(
        use_case: MockDeleteGalleryUseCase,
        caller_id: NumericID,
        id: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_delete_gallery()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn DeleteGalleryUseCase>);

        let mut request = Request::builder()
            .method("DELETE")
            .uri(format!("/api/v1/library/gallery/{id}"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/library/gallery/{id}", delete(delete_gallery))
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    /// Make the mocked use case fail with `error`.
    fn expect_error(use_case: &mut MockDeleteGalleryUseCase, error: DeleteGalleryError) {
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));
    }

    #[tokio::test]
    async fn delete_gallery_owned_gallery_returns_no_content() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockDeleteGalleryUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(DeleteGalleryCommand::new(1, 3)))
            .return_once(|_| Box::pin(async { Ok(()) }));

        // Act
        let response = send(use_case, 1, "3").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_default_gallery_returns_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockDeleteGalleryUseCase::new();
        expect_error(&mut use_case, DeleteGalleryError::DefaultGallery);

        // Act
        let response = send(use_case, 1, "0").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the default gallery cannot be deleted")
        );
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_not_owner_returns_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockDeleteGalleryUseCase::new();
        expect_error(&mut use_case, DeleteGalleryError::Forbidden);

        // Act
        let response = send(use_case, 1, "3").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("only the owner may delete this gallery")
        );
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_unknown_gallery_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockDeleteGalleryUseCase::new();
        expect_error(&mut use_case, DeleteGalleryError::NoSuchGallery);

        // Act
        let response = send(use_case, 1, "3").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_unknown_caller_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockDeleteGalleryUseCase::new();
        expect_error(&mut use_case, DeleteGalleryError::NoSuchCaller);

        // Act
        let response = send(use_case, 999, "3").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_dependency_failure_returns_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockDeleteGalleryUseCase::new();
        expect_error(
            &mut use_case,
            DeleteGalleryError::Unknown(anyhow::anyhow!("boom")),
        );

        // Act
        let response = send(use_case, 1, "3").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        Ok(())
    }

    #[tokio::test]
    async fn delete_gallery_invalid_id_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockDeleteGalleryUseCase::new();

        // Act
        let response = send(use_case, 1, "abc").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("invalid gallery identifier")
        );
        Ok(())
    }
}
