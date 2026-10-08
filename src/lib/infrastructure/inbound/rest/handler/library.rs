pub mod delete_gallery;
pub mod delete_gallery_item;
pub mod get_gallery;
pub mod get_gallery_item;
pub mod get_gallery_item_list;
pub mod get_gallery_list;
pub mod links;
pub mod post_gallery;
pub mod post_gallery_item;
pub mod representation;

use axum::Router;
use axum::middleware;
use axum::routing::get;

use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::library::delete_gallery::delete_gallery;
use crate::infrastructure::inbound::rest::handler::library::delete_gallery_item::delete_gallery_item;
use crate::infrastructure::inbound::rest::handler::library::get_gallery::get_gallery;
use crate::infrastructure::inbound::rest::handler::library::get_gallery_item::get_gallery_item;
use crate::infrastructure::inbound::rest::handler::library::get_gallery_item_list::get_gallery_item_list;
use crate::infrastructure::inbound::rest::handler::library::get_gallery_list::get_gallery_list;
use crate::infrastructure::inbound::rest::handler::library::post_gallery::post_gallery;
use crate::infrastructure::inbound::rest::handler::library::post_gallery_item::post_gallery_item;
use crate::infrastructure::inbound::rest::middleware::auth::authenticate;
use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;

/// Canonical URI reference of the gallery resource identified by `gallery_id`.
///
/// Shared by the gallery representations and the creation response so the
/// `_links.self` target and the `Location` header cannot drift (`API-032`).
#[expect(
    clippy::single_call_fn,
    reason = "centralising the canonical URI so `_links.self` and `Location` cannot drift is the point"
)]
#[must_use]
pub(crate) fn gallery_self_href(gallery_id: NumericID) -> String {
    format!("/api/v1/library/gallery/{gallery_id}")
}

/// Canonical URI reference of the item collection of `gallery_id`.
///
/// This is the alias path of `GET /library/gallery/{id}/item`; the `_links.self`
/// of the collection representation keeps pointing at the canonical gallery
/// resource (`API-042`).
#[expect(
    clippy::single_call_fn,
    reason = "centralising the items URI keeps the gallery links discoverable from one place (`API-037`)"
)]
#[must_use]
pub(crate) fn gallery_items_href(gallery_id: NumericID) -> String {
    format!("/api/v1/library/gallery/{gallery_id}/item")
}

/// Canonical URI reference of the gallery item identified by `item_id`.
#[must_use]
pub(crate) fn gallery_item_self_href(gallery_id: NumericID, item_id: NumericID) -> String {
    format!("/api/v1/library/gallery/{gallery_id}/item/{item_id}")
}

/// Routes of the Library bounded context.
///
/// Every route requires an authenticated caller.
pub fn library_routes(state: &AppState) -> Router {
    let gallery: Router<AppState> = Router::new()
        .route("/gallery", get(get_gallery_list).post(post_gallery))
        .route("/gallery/{id}", get(get_gallery).delete(delete_gallery))
        .route(
            "/gallery/{id}/item",
            get(get_gallery_item_list).post(post_gallery_item),
        )
        .route(
            "/gallery/{id}/item/{item_id}",
            get(get_gallery_item).delete(delete_gallery_item),
        );

    gallery
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            authenticate::<JwtTokenProvider>,
        ))
        .with_state(state.clone())
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
    use chrono::NaiveDate;
    use chrono::NaiveDateTime;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::library_routes;
    use crate::application::port::create_gallery::CreateGalleryResponse;
    use crate::application::port::create_gallery::CreateGalleryUseCase;
    use crate::application::port::create_gallery::MockCreateGalleryUseCase;
    use crate::application::port::delete_gallery::DeleteGalleryUseCase;
    use crate::application::port::delete_gallery::MockDeleteGalleryUseCase;
    use crate::application::port::get_gallery::GetGalleryResponse;
    use crate::application::port::get_gallery::GetGalleryUseCase;
    use crate::application::port::get_gallery::MockGetGalleryUseCase;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::domain::port::token_provider::TokenProvider as _;
    use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;
    use crate::test_helpers::TEST_JWT_SECRET;
    use crate::test_helpers::app_state_with_library;

    /// A fixed instant the fixtures timestamp with.
    fn ts() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 6)
            .unwrap_or_default()
            .and_hms_opt(12, 0, 0)
            .unwrap_or_default()
    }

    /// Mint a bearer token the real `authenticate` layer accepts.
    async fn bearer() -> Result<String, Box<dyn Error>> {
        let provider = JwtTokenProvider::new(TEST_JWT_SECRET.to_owned(), 3600);
        Ok(provider.issue(1).await?.value().to_owned())
    }

    #[tokio::test]
    async fn library_routes_gallery_route_reaches_handler() -> Result<(), Box<dyn Error>> {
        // Arrange: the real router, wrapped in its `authenticate` layer.
        let mut get_gallery = MockGetGalleryUseCase::new();
        get_gallery.expect_execute().times(1).return_once(|_| {
            Box::pin(async {
                Ok(GetGalleryResponse::new(
                    3,
                    Some(1),
                    "Holidays".to_owned(),
                    false,
                    ts(),
                    ts(),
                    Vec::new(),
                ))
            })
        });
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_get_gallery()
            .times(1)
            .return_once(move || Arc::new(get_gallery) as Arc<dyn GetGalleryUseCase>);
        let state = app_state_with_library(Arc::new(factory))?;
        let request = Request::builder()
            .method("GET")
            .uri("/gallery/3")
            .header(header::AUTHORIZATION, format!("Bearer {}", bearer().await?))
            .body(Body::empty())?;

        // Act
        let response: Response = library_routes(&state).oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn library_routes_create_route_reaches_handler() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut create_gallery = MockCreateGalleryUseCase::new();
        create_gallery.expect_execute().times(1).return_once(|_| {
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
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_create_gallery()
            .times(1)
            .return_once(move || Arc::new(create_gallery) as Arc<dyn CreateGalleryUseCase>);
        let state = app_state_with_library(Arc::new(factory))?;
        let request = Request::builder()
            .method("POST")
            .uri("/gallery")
            .header(header::AUTHORIZATION, format!("Bearer {}", bearer().await?))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({ "name": "Holidays" }).to_string()))?;

        // Act
        let response: Response = library_routes(&state).oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::CREATED);
        Ok(())
    }

    #[tokio::test]
    async fn library_routes_delete_route_reaches_handler() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut delete_gallery = MockDeleteGalleryUseCase::new();
        delete_gallery
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Ok(()) }));
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_delete_gallery()
            .times(1)
            .return_once(move || Arc::new(delete_gallery) as Arc<dyn DeleteGalleryUseCase>);
        let state = app_state_with_library(Arc::new(factory))?;
        let request = Request::builder()
            .method("DELETE")
            .uri("/gallery/3")
            .header(header::AUTHORIZATION, format!("Bearer {}", bearer().await?))
            .body(Body::empty())?;

        // Act
        let response: Response = library_routes(&state).oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        Ok(())
    }
}
