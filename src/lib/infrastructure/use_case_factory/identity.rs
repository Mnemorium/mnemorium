use std::sync::Arc;

use crate::application::port::identity_use_case_factory::IdentityUseCaseFactory;
use crate::application::port::login_user::LoginUserUseCase;
use crate::application::port::patch_credential::PatchCredentialUseCase;
use crate::application::port::register_user::RegisterUserUseCase;
use crate::application::use_case::login_user::LoginUser;
use crate::application::use_case::patch_credential::PatchCredential;
use crate::application::use_case::register_user::RegisterUser;
use crate::domain::model::configuration::Configuration;
use crate::domain::model::instance_state::InstanceState;
use crate::infrastructure::outbound::argon2::password_hasher::Argon2PasswordHasher;
use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;
use crate::infrastructure::outbound::sqlx::unit_of_work::SqlxUnitOfWorkFactory;

/// Builds the Identity use cases from the immutable configuration and identity.
///
/// Every accessor rebuilds the configuration-derived adapters. The pepper and
/// the signing secret come from the immutable instance identity, so building
/// the password hasher from them once per use case is correct; the token
/// lifetime still comes from the configuration.
pub struct RuntimeIdentityUseCaseFactory {
    /// Immutable application configuration.
    configuration: Arc<Configuration>,
    /// Immutable instance identity carrying the secrets.
    instance_state: Arc<InstanceState>,
    /// Factory opening the unit of work wrapping the Identity use cases.
    unit_of_work_factory: Arc<SqlxUnitOfWorkFactory>,
}

impl RuntimeIdentityUseCaseFactory {
    /// Create a new factory.
    #[must_use]
    pub fn new(
        configuration: Arc<Configuration>,
        instance_state: Arc<InstanceState>,
        unit_of_work_factory: Arc<SqlxUnitOfWorkFactory>,
    ) -> Self {
        Self {
            configuration,
            instance_state,
            unit_of_work_factory,
        }
    }

    /// Build the password hasher from the instance pepper.
    fn password_hasher(&self) -> Arc<Argon2PasswordHasher> {
        Arc::new(Argon2PasswordHasher::new(
            self.instance_state.pepper().as_bytes().to_vec(),
        ))
    }

    /// Build the token provider from the identity and the configured lifetime.
    fn token_provider(&self) -> Arc<JwtTokenProvider> {
        Arc::new(JwtTokenProvider::new(
            self.instance_state.jwt_secret().to_owned(),
            self.configuration.security().jwt().ttl(),
        ))
    }
}

impl IdentityUseCaseFactory for RuntimeIdentityUseCaseFactory {
    fn login_user(&self) -> Arc<dyn LoginUserUseCase> {
        Arc::new(LoginUser::new(
            Arc::clone(&self.unit_of_work_factory),
            self.password_hasher(),
            self.token_provider(),
        ))
    }

    fn patch_credential(&self) -> Arc<dyn PatchCredentialUseCase> {
        Arc::new(PatchCredential::new(
            Arc::clone(&self.unit_of_work_factory),
            self.password_hasher(),
        ))
    }

    fn register_user(&self) -> Arc<dyn RegisterUserUseCase> {
        Arc::new(RegisterUser::new(
            Arc::clone(&self.unit_of_work_factory),
            self.password_hasher(),
        ))
    }
}
