use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use email_address::EmailAddress;
use email_address::Options;

use crate::domain::alias::NumericID;

/// Error returned when initialising or updating a `User`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum UserError {
    /// The email does not respect a valid email format.
    #[error("email has an invalid format")]
    InvalidEmail(#[from] email_address::Error),
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Role of a `User`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize, utoipa::ToSchema,
)]
#[serde(rename_all = "UPPERCASE")]
#[non_exhaustive]
pub enum Role {
    Admin,
    Standard,
}

impl Role {
    /// Parse a role from its wire representation (`ADMIN`, `STANDARD`).
    ///
    /// Returns `None` when `value` is not a member of the discrete list, so an
    /// inbound handler can classify the rejection as `enum_validation_failed`
    /// (`OBS-006`). The names match the `serde(rename_all = "UPPERCASE")`
    /// representation of [`Role`].
    #[must_use]
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "ADMIN" => Some(Self::Admin),
            "STANDARD" => Some(Self::Standard),
            _ => None,
        }
    }
}

/// A user account.
#[derive(Clone, PartialEq, Eq)]
pub struct User {
    /// Identifier of the credential used to authenticate the user.
    credential_id: NumericID,
    /// Email address of the user, when set.
    email: Option<String>,
    /// Unique identifier of the user.
    id: NumericID,
    /// Role of the user.
    role: Role,
    /// Username of the user.
    username: String,
}

impl User {
    /// Return the identifier of the credential used to authenticate the user.
    #[must_use]
    pub fn credential_id(&self) -> NumericID {
        self.credential_id
    }

    /// Return the email, if any.
    #[must_use]
    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }

    /// Return the user identifier.
    #[must_use]
    pub fn id(&self) -> NumericID {
        self.id
    }

    /// Return whether the user is the Root Admin: the `Role::Admin` account
    /// with identifier `0`.
    #[must_use]
    pub fn is_root_admin(&self) -> bool {
        self.role == Role::Admin && self.id == 0
    }

    /// Return the role.
    #[must_use]
    pub fn role(&self) -> Role {
        self.role
    }

    /// Update the email.
    ///
    /// # Errors
    ///
    /// Returns [`UserError::InvalidEmail`] when `email` is not a valid email
    /// address.
    pub fn set_email(&mut self, email: Option<String>) -> Result<(), UserError> {
        self.email = Self::validate_email(email)?;
        Ok(())
    }

    /// Update the role.
    pub fn set_role(&mut self, role: Role) {
        self.role = role;
    }

    /// Update the username with an already-validated value.
    ///
    /// The caller is responsible for validating the value through
    /// [`UsernamePolicy`](crate::domain::service::username_policy::UsernamePolicy);
    /// the model stores it unchanged.
    pub fn set_username(&mut self, username: String) {
        self.username = username;
    }

    /// Initialise a new `User`, validating `email`.
    ///
    /// The username is supplied already validated and is stored unchanged; the
    /// caller applies
    /// [`UsernamePolicy`](crate::domain::service::username_policy::UsernamePolicy).
    ///
    /// # Errors
    ///
    /// Returns [`UserError::InvalidEmail`] when `email` is not a valid email
    /// address.
    pub fn try_new(
        id: NumericID,
        username: String,
        email: Option<String>,
        credential_id: NumericID,
        role: Role,
    ) -> Result<Self, UserError> {
        let validated_email = Self::validate_email(email)?;
        Ok(Self {
            credential_id,
            email: validated_email,
            id,
            role,
            username,
        })
    }

    /// Return the username.
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }

    /// Validate `email`.
    ///
    /// The parser is configured to reject display-name mailboxes: the field
    /// holds an address, not a mailbox such as `Alice <alice@example.com>`.
    ///
    /// # Errors
    ///
    /// Returns [`UserError::InvalidEmail`] when `email` is not a valid email
    /// address.
    fn validate_email(email: Option<String>) -> Result<Option<String>, UserError> {
        match email {
            None => Ok(None),
            Some(raw) => {
                EmailAddress::parse_with_options(&raw, Options::default().without_display_text())?;
                Ok(Some(raw))
            }
        }
    }
}

impl Debug for User {
    /// Redact the personal-data fields (`OBS-004`): the email address and the
    /// username never reach a log sink, while the identifiers and the role the
    /// server uses to correlate an event stay readable.
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("User")
            .field("credential_id", &self.credential_id)
            .field("email", &"**")
            .field("id", &self.id)
            .field("role", &self.role)
            .field("username", &"**")
            .finish_non_exhaustive()
    }
}
