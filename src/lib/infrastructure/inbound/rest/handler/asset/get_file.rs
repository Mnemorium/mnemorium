use axum::extract::Path;
use axum::extract::State;
use axum::response::Response;
use serde::Deserialize;
use serde::Serialize;
use utoipa::ToSchema;

use crate::application::port::get_file::GetFileCommand;
use crate::application::port::get_file::GetFileError;
use crate::application::port::get_file::GetFileResponse as GetFileResponseData;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::Link;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::asset::TIMESTAMP_FORMAT;
use crate::infrastructure::inbound::rest::handler::asset::asset_file_href;
use crate::infrastructure::inbound::rest::handler::asset::asset_image_href;
use crate::infrastructure::inbound::rest::handler::asset::format_date;
use crate::infrastructure::inbound::rest::handler::asset::parse_asset_id;
use crate::infrastructure::inbound::rest::handler::user::user_self_href;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// HAL links exposed by a stored-file representation.
///
/// The `self` link addresses the file; `owner` names the account that owns it,
/// and `image` names the typed image the file backs when an image row exists
/// (`API-032`, `API-034`, `API-037`).
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
struct FileLinks {
    /// Link to the typed image backed by the file, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<Link>,
    /// Link to the account owning the file.
    pub owner: Link,
    /// Link to the stored file itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

impl FileLinks {
    /// Build the links of the file identified by `file_id`.
    #[must_use]
    fn for_file(file_id: NumericID, owner_id: NumericID, image_id: Option<NumericID>) -> Self {
        Self {
            image: image_id.map(|id| Link::new(&asset_image_href(id))),
            owner: Link::new(&user_self_href(owner_id)),
            self_link: Link::new(&asset_file_href(file_id)),
        }
    }
}

/// Representation of a stored file.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
struct FileResponse {
    /// Unique identifier of the stored file.
    pub id: NumericID,
    /// Whether the file is marked public.
    pub is_public: bool,
    /// Links to the file and the resources related to it.
    #[serde(rename = "_links")]
    pub links: FileLinks,
    /// Media type of the file.
    #[schema(example = json!("image/png"))]
    pub mime_type: String,
    /// Instant at which the content was last modified, as
    /// `YYYY-MM-DDTHH:MM:SS`.
    #[schema(example = json!("2026-01-01T12:00:00"))]
    pub modified_at: String,
    /// Origin of the file's bytes.
    #[schema(example = json!("UPLOADED"))]
    pub origin: String,
    /// Path of the file relative to the library root.
    #[schema(example = json!("12_cover.png"))]
    pub path: String,
    /// Size of the file content, in bytes.
    pub size_bytes: i64,
    /// Date at which the file was uploaded, as `YYYY-MM-DD`.
    #[schema(example = json!("2026-01-01"))]
    pub uploaded_at: String,
}

impl FileResponse {
    /// Map a fetched file onto its HTTP representation.
    #[must_use]
    pub(crate) fn from_response(response: &GetFileResponseData) -> Self {
        Self {
            id: response.id(),
            is_public: response.is_public(),
            links: FileLinks::for_file(response.id(), response.owner_id(), response.image_id()),
            mime_type: response.mime_type_id().to_owned(),
            modified_at: response.modified_at().format(TIMESTAMP_FORMAT).to_string(),
            origin: response.origin().as_str().to_owned(),
            path: response.path().to_owned(),
            size_bytes: response.size_bytes(),
            uploaded_at: format_date(response.uploaded_at()),
        }
    }
}

/// Map a fetch-file error to its API error.
impl From<GetFileError> for ApiError {
    fn from(err: GetFileError) -> Self {
        match err {
            GetFileError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetFileError::NoSuchFile => Self::NotFound(err.to_string()),
            GetFileError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Fetch one stored file by its identifier.
///
/// Any authenticated caller may read a file it is allowed to read: a catalogue
/// asset follows `library.read`, a gallery asset its gallery and a playlist
/// cover its playlist. An unknown file and an unreadable file are one outcome,
/// `404 Not Found` (`API-044`).
#[utoipa::path(
    get,
    operation_id = "get_file",
    path = "/file/{file_id}",
    tag = "asset",
    params(
        ("file_id" = NumericID, Path, description = "Identifier of the file to fetch"),
    ),
    responses(
        (
            status = OK,
            body = FileResponse,
            content_type = "application/hal+json",
            description = "File found"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid file identifier"
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
            description = "Unknown or unreadable file"
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
    summary = "Fetch a stored file"
)]
pub(crate) async fn get_file(
    Path(file_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let file_id = parse_asset_id(&file_id_value, "file_id", "invalid file identifier")?;
    let response = state
        .asset_use_case_factory()
        .get_file()
        .execute(GetFileCommand::new(caller.user_id(), file_id))
        .await?;
    Ok(hal_json(FileResponse::from_response(&response)))
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
    use chrono::NaiveDateTime;
    use mockall::predicate::eq;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::get_file;
    use crate::application::port::asset_use_case_factory::MockAssetUseCaseFactory;
    use crate::application::port::get_file::GetFileCommand;
    use crate::application::port::get_file::GetFileError;
    use crate::application::port::get_file::GetFileResponse as GetFileResponseData;
    use crate::application::port::get_file::GetFileUseCase;
    use crate::application::port::get_file::MockGetFileUseCase;
    use crate::domain::alias::NumericID;
    use crate::domain::model::file::Origin;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_asset;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_parts;
    use crate::test_helpers::into_status_and_body;

    /// Send a GET to `/api/v1/asset/file/{file_id}` on behalf of `caller_id`.
    async fn send(
        use_case: MockGetFileUseCase,
        caller_id: NumericID,
        file_id: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockAssetUseCaseFactory::new();
        factory
            .expect_get_file()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn GetFileUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/asset/file/{file_id}"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/asset/file/{file_id}", get(get_file))
            .with_state(app_state_with_asset(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    fn sample_response() -> GetFileResponseData {
        GetFileResponseData::new(
            100,
            "100.png".to_owned(),
            "image/png".to_owned(),
            10,
            NaiveDate::default(),
            NaiveDateTime::default(),
            Origin::Uploaded,
            false,
            3,
            Some(19),
            None,
            None,
        )
    }

    #[tokio::test]
    async fn get_file_returns_the_representation_and_links() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetFileUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(GetFileCommand::new(3, 100)))
            .return_once(|_| Box::pin(async { Ok(sample_response()) }));

        // Act
        let response = send(use_case, 3, "100").await?;
        let (status, content_type, payload) = into_parts(response).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(
            payload,
            json!({
                "id": 100i64,
                "path": "100.png",
                "mime_type": "image/png",
                "size_bytes": 10i64,
                "uploaded_at": "1970-01-01",
                "modified_at": "1970-01-01T00:00:00",
                "origin": "UPLOADED",
                "is_public": false,
                "_links": {
                    "self": { "href": "/api/v1/asset/file/100" },
                    "owner": { "href": "/api/v1/user/3" },
                    "image": { "href": "/api/v1/asset/image/19" },
                },
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_file_without_a_media_row_omits_the_image_link() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetFileUseCase::new();
        use_case.expect_execute().times(1).return_once(|_| {
            Box::pin(async {
                Ok(GetFileResponseData::new(
                    100,
                    "100.bin".to_owned(),
                    "application/octet-stream".to_owned(),
                    10,
                    NaiveDate::default(),
                    NaiveDateTime::default(),
                    Origin::Scanned,
                    true,
                    3,
                    None,
                    None,
                    None,
                ))
            })
        });

        // Act
        let response = send(use_case, 3, "100").await?;
        let (status, payload) = into_status_and_body(response).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert!(
            payload.pointer("/_links/image").is_none(),
            "a plain file must not link a typed image"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_file_malformed_identifier_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange: the use case must never be reached.
        let use_case = MockGetFileUseCase::new();

        // Act
        let response = send(use_case, 3, "abc").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("invalid file identifier")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_file_unknown_file_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetFileUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(GetFileError::NoSuchFile) }));

        // Act
        let response = send(use_case, 3, "100").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn get_file_internal_error_returns_internal_server_error() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetFileUseCase::new();
        use_case.expect_execute().times(1).return_once(|_| {
            Box::pin(async { Err(GetFileError::Unknown(anyhow::anyhow!("boom"))) })
        });

        // Act
        let response = send(use_case, 3, "100").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        Ok(())
    }
}
