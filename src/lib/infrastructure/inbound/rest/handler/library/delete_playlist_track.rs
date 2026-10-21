use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse as _;
use axum::response::Response;

use crate::application::port::remove_playlist_track::RemovePlaylistTrackCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::library::parse_playlist_id;
use crate::infrastructure::inbound::rest::handler::library::parse_playlist_track_index;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Remove one track from a music playlist.
///
/// Any caller who can read the playlist may remove a track. The remaining
/// tracks keep their indices, leaving a gap. The shared recording and its media
/// survive.
#[utoipa::path(
    delete,
    operation_id = "delete_playlist_track",
    path = "/library/music/playlist/{playlist_id}/track/{track_index}",
    tag = "library",
    params(
        ("playlist_id" = NumericID, Path, description = "Identifier of the playlist the track belongs to"),
        ("track_index" = u32, Path, description = "Zero-based index of the track within the playlist"),
    ),
    responses(
        (
            status = NO_CONTENT,
            description = "Track removed"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid playlist identifier or track index"
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
            description = "Unknown or invisible playlist, or unknown track"
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
    summary = "Remove one track of a music playlist"
)]
pub async fn delete_playlist_track(
    Path((playlist_id_value, track_index_value)): Path<(String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let playlist_id = parse_playlist_id(&playlist_id_value)?;
    let track_index = parse_playlist_track_index(&track_index_value)?;
    state
        .library_use_case_factory()
        .remove_playlist_track()
        .execute(RemovePlaylistTrackCommand::new(
            caller.user_id(),
            playlist_id,
            track_index,
        ))
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

    use super::delete_playlist_track;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::remove_playlist_track::MockRemovePlaylistTrackUseCase;
    use crate::application::port::remove_playlist_track::RemovePlaylistTrackError;
    use crate::application::port::remove_playlist_track::RemovePlaylistTrackUseCase;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;

    async fn send(
        use_case: MockRemovePlaylistTrackUseCase,
        caller_id: NumericID,
        playlist_id: &str,
        track_index: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_remove_playlist_track()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn RemovePlaylistTrackUseCase>);

        let mut request = Request::builder()
            .method("DELETE")
            .uri(format!(
                "/api/v1/library/music/playlist/{playlist_id}/track/{track_index}"
            ))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/music/playlist/{playlist_id}/track/{track_index}",
                delete(delete_playlist_track),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn delete_playlist_track_removes_the_track() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockRemovePlaylistTrackUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                command.caller_id() == 4 && command.playlist_id() == 3 && command.track_index() == 0
            })
            .return_once(|_| Box::pin(async { Ok(()) }));

        // Act
        let response = send(use_case, 4, "3", "0").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        Ok(())
    }

    #[rstest]
    #[case::unknown_track(RemovePlaylistTrackError::NoSuchTrack, StatusCode::NOT_FOUND)]
    #[case::invisible(RemovePlaylistTrackError::NoSuchPlaylist, StatusCode::NOT_FOUND)]
    #[case::no_caller(RemovePlaylistTrackError::NoSuchCaller, StatusCode::UNAUTHORIZED)]
    #[tokio::test]
    async fn delete_playlist_track_error_maps_to_status(
        #[case] error: RemovePlaylistTrackError,
        #[case] status: StatusCode,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockRemovePlaylistTrackUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));

        // Act
        let response = send(use_case, 4, "3", "0").await?;

        // Assert
        assert_eq!(response.status(), status);
        Ok(())
    }

    #[rstest]
    #[case::playlist("abc", "0")]
    #[case::index("3", "abc")]
    #[tokio::test]
    async fn delete_playlist_track_malformed_identifier_returns_bad_request(
        #[case] playlist_id: &str,
        #[case] track_index: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockRemovePlaylistTrackUseCase::new();

        // Act
        let response = send(use_case, 4, playlist_id, track_index).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }
}
