use std::sync::Arc;

use arc_swap::ArcSwap;

use crate::application::port::identity_use_case_factory::IdentityUseCaseFactory;
use crate::application::port::login_user::LoginUserUseCase;
use crate::application::port::patch_credential::PatchCredentialUseCase;
use crate::application::port::register_user::RegisterUserUseCase;
use crate::application::use_case::login_user::LoginUser;
use crate::application::use_case::patch_credential::PatchCredential;
use crate::application::use_case::register_user::RegisterUser;
use crate::domain::model::configuration::Configuration;
use crate::infrastructure::outbound::argon2::password_hasher::Argon2PasswordHasher;
use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;
use crate::infrastructure::outbound::sqlx::unit_of_work::SqlxUnitOfWorkFactory;

/// Builds the Identity use cases from the live configuration.
///
/// Every accessor rebuilds the configuration-derived adapters (password hasher,
/// token provider) from the current configuration, so a runtime configuration
/// change is picked up by the next request.
pub struct RuntimeIdentityUseCaseFactory {
    /// Live application configuration.
    configuration: Arc<ArcSwap<Configuration>>,
    /// Factory opening the unit of work wrapping the Identity use cases.
    unit_of_work_factory: Arc<SqlxUnitOfWorkFactory>,
}

impl RuntimeIdentityUseCaseFactory {
    /// Create a new factory.
    #[must_use]
    pub fn new(
        configuration: Arc<ArcSwap<Configuration>>,
        unit_of_work_factory: Arc<SqlxUnitOfWorkFactory>,
    ) -> Self {
        Self {
            configuration,
            unit_of_work_factory,
        }
    }
}

impl IdentityUseCaseFactory for RuntimeIdentityUseCaseFactory {
    fn login_user(&self) -> Arc<dyn LoginUserUseCase> {
        let live = self.configuration.load();
        let password_hasher = Arc::new(Argon2PasswordHasher::new(
            live.security().pepper().as_bytes().to_vec(),
        ));
        let token_provider = Arc::new(JwtTokenProvider::new(
            live.security().jwt().secret().to_owned(),
            live.security().jwt().ttl(),
        ));
        Arc::new(LoginUser::new(
            Arc::clone(&self.unit_of_work_factory),
            password_hasher,
            token_provider,
        ))
    }

    fn patch_credential(&self) -> Arc<dyn PatchCredentialUseCase> {
        let live = self.configuration.load();
        let password_hasher = Arc::new(Argon2PasswordHasher::new(
            live.security().pepper().as_bytes().to_vec(),
        ));
        Arc::new(PatchCredential::new(
            Arc::clone(&self.unit_of_work_factory),
            password_hasher,
        ))
    }

    fn register_user(&self) -> Arc<dyn RegisterUserUseCase> {
        let live = self.configuration.load();
        let password_hasher = Arc::new(Argon2PasswordHasher::new(
            live.security().pepper().as_bytes().to_vec(),
        ));
        Arc::new(RegisterUser::new(
            Arc::clone(&self.unit_of_work_factory),
            password_hasher,
        ))
    }
}
