use axum::extract::Path;
use axum::extract::State;
use axum::response::Response;

use crate::application::port::get_gallery::GetGalleryCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::get_gallery::parse_gallery_id;
use crate::infrastructure::inbound::rest::handler::library::representation::GalleryResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// List the items of a gallery.
///
/// This route is a true alias of `GET /library/gallery/{id}`: it returns the
/// same gallery representation, and the `_links.self` of that representation is
/// the canonical gallery URI, never this alias path (`API-042`).
#[utoipa::path(
    get,
    operation_id = "get_gallery_item_list",
    path = "/library/gallery/{id}/item",
    tag = "library",
    params(
        ("id" = NumericID, Path, description = "Identifier of the gallery whose items are listed"),
    ),
    responses(
        (
            status = OK,
            body = GalleryResponse,
            content_type = "application/hal+json",
            description = "Gallery found"
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
            description = "Caller is not allowed to read the gallery"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown gallery"
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
    summary = "List the items of a gallery"
)]
pub async fn get_gallery_item_list(
    Path(id): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let gallery_id = parse_gallery_id(&id)?;
    let response = state
        .library_use_case_factory()
        .get_gallery()
        .execute(GetGalleryCommand::new(caller.user_id(), gallery_id))
        .await?;
    Ok(hal_json(GalleryResponse::from(response)))
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::body::to_bytes;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::response::Response;
    use axum::routing::get;
    use chrono::NaiveDate;
    use chrono::NaiveDateTime;
    use serde_json::Value;
    use tower::ServiceExt as _;

    use super::get_gallery_item_list;
    use crate::application::port::get_gallery::GetGalleryError;
    use crate::application::port::get_gallery::GetGalleryResponse;
    use crate::application::port::get_gallery::GetGalleryUseCase;
    use crate::application::port::get_gallery::MockGetGalleryUseCase;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;

    /// A fixed instant the fixtures timestamp with.
    fn ts() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 6)
            .unwrap_or_default()
            .and_hms_opt(12, 0, 0)
            .unwrap_or_default()
    }

    /// Send a GET to `/api/v1/library/gallery/{id}/item` on behalf of
    /// `caller_id`, injecting the caller identifier the way the auth
    /// middleware does.
    async fn send(
        use_case: MockGetGalleryUseCase,
        caller_id: NumericID,
        id: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_get_gallery()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn GetGalleryUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/library/gallery/{id}/item"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/gallery/{id}/item",
                get(get_gallery_item_list),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    /// Split `response` into its status and decoded JSON body.
    #[expect(
        clippy::single_call_fn,
        reason = "the response splitter keeps the alias test readable"
    )]
    async fn into_parts(response: Response) -> Result<(StatusCode, Value), Box<dyn Error>> {
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let body = serde_json::from_slice(&bytes)?;
        Ok((status, body))
    }

    #[tokio::test]
    async fn get_gallery_item_list_is_an_alias_with_canonical_self() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetGalleryUseCase::new();
        use_case.expect_execute().times(1).return_once(|_| {
            Box::pin(async {
                Ok(GetGalleryResponse::new(
                    3,
                    Some(2),
                    "Holidays".to_owned(),
                    true,
                    ts(),
                    ts(),
                    Vec::new(),
                ))
            })
        });

        // Act
        let (status, payload) = into_parts(send(use_case, 1, "3").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            payload
                .get("_links")
                .and_then(|links| links.get("self"))
                .and_then(|link| link.get("href")),
            Some(&Value::String("/api/v1/library/gallery/3".to_owned())),
            "the alias must report the canonical gallery URI, never the alias path (`API-042`)"
        );
        assert_eq!(
            payload
                .get("_links")
                .and_then(|links| links.get("items"))
                .and_then(|link| link.get("href")),
            Some(&Value::String("/api/v1/library/gallery/3/item".to_owned()))
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_item_list_unknown_gallery_returns_not_found() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut use_case = MockGetGalleryUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(GetGalleryError::NoSuchGallery) }));

        // Act
        let response = send(use_case, 1, "3").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("a gallery with this identifier does not exist")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_item_list_forbidden_returns_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetGalleryUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(GetGalleryError::Forbidden) }));

        // Act
        let response = send(use_case, 1, "3").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the caller is not allowed to read this gallery")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_gallery_item_list_invalid_id_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockGetGalleryUseCase::new();

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

    #[tokio::test]
    async fn get_gallery_item_list_unknown_caller_returns_unauthorized()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetGalleryUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(GetGalleryError::NoSuchCaller) }));

        // Act
        let response = send(use_case, 999, "3").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the authenticated user does not exist")
        );
        Ok(())
    }
}
