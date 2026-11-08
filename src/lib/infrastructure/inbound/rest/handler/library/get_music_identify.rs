use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use tracing::warn;
use utoipa::{IntoParams, ToSchema};

use crate::application::port::identify_music::{IdentifyMusicCommand, IdentifyMusicError};
use crate::domain::model::music_candidate::MusicCandidate;
use crate::domain::model::music_query::{MusicKind, MusicQuery};
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::{SelfLinks, hal_json};
use crate::infrastructure::inbound::rest::handler::library::{
    MusicMediumResponse, MusicReferenceResponse,
};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Kind of music work to search for.
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub(crate) enum MusicKindRequest {
    /// The abstract album, independent of any one edition.
    Album,
    /// One performance, independent of the release it appears on.
    Recording,
    /// One edition of an album.
    Release,
}

impl From<MusicKindRequest> for MusicKind {
    fn from(kind: MusicKindRequest) -> Self {
        match kind {
            MusicKindRequest::Album => Self::Album,
            MusicKindRequest::Recording => Self::Recording,
            MusicKindRequest::Release => Self::Release,
        }
    }
}

/// Evidence a caller knows about the music work it wants identified.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[non_exhaustive]
pub(crate) struct IdentifyMusicQuery {
    /// Group the caller knows, when any.
    pub group: Option<String>,
    /// Kind of work to search for: `album`, `release` or `recording`.
    #[param(inline)]
    pub kind: MusicKindRequest,
    /// Title to search for; never empty.
    #[param(min_length = 1)]
    pub title: String,
    /// Release or first-release year, when the caller knows one.
    #[param(maximum = 4_294_967_295u64)]
    pub year: Option<u64>,
}

/// One candidate the provider reported, shaped by the kind of work it denotes.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
#[non_exhaustive]
enum MusicCandidateResponse {
    /// The abstract album, independent of any one edition.
    Album {
        /// Group the provider reports, when it reports one.
        group: Option<String>,
        /// Opaque reference the provider issued for the album.
        reference: MusicReferenceResponse,
        /// Title the provider reports.
        title: String,
        /// Release or first-release year, when the provider reports one.
        year: Option<u32>,
    },
    /// One performance, independent of the release it appears on.
    Recording {
        /// Group the provider reports, when it reports one.
        group: Option<String>,
        /// Opaque reference the provider issued for the recording.
        reference: MusicReferenceResponse,
        /// Title the provider reports.
        title: String,
        /// Release or first-release year, when the provider reports one.
        year: Option<u32>,
    },
    /// One edition of an album.
    Release {
        /// Reference to the album the release belongs to, when reported.
        album: Option<MusicReferenceResponse>,
        /// Group the provider reports, when it reports one.
        group: Option<String>,
        /// Mediums of the release, in the provider's order.
        mediums: Vec<MusicMediumResponse>,
        /// Opaque reference the provider issued for the release.
        reference: MusicReferenceResponse,
        /// Status the provider reports, when it reports a mappable one.
        status: Option<String>,
        /// Title the provider reports.
        title: String,
        /// Release year, when the provider reports one.
        year: Option<u32>,
    },
}

impl From<&MusicCandidate> for MusicCandidateResponse {
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "the arms borrow the candidate out of a non-Copy enum matched by reference"
    )]
    fn from(candidate: &MusicCandidate) -> Self {
        match candidate {
            MusicCandidate::Album {
                group,
                reference,
                title,
                year,
            } => Self::Album {
                group: group.clone(),
                reference: MusicReferenceResponse::new(reference),
                title: title.clone(),
                year: *year,
            },
            MusicCandidate::Recording {
                group,
                reference,
                title,
                year,
            } => Self::Recording {
                group: group.clone(),
                reference: MusicReferenceResponse::new(reference),
                title: title.clone(),
                year: *year,
            },
            MusicCandidate::Release {
                album,
                group,
                mediums,
                reference,
                status,
                title,
                year,
            } => Self::Release {
                album: album.as_ref().map(MusicReferenceResponse::new),
                group: group.clone(),
                mediums: mediums.iter().map(MusicMediumResponse::new).collect(),
                reference: MusicReferenceResponse::new(reference),
                status: status.map(|value| value.as_wire().to_owned()),
                title: title.clone(),
                year: *year,
            },
        }
    }
}

/// Candidates the configured provider reported, in its order.
///
/// `available` tells a configured provider apart from no provider at all: an
/// empty `items` list is a search that ran and found nothing, while
/// `available: false` is a surface the server cannot offer. The `self` link
/// addresses the search view the representation reports.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
struct MusicCandidateListResponse {
    /// Whether a music metadata provider is configured.
    pub available: bool,
    /// Candidates the provider reported, in its order.
    pub items: Vec<MusicCandidateResponse>,
    /// Link to the search view the representation reports.
    #[serde(rename = "_links")]
    pub links: SelfLinks,
}

impl MusicCandidateListResponse {
    /// Build the representation of a completed search over `href`.
    #[must_use]
    pub(crate) fn new(href: &str, candidates: &[MusicCandidate]) -> Self {
        Self {
            links: SelfLinks::new(href),
            available: true,
            items: candidates
                .iter()
                .map(MusicCandidateResponse::from)
                .collect(),
        }
    }

    /// Build the representation of a surface with no configured provider.
    #[must_use]
    pub(crate) fn unavailable(href: &str) -> Self {
        Self {
            links: SelfLinks::new(href),
            available: false,
            items: Vec::new(),
        }
    }
}

/// Search the configured music metadata provider.
///
/// The route is a read of the provider, not of the catalogue: it is guarded by
/// `library.read`, returns the provider's candidates in its order and never
/// ranks them. With no provider configured it answers an `available: false`
/// document, which a client tells apart from a search that found nothing.
#[utoipa::path(
    get,
    operation_id = "identify_music",
    path = "/music/identify",
    tag = "library",
    params(IdentifyMusicQuery),
    responses(
        (
            status = OK,
            body = MusicCandidateListResponse,
            content_type = "application/hal+json",
            description = "Candidates the configured provider reported, in its order"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Malformed query string"
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
            description = "The caller may not read the library"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The kind is not one of album, release or recording, or the title is empty"
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
    summary = "Search the configured music metadata provider"
)]
pub(crate) async fn get_music_identify(
    raw_query: Result<Query<IdentifyMusicQuery>, QueryRejection>,
    caller: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Response, ApiError> {
    let Query(query) = raw_query.map_err(ApiError::from)?;
    let kind = MusicKind::from(query.kind);
    let year = query
        .year
        .map(|value| {
            u32::try_from(value).map_err(|_| {
                warn!(
                    target: "security",
                    event = "input_validation_failed",
                    field = "year",
                    reason = "index_out_of_range",
                    "rejected an out-of-range music query year"
                );
                ApiError::UnprocessableEntity("the year is out of range".to_owned())
            })
        })
        .transpose()?;
    let music_query = MusicQuery::try_new(kind, query.title, query.group, year).map_err(|_| {
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "title",
            reason = "empty",
            "rejected an empty music query title"
        );
        ApiError::UnprocessableEntity("the title must not be empty".to_owned())
    })?;
    // The representation is the answer to the search view, so its `self` link
    // addresses the surface rather than repeating the caller's filters.
    let href = "/api/v1/library/music/identify";

    let result = state
        .library_use_case_factory()
        .identify_music()
        .execute(IdentifyMusicCommand::new(caller.user_id(), music_query))
        .await;
    match result {
        Ok(response) => Ok(hal_json(MusicCandidateListResponse::new(
            href,
            response.candidates(),
        ))),
        Err(IdentifyMusicError::Unavailable) => {
            Ok(hal_json(MusicCandidateListResponse::unavailable(href)))
        }
        Err(error) => Err(ApiError::from(error)),
    }
}
