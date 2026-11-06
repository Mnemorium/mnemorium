use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::port::apply_music_album::{AlbumPlacement, ApplyMusicAlbumCommand};
use crate::domain::alias::NumericID;
use crate::domain::model::configuration::ProviderId;
use crate::domain::model::external_ref::ExternalRef;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::{AlbumResponse, parse_album_id};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Reference to the provider work an apply resolves.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct ApplyReferenceRequest {
    /// Identifier the provider issued.
    #[schema(example = "75b3b57b-5f6f-3c1f-9b7e-9b2f0a2b4b6e")]
    pub id: String,
    /// Provider that issued the identifier (`musicbrainz`).
    #[schema(example = "musicbrainz")]
    pub provider: String,
}

/// One recording the apply places at a destination address.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct ApplyPlacementRequest {
    /// Album-scoped index of the destination medium.
    #[schema(example = 1)]
    pub medium_index: u8,
    /// Identifier of the recording to move.
    #[schema(example = 5)]
    pub recording_id: NumericID,
    /// Index of the destination track within the medium.
    #[schema(example = 1)]
    pub track_index: u8,
}

/// Payload to apply external metadata to a music album.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct ApplyAlbumRequest {
    /// Recordings to move into the applied structure; omit to leave every
    /// placement unchanged.
    pub placements: Option<Vec<ApplyPlacementRequest>>,
    /// Reference to the provider work.
    pub reference: ApplyReferenceRequest,
}

/// Apply external metadata to a music album.
///
/// Only a caller whose `library.manage` scope covers the album may write it.
/// The provider reference is resolved once; the album and its medium structure
/// are written, and only the supplied placements move a recording.
#[utoipa::path(
    post,
    operation_id = "apply_album_metadata",
    path = "/music/album/{album_id}/apply",
    tag = "library",
    params(
        ("album_id" = NumericID, Path, description = "Identifier of the album"),
    ),
    request_body = ApplyAlbumRequest,
    responses(
        (
            status = OK,
            body = AlbumResponse,
            content_type = "application/hal+json",
            description = "Metadata applied"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid album identifier or malformed body"
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
            description = "Unknown or invisible album, or the provider does not know the reference"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The applied metadata duplicates another album, or a placement collides"
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
            description = "The reference, provider, placement or default album cannot be applied"
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unexpected error, or no music metadata provider is configured"
        ),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "Apply external metadata to a music album"
)]
pub(crate) async fn post_album_apply(
    Path(album_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<ApplyAlbumRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let album_id = parse_album_id(&album_id_value)?;
    let Json(request) = payload.map_err(ApiError::from)?;
    let provider = ProviderId::parse(&request.reference.provider)
        .filter(|provider| provider.serves_music())
        .ok_or_else(|| ApiError::reject_enum("provider"))?;
    let reference = ExternalRef::new(provider, request.reference.id);
    let placements = request
        .placements
        .unwrap_or_default()
        .into_iter()
        .map(|placement| {
            AlbumPlacement::new(
                placement.medium_index,
                placement.track_index,
                placement.recording_id,
            )
        })
        .collect();
    let response = state
        .library_use_case_factory()
        .apply_music_album()
        .execute(ApplyMusicAlbumCommand::new(
            caller.user_id(),
            album_id,
            reference,
            placements,
        ))
        .await?;
    Ok(hal_json(AlbumResponse::from(response.album())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::port::apply_music_album::{
        ApplyMusicAlbumError, ApplyMusicAlbumResponse, ApplyMusicAlbumUseCase,
        MockApplyMusicAlbumUseCase,
    };
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::test_helpers::*;
    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::{StatusCode, header};
    use axum::routing::post;
    use rstest::rstest;
    use serde_json::json;
    use std::error::Error;
    use std::sync::Arc;
    use tower::ServiceExt as _;

    /// The reference every test resolves.
    const REFERENCE: &str = "75b3b57b-5f6f-3c1f-9b7e-9b2f0a2b4b6e";

    /// Send a POST to the apply route on behalf of `caller_id`.
    async fn send(
        use_case: MockApplyMusicAlbumUseCase,
        caller_id: NumericID,
        body: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_apply_music_album()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn ApplyMusicAlbumUseCase>);

        let mut request = Request::builder()
            .method("POST")
            .uri("/api/v1/library/music/album/7/apply")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned()))?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/music/album/{album_id}/apply",
                post(post_album_apply),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    /// The payload that resolves [`REFERENCE`] without placements.
    fn payload() -> String {
        json!({
            "reference": { "id": REFERENCE, "provider": "musicbrainz" }
        })
        .to_string()
    }

    #[tokio::test]
    async fn post_album_apply_returns_the_album() -> Result<(), Box<dyn Error>> {
        // Arrange
        let album = sample_album(7, Vec::new())?;
        let mut use_case = MockApplyMusicAlbumUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(move |command| {
                command.caller_id() == 1
                    && command.album_id() == 7
                    && command.reference().provider() == ProviderId::MusicBrainz
                    && command.reference().id() == REFERENCE
                    && command.placements().is_empty()
            })
            .return_once(move |_| {
                let response = ApplyMusicAlbumResponse::new(album.clone());
                Box::pin(async move { Ok(response) })
            });

        // Act
        let (status, _, payload) = into_parts(send(use_case, 1, &payload()).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload.get("id"), Some(&json!(7i64)));
        Ok(())
    }

    #[tokio::test]
    async fn post_album_apply_forwards_the_placements() -> Result<(), Box<dyn Error>> {
        // Arrange
        let album = sample_album(7, Vec::new())?;
        let mut use_case = MockApplyMusicAlbumUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                command.placements().len() == 1
                    && command.placements().first().is_some_and(|placement| {
                        placement.medium_index() == 1
                            && placement.track_index() == 2
                            && placement.recording_id() == 5
                    })
            })
            .return_once(move |_| {
                let response = ApplyMusicAlbumResponse::new(album.clone());
                Box::pin(async move { Ok(response) })
            });
        let body = json!({
            "placements": [{ "medium_index": 1u8, "track_index": 2u8, "recording_id": 5i64 }],
            "reference": { "id": REFERENCE, "provider": "musicbrainz" }
        })
        .to_string();

        // Act
        let response = send(use_case, 1, &body).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }

    #[rstest]
    #[case::default_album(
        ApplyMusicAlbumError::DefaultAlbum,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the default music album cannot be written"
    )]
    #[case::invalid_reference(
        ApplyMusicAlbumError::InvalidReference,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the provider reports a work that cannot be applied to an album"
    )]
    #[case::unknown_reference(
        ApplyMusicAlbumError::UnknownReference,
        StatusCode::NOT_FOUND,
        "the music metadata provider does not know the reference"
    )]
    #[case::conflict(
        ApplyMusicAlbumError::Conflict,
        StatusCode::CONFLICT,
        "the applied metadata conflicts with the stored catalogue"
    )]
    #[tokio::test]
    async fn post_album_apply_error_maps_to_status(
        #[case] error: ApplyMusicAlbumError,
        #[case] status: StatusCode,
        #[case] message: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockApplyMusicAlbumUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));

        // Act
        let response = send(use_case, 1, &payload()).await?;

        // Assert
        assert_eq!(response.status(), status);
        assert_eq!(error_message_of(&response).as_deref(), Some(message));
        Ok(())
    }

    #[tokio::test]
    async fn post_album_apply_unknown_provider_is_unprocessable() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockApplyMusicAlbumUseCase::new();
        let body = json!({
            "reference": { "id": REFERENCE, "provider": "spotify" }
        })
        .to_string();

        // Act
        let response = send(use_case, 1, &body).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[tokio::test]
    async fn post_album_apply_missing_credentials_returns_unauthorized()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("POST")
            .uri("/music/album/7/apply")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(payload()))?;

        // Act
        let response: Response = axum::Router::from(library_routes(&state))
            .oneshot(request)
            .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
