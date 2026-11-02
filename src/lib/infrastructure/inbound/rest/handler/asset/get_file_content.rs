use axum::extract::Path;
use axum::extract::State;
use axum::response::Response;

use crate::application::port::get_asset_content::AssetTarget;
use crate::application::port::get_asset_content::GetAssetContentCommand;
use crate::application::port::get_asset_content::GetAssetContentError;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::asset::asset_content_response;
use crate::infrastructure::inbound::rest::handler::asset::parse_asset_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Map an asset-content error to its API error.
impl From<GetAssetContentError> for ApiError {
    fn from(err: GetAssetContentError) -> Self {
        match err {
            GetAssetContentError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetAssetContentError::NoSuchAsset => Self::NotFound(err.to_string()),
            GetAssetContentError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Read the bytes of a stored file.
///
/// The file is authorized before any byte is read, so an unreadable file is
/// `404 Not Found` with no content, indistinguishable from an unknown one
/// (`API-044`).
#[utoipa::path(
    get,
    operation_id = "get_file_content",
    path = "/asset/file/{file_id}/content",
    tag = "asset",
    params(
        ("file_id" = NumericID, Path, description = "Identifier of the file whose content is read"),
    ),
    responses(
        (
            status = OK,
            body = Vec<u8>,
            content_type = "application/octet-stream",
            description = "Stored bytes of the file"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid file identifier"
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
            description = "Unknown or unreadable file"
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
    summary = "Read the content of a stored file"
)]
pub async fn get_file_content(
    Path(file_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let file_id = parse_asset_id(&file_id_value, "file_id", "invalid file identifier")?;
    let response = state
        .asset_use_case_factory()
        .get_asset_content()
        .execute(GetAssetContentCommand::new(
            caller.user_id(),
            AssetTarget::File(file_id),
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

    use super::get_file_content;
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

    /// Send a GET to `/api/v1/asset/file/{file_id}/content` on behalf of
    /// `caller_id`.
    async fn send(
        use_case: MockGetAssetContentUseCase,
        caller_id: NumericID,
        file_id: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockAssetUseCaseFactory::new();
        factory
            .expect_get_asset_content()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn GetAssetContentUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/asset/file/{file_id}/content"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/asset/file/{file_id}/content",
                get(get_file_content),
            )
            .with_state(app_state_with_asset(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn get_file_content_returns_the_bytes_and_media_type() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetAssetContentUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(GetAssetContentCommand::new(3, AssetTarget::File(100))))
            .return_once(|_| {
                Box::pin(async {
                    Ok(GetAssetContentResponse::new(
                        "image/png".to_owned(),
                        vec![1, 2, 3],
                    ))
                })
            });

        // Act
        let response = send(use_case, 3, "100").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("image/png")
        );
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        assert_eq!(bytes.as_ref(), &[1, 2, 3]);
        Ok(())
    }

    #[tokio::test]
    async fn get_file_content_of_an_unreadable_file_returns_not_found() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut use_case = MockGetAssetContentUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(GetAssetContentError::NoSuchAsset) }));

        // Act
        let response = send(use_case, 3, "100").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        assert!(
            bytes.is_empty(),
            "an unreadable asset must not serve any bytes"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_file_content_malformed_identifier_returns_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockGetAssetContentUseCase::new();

        // Act
        let response = send(use_case, 3, "abc").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }
}
