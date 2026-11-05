use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::port::put_album_track::PutAlbumTrackCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::TrackResponse;
use crate::infrastructure::inbound::rest::handler::library::parse_album_id;
use crate::infrastructure::inbound::rest::handler::library::parse_write_index;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to place a recording at a medium track address.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PutAlbumTrackRequest {
    /// Identifier of the recording to place.
    #[schema(example = 5)]
    pub recording_id: NumericID,
}

/// Place or move a recording at a medium track address.
///
/// Only a caller whose `library.manage` scope covers the album may write it.
/// The server removes the recording's previous medium placement and inserts the
/// new one in one transaction; an occupied destination is a conflict.
#[utoipa::path(
    put,
    operation_id = "put_album_track",
    path = "/music/album/{album_id}/medium/{medium_index}/track/{track_index}",
    tag = "library",
    params(
        ("album_id" = NumericID, Path, description = "Identifier of the album"),
        ("medium_index" = u8, Path, description = "Album-scoped index of the medium"),
        ("track_index" = u8, Path, description = "Index of the track within the medium"),
    ),
    request_body = PutAlbumTrackRequest,
    responses(
        (
            status = CREATED,
            body = TrackResponse,
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "URI of the new track placement"),
            ),
            description = "Recording placed"
        ),
        (
            status = OK,
            body = TrackResponse,
            content_type = "application/hal+json",
            description = "Recording moved or already placed"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid album identifier or track address"
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
            description = "The caller may not manage the library"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown or invisible album, or unknown medium"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The destination track holds another recording"
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
            description = "The recording is unknown, an index is out of range, or the album is the default one"
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
    summary = "Place or move a recording at a medium track address"
)]
pub(crate) async fn put_album_track(
    Path((album_id_value, medium_index_value, track_index_value)): Path<(String, String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PutAlbumTrackRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let album_id = parse_album_id(&album_id_value)?;
    let medium_index = parse_write_index(
        &medium_index_value,
        "medium_index",
        "invalid music medium index",
    )?;
    let track_index = parse_write_index(
        &track_index_value,
        "track_index",
        "invalid music track index",
    )?;
    let Json(request) = payload.map_err(ApiError::from)?;
    let response = state
        .library_use_case_factory()
        .put_album_track()
        .execute(PutAlbumTrackCommand::new(
            caller.user_id(),
            album_id,
            medium_index,
            track_index,
            request.recording_id,
        ))
        .await?;
    let body = TrackResponse::from_album(
        response.album(),
        response.medium_index(),
        response.track_index(),
    )
    .ok_or(ApiError::InternalServerError)?;
    let location = body.links.self_link.href.clone();
    let mut http_response = hal_json(body);
    if response.created() {
        *http_response.status_mut() = StatusCode::CREATED;
        http_response.headers_mut().insert(
            header::LOCATION,
            HeaderValue::from_str(&location).map_err(|_| ApiError::InternalServerError)?,
        );
    }
    Ok(http_response)
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
    use axum::routing::put;
    use rstest::rstest;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::put_album_track;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::put_album_track::MockPutAlbumTrackUseCase;
    use crate::application::port::put_album_track::PutAlbumTrackError;
    use crate::application::port::put_album_track::PutAlbumTrackResponse;
    use crate::application::port::put_album_track::PutAlbumTrackUseCase;
    use crate::domain::alias::NumericID;
    use crate::domain::model::music_album::MusicAlbum;
    use crate::domain::model::music_medium::MediumIndex;
    use crate::domain::model::music_medium::MusicMedium;
    use crate::domain::model::music_recording::MusicRecording;
    use crate::domain::model::music_recording::TrackIndex;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_parts;
    use crate::test_helpers::sample_album;

    /// Send a PUT to the track route on behalf of `caller_id`.
    async fn send(
        use_case: MockPutAlbumTrackUseCase,
        caller_id: NumericID,
        address: (&str, &str, &str),
        body: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_put_album_track()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn PutAlbumTrackUseCase>);

        let mut request = Request::builder()
            .method("PUT")
            .uri(format!(
                "/api/v1/library/music/album/{}/medium/{}/track/{}",
                address.0, address.1, address.2
            ))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned()))?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/music/album/{album_id}/medium/{medium_index}/track/{track_index}",
                put(put_album_track),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    /// The album holding one `CD` medium with the recording at index 1.
    fn album() -> Result<MusicAlbum, Box<dyn Error>> {
        let recording = MusicRecording::try_new(
            5,
            Some("So What".to_owned()),
            None,
            1,
            2,
            None,
            TrackIndex::MediumIndex(1),
        )?;
        sample_album(
            7,
            vec![MusicMedium::new(MediumIndex::Cd(1), vec![recording])],
        )
    }

    #[tokio::test]
    async fn put_album_track_places_the_recording() -> Result<(), Box<dyn Error>> {
        // Arrange
        let stored = album()?;
        let mut use_case = MockPutAlbumTrackUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                command.album_id() == 7
                    && command.medium_index() == 1
                    && command.track_index() == 1
                    && command.recording_id() == 5
            })
            .return_once(move |_| {
                let response = PutAlbumTrackResponse::new(stored.clone(), 1, 1, true);
                Box::pin(async move { Ok(response) })
            });

        // Act
        let (status, _, payload) =
            into_parts(send(use_case, 1, ("7", "1", "1"), r#"{"recording_id":5}"#).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(payload.get("recording_id"), Some(&json!(5i64)));
        assert_eq!(
            payload.pointer("/_links/self/href"),
            Some(&json!("/api/v1/library/music/album/7/medium/1/track/1"))
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_album_track_move_returns_ok() -> Result<(), Box<dyn Error>> {
        // Arrange
        let stored = album()?;
        let mut use_case = MockPutAlbumTrackUseCase::new();
        use_case.expect_execute().times(1).return_once(move |_| {
            let response = PutAlbumTrackResponse::new(stored.clone(), 1, 1, false);
            Box::pin(async move { Ok(response) })
        });

        // Act
        let response = send(use_case, 1, ("7", "1", "1"), r#"{"recording_id":5}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }

    #[rstest]
    #[case::occupied(
        PutAlbumTrackError::Conflict,
        StatusCode::CONFLICT,
        "the destination track already holds another recording"
    )]
    #[case::unknown_recording(
        PutAlbumTrackError::NoSuchRecording,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a music recording with this identifier does not exist"
    )]
    #[case::unknown_medium(
        PutAlbumTrackError::NoSuchMedium,
        StatusCode::NOT_FOUND,
        "a music medium with this index does not exist"
    )]
    #[case::unknown_album(
        PutAlbumTrackError::NoSuchAlbum,
        StatusCode::NOT_FOUND,
        "a music album with this identifier does not exist"
    )]
    #[tokio::test]
    async fn put_album_track_error_maps_to_status(
        #[case] error: PutAlbumTrackError,
        #[case] status: StatusCode,
        #[case] message: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockPutAlbumTrackUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));

        // Act
        let response = send(use_case, 1, ("7", "1", "1"), r#"{"recording_id":5}"#).await?;

        // Assert
        assert_eq!(response.status(), status);
        assert_eq!(error_message_of(&response).as_deref(), Some(message));
        Ok(())
    }

    #[tokio::test]
    async fn put_album_track_out_of_range_index_is_unprocessable() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockPutAlbumTrackUseCase::new();

        // Act
        let response = send(use_case, 1, ("7", "1", "256"), r#"{"recording_id":5}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[tokio::test]
    async fn put_album_track_missing_credentials_returns_unauthorized() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("PUT")
            .uri("/music/album/7/medium/1/track/1")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"recording_id":5}"#))?;

        // Act
        let response: Response = axum::Router::from(library_routes(&state))
            .oneshot(request)
            .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
