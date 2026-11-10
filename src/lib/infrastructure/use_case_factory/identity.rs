use std::sync::Arc;

use crate::application::port::{
    GetPermissionUseCase, IdentityUseCaseFactory, LoginUserUseCase, PatchCredentialUseCase,
    RegisterUserUseCase, ReplacePermissionUseCase,
};
use crate::application::use_case::{
    GetPermission, LoginUser, PatchCredential, RegisterUser, ReplacePermission,
};
use crate::domain::model::{Configuration, InstanceState};
use crate::infrastructure::outbound::argon2::Argon2PasswordHasher;
use crate::infrastructure::outbound::jwt::JwtTokenProvider;
use crate::infrastructure::outbound::sqlx::SqlxUnitOfWorkFactory;

/// Builds the Identity use cases from the immutable configuration and identity.
///
/// Every accessor rebuilds the configuration-derived adapters. The pepper and
/// the signing secret come from the immutable instance identity, so building
/// the password hasher from them once per use case is correct; the token
/// lifetime still comes from the configuration.
pub(crate) struct RuntimeIdentityUseCaseFactory {
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
    pub(crate) fn new(
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
    fn get_permission(&self) -> Arc<dyn GetPermissionUseCase> {
        Arc::new(GetPermission::new(Arc::clone(&self.unit_of_work_factory)))
    }

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

    fn replace_permission(&self) -> Arc<dyn ReplacePermissionUseCase> {
        Arc::new(ReplacePermission::new(Arc::clone(
            &self.unit_of_work_factory,
        )))
    }
}
