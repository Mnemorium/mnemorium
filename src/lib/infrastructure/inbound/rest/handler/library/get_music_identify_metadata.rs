use axum::extract::{Path, State};
use axum::response::Response;
use serde::Serialize;
use tracing::warn;
use utoipa::ToSchema;

use crate::application::port::{ResolveMusicMetadataCommand, ResolveMusicMetadataError};
use crate::domain::model::{
    Edition, ExternalRef, MusicKind, MusicMetadata, MusicTarget, ProductionType, ProviderId,
};
use crate::infrastructure::inbound::rest::handler::library::{
    ArtistCreditResponse, MusicMediumResponse, MusicReferenceResponse, format_date,
};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, SelfLinks, hal_json};

/// One edition of an album, as identification reports it.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
struct MusicEditionResponse {
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
    pub(crate) fn new(edition: &Edition) -> Self {
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
enum MusicMetadataResponse {
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
/// resolve the reference. The `self` link addresses the resolve view the
/// representation reports. Artwork is absent: this surface publishes candidate
/// metadata, and collecting group artwork is a separate capability.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
struct MusicMetadataDocumentResponse {
    /// Whether a music metadata provider is configured.
    pub available: bool,
    /// Link to the resolve view the representation reports.
    #[serde(rename = "_links")]
    pub links: SelfLinks,
    /// The resolved document, absent when no provider is configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<MusicMetadataResponse>,
}

impl MusicMetadataDocumentResponse {
    /// Build the representation of a resolved document over `href`.
    #[must_use]
    pub(crate) fn new(href: &str, metadata: &MusicMetadata) -> Self {
        Self {
            links: SelfLinks::new(href),
            available: true,
            metadata: Some(MusicMetadataResponse::from(metadata)),
        }
    }

    /// Build the representation of a surface with no configured provider.
    #[must_use]
    pub(crate) fn unavailable(href: &str) -> Self {
        Self {
            links: SelfLinks::new(href),
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
    path = "/music/identify/{provider}/{kind}/{reference}",
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
            content_type = "application/hal+json",
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
pub(crate) async fn get_music_identify_metadata(
    Path((provider_value, kind_value, reference_value)): Path<(String, String, String)>,
    caller: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Response, ApiError> {
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
    let href = format!(
        "/api/v1/library/music/identify/{}/{}/{reference_value}",
        provider.as_str(),
        kind.as_wire()
    );
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
        Ok(Some(response)) => Ok(hal_json(MusicMetadataDocumentResponse::new(
            &href,
            response.metadata(),
        ))),
        Ok(None) => Err(ApiError::NotFound(
            "the music metadata provider does not know the reference".to_owned(),
        )),
        Err(ResolveMusicMetadataError::Unavailable) => {
            Ok(hal_json(MusicMetadataDocumentResponse::unavailable(&href)))
        }
        Err(error) => Err(ApiError::from(error)),
    }
}
