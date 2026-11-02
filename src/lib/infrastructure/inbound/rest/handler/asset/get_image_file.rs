use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::IntoResponse as _;
use axum::response::Response;

use crate::application::port::get_image::GetImageCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::asset::asset_file_href;
use crate::infrastructure::inbound::rest::handler::asset::parse_asset_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Redirect the typed image to its backing file.
///
/// The alias authorizes exactly like the image representation and then answers
/// `303 See Other`, so the client continues under the canonical file resource
/// (`design.md`, "One file resource, typed views over it").
#[utoipa::path(
    get,
    operation_id = "get_image_file",
    path = "/asset/image/{image_id}/file",
    tag = "asset",
    params(
        ("image_id" = NumericID, Path, description = "Identifier of the image whose file is addressed"),
    ),
    responses(
        (
            status = SEE_OTHER,
            description = "Redirect to the stored file resource",
            headers(
                ("Location" = String, description = "Canonical URI of the stored file"),
            )
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
    summary = "Redirect an image to its stored file"
)]
pub async fn get_image_file(
    Path(image_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let image_id = parse_asset_id(&image_id_value, "image_id", "invalid image identifier")?;
    let response = state
        .asset_use_case_factory()
        .get_image()
        .execute(GetImageCommand::new(caller.user_id(), image_id))
        .await?;
    Ok((
        StatusCode::SEE_OTHER,
        [(header::LOCATION, asset_file_href(response.file_id()))],
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::response::Response;
    use axum::routing::get;
    use chrono::NaiveDate;
    use mockall::predicate::eq;
    use tower::ServiceExt as _;

    use super::get_image_file;
    use crate::application::port::asset_use_case_factory::MockAssetUseCaseFactory;
    use crate::application::port::get_image::GetImageCommand;
    use crate::application::port::get_image::GetImageError;
    use crate::application::port::get_image::GetImageResponse as GetImageResponseData;
    use crate::application::port::get_image::GetImageUseCase;
    use crate::application::port::get_image::MockGetImageUseCase;
    use crate::domain::alias::NumericID;
    use crate::domain::model::image::Orientation;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_asset;

    /// Send a GET to `/api/v1/asset/image/{image_id}/file` on behalf of
    /// `caller_id`.
    async fn send(
        use_case: MockGetImageUseCase,
        caller_id: NumericID,
        image_id: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockAssetUseCaseFactory::new();
        factory
            .expect_get_image()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn GetImageUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/asset/image/{image_id}/file"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/asset/image/{image_id}/file", get(get_image_file))
            .with_state(app_state_with_asset(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn get_image_file_redirects_to_the_canonical_file_uri() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetImageUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(GetImageCommand::new(3, 19)))
            .return_once(|_| {
                Box::pin(async {
                    Ok(GetImageResponseData::new(
                        19,
                        "cover.png".to_owned(),
                        10,
                        20,
                        Orientation::Portrait,
                        NaiveDate::default(),
                        100,
                    ))
                })
            });

        // Act
        let response = send(use_case, 3, "19").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some("/api/v1/asset/file/100")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_image_file_unknown_image_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetImageUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(GetImageError::NoSuchImage) }));

        // Act
        let response = send(use_case, 3, "19").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }
}
