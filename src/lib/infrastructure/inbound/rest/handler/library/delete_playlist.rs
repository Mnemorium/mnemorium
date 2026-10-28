use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse as _;
use axum::response::Response;

use crate::application::port::delete_playlist::DeletePlaylistCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::library::parse_playlist_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Delete a music playlist and its track placements.
///
/// The owner and the Root Admin may delete a playlist. A caller who can read a
/// playlist but may not manage it answers `403 Forbidden`; one that cannot read
/// it answers `404 Not Found`. The shared recordings, audio and files
/// survive.
#[utoipa::path(
    delete,
    operation_id = "delete_playlist",
    path = "/library/music/playlist/{playlist_id}",
    tag = "library",
    params(
        ("playlist_id" = NumericID, Path, description = "Identifier of the playlist"),
    ),
    responses(
        (
            status = NO_CONTENT,
            description = "Playlist deleted"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid playlist identifier"
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
            description = "The caller can read the playlist but may not manage it"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown or invisible playlist"
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
    summary = "Delete a music playlist"
)]
pub async fn delete_playlist(
    Path(playlist_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let playlist_id = parse_playlist_id(&playlist_id_value)?;
    state
        .library_use_case_factory()
        .delete_playlist()
        .execute(DeletePlaylistCommand::new(caller.user_id(), playlist_id))
        .await?;
    Ok(StatusCode::NO_CONTENT.into_response())
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
    use rstest::rstest;
    use tower::ServiceExt as _;

    use super::delete_playlist;
    use crate::application::port::delete_playlist::DeletePlaylistError;
    use crate::application::port::delete_playlist::DeletePlaylistUseCase;
    use crate::application::port::delete_playlist::MockDeletePlaylistUseCase;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;

    async fn send(
        use_case: MockDeletePlaylistUseCase,
        caller_id: NumericID,
        playlist_id: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_delete_playlist()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn DeletePlaylistUseCase>);

        let mut request = Request::builder()
            .method("DELETE")
            .uri(format!("/api/v1/library/music/playlist/{playlist_id}"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/music/playlist/{playlist_id}",
                delete(delete_playlist),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn delete_playlist_owner_returns_no_content() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockDeletePlaylistUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| command.caller_id() == 4 && command.playlist_id() == 3)
            .return_once(|_| Box::pin(async { Ok(()) }));

        // Act
        let response = send(use_case, 4, "3").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        Ok(())
    }

    #[rstest]
    #[case::forbidden(DeletePlaylistError::Forbidden, StatusCode::FORBIDDEN)]
    #[case::no_playlist(DeletePlaylistError::NoSuchPlaylist, StatusCode::NOT_FOUND)]
    #[case::no_caller(DeletePlaylistError::NoSuchCaller, StatusCode::UNAUTHORIZED)]
    #[tokio::test]
    async fn delete_playlist_error_maps_to_status(
        #[case] error: DeletePlaylistError,
        #[case] status: StatusCode,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockDeletePlaylistUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));

        // Act
        let response = send(use_case, 4, "3").await?;

        // Assert
        assert_eq!(response.status(), status);
        Ok(())
    }
}
