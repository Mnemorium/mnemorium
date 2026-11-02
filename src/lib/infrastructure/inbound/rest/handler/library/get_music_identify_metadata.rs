use axum::extract::Path;
use axum::extract::State;
use axum::response::Json;
use serde::Serialize;
use tracing::warn;
use utoipa::ToSchema;

use crate::application::port::resolve_music_metadata::ResolveMusicMetadataCommand;
use crate::application::port::resolve_music_metadata::ResolveMusicMetadataError;
use crate::domain::model::media_candidate::ExternalRef;
use crate::domain::model::media_candidate::ProviderId;
use crate::domain::model::music_album::ProductionType;
use crate::domain::model::music_candidate::MusicTarget;
use crate::domain::model::music_metadata::Edition;
use crate::domain::model::music_metadata::MusicMetadata;
use crate::domain::model::music_query::MusicKind;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::library::ArtistCreditResponse;
use crate::infrastructure::inbound::rest::handler::library::MusicMediumResponse;
use crate::infrastructure::inbound::rest::handler::library::MusicReferenceResponse;
use crate::infrastructure::inbound::rest::handler::library::format_date;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// One edition of an album, as identification reports it.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct MusicEditionResponse {
    /// Country of release as an uppercase ISO 3166-1 alpha-2 code, when known.
    pub country: Option<String>,
    /// Mediums of the edition, in the provider's order.
    pub mediums: Vec<MusicMediumResponse>,
    /// Opaque reference the provider issued for the edition.
    pub reference: MusicReferenceResponse,
    /// Date of release, in `YYYY-MM-DD` form, when the provider reports one.
    pub release_date: Option<String>,
    /// Status the provider reports, when it reports a mappable one.
    pub status: Option<String>,
    /// Title the provider reports for the edition.
    pub title: String,
}

impl MusicEditionResponse {
    /// Build the representation of an edition.
    #[must_use]
    pub fn new(edition: &Edition) -> Self {
        Self {
            country: edition.country().map(str::to_owned),
            mediums: edition
                .mediums()
                .iter()
                .map(MusicMediumResponse::new)
                .collect(),
            reference: MusicReferenceResponse::new(edition.reference()),
            release_date: edition.release_date().map(format_date),
            status: edition.status().map(|status| status.as_wire().to_owned()),
            title: edition.title().to_owned(),
        }
    }
}

/// The provider-independent metadata document of an identified work.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
#[non_exhaustive]
pub enum MusicMetadataResponse {
    /// One album and the editions the provider lists for it.
    Album {
        /// Names the provider credits.
        artist_credits: Vec<ArtistCreditResponse>,
        /// Editions the provider lists, ordered by the edition rule.
        editions: Vec<MusicEditionResponse>,
        /// Date of first release, in `YYYY-MM-DD` form, when known.
        first_release_date: Option<String>,
        /// Genres from the provider's curated vocabulary.
        genres: Vec<String>,
        /// Production type from the catalogue's list, when it maps.
        production_type: Option<String>,
        /// Title the provider reports.
        title: String,
    },
    /// One recording, independent of any release.
    Recording {
        /// Names the provider credits.
        artist_credits: Vec<ArtistCreditResponse>,
        /// Genres from the provider's curated vocabulary.
        genres: Vec<String>,
        /// ISRC codes the provider reports.
        isrcs: Vec<String>,
        /// Title the provider reports.
        title: String,
    },
    /// One edition of an album.
    Release {
        /// Reference to the album the release belongs to, when reported.
        album: Option<MusicReferenceResponse>,
        /// Names the provider credits.
        artist_credits: Vec<ArtistCreditResponse>,
        /// The edition itself: title, date, country, status and mediums.
        edition: MusicEditionResponse,
        /// Genres from the provider's curated vocabulary.
        genres: Vec<String>,
    },
}

impl From<&MusicMetadata> for MusicMetadataResponse {
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "the arms borrow the metadata out of a non-Copy enum matched by reference"
    )]
    fn from(metadata: &MusicMetadata) -> Self {
        match metadata {
            MusicMetadata::Album {
                artist_credits,
                editions,
                first_release_date,
                genres,
                production_type,
                title,
                ..
            } => Self::Album {
                artist_credits: artist_credits
                    .iter()
                    .map(ArtistCreditResponse::new)
                    .collect(),
                editions: editions.iter().map(MusicEditionResponse::new).collect(),
                first_release_date: first_release_date.map(format_date),
                genres: genres.clone(),
                production_type: production_type
                    .map(ProductionType::as_wire)
                    .map(str::to_owned),
                title: title.clone(),
            },
            MusicMetadata::Recording {
                artist_credits,
                genres,
                isrcs,
                title,
            } => Self::Recording {
                artist_credits: artist_credits
                    .iter()
                    .map(ArtistCreditResponse::new)
                    .collect(),
                genres: genres.clone(),
                isrcs: isrcs.clone(),
                title: title.clone(),
            },
            MusicMetadata::Release {
                album,
                artist_credits,
                edition,
                genres,
                ..
            } => Self::Release {
                album: album.as_ref().map(MusicReferenceResponse::new),
                artist_credits: artist_credits
                    .iter()
                    .map(ArtistCreditResponse::new)
                    .collect(),
                edition: MusicEditionResponse::new(edition),
                genres: genres.clone(),
            },
        }
    }
}

/// The metadata document of one identified work, when one is available.
///
/// `available` tells a configured provider apart from no provider at all, so a
/// client does not read an unconfigured server as a provider that could not
/// resolve the reference. Artwork is absent: this surface publishes candidate
/// metadata, and collecting group artwork is a separate capability.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct MusicMetadataDocumentResponse {
    /// Whether a music metadata provider is configured.
    pub available: bool,
    /// The resolved document, absent when no provider is configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<MusicMetadataResponse>,
}

impl MusicMetadataDocumentResponse {
    /// Build the representation of a resolved document.
    #[must_use]
    pub fn new(metadata: &MusicMetadata) -> Self {
        Self {
            available: true,
            metadata: Some(MusicMetadataResponse::from(metadata)),
        }
    }

    /// Build the representation of a surface with no configured provider.
    #[must_use]
    pub fn unavailable() -> Self {
        Self {
            available: false,
            metadata: None,
        }
    }
}

/// Resolve one provider reference into its metadata document.
///
/// The reference names the provider that issued it and the kind of work it
/// denotes; a provider the server does not serve answers `404 Not Found`. The
/// route is a read of the provider, not of the catalogue, and is guarded by
/// `library.read`. With no provider configured it answers an
/// `available: false` document.
#[utoipa::path(
    get,
    operation_id = "resolve_music_metadata",
    path = "/library/music/identify/{provider}/{kind}/{reference}",
    tag = "library",
    params(
        ("provider" = String, Path, description = "Provider that issued the reference"),
        ("kind" = String, Path, description = "Kind of work: album, release or recording"),
        ("reference" = String, Path, description = "Identifier the provider issued"),
    ),
    responses(
        (
            status = OK,
            body = MusicMetadataDocumentResponse,
            content_type = "application/json",
            description = "The provider-independent metadata document"
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
            description = "The provider is not configured, the caller may not read the library, or the provider does not know the reference"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The kind is not one of album, release or recording"
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
    summary = "Resolve a music provider reference into its metadata"
)]
pub async fn get_music_identify_metadata(
    Path((provider_value, kind_value, reference_value)): Path<(String, String, String)>,
    caller: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Json<MusicMetadataDocumentResponse>, ApiError> {
    let provider = ProviderId::parse(&provider_value)
        .filter(|provider| provider.serves_music())
        .ok_or_else(|| {
            warn!(
                target: "security",
                event = "input_validation_failed",
                field = "provider",
                reason = "unknown_provider",
                "rejected an unknown music metadata provider"
            );
            ApiError::NotFound("the music metadata provider is not configured".to_owned())
        })?;
    let kind = MusicKind::parse(&kind_value).ok_or_else(|| ApiError::reject_enum("kind"))?;
    let reference = ExternalRef::new(provider, reference_value);
    let target = match kind {
        MusicKind::Album => MusicTarget::Album(reference),
        MusicKind::Recording => MusicTarget::Recording(reference),
        MusicKind::Release => MusicTarget::Release(reference),
    };

    let result = state
        .library_use_case_factory()
        .resolve_music_metadata()
        .execute(ResolveMusicMetadataCommand::new(caller.user_id(), target))
        .await;
    match result {
        Ok(Some(response)) => Ok(Json(MusicMetadataDocumentResponse::new(
            response.metadata(),
        ))),
        Ok(None) => Err(ApiError::NotFound(
            "the music metadata provider does not know the reference".to_owned(),
        )),
        Err(ResolveMusicMetadataError::Unavailable) => {
            Ok(Json(MusicMetadataDocumentResponse::unavailable()))
        }
        Err(error) => Err(ApiError::from(error)),
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::response::Response;
    use axum::routing::get;
    use chrono::NaiveDate;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::get_music_identify_metadata;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::resolve_music_metadata::MockResolveMusicMetadataUseCase;
    use crate::application::port::resolve_music_metadata::ResolveMusicMetadataError;
    use crate::application::port::resolve_music_metadata::ResolveMusicMetadataResponse;
    use crate::application::port::resolve_music_metadata::ResolveMusicMetadataUseCase;
    use crate::domain::alias::NumericID;
    use crate::domain::model::media_candidate::ExternalRef;
    use crate::domain::model::media_candidate::ProviderId;
    use crate::domain::model::music_album::ProductionType;
    use crate::domain::model::music_metadata::ArtistCredit;
    use crate::domain::model::music_metadata::Edition;
    use crate::domain::model::music_metadata::MediumFormat;
    use crate::domain::model::music_metadata::MediumSummary;
    use crate::domain::model::music_metadata::MusicMetadata;
    use crate::domain::model::music_metadata::ReleaseStatus;
    use crate::domain::model::music_query::MusicKind;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::into_parts;

    /// The reference every test resolves.
    const REFERENCE: &str = "75b3b57b-5f6f-3c1f-9b7e-9b2f0a2b4b6e";

    /// Send a GET to the resolve route on behalf of `caller_id`, injecting the
    /// caller identifier the way the auth middleware does.
    async fn send(
        use_case: MockResolveMusicMetadataUseCase,
        caller_id: NumericID,
        provider: &str,
        kind: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_resolve_music_metadata()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn ResolveMusicMetadataUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!(
                "/api/v1/library/music/identify/{provider}/{kind}/{REFERENCE}"
            ))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/music/identify/{provider}/{kind}/{reference}",
                get(get_music_identify_metadata),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    /// The album document a provider reports for [`REFERENCE`].
    fn album() -> MusicMetadata {
        MusicMetadata::Album {
            artwork: None,
            artist_credits: vec![ArtistCredit::new("Miles Davis".to_owned(), None)],
            editions: vec![Edition::new(
                ExternalRef::new(ProviderId::MusicBrainz, "release-id".to_owned()),
                "Kind of Blue".to_owned(),
                NaiveDate::from_ymd_opt(1959, 8, 17),
                Some("US".to_owned()),
                Some(ReleaseStatus::Official),
                vec![MediumSummary::new(MediumFormat::Cd, 1, 5)],
            )],
            first_release_date: NaiveDate::from_ymd_opt(1959, 8, 17),
            genres: vec!["jazz".to_owned()],
            production_type: Some(ProductionType::Studio),
            title: "Kind of Blue".to_owned(),
        }
    }

    #[tokio::test]
    async fn get_music_identify_metadata_returns_the_document() -> Result<(), Box<dyn Error>> {
        // Arrange
        let expected = album();
        let mut use_case = MockResolveMusicMetadataUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                command.caller_id() == 1
                    && command.target().kind() == MusicKind::Album
                    && command.target().reference().provider() == ProviderId::MusicBrainz
            })
            .return_once(move |_| {
                let copy = expected.clone();
                Box::pin(async move { Ok(Some(ResolveMusicMetadataResponse::new(copy))) })
            });

        // Act
        let (status, content_type, payload) =
            into_parts(send(use_case, 1, "musicbrainz", "album").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/json"));
        assert_eq!(
            payload,
            json!({
                "available": true,
                "metadata": {
                    "kind": "album",
                    "artist_credits": [{ "join_phrase": null, "name": "Miles Davis" }],
                    "editions": [{
                        "country": "US",
                        "mediums": [{ "format": "CD", "index": 1u8, "track_count": 5u32 }],
                        "reference": { "id": "release-id", "provider": "musicbrainz" },
                        "release_date": "1959-08-17",
                        "status": "OFFICIAL",
                        "title": "Kind of Blue"
                    }],
                    "first_release_date": "1959-08-17",
                    "genres": ["jazz"],
                    "production_type": "STUDIO",
                    "title": "Kind of Blue"
                }
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_music_identify_metadata_reports_an_unavailable_provider()
    -> Result<(), Box<dyn Error>> {
        // Arrange: no provider is configured.
        let mut use_case = MockResolveMusicMetadataUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(ResolveMusicMetadataError::Unavailable) }));

        // Act
        let (status, _, payload) =
            into_parts(send(use_case, 1, "musicbrainz", "album").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload, json!({ "available": false }));
        Ok(())
    }

    #[tokio::test]
    async fn get_music_identify_metadata_reports_an_unknown_provider() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the reference names a provider the server does not serve.
        let use_case = MockResolveMusicMetadataUseCase::new();

        // Act
        let response = send(use_case, 1, "spotify", "album").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn get_music_identify_metadata_reports_an_unknown_reference() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the provider does not know the reference.
        let mut use_case = MockResolveMusicMetadataUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Ok(None) }));

        // Act
        let response = send(use_case, 1, "musicbrainz", "album").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn get_music_identify_metadata_conceals_a_denied_caller() -> Result<(), Box<dyn Error>> {
        // Arrange: the use case denies the caller.
        let mut use_case = MockResolveMusicMetadataUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(ResolveMusicMetadataError::NotPermitted) }));

        // Act
        let response = send(use_case, 1, "musicbrainz", "album").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn get_music_identify_metadata_rejects_an_unknown_kind() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockResolveMusicMetadataUseCase::new();

        // Act
        let response = send(use_case, 1, "musicbrainz", "single").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[tokio::test]
    async fn get_music_identify_metadata_missing_credentials_returns_unauthorized()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the real router, wrapped in its `authenticate` layer, with no
        // authorization header.
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("GET")
            .uri(format!("/music/identify/musicbrainz/album/{REFERENCE}"))
            .body(Body::empty())?;

        // Act
        let response: Response = library_routes(&state).oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
