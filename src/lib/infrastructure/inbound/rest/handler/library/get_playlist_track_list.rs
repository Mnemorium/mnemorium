use axum::extract::Path;
use axum::extract::State;
use axum::response::Response;

use crate::application::port::get_playlist::GetPlaylistCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::PlaylistResponse;
use crate::infrastructure::inbound::rest::handler::library::parse_playlist_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Fetch one music playlist through the collection alias.
///
/// This is a literal alias of `GET /library/music/playlist/{playlist_id}`: it
/// returns the same playlist representation with its tracks embedded, and
/// `_links.self` keeps naming the canonical playlist URI (`API-042`). The
/// playlist's `tracks` link points here.
#[utoipa::path(
    get,
    operation_id = "get_playlist_track_list",
    path = "/library/music/playlist/{playlist_id}/track",
    tag = "library",
    params(
        ("playlist_id" = NumericID, Path, description = "Identifier of the playlist"),
    ),
    responses(
        (
            status = OK,
            body = PlaylistResponse,
            content_type = "application/hal+json",
            description = "Playlist found with its tracks embedded"
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
    summary = "Fetch a music playlist's tracks"
)]
pub async fn get_playlist_track_list(
    Path(playlist_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let playlist_id = parse_playlist_id(&playlist_id_value)?;
    let response = state
        .library_use_case_factory()
        .get_playlist()
        .execute(GetPlaylistCommand::new(caller.user_id(), playlist_id))
        .await?;
    Ok(hal_json(PlaylistResponse::new(response.playlist())))
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::routing::get;
    use chrono::NaiveDate;
    use chrono::NaiveDateTime;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::get_playlist_track_list;
    use crate::application::port::get_playlist::GetPlaylistResponse;
    use crate::application::port::get_playlist::GetPlaylistUseCase;
    use crate::application::port::get_playlist::MockGetPlaylistUseCase;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::domain::model::music_playlist::MusicPlaylist;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::into_parts;

    fn ts() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 6)
            .unwrap_or_default()
            .and_hms_opt(12, 0, 0)
            .unwrap_or_default()
    }

    #[tokio::test]
    async fn get_playlist_track_list_returns_the_playlist_with_a_canonical_self()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let fetched = GetPlaylistResponse::new(MusicPlaylist::try_new(
            3,
            "Mix".to_owned(),
            ts(),
            ts(),
            4,
            true,
            Vec::new(),
        )?);
        let mut use_case = MockGetPlaylistUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| command.caller_id() == 4 && command.playlist_id() == 3)
            .return_once(move |_| Box::pin(async move { Ok(fetched) }));
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_get_playlist()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn GetPlaylistUseCase>);
        let mut request = Request::builder()
            .method("GET")
            .uri("/api/v1/library/music/playlist/3/track")
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(4i64));
        let router = axum::Router::new()
            .route(
                "/api/v1/library/music/playlist/{playlist_id}/track",
                get(get_playlist_track_list),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);

        // Act
        let (status, _, payload) = into_parts(router.oneshot(request).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            payload
                .get("_links")
                .and_then(|links| links.get("self"))
                .and_then(|link| link.get("href")),
            Some(&json!("/api/v1/library/music/playlist/3")),
            "the alias must keep the canonical playlist self link (`API-042`)"
        );
        Ok(())
    }
}
