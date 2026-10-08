use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::Response;

use crate::application::port::create_gallery::CreateGalleryCommand;
use crate::application::port::create_gallery::CreateGalleryError;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::representation::GalleryResponse;
use crate::infrastructure::inbound::rest::handler::library::representation::PostGalleryRequest;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Map a create-gallery error to its API error.
impl From<CreateGalleryError> for ApiError {
    fn from(err: CreateGalleryError) -> Self {
        match err {
            CreateGalleryError::InvalidName | CreateGalleryError::NameTooLong => {
                Self::UnprocessableEntity(err.to_string())
            }
            CreateGalleryError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            CreateGalleryError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Create a gallery owned by the caller.
///
/// Any authenticated caller may create a gallery. The name is required, trimmed
/// and at most 100 characters long; `is_public` defaults to `false`.
#[utoipa::path(
    post,
    operation_id = "post_gallery",
    path = "/library/gallery",
    tag = "library",
    request_body = PostGalleryRequest,
    responses(
        (
            status = CREATED,
            body = GalleryResponse,
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "URI of the newly created gallery"),
            ),
            description = "Gallery created",
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Malformed request body"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
        ),
        (
            status = PAYLOAD_TOO_LARGE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body exceeds the maximum allowed size"
        ),
        (
            status = UNSUPPORTED_MEDIA_TYPE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body is not application/json"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The gallery name is empty or too long"
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
    summary = "Create a gallery"
)]
pub async fn post_gallery(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PostGalleryRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(request) = payload.map_err(ApiError::from)?;
    let response = state
        .library_use_case_factory()
        .create_gallery()
        .execute(CreateGalleryCommand::new(
            caller.user_id(),
            request.name,
            request.is_public,
        ))
        .await?;
    let body = GalleryResponse::from(response);
    let location = body.links.self_link.href.clone();
    let mut http_response = hal_json(body);
    *http_response.status_mut() = StatusCode::CREATED;
    http_response.headers_mut().insert(
        header::LOCATION,
        HeaderValue::from_str(&location).map_err(|_| ApiError::InternalServerError)?,
    );
    Ok(http_response)
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::body::to_bytes;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::response::Response;
    use axum::routing::post;
    use chrono::NaiveDate;
    use chrono::NaiveDateTime;
    use mockall::predicate::eq;
    use serde_json::Value;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::post_gallery;
    use crate::application::port::create_gallery::CreateGalleryCommand;
    use crate::application::port::create_gallery::CreateGalleryError;
    use crate::application::port::create_gallery::CreateGalleryResponse;
    use crate::application::port::create_gallery::CreateGalleryUseCase;
    use crate::application::port::create_gallery::MockCreateGalleryUseCase;
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

    /// Send `body` to `/api/v1/library/gallery` on behalf of `caller_id`,
    /// injecting the caller identifier the way the auth middleware does.
    async fn send(
        use_case: MockCreateGalleryUseCase,
        caller_id: NumericID,
        body: Body,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_create_gallery()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn CreateGalleryUseCase>);

        let mut request = Request::builder()
            .method("POST")
            .uri("/api/v1/library/gallery")
            .header(header::CONTENT_TYPE, "application/json")
            .body(body)?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/library/gallery", post(post_gallery))
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    /// Split `response` into its status, `Location` header, `Content-Type`
    /// header and decoded JSON body.
    async fn into_parts(
        response: Response,
    ) -> Result<(StatusCode, Option<String>, Option<String>, Value), Box<dyn Error>> {
        let status = response.status();
        let location = response
            .headers()
            .get(header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let body = serde_json::from_slice(&bytes)?;
        Ok((status, location, content_type, body))
    }

    /// Make the mocked use case fail with `error`.
    fn expect_error(use_case: &mut MockCreateGalleryUseCase, error: CreateGalleryError) {
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));
    }

    #[tokio::test]
    async fn post_gallery_defaults_is_public_to_false() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCreateGalleryUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(CreateGalleryCommand::new(
                1,
                "Holidays".to_owned(),
                false,
            )))
            .return_once(|_| {
                Box::pin(async {
                    Ok(CreateGalleryResponse::new(
                        5,
                        1,
                        "Holidays".to_owned(),
                        false,
                        ts(),
                        ts(),
                    ))
                })
            });

        // Act
        let (status, location, content_type, payload) = into_parts(
            send(
                use_case,
                1,
                Body::from(json!({ "name": "Holidays" }).to_string()),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(location.as_deref(), Some("/api/v1/library/gallery/5"));
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(
            payload,
            json!({
                "_links": {
                    "self": { "href": "/api/v1/library/gallery/5" },
                    "items": { "href": "/api/v1/library/gallery/5/item" }
                },
                "id": 5i64,
                "name": "Holidays",
                "owner_id": 1i64,
                "is_public": false,
                "created_at": "2026-10-06T12:00:00",
                "last_modified_at": "2026-10-06T12:00:00",
                "item_count": 0i64,
                "items": []
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_gallery_explicit_is_public_is_forwarded() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCreateGalleryUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(CreateGalleryCommand::new(1, "Shared".to_owned(), true)))
            .return_once(|_| {
                Box::pin(async {
                    Ok(CreateGalleryResponse::new(
                        5,
                        1,
                        "Shared".to_owned(),
                        true,
                        ts(),
                        ts(),
                    ))
                })
            });

        // Act
        let (status, _, _, _) = into_parts(
            send(
                use_case,
                1,
                Body::from(json!({ "name": "Shared", "is_public": true }).to_string()),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::CREATED);
        Ok(())
    }

    #[tokio::test]
    async fn post_gallery_empty_name_returns_unprocessable_entity() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCreateGalleryUseCase::new();
        expect_error(&mut use_case, CreateGalleryError::InvalidName);

        // Act
        let response = send(use_case, 1, Body::from(json!({ "name": "" }).to_string())).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("gallery name must not be empty")
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_gallery_too_long_name_returns_unprocessable_entity() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut use_case = MockCreateGalleryUseCase::new();
        expect_error(&mut use_case, CreateGalleryError::NameTooLong);

        // Act
        let response = send(
            use_case,
            1,
            Body::from(json!({ "name": "x".repeat(101) }).to_string()),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[tokio::test]
    async fn post_gallery_unknown_caller_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCreateGalleryUseCase::new();
        expect_error(&mut use_case, CreateGalleryError::NoSuchCaller);

        // Act
        let response = send(
            use_case,
            999,
            Body::from(json!({ "name": "x" }).to_string()),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the authenticated user does not exist")
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_gallery_dependency_failure_returns_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCreateGalleryUseCase::new();
        expect_error(
            &mut use_case,
            CreateGalleryError::Unknown(anyhow::anyhow!("boom")),
        );

        // Act
        let response = send(use_case, 1, Body::from(json!({ "name": "x" }).to_string())).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("an unexpected error occurred")
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_gallery_malformed_body_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockCreateGalleryUseCase::new();

        // Act
        let response = send(use_case, 1, Body::from("not json")).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(error_message_of(&response).is_some());
        Ok(())
    }

    #[tokio::test]
    async fn post_gallery_missing_name_returns_unprocessable_entity() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = MockCreateGalleryUseCase::new();

        // Act
        let response = send(
            use_case,
            1,
            Body::from(json!({ "is_public": true }).to_string()),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert!(error_message_of(&response).is_some());
        Ok(())
    }
}
