use crate::domain::alias::NumericID;

/// Minimum number of characters allowed in `User::username`.
pub const MIN_USERNAME_LENGTH: usize = 4;

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
    /// The username is shorter than [`MIN_USERNAME_LENGTH`].
    #[error("username must be at least {MIN_USERNAME_LENGTH} characters long")]
    UsernameTooShort,
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

/// A user account.
#[derive(Debug, Clone, PartialEq, Eq)]
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

    /// Update the username.
    ///
    /// # Errors
    ///
    /// Returns [`UserError::UsernameTooShort`] when `username` is shorter than
    /// [`MIN_USERNAME_LENGTH`].
    pub fn set_username(&mut self, username: String) -> Result<(), UserError> {
        self.username = Self::validate_username(username)?;
        Ok(())
    }

    /// Initialise a new `User`, validating `username` and `email`.
    ///
    /// # Errors
    ///
    /// Returns [`UserError::UsernameTooShort`] when `username` is shorter than
    /// [`MIN_USERNAME_LENGTH`], and [`UserError::InvalidEmail`] when `email` is
    /// not a valid email address.
    pub fn try_new(
        id: NumericID,
        username: String,
        email: Option<String>,
        credential_id: NumericID,
        role: Role,
    ) -> Result<Self, UserError> {
        let validated_username = Self::validate_username(username)?;
        let validated_email = Self::validate_email(email)?;
        Ok(Self {
            credential_id,
            email: validated_email,
            id,
            role,
            username: validated_username,
        })
    }

    /// Return the username.
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }

    /// Validate and normalise `email`.
    ///
    /// # Errors
    ///
    /// Returns [`UserError::InvalidEmail`] when `email` is not a valid email
    /// address.
    fn validate_email(email: Option<String>) -> Result<Option<String>, UserError> {
        match email {
            None => Ok(None),
            Some(raw) => {
                raw.parse::<email_address::EmailAddress>()?;
                Ok(Some(raw))
            }
        }
    }

    /// Validate `username`.
    ///
    /// # Errors
    ///
    /// Returns [`UserError::UsernameTooShort`] when `username` is shorter than
    /// [`MIN_USERNAME_LENGTH`].
    fn validate_username(username: String) -> Result<String, UserError> {
        if username.chars().count() >= MIN_USERNAME_LENGTH {
            Ok(username)
        } else {
            Err(UserError::UsernameTooShort)
        }
    }
}
