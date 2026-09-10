use std::sync::Arc;

use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::application::port::patch_credential::PatchCredentialCommand;
use crate::application::port::patch_credential::PatchCredentialError;
use crate::application::port::patch_credential::PatchCredentialUseCase;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to change the password behind a credential.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct PatchCredentialRequest {
    /// New password; never returned by the API.
    ///
    /// Must be at least 8 characters long and contain at least one symbol
    /// (a non-alphanumeric character).
    #[schema(write_only, min_length = 8, pattern = r"[^A-Za-z0-9]")]
    pub password: String,
}

/// Map a change-password error to its API error.
impl From<PatchCredentialError> for ApiError {
    fn from(err: PatchCredentialError) -> Self {
        match err {
            PatchCredentialError::Forbidden => Self::Forbidden(err.to_string()),
            PatchCredentialError::InvalidPassword => Self::BadRequest(err.to_string()),
            PatchCredentialError::Unknown(_) => Self::InternalServerError,
            PatchCredentialError::UnknownCredential => Self::NotFound(err.to_string()),
        }
    }
}

/// Change the password behind a credential.
///
/// Only the Root Admin (user identifier `0`, role `Admin`) may invoke this
/// endpoint; any other authenticated caller is rejected with `403`. The Root
/// Admin may change its own credential as well as the credential of any other
/// user. The new password must satisfy the password policy: at least 8
/// characters long and containing at least one symbol.
#[utoipa::path(
    patch,
    operation_id = "patch_credential",
    path = "/identity/credential/{id}",
    tag = "identity",
    request_body = PatchCredentialRequest,
    params(
        ("id" = NumericID, Path, description = "Identifier of the credential whose password is changed"),
    ),
    responses(
        (
            status = NO_CONTENT,
            description = "Password changed",
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            description = "Invalid payload or password policy violation"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            description = "Missing or invalid credentials"
        ),
        (
            status = FORBIDDEN,
            body = ErrorBody,
            description = "Caller is not the Root Admin"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            description = "Unknown credential"
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            body = ErrorBody,
            description = "Unexpected error"
        ),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "Change the password behind a credential"
)]
pub async fn patch_credential(
    State(patch_credential): State<Arc<dyn PatchCredentialUseCase>>,
    caller: AuthenticatedUser,
    Path(credential_id): Path<NumericID>,
    payload: Result<Json<PatchCredentialRequest>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Json(request) = payload.map_err(ApiError::from)?;
    patch_credential
        .execute(PatchCredentialCommand::new(
            caller.user_id(),
            credential_id,
            request.password,
        ))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::body::to_bytes;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::response::Response;
    use axum::routing::patch;
    use mockall::predicate::eq;
    use serde_json::Value;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::patch_credential;
    use crate::application::port::patch_credential::MockPatchCredentialUseCase;
    use crate::application::port::patch_credential::PatchCredentialCommand;
    use crate::application::port::patch_credential::PatchCredentialError;
    use crate::application::port::patch_credential::PatchCredentialUseCase;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

    /// Send `body` through the endpoint router on behalf of `caller_id`,
    /// targeting credential `credential_id`, injecting the caller the way the
    /// auth middleware does.
    async fn send(
        use_case: MockPatchCredentialUseCase,
        caller_id: NumericID,
        credential_id: NumericID,
        body: Body,
    ) -> Result<Response, Box<dyn Error>> {
        let mut request = Request::builder()
            .method("PATCH")
            .uri(format!("/api/v1/identity/credential/{credential_id}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(body)?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/identity/credential/{id}", patch(patch_credential))
            .with_state(Arc::new(use_case) as Arc<dyn PatchCredentialUseCase>);
        Ok(router.oneshot(request).await?)
    }

    /// Split `response` into its status and decoded JSON body; the body is
    /// `None` when the response carries no content.
    async fn into_parts(response: Response) -> Result<(StatusCode, Option<Value>), Box<dyn Error>> {
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let body = if bytes.is_empty() {
            None
        } else {
            Some(serde_json::from_slice(&bytes)?)
        };
        Ok((status, body))
    }

    /// Unwrap the JSON body of a response known to carry one.
    fn payload(body: Option<Value>) -> Result<Value, Box<dyn Error>> {
        Ok(body.ok_or_else(|| anyhow::anyhow!("expected a json body"))?)
    }

    /// Build a valid JSON request body carrying `password`.
    fn request_body(password: &str) -> String {
        json!({ "password": password }).to_string()
    }

    /// Make the mocked use case fail with `error`.
    fn expect_error(use_case: &mut MockPatchCredentialUseCase, error: PatchCredentialError) {
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));
    }

    #[tokio::test]
    async fn patch_credential_valid_body_returns_no_content() -> Result<(), Box<dyn Error>> {
        // Arrange
        let expected_command = PatchCredentialCommand::new(0, 0, "new-password!".to_owned());
        let mut use_case = MockPatchCredentialUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(expected_command))
            .return_once(|_| Box::pin(async { Ok(()) }));

        // Act
        let (status, body) =
            into_parts(send(use_case, 0, 0, Body::from(request_body("new-password!"))).await?)
                .await?;

        // Assert
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert!(body.is_none(), "a no-content response carries no body");
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_other_user_credential_returns_no_content()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockPatchCredentialUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| command.credential_id() == 4)
            .return_once(|_| Box::pin(async { Ok(()) }));

        // Act
        let (status, body) =
            into_parts(send(use_case, 0, 4, Body::from(request_body("other-user!"))).await?)
                .await?;

        // Assert
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert!(body.is_none(), "a no-content response carries no body");
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_standard_caller_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockPatchCredentialUseCase::new();
        expect_error(&mut use_case, PatchCredentialError::Forbidden);

        // Act
        let (status, body) =
            into_parts(send(use_case, 3, 1, Body::from(request_body("secret-one!"))).await?)
                .await?;
        let payload = payload(body)?;

        // Assert
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(
            payload,
            json!({ "error": "only the Root Admin may change a credential password" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_unknown_credential_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockPatchCredentialUseCase::new();
        expect_error(&mut use_case, PatchCredentialError::UnknownCredential);

        // Act
        let (status, body) =
            into_parts(send(use_case, 0, 42, Body::from(request_body("secret-two!"))).await?)
                .await?;
        let payload = payload(body)?;

        // Assert
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            payload,
            json!({ "error": "no credential matches the provided identifier" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_invalid_password_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockPatchCredentialUseCase::new();
        expect_error(&mut use_case, PatchCredentialError::InvalidPassword);

        // Act
        let (status, body) =
            into_parts(send(use_case, 0, 0, Body::from(request_body("password123"))).await?)
                .await?;
        let payload = payload(body)?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            payload,
            json!({ "error": "password does not satisfy the password policy" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_dependency_failure_returns_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockPatchCredentialUseCase::new();
        expect_error(
            &mut use_case,
            PatchCredentialError::Unknown(anyhow::anyhow!("boom")),
        );

        // Act
        let (status, body) =
            into_parts(send(use_case, 0, 0, Body::from(request_body("secret-three!"))).await?)
                .await?;
        let payload = payload(body)?;

        // Assert
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(payload, json!({ "error": "an unexpected error occurred" }));
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_missing_required_field_returns_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockPatchCredentialUseCase::new();

        // Act
        let (status, body) =
            into_parts(send(use_case, 0, 0, Body::from(json!({}).to_string())).await?).await?;
        let payload = payload(body)?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            payload.get("error").is_some(),
            "a bad request must carry an error message"
        );
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_wrong_type_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockPatchCredentialUseCase::new();

        // Act
        let (status, body) = into_parts(
            send(
                use_case,
                0,
                0,
                Body::from(json!({ "password": 1i64 }).to_string()),
            )
            .await?,
        )
        .await?;
        let payload = payload(body)?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            payload.get("error").is_some(),
            "a bad request must carry an error message"
        );
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_malformed_body_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockPatchCredentialUseCase::new();

        // Act
        let (status, body) =
            into_parts(send(use_case, 0, 0, Body::from("not json")).await?).await?;
        let payload = payload(body)?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            payload.get("error").is_some(),
            "a bad request must carry an error message"
        );
        Ok(())
    }

    #[tokio::test]
    async fn patch_credential_unknown_credential_id_path_returns_not_found()
    -> Result<(), Box<dyn Error>> {
        // Combines the unknown-credential authorization rule with the `404`
        // mapping of the endpoint.
        // Arrange
        let mut use_case = MockPatchCredentialUseCase::new();
        expect_error(&mut use_case, PatchCredentialError::UnknownCredential);

        // Act
        let (status, body) =
            into_parts(send(use_case, 0, 404, Body::from(request_body("secret-four!"))).await?)
                .await?;
        let payload = payload(body)?;

        // Assert
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            payload,
            json!({ "error": "no credential matches the provided identifier" })
        );
        Ok(())
    }
}
