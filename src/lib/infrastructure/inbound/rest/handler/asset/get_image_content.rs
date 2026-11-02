use axum::extract::Path;
use axum::extract::State;
use axum::response::Response;

use crate::application::port::get_asset_content::AssetTarget;
use crate::application::port::get_asset_content::GetAssetContentCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::asset::asset_content_response;
use crate::infrastructure::inbound::rest::handler::asset::parse_asset_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Read the bytes of a typed image.
///
/// The image is authorized through the resource that owns it before any byte is
/// read; an unreadable image is `404 Not Found` with no content (`API-044`).
#[utoipa::path(
    get,
    operation_id = "get_image_content",
    path = "/asset/image/{image_id}/content",
    tag = "asset",
    params(
        ("image_id" = NumericID, Path, description = "Identifier of the image whose content is read"),
    ),
    responses(
        (
            status = OK,
            body = Vec<u8>,
            content_type = "application/octet-stream",
            description = "Stored bytes of the image"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid image identifier"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown or unreadable image"
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
    summary = "Read the content of a typed image"
)]
pub async fn get_image_content(
    Path(image_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let image_id = parse_asset_id(&image_id_value, "image_id", "invalid image identifier")?;
    let response = state
        .asset_use_case_factory()
        .get_asset_content()
        .execute(GetAssetContentCommand::new(
            caller.user_id(),
            AssetTarget::Image(image_id),
        ))
        .await?;
    Ok(asset_content_response(
        response.mime_type_id(),
        response.bytes().to_vec(),
    ))
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
    use axum::routing::get;
    use mockall::predicate::eq;
    use tower::ServiceExt as _;

    use super::get_image_content;
    use crate::application::port::asset_use_case_factory::MockAssetUseCaseFactory;
    use crate::application::port::get_asset_content::AssetTarget;
    use crate::application::port::get_asset_content::GetAssetContentCommand;
    use crate::application::port::get_asset_content::GetAssetContentError;
    use crate::application::port::get_asset_content::GetAssetContentResponse;
    use crate::application::port::get_asset_content::GetAssetContentUseCase;
    use crate::application::port::get_asset_content::MockGetAssetContentUseCase;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_asset;

    /// Send a GET to `/api/v1/asset/image/{image_id}/content` on behalf of
    /// `caller_id`.
    async fn send(
        use_case: MockGetAssetContentUseCase,
        caller_id: NumericID,
        image_id: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockAssetUseCaseFactory::new();
        factory
            .expect_get_asset_content()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn GetAssetContentUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/asset/image/{image_id}/content"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/asset/image/{image_id}/content",
                get(get_image_content),
            )
            .with_state(app_state_with_asset(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn get_image_content_returns_the_bytes_and_image_type() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetAssetContentUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(GetAssetContentCommand::new(3, AssetTarget::Image(19))))
            .return_once(|_| {
                Box::pin(async {
                    Ok(GetAssetContentResponse::new(
                        "image/webp".to_owned(),
                        vec![0x52, 0x49],
                    ))
                })
            });

        // Act
        let response = send(use_case, 3, "19").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("image/webp")
        );
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        assert_eq!(bytes.as_ref(), &[0x52, 0x49]);
        Ok(())
    }

    #[tokio::test]
    async fn get_image_content_of_an_unreadable_image_returns_not_found()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetAssetContentUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(GetAssetContentError::NoSuchAsset) }));

        // Act
        let response = send(use_case, 3, "19").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }
}
