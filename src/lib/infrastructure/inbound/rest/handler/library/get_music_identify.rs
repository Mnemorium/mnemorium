use axum::extract::Query;
use axum::extract::State;
use axum::extract::rejection::QueryRejection;
use axum::response::Response;
use serde::Deserialize;
use serde::Serialize;
use tracing::warn;
use utoipa::IntoParams;
use utoipa::ToSchema;

use crate::application::port::identify_music::IdentifyMusicCommand;
use crate::application::port::identify_music::IdentifyMusicError;
use crate::domain::model::music_candidate::MusicCandidate;
use crate::domain::model::music_query::MusicKind;
use crate::domain::model::music_query::MusicQuery;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::SelfLinks;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::MusicMediumResponse;
use crate::infrastructure::inbound::rest::handler::library::MusicReferenceResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Evidence a caller knows about the music work it wants identified.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[non_exhaustive]
pub struct IdentifyMusicQuery {
    /// Group the caller knows, when any.
    pub group: Option<String>,
    /// Kind of work to search for: `album`, `release` or `recording`.
    pub kind: String,
    /// Title to search for; never empty.
    pub title: String,
    /// Release or first-release year, when the caller knows one.
    pub year: Option<u32>,
}

/// One candidate the provider reported, shaped by the kind of work it denotes.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
#[non_exhaustive]
pub enum MusicCandidateResponse {
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
pub struct MusicCandidateListResponse {
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
    pub fn new(href: &str, candidates: &[MusicCandidate]) -> Self {
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
    pub fn unavailable(href: &str) -> Self {
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
    path = "/library/music/identify",
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
pub async fn get_music_identify(
    raw_query: Result<Query<IdentifyMusicQuery>, QueryRejection>,
    caller: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Response, ApiError> {
    let Query(query) = raw_query.map_err(ApiError::from)?;
    let kind = MusicKind::parse(&query.kind).ok_or_else(|| ApiError::reject_enum("kind"))?;
    let music_query =
        MusicQuery::try_new(kind, query.title, query.group, query.year).map_err(|_| {
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

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::response::Response;
    use axum::routing::get;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::get_music_identify;
    use crate::application::port::identify_music::IdentifyMusicError;
    use crate::application::port::identify_music::IdentifyMusicResponse;
    use crate::application::port::identify_music::IdentifyMusicUseCase;
    use crate::application::port::identify_music::MockIdentifyMusicUseCase;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::domain::alias::NumericID;
    use crate::domain::model::media_candidate::ExternalRef;
    use crate::domain::model::media_candidate::ProviderId;
    use crate::domain::model::music_candidate::MusicCandidate;
    use crate::domain::model::music_metadata::MediumFormat;
    use crate::domain::model::music_metadata::MediumSummary;
    use crate::domain::model::music_metadata::ReleaseStatus;
    use crate::domain::model::music_query::MusicKind;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::into_parts;

    /// Send a GET to the identify route on behalf of `caller_id`, injecting the
    /// caller identifier the way the auth middleware does.
    async fn send(
        use_case: MockIdentifyMusicUseCase,
        caller_id: NumericID,
        query: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_identify_music()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn IdentifyMusicUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/library/music/identify?{query}"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/library/music/identify", get(get_music_identify))
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    /// An album candidate for `Kind of Blue`.
    fn album_candidate() -> MusicCandidate {
        MusicCandidate::Album {
            group: Some("Miles Davis".to_owned()),
            reference: ExternalRef::new(ProviderId::MusicBrainz, "album-id".to_owned()),
            title: "Kind of Blue".to_owned(),
            year: Some(1959),
        }
    }

    /// A release candidate for the compact-disc edition of `Kind of Blue`.
    fn release_candidate() -> MusicCandidate {
        MusicCandidate::Release {
            album: Some(ExternalRef::new(
                ProviderId::MusicBrainz,
                "album-id".to_owned(),
            )),
            group: Some("Miles Davis".to_owned()),
            mediums: vec![MediumSummary::new(MediumFormat::Cd, 1, 5)],
            reference: ExternalRef::new(ProviderId::MusicBrainz, "release-id".to_owned()),
            status: Some(ReleaseStatus::Official),
            title: "Kind of Blue".to_owned(),
            year: Some(1959),
        }
    }

    #[tokio::test]
    async fn get_music_identify_returns_the_candidates() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockIdentifyMusicUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| command.caller_id() == 1 && command.query().kind() == MusicKind::Album)
            .return_once(|_| {
                Box::pin(async { Ok(IdentifyMusicResponse::new(vec![album_candidate()])) })
            });

        // Act
        let (status, content_type, payload) = into_parts(
            send(
                use_case,
                1,
                "kind=album&title=Kind%20of%20Blue&group=Miles%20Davis",
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(
            payload,
            json!({
                "_links": {
                    "self": {
                        "href": "/api/v1/library/music/identify"
                    }
                },
                "available": true,
                "items": [{
                    "kind": "album",
                    "group": "Miles Davis",
                    "reference": { "id": "album-id", "provider": "musicbrainz" },
                    "title": "Kind of Blue",
                    "year": 1959i64
                }]
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_music_identify_returns_every_candidate_kind() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockIdentifyMusicUseCase::new();
        use_case.expect_execute().times(1).return_once(|_| {
            Box::pin(async { Ok(IdentifyMusicResponse::new(vec![release_candidate()])) })
        });

        // Act
        let (status, _, payload) =
            into_parts(send(use_case, 1, "kind=release&title=Kind%20of%20Blue").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        let item = payload
            .get("items")
            .and_then(|items| items.get(0))
            .cloned()
            .unwrap_or_default();
        assert_eq!(item.get("kind"), Some(&json!("release")));
        assert_eq!(item.pointer("/reference/id"), Some(&json!("release-id")));
        assert_eq!(item.pointer("/album/id"), Some(&json!("album-id")));
        assert_eq!(item.get("status"), Some(&json!("OFFICIAL")));
        assert_eq!(
            item.pointer("/mediums/0"),
            Some(&json!({ "format": "CD", "index": 1u8, "track_count": 5u32 }))
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_music_identify_reports_an_unavailable_provider() -> Result<(), Box<dyn Error>> {
        // Arrange: no provider is configured.
        let mut use_case = MockIdentifyMusicUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(IdentifyMusicError::Unavailable) }));

        // Act
        let (status, _, payload) =
            into_parts(send(use_case, 1, "kind=album&title=Kind%20of%20Blue").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            payload,
            json!({
                "_links": {
                    "self": {
                        "href": "/api/v1/library/music/identify"
                    }
                },
                "available": false,
                "items": []
            }),
            "unavailable must be distinct from a search that found nothing"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_music_identify_conceals_a_denied_caller() -> Result<(), Box<dyn Error>> {
        // Arrange: the use case denies the caller.
        let mut use_case = MockIdentifyMusicUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(IdentifyMusicError::NotPermitted) }));

        // Act
        let response = send(use_case, 1, "kind=album&title=Kind%20of%20Blue").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn get_music_identify_rejects_an_unknown_kind() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockIdentifyMusicUseCase::new();

        // Act
        let response = send(use_case, 1, "kind=single&title=Kind%20of%20Blue").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[tokio::test]
    async fn get_music_identify_rejects_an_empty_title() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockIdentifyMusicUseCase::new();

        // Act
        let response = send(use_case, 1, "kind=album&title=").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[tokio::test]
    async fn get_music_identify_missing_credentials_returns_unauthorized()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the real router, wrapped in its `authenticate` layer, with no
        // authorization header.
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("GET")
            .uri("/music/identify?kind=album&title=Kind%20of%20Blue")
            .body(Body::empty())?;

        // Act
        let response: Response = library_routes(&state).oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
